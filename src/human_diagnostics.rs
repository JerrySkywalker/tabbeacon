//! Typed Human documents for the shared management diagnostics projection.
//!
//! JSON and legacy key/value output remain in `diagnostics`. This module
//! converts the same normalized management state into locale-neutral
//! [`HumanDocument`] values for Human CLI and TUI renderers.

use crate::{
    diagnostics::{
        DiagnosticStatus, DoctorDiagnostics, HookTrustState, OperationalDiagnostics,
        TitleOwnershipState,
    },
    human_output::HumanTone,
    human_presentation::{
        HumanAction, HumanDocument, HumanField, HumanLine, HumanMessage, HumanMessageKey,
        HumanRenderer, HumanSection, HumanText, ManagementTextKind, ResolvedLocale,
        management_action_text, management_text, protected_state_text,
    },
    management::{ChangePlan, HealthIssue, ManagementHealth, ManagementSnapshot},
};

const DEFAULT_WIDTH: usize = 80;
const MIN_WIDTH: usize = 24;
const MAX_STATUS_ISSUES: usize = 3;

/// Returns the current text width without entering a terminal UI mode.
#[must_use]
pub fn terminal_width() -> usize {
    if !std::io::IsTerminal::is_terminal(&std::io::stdout()) {
        return DEFAULT_WIDTH;
    }
    std::env::var("COLUMNS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|width| *width >= MIN_WIDTH)
        .unwrap_or(DEFAULT_WIDTH)
}

/// Produces the shared semantic Human status document.
#[must_use]
pub fn human_status_document(
    report: &OperationalDiagnostics,
    snapshot: &ManagementSnapshot,
) -> HumanDocument {
    HumanDocument::new(
        HumanText::message(HumanMessageKey::Status),
        Some(health_text(snapshot.health)),
    )
    .with_section(integration_section(report, snapshot))
    .with_section(presentation_section(report, snapshot))
    .with_section(runtime_section(report, snapshot))
    .with_section(status_action_section(snapshot))
}

/// Produces the shared semantic Human doctor document.
#[must_use]
pub fn human_doctor_document(
    report: &DoctorDiagnostics,
    snapshot: &ManagementSnapshot,
) -> HumanDocument {
    let document = HumanDocument::new(
        HumanText::message(HumanMessageKey::Doctor),
        Some(health_text(snapshot.health)),
    );
    if snapshot.is_healthy() {
        let passed = report
            .checks
            .iter()
            .filter(|check| check.status == DiagnosticStatus::Pass)
            .count();
        let section = report.checks.iter().fold(
            HumanSection::new(None)
                .with_message(HumanMessage::plain(
                    HumanText::template(HumanMessageKey::ChecksPassed, [passed.to_string()]),
                    HumanTone::Success,
                ))
                .with_message(HumanMessage::plain(
                    HumanText::message(HumanMessageKey::NoActionRequired),
                    HumanTone::Success,
                )),
            |section, check| {
                section.with_message(HumanMessage::marked(
                    "OK",
                    management_text(
                        ManagementTextKind::CheckSummary,
                        &check.id,
                        check.summary.clone(),
                    ),
                    HumanTone::Success,
                ))
            },
        );
        return document.with_section(section);
    }

    let document =
        document.with_section(HumanSection::new(None).with_message(HumanMessage::plain(
            HumanText::template(
                HumanMessageKey::WarningsAndFailures,
                [
                    report.warnings.len().to_string(),
                    report.failures.len().to_string(),
                ],
            ),
            HumanTone::Attention,
        )));
    snapshot.issues.iter().fold(document, |document, issue| {
        document.with_section(doctor_issue_section(issue, &snapshot.change_plans))
    })
}

/// Renders a status document with explicit per-line semantic tones.
#[must_use]
pub fn render_human_status(
    report: &OperationalDiagnostics,
    snapshot: &ManagementSnapshot,
    locale: ResolvedLocale,
    width: usize,
) -> Vec<HumanLine> {
    HumanRenderer::new(locale, normalize_width(width))
        .render(&human_status_document(report, snapshot))
}

/// Renders a doctor document with explicit per-line semantic tones.
#[must_use]
pub fn render_human_doctor(
    report: &DoctorDiagnostics,
    snapshot: &ManagementSnapshot,
    locale: ResolvedLocale,
    width: usize,
) -> Vec<HumanLine> {
    HumanRenderer::new(locale, normalize_width(width))
        .render(&human_doctor_document(report, snapshot))
}

/// Compatibility English text view for existing Human callers and tests.
#[must_use]
pub fn human_status_lines(
    report: &OperationalDiagnostics,
    snapshot: &ManagementSnapshot,
    width: usize,
) -> Vec<String> {
    render_human_status(report, snapshot, ResolvedLocale::EnUs, width)
        .into_iter()
        .map(|line| line.text().to_owned())
        .collect()
}

/// Compatibility English text view for existing Human callers and tests.
#[must_use]
pub fn human_doctor_lines(
    report: &DoctorDiagnostics,
    snapshot: &ManagementSnapshot,
    width: usize,
) -> Vec<String> {
    render_human_doctor(report, snapshot, ResolvedLocale::EnUs, width)
        .into_iter()
        .map(|line| line.text().to_owned())
        .collect()
}

fn integration_section(
    report: &OperationalDiagnostics,
    snapshot: &ManagementSnapshot,
) -> HumanSection {
    HumanSection::new(Some(HumanText::message(HumanMessageKey::Integration)))
        .with_field(HumanField::new(
            Some(state_marker(snapshot.health)),
            HumanText::message(HumanMessageKey::TabBeacon),
            HumanText::literal(report.tabbeacon.version.clone()),
            health_tone(snapshot.health),
        ))
        .with_field(HumanField::new(
            Some(state_marker(if report.codex.profile_supported {
                ManagementHealth::Healthy
            } else {
                ManagementHealth::Error
            })),
            HumanText::message(HumanMessageKey::Codex),
            codex_version_text(report),
            if report.codex.profile_supported {
                HumanTone::Success
            } else {
                HumanTone::Failure
            },
        ))
        .with_field(HumanField::new(
            Some(marker_for_status(report.integration.declaration_status)),
            HumanText::message(HumanMessageKey::Hooks),
            hook_summary_text(report),
            diagnostic_tone(report.integration.declaration_status),
        ))
        .with_field(HumanField::new(
            Some(state_marker_for_issue(snapshot, "hooks.")),
            HumanText::message(HumanMessageKey::HookTrust),
            hook_trust_text(report),
            issue_tone(snapshot, "hooks."),
        ))
}

fn presentation_section(
    report: &OperationalDiagnostics,
    snapshot: &ManagementSnapshot,
) -> HumanSection {
    HumanSection::new(Some(HumanText::message(HumanMessageKey::Presentation)))
        .with_field(HumanField::new(
            Some(state_marker_for_issue(snapshot, "terminal.title")),
            HumanText::message(HumanMessageKey::Title),
            title_text(report),
            issue_tone(snapshot, "terminal.title"),
        ))
        .with_field(HumanField::new(
            Some(state_marker_for_issue(snapshot, "workers.")),
            HumanText::message(HumanMessageKey::Activity),
            option_text(report.presentation.spinner_preset.as_deref()),
            issue_tone(snapshot, "workers."),
        ))
        .with_field(HumanField::new(
            Some(state_marker_for_issue(snapshot, "settings.")),
            HumanText::message(HumanMessageKey::Theme),
            option_text(report.presentation.theme.as_deref()),
            issue_tone(snapshot, "settings."),
        ))
}

fn runtime_section(report: &OperationalDiagnostics, snapshot: &ManagementSnapshot) -> HumanSection {
    HumanSection::new(Some(HumanText::message(HumanMessageKey::Runtime)))
        .with_field(HumanField::new(
            Some(state_marker_for_issue(snapshot, "workers.")),
            HumanText::message(HumanMessageKey::Workers),
            worker_health_text(&report.activity.worker_state_health),
            issue_tone(snapshot, "workers."),
        ))
        .with_message(HumanMessage::plain(
            HumanText::template(
                HumanMessageKey::ActiveAndStale,
                [
                    report.activity.active_leases.to_string(),
                    report.activity.stale_leases.to_string(),
                ],
            ),
            HumanTone::Dim,
        ))
}

fn status_action_section(snapshot: &ManagementSnapshot) -> HumanSection {
    if snapshot.is_healthy() {
        return HumanSection::new(None).with_message(HumanMessage::plain(
            HumanText::message(HumanMessageKey::NoActionRequired),
            HumanTone::Success,
        ));
    }

    let mut section = HumanSection::new(Some(HumanText::message(HumanMessageKey::Attention)));
    for issue in snapshot.issues.iter().take(MAX_STATUS_ISSUES) {
        section = section.with_message(HumanMessage::marked(
            "!",
            management_text(
                ManagementTextKind::IssueTitle,
                &issue.id,
                issue.title.clone(),
            ),
            severity_tone(issue),
        ));
        if let Some(action) = &issue.remediation {
            section = section.with_action(HumanAction::new(
                management_action_text(&issue.id, &action.id, action.instruction.clone()),
                HumanTone::Dim,
            ));
        }
    }
    let remaining = snapshot.issues.len().saturating_sub(MAX_STATUS_ISSUES);
    if remaining > 0 {
        section = section.with_message(HumanMessage::plain(
            HumanText::template(
                HumanMessageKey::AdditionalConditions,
                [remaining.to_string()],
            ),
            HumanTone::Dim,
        ));
    }
    section
}

fn doctor_issue_section(issue: &HealthIssue, plans: &[ChangePlan]) -> HumanSection {
    let mut section = HumanSection::new(None)
        .with_message(HumanMessage::marked(
            severity_marker(issue),
            management_text(
                ManagementTextKind::IssueTitle,
                &issue.id,
                issue.title.clone(),
            ),
            severity_tone(issue),
        ))
        .with_message(HumanMessage::prefixed(
            HumanText::message(HumanMessageKey::Why),
            management_text(
                ManagementTextKind::IssueExplanation,
                &issue.id,
                issue.explanation.clone(),
            ),
            HumanTone::Dim,
        ));
    if let Some(action) = &issue.remediation {
        section = section.with_action(HumanAction::new(
            management_action_text(&issue.id, &action.id, action.instruction.clone()),
            HumanTone::Dim,
        ));
        if let Some(plan) = plans.iter().find(|plan| plan.action_id == action.id) {
            for _protected in &plan.protected_state {
                section = section.with_message(HumanMessage::prefixed(
                    HumanText::message(HumanMessageKey::ProtectedState),
                    protected_state_text(plan.safety),
                    HumanTone::Dim,
                ));
            }
        }
    }
    section
}

fn normalize_width(width: usize) -> usize {
    width.max(MIN_WIDTH)
}

fn health_text(health: ManagementHealth) -> HumanText {
    HumanText::message(match health {
        ManagementHealth::Healthy => HumanMessageKey::Healthy,
        ManagementHealth::Warning => HumanMessageKey::NeedsAttention,
        ManagementHealth::Error => HumanMessageKey::ActionNeeded,
    })
}

fn health_tone(health: ManagementHealth) -> HumanTone {
    match health {
        ManagementHealth::Healthy => HumanTone::Success,
        ManagementHealth::Warning => HumanTone::Attention,
        ManagementHealth::Error => HumanTone::Failure,
    }
}

fn diagnostic_tone(status: DiagnosticStatus) -> HumanTone {
    match status {
        DiagnosticStatus::Pass => HumanTone::Success,
        DiagnosticStatus::Warning => HumanTone::Attention,
        DiagnosticStatus::Fail => HumanTone::Failure,
    }
}

fn issue_tone(snapshot: &ManagementSnapshot, prefix: &str) -> HumanTone {
    snapshot
        .issues
        .iter()
        .find(|issue| issue.id.starts_with(prefix))
        .map_or(HumanTone::Success, severity_tone)
}

fn severity_tone(issue: &HealthIssue) -> HumanTone {
    match issue.severity {
        crate::management::HealthSeverity::Warning => HumanTone::Attention,
        crate::management::HealthSeverity::Error => HumanTone::Failure,
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

fn option_text(value: Option<&str>) -> HumanText {
    value.map_or_else(
        || HumanText::message(HumanMessageKey::Unavailable),
        |value| HumanText::literal(bounded_label(value)),
    )
}

fn codex_version_text(report: &OperationalDiagnostics) -> HumanText {
    let Some(version) = report.codex.version.as_deref() else {
        return HumanText::message(HumanMessageKey::CodexVersionUnavailable);
    };
    HumanText::template(
        if report.codex.profile_supported {
            HumanMessageKey::CodexVersionSupported
        } else {
            HumanMessageKey::CodexVersionNotAdmitted
        },
        [version.to_owned()],
    )
}

fn hook_trust_text(report: &OperationalDiagnostics) -> HumanText {
    HumanText::message(match report.integration.hook_trust {
        HookTrustState::Active => HumanMessageKey::TrustActive,
        HookTrustState::ReviewRequired => HumanMessageKey::TrustReviewRequired,
        HookTrustState::Failed => HumanMessageKey::TrustNotProven,
        HookTrustState::Unavailable => HumanMessageKey::Unavailable,
    })
}

fn title_text(report: &OperationalDiagnostics) -> HumanText {
    HumanText::message(match report.integration.title_ownership {
        TitleOwnershipState::Tabbeacon => HumanMessageKey::TitleOwnedByTabBeacon,
        TitleOwnershipState::NativeOrOff => HumanMessageKey::TitleNativeOrOff,
        TitleOwnershipState::Conflict => HumanMessageKey::TitleOwnershipConflict,
        TitleOwnershipState::Unavailable => HumanMessageKey::Unavailable,
    })
}

fn worker_health_text(value: &str) -> HumanText {
    HumanText::message(match value {
        "healthy" => HumanMessageKey::Healthy,
        "warning" => HumanMessageKey::NeedsAttention,
        "unavailable" => HumanMessageKey::Unavailable,
        _ => return HumanText::literal(bounded_label(value)),
    })
}

fn bounded_label(value: &str) -> String {
    value.replace(['_', '-'], " ")
}

fn hook_summary_text(report: &OperationalDiagnostics) -> HumanText {
    match report.integration.owned_hook_count {
        Some(count) if report.integration.declaration_status == DiagnosticStatus::Pass => {
            HumanText::template(HumanMessageKey::ActiveCount, [count.to_string()])
        }
        Some(count) => {
            HumanText::template(HumanMessageKey::NeedsAttentionCount, [count.to_string()])
        }
        None => HumanText::message(HumanMessageKey::Unavailable),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        human_doctor_document, human_doctor_lines, human_status_document, human_status_lines,
        render_human_doctor, render_human_status,
    };
    use crate::{
        diagnostics::{
            ActivityDiagnostics, CodexDiagnostics, DiagnosticCheck, DiagnosticStatus,
            DoctorDiagnostics, HookTrustState, IntegrationDiagnostics, OperationalDiagnostics,
            PresentationDiagnostics, PresentationSettingsSource, TabBeaconDiagnostics,
            TitleOwnershipState, WorkspaceDiagnostics,
        },
        human_presentation::{ResolvedLocale, display_width},
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
    fn status_and_doctor_build_typed_documents_then_render_english() {
        let report = report();
        let snapshot = ManagementSnapshot::from_diagnostics(&report);
        assert!(human_status_document(&report, &snapshot).status().is_some());
        assert!(
            !human_status_document(&report, &snapshot)
                .sections()
                .is_empty()
        );
        assert!(
            human_doctor_document(&report.doctor, &snapshot)
                .status()
                .is_some()
        );
        let status = human_status_lines(&report, &snapshot, 80).join("\n");
        let doctor = human_doctor_lines(&report.doctor, &snapshot, 80).join("\n");
        assert!(status.contains("TabBeacon Status — Healthy"));
        assert!(status.contains("No action required."));
        assert!(doctor.contains("7 checks passed."));
    }

    #[test]
    fn doctor_keeps_manual_next_action_and_trust_boundary() {
        let mut report = report();
        report.integration.hook_trust = HookTrustState::ReviewRequired;
        add_warning(&mut report, "hooks.trust", "Hook trust review is required");
        let snapshot = ManagementSnapshot::from_diagnostics(&report);
        let output = human_doctor_lines(&report.doctor, &snapshot, 120).join("\n");

        assert!(output.contains("Codex Hook review is required"));
        assert!(output.contains("Why:"));
        assert!(output.contains("Next: Launch codex, open /hooks"));
        assert!(output.contains(
            "TabBeacon did not change: TabBeacon does not change application trust state."
        ));
    }

    #[test]
    fn cjk_rendering_obeys_display_cell_width_without_escape_sequences() {
        let report = report();
        let snapshot = ManagementSnapshot::from_diagnostics(&report);
        let lines = render_human_status(&report, &snapshot, ResolvedLocale::ZhCn, 24);
        assert!(lines.iter().all(|line| display_width(line.text()) <= 24));
        assert!(lines.iter().all(|line| !line.text().contains('\u{1b}')));
        assert!(lines.iter().any(|line| line.text().contains("状态")));
    }

    #[test]
    fn known_management_diagnostic_ids_render_from_the_chinese_catalog() {
        let mut report = report();
        report.integration.hook_trust = HookTrustState::ReviewRequired;
        add_warning(&mut report, "hooks.trust", "Hook trust review is required");
        let snapshot = ManagementSnapshot::from_diagnostics(&report);
        let output = render_human_doctor(&report.doctor, &snapshot, ResolvedLocale::ZhCn, 500)
            .into_iter()
            .map(|line| line.text().to_owned())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(output.contains("需要审查 Codex 钩子"));
        assert!(output.contains("原因: 受管定义已存在，但 Codex 信任仍是人工审查边界。"));
        assert!(output.contains("下一步: 启动 codex，打开 /hooks，并审查 TabBeacon 定义。"));
        assert!(output.contains("TabBeacon 未更改: TabBeacon 不会更改应用信任状态。"));
        assert!(!output.contains("Codex Hook review is required"));
        assert!(!output.contains("Hook trust review is required"));
    }

    #[test]
    fn shared_status_actions_remain_present_for_existing_issue_families() {
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
}
