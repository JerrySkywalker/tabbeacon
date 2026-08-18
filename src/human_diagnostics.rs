//! Compact, monochrome-safe human renderers for management diagnostics.
//!
//! JSON and the legacy key/value views remain in `diagnostics`. This module
//! consumes the shared `ManagementSnapshot`, so human frontends render the
//! same issue and action semantics without interpreting raw diagnostics.

use crate::{
    diagnostics::{DiagnosticStatus, DoctorDiagnostics, OperationalDiagnostics},
    management::{ChangePlan, HealthIssue, ManagementHealth, ManagementSnapshot},
};

const DEFAULT_WIDTH: usize = 80;
const MIN_WIDTH: usize = 24;
const MAX_STATUS_ISSUES: usize = 3;

/// Returns the current text width without entering a terminal UI mode.
#[must_use]
pub fn terminal_width() -> usize {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|width| *width >= MIN_WIDTH)
        .unwrap_or(DEFAULT_WIDTH)
}

/// Renders the default concise human status view for one diagnostic collection.
#[must_use]
pub fn human_status_lines(
    report: &OperationalDiagnostics,
    snapshot: &ManagementSnapshot,
    width: usize,
) -> Vec<String> {
    let width = normalize_width(width);
    let mut lines = Vec::new();
    push(
        &mut lines,
        width,
        format!("TabBeacon Status — {}", health_label(snapshot.health)),
    );
    lines.push(String::new());

    append_integration(&mut lines, report, snapshot, width);
    append_presentation(&mut lines, report, snapshot, width);
    append_runtime(&mut lines, report, snapshot, width);

    append_status_action_summary(&mut lines, snapshot, width);
    lines
}

fn append_integration(
    lines: &mut Vec<String>,
    report: &OperationalDiagnostics,
    snapshot: &ManagementSnapshot,
    width: usize,
) {
    push(lines, width, "Integration");
    push(
        lines,
        width,
        format!(
            "  {} TabBeacon  {}",
            state_marker(snapshot.health),
            report.tabbeacon.version
        ),
    );
    push(
        lines,
        width,
        format!(
            "  {} Codex      {} — {}",
            state_marker(if report.codex.profile_supported {
                ManagementHealth::Healthy
            } else {
                ManagementHealth::Error
            }),
            option_label(report.codex.version.as_deref()),
            if report.codex.profile_supported {
                "Supported"
            } else {
                "Not admitted"
            }
        ),
    );
    push(
        lines,
        width,
        format!(
            "  {} Hooks      {}",
            marker_for_status(report.integration.declaration_status),
            hook_summary(report)
        ),
    );
    push(
        lines,
        width,
        format!(
            "  {} Hook trust {}",
            state_marker_for_issue(snapshot, "hooks."),
            hook_trust_label(report)
        ),
    );
}

fn append_presentation(
    lines: &mut Vec<String>,
    report: &OperationalDiagnostics,
    snapshot: &ManagementSnapshot,
    width: usize,
) {
    lines.push(String::new());
    push(lines, width, "Presentation");
    push(
        lines,
        width,
        format!(
            "  {} Title      {}",
            state_marker_for_issue(snapshot, "terminal.title"),
            title_label(report)
        ),
    );
    push(
        lines,
        width,
        format!(
            "  {} Activity   {}",
            state_marker_for_issue(snapshot, "workers."),
            option_label(report.presentation.spinner_preset.as_deref())
        ),
    );
    push(
        lines,
        width,
        format!(
            "  {} Theme      {}",
            state_marker_for_issue(snapshot, "settings."),
            option_label(report.presentation.theme.as_deref())
        ),
    );
}

fn append_runtime(
    lines: &mut Vec<String>,
    report: &OperationalDiagnostics,
    snapshot: &ManagementSnapshot,
    width: usize,
) {
    lines.push(String::new());
    push(lines, width, "Runtime");
    push(
        lines,
        width,
        format!(
            "  {} Workers    {}",
            state_marker_for_issue(snapshot, "workers."),
            bounded_label(&report.activity.worker_state_health)
        ),
    );
    push(
        lines,
        width,
        format!(
            "    Active {} · Stale {}",
            report.activity.active_leases, report.activity.stale_leases
        ),
    );
}

/// Renders the default human doctor view from the shared management projection.
#[must_use]
pub fn human_doctor_lines(
    report: &DoctorDiagnostics,
    snapshot: &ManagementSnapshot,
    width: usize,
) -> Vec<String> {
    let width = normalize_width(width);
    let mut lines = Vec::new();
    push(
        &mut lines,
        width,
        format!("TabBeacon Doctor — {}", health_label(snapshot.health)),
    );
    lines.push(String::new());

    if snapshot.is_healthy() {
        let passed = report
            .checks
            .iter()
            .filter(|check| check.status == DiagnosticStatus::Pass)
            .count();
        push(&mut lines, width, format!("{passed} checks passed."));
        lines.push(String::new());
        for check in &report.checks {
            push(&mut lines, width, format!("OK {}", check.summary));
        }
        lines.push(String::new());
        push(&mut lines, width, "No action required.");
        return lines;
    }

    push(
        &mut lines,
        width,
        format!(
            "{} warning(s), {} failure(s).",
            report.warnings.len(),
            report.failures.len()
        ),
    );
    lines.push(String::new());
    for issue in &snapshot.issues {
        append_doctor_issue(&mut lines, issue, &snapshot.change_plans, width);
    }
    lines
}

fn append_status_action_summary(
    lines: &mut Vec<String>,
    snapshot: &ManagementSnapshot,
    width: usize,
) {
    lines.push(String::new());
    if snapshot.is_healthy() {
        push(lines, width, "No action required.");
        return;
    }

    push(lines, width, "Attention");
    for issue in snapshot.issues.iter().take(MAX_STATUS_ISSUES) {
        push(lines, width, format!("! {}", issue.title));
        if let Some(action) = &issue.remediation {
            push(lines, width, format!("  Next: {}", action.instruction));
        }
    }
    let remaining = snapshot.issues.len().saturating_sub(MAX_STATUS_ISSUES);
    if remaining > 0 {
        push(
            lines,
            width,
            format!("  {remaining} additional condition(s): run tabbeacon doctor."),
        );
    }
}

fn append_doctor_issue(
    lines: &mut Vec<String>,
    issue: &HealthIssue,
    plans: &[ChangePlan],
    width: usize,
) {
    push(
        lines,
        width,
        format!("{} {}", severity_marker(issue), issue.title),
    );
    push(lines, width, format!("  Why: {}", issue.explanation));
    if let Some(action) = &issue.remediation {
        push(lines, width, format!("  Next: {}", action.instruction));
        if let Some(plan) = plans.iter().find(|plan| plan.action_id == action.id) {
            for protected in &plan.protected_state {
                push(
                    lines,
                    width,
                    format!("  TabBeacon did not change: {protected}"),
                );
            }
        }
    }
    lines.push(String::new());
}

fn normalize_width(width: usize) -> usize {
    width.max(MIN_WIDTH)
}

fn push(lines: &mut Vec<String>, width: usize, line: impl AsRef<str>) {
    lines.push(fit(line.as_ref(), width));
}

fn fit(value: &str, width: usize) -> String {
    let count = value.chars().count();
    if count <= width {
        return value.to_owned();
    }
    if width <= 3 {
        return value.chars().take(width).collect();
    }
    let mut shortened = value.chars().take(width - 3).collect::<String>();
    shortened.push_str("...");
    shortened
}

fn health_label(health: ManagementHealth) -> &'static str {
    match health {
        ManagementHealth::Healthy => "Healthy",
        ManagementHealth::Warning => "Needs attention",
        ManagementHealth::Error => "Action needed",
    }
}

fn state_marker(health: ManagementHealth) -> &'static str {
    match health {
        ManagementHealth::Healthy => "OK",
        ManagementHealth::Warning => "!",
        ManagementHealth::Error => "X",
    }
}

fn severity_marker(issue: &HealthIssue) -> &'static str {
    match issue.severity {
        crate::management::HealthSeverity::Warning => "!",
        crate::management::HealthSeverity::Error => "X",
    }
}

fn marker_for_status(status: DiagnosticStatus) -> &'static str {
    match status {
        DiagnosticStatus::Pass => "OK",
        DiagnosticStatus::Warning => "!",
        DiagnosticStatus::Fail => "X",
    }
}

fn state_marker_for_issue(snapshot: &ManagementSnapshot, prefix: &str) -> &'static str {
    snapshot
        .issues
        .iter()
        .find(|issue| issue.id.starts_with(prefix))
        .map_or("OK", severity_marker)
}

fn option_label(value: Option<&str>) -> String {
    value.map_or_else(|| "Unavailable".to_owned(), bounded_label)
}

fn bounded_label(value: &str) -> String {
    value.replace(['_', '-'], " ")
}

fn hook_summary(report: &OperationalDiagnostics) -> String {
    match report.integration.owned_hook_count {
        Some(count) if report.integration.declaration_status == DiagnosticStatus::Pass => {
            format!("{count} active")
        }
        Some(count) => format!("{count} need attention"),
        None => "Unavailable".to_owned(),
    }
}

fn hook_trust_label(report: &OperationalDiagnostics) -> String {
    bounded_label(report.integration.hook_trust.as_str())
}

fn title_label(report: &OperationalDiagnostics) -> String {
    bounded_label(report.integration.title_ownership.as_str())
}

#[cfg(test)]
mod tests {
    use super::{human_doctor_lines, human_status_lines};
    use crate::{
        diagnostics::{
            ActivityDiagnostics, CodexDiagnostics, DiagnosticCheck, DiagnosticStatus,
            DoctorDiagnostics, HookTrustState, IntegrationDiagnostics, OperationalDiagnostics,
            PresentationDiagnostics, PresentationSettingsSource, TabBeaconDiagnostics,
            TitleOwnershipState, WorkspaceDiagnostics,
        },
        management::ManagementSnapshot,
        title_authority::{
            TitleAuthority, TitleAuthorityDiagnostics, TitleConflictClass, VisibleTitleProbe,
        },
        windows_terminal_policy::{
            ActiveProfileResolution, ApplicationTitlePolicy, PolicySource, TitleRemediationState,
        },
    };

    fn title(remediation: TitleRemediationState) -> TitleAuthorityDiagnostics {
        TitleAuthorityDiagnostics {
            desired_owner: "tabbeacon".to_owned(),
            codex_writer_state: "tabbeacon".to_owned(),
            application_title_policy: ApplicationTitlePolicy::ApplicationTitlesAllowed,
            policy_source: PolicySource::Profile,
            active_profile_resolution: ActiveProfileResolution::Resolved,
            remediation_available: remediation,
            remediation_scope: "active_profile",
            visible_probe: VisibleTitleProbe::NotRun,
            probe_boundary: "not_run".to_owned(),
            authority: TitleAuthority::Unverified,
            conflict_class: TitleConflictClass::Unverified,
        }
    }

    fn report() -> OperationalDiagnostics {
        let title = title(TitleRemediationState::NotNeeded);
        let checks = [
            "Codex compatibility",
            "Executable",
            "Ownership",
            "Hooks",
            "Hook trust",
            "Terminal title",
            "Workers",
        ]
        .into_iter()
        .enumerate()
        .map(|(index, summary)| DiagnosticCheck {
            id: format!("fixture.{index}"),
            status: DiagnosticStatus::Pass,
            summary: summary.to_owned(),
        })
        .collect();
        OperationalDiagnostics {
            schema_version: 1,
            tabbeacon: TabBeaconDiagnostics {
                version: "0.4-dev".to_owned(),
                binary_path: None,
            },
            codex: CodexDiagnostics {
                version: Some("0.147.0".to_owned()),
                hook_profile: Some("codex-hooks-rust-v0.147.0".to_owned()),
                profile_state: "supported".to_owned(),
                profile_supported: true,
            },
            integration: IntegrationDiagnostics {
                installed: true,
                owned_hook_count: Some(11),
                declaration_status: DiagnosticStatus::Pass,
                hook_trust: HookTrustState::Active,
                title_ownership: TitleOwnershipState::Tabbeacon,
            },
            presentation: PresentationDiagnostics {
                source: PresentationSettingsSource::Configured,
                title_mode: Some("tabbeacon".to_owned()),
                tab_color_mode: Some("tabbeacon".to_owned()),
                activity_mode: Some("title-spinner".to_owned()),
                spinner_preset: Some("braille".to_owned()),
                theme: Some("muted-dark".to_owned()),
            },
            title: title.clone(),
            activity: ActivityDiagnostics {
                observation: "lease_based".to_owned(),
                worker_state_health: "healthy".to_owned(),
                active_leases: 0,
                stale_leases: 0,
                invalid_leases: 0,
            },
            workspace: WorkspaceDiagnostics {
                identity_subsystem_health: "healthy".to_owned(),
                alias_registry_count: Some(1),
            },
            doctor: DoctorDiagnostics {
                schema_version: 1,
                overall: DiagnosticStatus::Pass,
                checks,
                warnings: Vec::new(),
                failures: Vec::new(),
                title,
            },
        }
    }

    fn add_warning(report: &mut OperationalDiagnostics, id: &str, summary: &str) {
        report.doctor.checks.push(DiagnosticCheck {
            id: id.to_owned(),
            status: DiagnosticStatus::Warning,
            summary: summary.to_owned(),
        });
        report
            .doctor
            .warnings
            .push(crate::diagnostics::DiagnosticIssue {
                id: id.to_owned(),
                summary: summary.to_owned(),
            });
        report.doctor.overall = DiagnosticStatus::Warning;
    }

    #[test]
    fn healthy_status_and_doctor_are_compact_and_monochrome_safe() {
        let report = report();
        let snapshot = ManagementSnapshot::from_diagnostics(&report);
        let status = human_status_lines(&report, &snapshot, 80);
        let doctor = human_doctor_lines(&report.doctor, &snapshot, 80);

        assert!(status.join("\n").contains("TabBeacon Status — Healthy"));
        assert!(status.join("\n").contains("No action required."));
        assert!(status.len() < 24, "healthy status fits one ordinary screen");
        assert!(doctor.join("\n").contains("7 checks passed."));
        assert!(doctor.join("\n").contains("No action required."));
        assert!(
            status
                .iter()
                .chain(&doctor)
                .all(|line| !line.contains('\u{1b}'))
        );
    }

    #[test]
    fn hook_review_required_has_manual_next_action_and_trust_boundary() {
        let mut report = report();
        report.integration.hook_trust = HookTrustState::ReviewRequired;
        add_warning(&mut report, "hooks.trust", "Hook trust review is required");
        let snapshot = ManagementSnapshot::from_diagnostics(&report);
        let output = human_doctor_lines(&report.doctor, &snapshot, 120).join("\n");

        assert!(output.contains("Codex hook review is required"));
        assert!(output.contains("Why:"));
        assert!(output.contains("Next: Launch codex, open /hooks"));
        assert!(output.contains(
            "TabBeacon did not change: TabBeacon does not change application trust state."
        ));
    }

    #[test]
    fn unsupported_profile_title_repair_and_worker_warning_use_shared_actions() {
        let mut report = report();
        report.codex.profile_supported = false;
        report.codex.profile_state = "known_unadmitted".to_owned();
        report.title = title(TitleRemediationState::Available);
        report.activity.worker_state_health = "warning".to_owned();
        let snapshot = ManagementSnapshot::from_diagnostics(&report);
        let output = human_status_lines(&report, &snapshot, 120).join("\n");

        assert!(output.contains("Codex profile is not admitted"));
        assert!(output.contains("Windows Terminal title repair is available"));
        assert!(output.contains("Activity worker state needs attention"));
    }

    #[test]
    fn narrow_rendering_keeps_textual_state_without_control_sequences() {
        let mut report = report();
        report.activity.worker_state_health = "warning".to_owned();
        let snapshot = ManagementSnapshot::from_diagnostics(&report);
        let lines = human_status_lines(&report, &snapshot, 24);

        assert!(lines.iter().all(|line| line.chars().count() <= 24));
        assert!(lines.join("\n").contains("TabBeacon Status"));
        assert!(lines.iter().all(|line| !line.contains('\u{1b}')));
    }
}
