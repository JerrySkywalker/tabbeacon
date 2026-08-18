//! Shared, non-mutating management projection for human-facing frontends.
//!
//! The model receives already-bounded operational diagnostics. It never reads
//! configuration itself and offers plans only; existing stores and integration
//! modules remain the sole owners of persistent mutations.

use serde::Serialize;

use crate::{
    diagnostics::{
        DiagnosticStatus, DoctorDiagnostics, HookTrustState, OperationalDiagnostics,
        PresentationSettingsSource, TitleOwnershipState,
    },
    windows_terminal_policy::TitleRemediationState,
};

/// Aggregate health used by every human management surface.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagementHealth {
    /// No intervention is currently recommended.
    Healthy,
    /// A safe condition needs a human's attention.
    Warning,
    /// An installation or runtime condition needs a next action.
    Error,
}

/// Severity for one actionable management issue.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthSeverity {
    /// A bounded condition needs attention but does not block normal use.
    Warning,
    /// A condition prevents `TabBeacon` from proving its managed behavior.
    Error,
}

impl From<HealthSeverity> for ManagementHealth {
    fn from(value: HealthSeverity) -> Self {
        match value {
            HealthSeverity::Warning => Self::Warning,
            HealthSeverity::Error => Self::Error,
        }
    }
}

/// The safety boundary attached to every recommended action.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionSafety {
    /// The action only inspects existing state.
    ReadOnly,
    /// A person must perform the action in the owning application.
    ManualAction,
    /// Existing ownership checks can safely preview the scoped repair.
    PreviewableSafeRepair,
    /// The Owner must explicitly choose the existing mutation operation.
    OwnerExplicitRequired,
    /// `TabBeacon` must not fabricate an automation for this condition.
    UnsupportedAutomation,
}

/// A bounded next step derived from management state.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RecommendedAction {
    /// Stable action identity shared by status, doctor, setup, and Control Center.
    pub id: String,
    /// Concise human label.
    pub title: String,
    /// Copyable, bounded instruction.
    pub instruction: String,
    /// Explicit automation and ownership boundary.
    pub safety: ActionSafety,
}

/// One health condition projected from safe diagnostics.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct HealthIssue {
    /// Stable management condition identifier.
    pub id: String,
    /// Human-safe severity.
    pub severity: HealthSeverity,
    /// Concise title for a status row or doctor section.
    pub title: String,
    /// Bounded explanation of why the condition matters.
    pub explanation: String,
    /// Typed next step when a safe next step exists.
    pub remediation: Option<RecommendedAction>,
}

/// A frontend-safe description of a requested change, not an executor.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ChangePlan {
    /// Action that this plan explains.
    pub action_id: String,
    /// Action safety remains explicit when the plan is rendered separately.
    pub safety: ActionSafety,
    /// Safe summary of the requested change, if any.
    pub proposed_changes: Vec<String>,
    /// Ownership guarantees retained by the existing mutation authority.
    pub protected_state: Vec<String>,
    /// Manual follow-up that no frontend may automate.
    pub manual_follow_up: Vec<String>,
}

/// Bounded daily facts selected by the management layer for the Control Center.
///
/// This is intentionally a projection of already-safe diagnostics. It contains
/// no configuration text, hook bodies, provider payloads, or session identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ManagementOverview {
    /// Compiled `TabBeacon` version.
    pub tabbeacon_version: String,
    /// Detected Codex version or a bounded unavailable label.
    pub codex_version: String,
    /// Admitted Codex profile state.
    pub codex_profile: String,
    /// Bounded owned-hook declaration state.
    pub hooks: String,
    /// Human-readable trust state; trust itself remains manual.
    pub hook_trust: String,
    /// Existing title ownership diagnosis.
    pub title_ownership: String,
    /// Presentation settings source health.
    pub settings_source: String,
    /// Lease-based worker health.
    pub worker_health: String,
    /// Count of active safe worker leases.
    pub active_workers: usize,
    /// Count of stale safe worker leases.
    pub stale_workers: usize,
}

impl ManagementOverview {
    /// Produces the safe daily summary consumed by the Control Center.
    #[must_use]
    pub fn from_diagnostics(report: &OperationalDiagnostics) -> Self {
        let hooks = report.integration.owned_hook_count.map_or_else(
            || "Unavailable".to_owned(),
            |count| format!("{count} managed"),
        );
        Self {
            tabbeacon_version: report.tabbeacon.version.clone(),
            codex_version: report
                .codex
                .version
                .clone()
                .unwrap_or_else(|| "Unavailable".to_owned()),
            codex_profile: report.codex.profile_state.clone(),
            hooks,
            hook_trust: report.integration.hook_trust.as_str().to_owned(),
            title_ownership: report.integration.title_ownership.as_str().to_owned(),
            settings_source: report.presentation.source.as_str().to_owned(),
            worker_health: report.activity.worker_state_health.clone(),
            active_workers: report.activity.active_leases,
            stale_workers: report.activity.stale_leases,
        }
    }
}

impl Default for ManagementOverview {
    fn default() -> Self {
        Self {
            tabbeacon_version: "Unavailable".to_owned(),
            codex_version: "Unavailable".to_owned(),
            codex_profile: "unavailable".to_owned(),
            hooks: "Unavailable".to_owned(),
            hook_trust: "unavailable".to_owned(),
            title_ownership: "unavailable".to_owned(),
            settings_source: "unavailable".to_owned(),
            worker_health: "unavailable".to_owned(),
            active_workers: 0,
            stale_workers: 0,
        }
    }
}

/// The one management projection consumed by human-facing frontends.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ManagementSnapshot {
    /// Aggregate state derived only from `issues`.
    pub health: ManagementHealth,
    /// Stable, safe problem list ordered by remediation priority.
    pub issues: Vec<HealthIssue>,
    /// De-duplicated next steps for the current state.
    pub recommended_actions: Vec<RecommendedAction>,
    /// Explanatory plans only; applying remains owned by existing modules.
    pub change_plans: Vec<ChangePlan>,
}

impl ManagementSnapshot {
    /// Builds the shared management projection from one read-only diagnostic pass.
    #[must_use]
    pub fn from_diagnostics(report: &OperationalDiagnostics) -> Self {
        let mut issues = Vec::new();
        let mut actions = Vec::new();

        project_installation(report, &mut issues, &mut actions);
        project_profile(report, &mut issues, &mut actions);
        project_hook_trust(report, &mut issues, &mut actions);
        project_title_policy(report, &mut issues, &mut actions);
        project_settings(report, &mut issues, &mut actions);
        project_workers(report, &mut issues, &mut actions);
        project_remaining_checks(report, &mut issues, &mut actions);

        let health = issues
            .iter()
            .map(|issue| ManagementHealth::from(issue.severity))
            .max()
            .unwrap_or(ManagementHealth::Healthy);
        let change_plans = actions.iter().map(ChangePlan::for_action).collect();
        Self {
            health,
            issues,
            recommended_actions: actions,
            change_plans,
        }
    }

    /// Whether every management surface can safely show no-action-required.
    #[must_use]
    pub const fn is_healthy(&self) -> bool {
        matches!(self.health, ManagementHealth::Healthy)
    }
}

impl ChangePlan {
    fn for_action(action: &RecommendedAction) -> Self {
        let (proposed_changes, protected_state, manual_follow_up) = match action.safety {
            ActionSafety::ReadOnly => (
                Vec::new(),
                vec!["No persistent configuration change is requested.".to_owned()],
                vec![action.instruction.clone()],
            ),
            ActionSafety::ManualAction => (
                Vec::new(),
                vec!["TabBeacon does not change application trust state.".to_owned()],
                vec![action.instruction.clone()],
            ),
            ActionSafety::PreviewableSafeRepair => (
                vec!["Preview the existing ownership-scoped repair before applying it.".to_owned()],
                vec!["Unrelated Windows Terminal settings remain untouched.".to_owned()],
                vec![action.instruction.clone()],
            ),
            ActionSafety::OwnerExplicitRequired => (
                vec!["Request the existing ownership-aware operation.".to_owned()],
                vec!["No change occurs until the Owner explicitly applies it.".to_owned()],
                Vec::new(),
            ),
            ActionSafety::UnsupportedAutomation => (
                Vec::new(),
                vec!["TabBeacon will not fabricate an unsupported automation path.".to_owned()],
                vec![action.instruction.clone()],
            ),
        };
        Self {
            action_id: action.id.clone(),
            safety: action.safety,
            proposed_changes,
            protected_state,
            manual_follow_up,
        }
    }
}

fn project_installation(
    report: &OperationalDiagnostics,
    issues: &mut Vec<HealthIssue>,
    actions: &mut Vec<RecommendedAction>,
) {
    if !report.integration.installed {
        add_issue(
            issues,
            actions,
            "integration.not_installed",
            HealthSeverity::Error,
            "TabBeacon integration is not installed",
            "TabBeacon cannot prove that its managed Codex hooks are present.",
            Some(action(
                "integration.setup_codex",
                "Install Codex integration",
                "Run tabbeacon setup codex when you are ready to apply the ownership-aware setup.",
                ActionSafety::OwnerExplicitRequired,
            )),
        );
    }
    if check_is_not_pass(&report.doctor, "hooks.declarations") {
        add_issue(
            issues,
            actions,
            "hooks.declarations_out_of_sync",
            HealthSeverity::Error,
            "Managed hook declarations need attention",
            "The installed declarations are missing or modified, so the integration is not proven current.",
            Some(action(
                "integration.setup_codex",
                "Reconcile Codex integration",
                "Run tabbeacon setup codex to request the existing ownership-aware reconciliation.",
                ActionSafety::OwnerExplicitRequired,
            )),
        );
    }
    if check_is_not_pass(&report.doctor, "hooks.currentness") {
        add_issue(
            issues,
            actions,
            "hooks.integration_upgrade_required",
            HealthSeverity::Warning,
            "TabBeacon integration upgrade required",
            "Owned hook declarations do not match the current admitted integration shape.",
            Some(action(
                "integration.setup_codex",
                "Upgrade Codex integration",
                "Run tabbeacon setup codex to request the existing ownership-aware upgrade.",
                ActionSafety::OwnerExplicitRequired,
            )),
        );
    }
    if check_is_not_pass(&report.doctor, "tabbeacon.executable") {
        add_issue(
            issues,
            actions,
            "integration.executable_unavailable",
            HealthSeverity::Error,
            "Managed executable is unavailable",
            "The owned hook integration cannot find the executable it was configured to invoke.",
            Some(action(
                "integration.executable_guidance",
                "Repair the TabBeacon installation",
                "Restore an admitted TabBeacon executable, then inspect status again.",
                ActionSafety::UnsupportedAutomation,
            )),
        );
    }
}

fn project_profile(
    report: &OperationalDiagnostics,
    issues: &mut Vec<HealthIssue>,
    actions: &mut Vec<RecommendedAction>,
) {
    if report.codex.profile_supported {
        return;
    }
    let (id, title, explanation) = match report.codex.profile_state.as_str() {
        "known_unadmitted" => (
            "codex.profile_unadmitted",
            "Codex profile is not admitted",
            "This detected Codex version has no admitted TabBeacon hook profile.",
        ),
        _ => (
            "codex.profile_unavailable",
            "Codex compatibility is unavailable",
            "TabBeacon cannot safely prove an admitted Codex hook profile.",
        ),
    };
    add_issue(
        issues,
        actions,
        id,
        HealthSeverity::Error,
        title,
        explanation,
        Some(action(
            "codex.profile_guidance",
            "Use an admitted Codex profile",
            "Use a supported Codex version or wait for an explicitly admitted TabBeacon profile; no support is fabricated automatically.",
            ActionSafety::UnsupportedAutomation,
        )),
    );
}

fn project_hook_trust(
    report: &OperationalDiagnostics,
    issues: &mut Vec<HealthIssue>,
    actions: &mut Vec<RecommendedAction>,
) {
    let (id, severity, title, explanation) = match report.integration.hook_trust {
        HookTrustState::Active => return,
        HookTrustState::ReviewRequired => (
            "hooks.review_required",
            HealthSeverity::Warning,
            "Codex hook review is required",
            "The owned definitions are present, but Codex trust remains a human review boundary.",
        ),
        HookTrustState::Failed | HookTrustState::Unavailable => (
            "hooks.trust_unproven",
            HealthSeverity::Error,
            "Codex hook trust is not proven",
            "TabBeacon cannot mark hook definitions trusted or infer that trust from configuration alone.",
        ),
    };
    add_issue(
        issues,
        actions,
        id,
        severity,
        title,
        explanation,
        Some(action(
            "hooks.review_in_codex",
            "Review hooks in Codex",
            "Launch codex, open /hooks, and review the TabBeacon definitions.",
            ActionSafety::ManualAction,
        )),
    );
}

fn project_title_policy(
    report: &OperationalDiagnostics,
    issues: &mut Vec<HealthIssue>,
    actions: &mut Vec<RecommendedAction>,
) {
    match report.title.remediation_available {
        TitleRemediationState::Available => add_issue(
            issues,
            actions,
            "terminal.title_repair_available",
            HealthSeverity::Warning,
            "Windows Terminal title repair is available",
            "The existing policy subsystem proved one active-profile repair scope without guessing unrelated settings.",
            Some(action(
                "terminal.title_policy_repair",
                "Preview title policy repair",
                "Inspect with tabbeacon title-policy inspect, then explicitly choose tabbeacon title-policy repair if the scoped change is correct.",
                ActionSafety::PreviewableSafeRepair,
            )),
        ),
        TitleRemediationState::NotNeeded | TitleRemediationState::AlreadyOwned => {}
        TitleRemediationState::DiagnoseOnly
        | TitleRemediationState::BlockedAmbiguous
        | TitleRemediationState::BlockedDrift
        | TitleRemediationState::Unavailable => {
            if !report
                .title
                .application_title_policy
                .permits_application_titles()
            {
                add_issue(
                    issues,
                    actions,
                    "terminal.title_diagnose_only",
                    HealthSeverity::Warning,
                    "Windows Terminal title policy needs diagnosis",
                    "The current policy cannot safely identify a repair scope, so TabBeacon will not mutate settings.",
                    Some(action(
                        "terminal.title_policy_inspect",
                        "Inspect title policy",
                        "Run tabbeacon title-policy inspect for the bounded policy diagnosis.",
                        ActionSafety::ReadOnly,
                    )),
                );
            }
        }
    }
    if report.integration.title_ownership == TitleOwnershipState::Conflict {
        add_issue(
            issues,
            actions,
            "terminal.title_ownership_conflict",
            HealthSeverity::Error,
            "Codex title ownership conflicts with the selected preference",
            "The existing owned integration cannot prove its terminal-title preference is reconciled.",
            Some(action(
                "integration.setup_codex",
                "Reconcile Codex title ownership",
                "Run tabbeacon setup codex to request the existing ownership-aware reconciliation.",
                ActionSafety::OwnerExplicitRequired,
            )),
        );
    }
}

fn project_settings(
    report: &OperationalDiagnostics,
    issues: &mut Vec<HealthIssue>,
    actions: &mut Vec<RecommendedAction>,
) {
    let (id, severity, title, explanation, action) = match report.presentation.source {
        PresentationSettingsSource::Default | PresentationSettingsSource::Configured => return,
        PresentationSettingsSource::Invalid => (
            "settings.invalid",
            HealthSeverity::Error,
            "Presentation settings are invalid",
            "TabBeacon did not interpret or overwrite the malformed settings document.",
            action(
                "settings.reset_explicitly",
                "Choose an explicit settings reset",
                "Inspect the settings first; run tabbeacon config reset only if you intentionally want the default presentation settings.",
                ActionSafety::OwnerExplicitRequired,
            ),
        ),
        PresentationSettingsSource::Unavailable => (
            "settings.unavailable",
            HealthSeverity::Warning,
            "Presentation settings are unavailable",
            "TabBeacon cannot safely inspect the current settings location.",
            action(
                "settings.inspect_environment",
                "Inspect settings availability",
                "Restore access to the settings location, then run tabbeacon status again.",
                ActionSafety::ReadOnly,
            ),
        ),
    };
    add_issue(
        issues,
        actions,
        id,
        severity,
        title,
        explanation,
        Some(action),
    );
}

fn project_workers(
    report: &OperationalDiagnostics,
    issues: &mut Vec<HealthIssue>,
    actions: &mut Vec<RecommendedAction>,
) {
    let (id, severity, title, explanation) = match report.activity.worker_state_health.as_str() {
        "healthy" => return,
        "warning" => (
            "workers.warning",
            HealthSeverity::Warning,
            "Activity worker state needs attention",
            "Stale, invalid, or bounded-out activity leases were observed without exposing their contents.",
        ),
        _ => (
            "workers.unavailable",
            HealthSeverity::Warning,
            "Activity worker state is unavailable",
            "TabBeacon cannot safely inspect the activity-lease aggregate.",
        ),
    };
    add_issue(
        issues,
        actions,
        id,
        severity,
        title,
        explanation,
        Some(action(
            "workers.inspect_status",
            "Inspect activity diagnostics",
            "Run tabbeacon status or tabbeacon doctor to review the bounded worker health summary.",
            ActionSafety::ReadOnly,
        )),
    );
}

fn project_remaining_checks(
    report: &OperationalDiagnostics,
    issues: &mut Vec<HealthIssue>,
    actions: &mut Vec<RecommendedAction>,
) {
    for check in &report.doctor.checks {
        if check.status == DiagnosticStatus::Pass || issue_for_check(&check.id, issues) {
            continue;
        }
        let severity = severity_from_status(check.status);
        add_issue(
            issues,
            actions,
            format!("diagnostics.{}", check.id),
            severity,
            "Additional diagnostic attention is required",
            "A bounded diagnostic check needs review; its underlying state is not changed by this management projection.",
            Some(action(
                "diagnostics.inspect",
                "Inspect diagnostics",
                "Run tabbeacon doctor to review the current bounded diagnostic result.",
                ActionSafety::ReadOnly,
            )),
        );
    }
}

fn add_issue(
    issues: &mut Vec<HealthIssue>,
    actions: &mut Vec<RecommendedAction>,
    id: impl Into<String>,
    severity: HealthSeverity,
    title: impl Into<String>,
    explanation: impl Into<String>,
    remediation: Option<RecommendedAction>,
) {
    let id = id.into();
    if issues.iter().any(|issue| issue.id == id) {
        return;
    }
    if let Some(action) = remediation.as_ref()
        && !actions.iter().any(|existing| existing.id == action.id)
    {
        actions.push(action.clone());
    }
    issues.push(HealthIssue {
        id,
        severity,
        title: title.into(),
        explanation: explanation.into(),
        remediation,
    });
}

fn action(
    id: impl Into<String>,
    title: impl Into<String>,
    instruction: impl Into<String>,
    safety: ActionSafety,
) -> RecommendedAction {
    RecommendedAction {
        id: id.into(),
        title: title.into(),
        instruction: instruction.into(),
        safety,
    }
}

fn check_is_not_pass(doctor: &DoctorDiagnostics, id: &str) -> bool {
    doctor
        .checks
        .iter()
        .find(|check| check.id == id)
        .is_some_and(|check| check.status != DiagnosticStatus::Pass)
}

fn issue_for_check(id: &str, issues: &[HealthIssue]) -> bool {
    match id {
        "tabbeacon.executable" => issues
            .iter()
            .any(|issue| issue.id == "integration.executable_unavailable"),
        "ownership.manifest" => issues
            .iter()
            .any(|issue| issue.id == "integration.not_installed"),
        "hooks.declarations" => issues
            .iter()
            .any(|issue| issue.id == "hooks.declarations_out_of_sync"),
        "hooks.currentness" => issues
            .iter()
            .any(|issue| issue.id == "hooks.integration_upgrade_required"),
        "hooks.trust" => issues.iter().any(|issue| {
            issue.id == "hooks.review_required" || issue.id.starts_with("hooks.trust")
        }),
        "codex.version" | "codex.hook-profile" => issues
            .iter()
            .any(|issue| issue.id.starts_with("codex.profile_")),
        "terminal.title" => issues
            .iter()
            .any(|issue| issue.id.starts_with("terminal.title_")),
        _ => false,
    }
}

fn severity_from_status(status: DiagnosticStatus) -> HealthSeverity {
    match status {
        DiagnosticStatus::Pass => {
            unreachable!("passed checks are filtered before severity mapping")
        }
        DiagnosticStatus::Warning => HealthSeverity::Warning,
        DiagnosticStatus::Fail => HealthSeverity::Error,
    }
}

#[cfg(test)]
mod tests {
    use super::{ActionSafety, ManagementHealth, ManagementSnapshot};
    use crate::{
        diagnostics::{
            ActivityDiagnostics, CodexDiagnostics, DiagnosticCheck, DiagnosticIssue,
            DiagnosticStatus, DoctorDiagnostics, HookTrustState, IntegrationDiagnostics,
            OperationalDiagnostics, PresentationDiagnostics, PresentationSettingsSource,
            TabBeaconDiagnostics, TitleOwnershipState, WorkspaceDiagnostics,
        },
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
                checks: Vec::new(),
                warnings: Vec::new(),
                failures: Vec::new(),
                title,
            },
        }
    }

    fn add_check(report: &mut OperationalDiagnostics, id: &str, status: DiagnosticStatus) {
        report.doctor.checks.push(DiagnosticCheck {
            id: id.to_owned(),
            status,
            summary: "bounded fixture summary".to_owned(),
        });
        match status {
            DiagnosticStatus::Pass => {}
            DiagnosticStatus::Warning => report.doctor.warnings.push(DiagnosticIssue {
                id: id.to_owned(),
                summary: "bounded fixture summary".to_owned(),
            }),
            DiagnosticStatus::Fail => report.doctor.failures.push(DiagnosticIssue {
                id: id.to_owned(),
                summary: "bounded fixture summary".to_owned(),
            }),
        }
    }

    fn issue<'a>(snapshot: &'a ManagementSnapshot, id: &str) -> &'a super::HealthIssue {
        snapshot
            .issues
            .iter()
            .find(|issue| issue.id == id)
            .unwrap_or_else(|| panic!("missing issue {id}"))
    }

    #[test]
    fn healthy_installation_has_no_actions() {
        let snapshot = ManagementSnapshot::from_diagnostics(&report());

        assert_eq!(snapshot.health, ManagementHealth::Healthy);
        assert!(snapshot.is_healthy());
        assert!(snapshot.issues.is_empty());
        assert!(snapshot.recommended_actions.is_empty());
        assert!(snapshot.change_plans.is_empty());
    }

    #[test]
    fn hook_review_is_manual_and_never_a_trust_automation() {
        let mut report = report();
        report.integration.hook_trust = HookTrustState::ReviewRequired;
        add_check(&mut report, "hooks.trust", DiagnosticStatus::Warning);

        let snapshot = ManagementSnapshot::from_diagnostics(&report);
        let issue = issue(&snapshot, "hooks.review_required");
        let action = issue.remediation.as_ref().expect("manual action exists");

        assert_eq!(action.safety, ActionSafety::ManualAction);
        assert!(action.instruction.contains("Launch codex"));
        assert!(action.instruction.contains("/hooks"));
        assert_eq!(snapshot.issues.len(), 1);
        assert_eq!(snapshot.recommended_actions.len(), 1);
        assert_eq!(snapshot.change_plans[0].proposed_changes.len(), 0);
        assert!(snapshot.change_plans[0].protected_state[0].contains("trust"));
    }

    #[test]
    fn stale_hooks_profile_and_title_repair_have_explicit_distinct_safety_classes() {
        let mut report = report();
        add_check(&mut report, "hooks.currentness", DiagnosticStatus::Fail);
        report.codex.profile_supported = false;
        report.codex.profile_state = "known_unadmitted".to_owned();
        report.title = title(TitleRemediationState::Available);

        let snapshot = ManagementSnapshot::from_diagnostics(&report);
        assert_eq!(
            issue(&snapshot, "hooks.integration_upgrade_required")
                .remediation
                .as_ref()
                .expect("upgrade action")
                .safety,
            ActionSafety::OwnerExplicitRequired
        );
        assert_eq!(
            issue(&snapshot, "codex.profile_unadmitted")
                .remediation
                .as_ref()
                .expect("profile action")
                .safety,
            ActionSafety::UnsupportedAutomation
        );
        assert_eq!(
            issue(&snapshot, "terminal.title_repair_available")
                .remediation
                .as_ref()
                .expect("title action")
                .safety,
            ActionSafety::PreviewableSafeRepair
        );
    }

    #[test]
    fn settings_and_worker_conditions_project_once_for_every_frontend_consumer() {
        let mut report = report();
        report.presentation.source = PresentationSettingsSource::Invalid;
        report.activity.worker_state_health = "warning".to_owned();

        let status = ManagementSnapshot::from_diagnostics(&report);
        let doctor = ManagementSnapshot::from_diagnostics(&report);
        let guided_setup = ManagementSnapshot::from_diagnostics(&report);
        let control_center = ManagementSnapshot::from_diagnostics(&report);

        assert_eq!(status, doctor);
        assert_eq!(status, guided_setup);
        assert_eq!(status, control_center);
        assert_eq!(
            issue(&status, "settings.invalid")
                .remediation
                .as_ref()
                .expect("settings action")
                .safety,
            ActionSafety::OwnerExplicitRequired
        );
        assert_eq!(
            issue(&status, "workers.warning")
                .remediation
                .as_ref()
                .expect("worker action")
                .safety,
            ActionSafety::ReadOnly
        );
    }

    #[test]
    fn unavailable_settings_and_workers_remain_read_only_diagnostics() {
        let mut report = report();
        report.presentation.source = PresentationSettingsSource::Unavailable;
        report.activity.worker_state_health = "unavailable".to_owned();

        let snapshot = ManagementSnapshot::from_diagnostics(&report);

        assert_eq!(
            issue(&snapshot, "settings.unavailable")
                .remediation
                .as_ref()
                .expect("settings inspection action")
                .safety,
            ActionSafety::ReadOnly
        );
        assert_eq!(
            issue(&snapshot, "workers.unavailable")
                .remediation
                .as_ref()
                .expect("worker inspection action")
                .safety,
            ActionSafety::ReadOnly
        );
    }

    #[test]
    fn ambiguous_title_policy_offers_diagnosis_not_repair() {
        let mut report = report();
        report.title = title(TitleRemediationState::BlockedAmbiguous);
        report.title.application_title_policy = ApplicationTitlePolicy::AmbiguousProfile;

        let snapshot = ManagementSnapshot::from_diagnostics(&report);
        let action = issue(&snapshot, "terminal.title_diagnose_only")
            .remediation
            .as_ref()
            .expect("title inspection action");

        assert_eq!(action.safety, ActionSafety::ReadOnly);
        assert!(action.instruction.contains("title-policy inspect"));
    }

    #[test]
    fn serialized_management_projection_has_no_raw_content_or_identity_fields() {
        let mut report = report();
        report.integration.hook_trust = HookTrustState::ReviewRequired;
        let snapshot = ManagementSnapshot::from_diagnostics(&report);
        let serialized = serde_json::to_string(&snapshot).expect("management snapshot serializes");

        for forbidden in [
            "prompt",
            "assistant",
            "tool_input",
            "tool_output",
            "credential",
            "session_id",
            "turn_id",
            "workspace_identity",
            "binary_path",
        ] {
            assert!(
                !serialized.contains(forbidden),
                "management serialization leaked {forbidden}"
            );
        }
    }
}
