//! Read-only operational status and machine-readable diagnostics.
//!
//! This module projects bounded, typed facts from existing product state. It
//! never serializes provider manifests, Hook payloads, worker leases, or alias
//! assignments directly.

use std::{env, path::Path};

use serde::Serialize;

use crate::{
    activity::inspect_system_activity_leases,
    providers::codex::{CodexDoctorReport, CodexIntegration, DoctorStatus},
    repo::StableAliasRegistry,
    settings::{PresentationSettings, PresentationSettingsStore, SettingsError},
    title_authority::TitleAuthorityDiagnostics,
    windows_terminal_policy::{TitlePolicyDiagnostics, WindowsTerminalPolicyStore},
};

/// Stable JSON schema version for `status` and `doctor` diagnostics.
pub const DIAGNOSTICS_SCHEMA_VERSION: u32 = 1;

/// One stable diagnosis disposition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticStatus {
    /// The condition is proven correct.
    Pass,
    /// The condition is safe but needs attention or an expected user action.
    Warning,
    /// The condition is missing, modified, or unavailable.
    Fail,
}

impl DiagnosticStatus {
    fn from_doctor(status: DoctorStatus) -> Self {
        match status {
            DoctorStatus::Pass => Self::Pass,
            DoctorStatus::Warning => Self::Warning,
            DoctorStatus::Fail => Self::Fail,
        }
    }

    /// Stable human-readable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Warning => "WARNING",
            Self::Fail => "FAIL",
        }
    }
}

/// One non-sensitive diagnostic check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiagnosticCheck {
    /// Stable machine-oriented identifier.
    pub id: String,
    /// Check disposition.
    pub status: DiagnosticStatus,
    /// Bounded human summary with no raw configuration content.
    pub summary: String,
}

/// A structured warning or failure retained separately for automation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiagnosticIssue {
    /// Stable check identifier.
    pub id: String,
    /// Bounded safe summary.
    pub summary: String,
}

/// The one typed doctor report used by human and JSON output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DoctorDiagnostics {
    /// Stable schema version.
    pub schema_version: u32,
    /// Aggregate verdict.
    pub overall: DiagnosticStatus,
    /// Complete ordered safe check set.
    pub checks: Vec<DiagnosticCheck>,
    /// Warning checks, represented structurally for automation.
    pub warnings: Vec<DiagnosticIssue>,
    /// Failing checks, represented structurally for automation.
    pub failures: Vec<DiagnosticIssue>,
    /// Shared visible-title authority facts.
    pub title: TitleAuthorityDiagnostics,
}

impl DoctorDiagnostics {
    fn from_checks(checks: Vec<DiagnosticCheck>) -> Self {
        let warnings = checks
            .iter()
            .filter(|check| check.status == DiagnosticStatus::Warning)
            .map(|check| DiagnosticIssue {
                id: check.id.clone(),
                summary: check.summary.clone(),
            })
            .collect();
        let failures = checks
            .iter()
            .filter(|check| check.status == DiagnosticStatus::Fail)
            .map(|check| DiagnosticIssue {
                id: check.id.clone(),
                summary: check.summary.clone(),
            })
            .collect();
        let overall = checks
            .iter()
            .map(|check| check.status)
            .max_by_key(|status| match status {
                DiagnosticStatus::Pass => 0_u8,
                DiagnosticStatus::Warning => 1,
                DiagnosticStatus::Fail => 2,
            })
            .unwrap_or(DiagnosticStatus::Fail);
        Self {
            schema_version: DIAGNOSTICS_SCHEMA_VERSION,
            overall,
            checks,
            warnings,
            failures,
            title: TitleAuthorityDiagnostics::passive(
                "unavailable",
                TitlePolicyDiagnostics::not_inspected(),
            ),
        }
    }

    fn with_title(mut self, title: TitleAuthorityDiagnostics) -> Self {
        self.title = title;
        self
    }

    fn from_codex_report(report: &CodexDoctorReport) -> Self {
        Self::from_checks(
            report
                .checks()
                .iter()
                .map(|check| DiagnosticCheck {
                    id: check.id().to_owned(),
                    status: DiagnosticStatus::from_doctor(check.status()),
                    summary: check.summary().to_owned(),
                })
                .collect(),
        )
    }

    fn unavailable() -> Self {
        Self::from_checks(vec![DiagnosticCheck {
            id: "diagnostics.integration".to_owned(),
            status: DiagnosticStatus::Fail,
            summary: "Codex integration environment is unavailable".to_owned(),
        }])
    }

    /// Whether the existing doctor exit contract requires a failure status.
    #[must_use]
    pub fn is_failure(&self) -> bool {
        self.overall == DiagnosticStatus::Fail
    }
}

/// Read-only `TabBeacon` binary facts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TabBeaconDiagnostics {
    /// Compiled product version.
    pub version: String,
    /// Current executable path when the operating system provides one.
    pub binary_path: Option<String>,
}

/// Read-only Codex compatibility facts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CodexDiagnostics {
    /// Detected Codex semantic version when the bounded probe succeeded.
    pub version: Option<String>,
    /// Exact source-audited Hook profile identifier when known.
    pub hook_profile: Option<String>,
    /// Exact offline registry state; never infers support from a version range.
    pub profile_state: String,
    /// Whether the detected version maps to an admitted Hook profile.
    pub profile_supported: bool,
}

/// Trust state derived from the safe doctor report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HookTrustState {
    /// Exact declarations have been reviewed and are active.
    Active,
    /// Exact declarations exist but require official Codex review in `/hooks`.
    ReviewRequired,
    /// Trust is modified, disabled, or otherwise not proven.
    Failed,
    /// Trust could not be assessed from the current state.
    Unavailable,
}

impl HookTrustState {
    /// Stable machine-oriented spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::ReviewRequired => "review_required",
            Self::Failed => "failed",
            Self::Unavailable => "unavailable",
        }
    }
}

/// Terminal-title ownership state derived from the safe doctor report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TitleOwnershipState {
    /// `TabBeacon` owns the Codex terminal-title setting.
    Tabbeacon,
    /// Native Codex title handling or title-off preference is restored.
    NativeOrOff,
    /// The declared and observed title settings conflict.
    Conflict,
    /// Ownership could not be assessed.
    Unavailable,
}

impl TitleOwnershipState {
    /// Stable machine-oriented spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tabbeacon => "tabbeacon",
            Self::NativeOrOff => "native_or_off",
            Self::Conflict => "conflict",
            Self::Unavailable => "unavailable",
        }
    }
}

/// Read-only owned-integration facts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IntegrationDiagnostics {
    /// Whether a valid `TabBeacon` ownership manifest is present.
    pub installed: bool,
    /// Count of manifest-owned Hook declarations when safely available.
    pub owned_hook_count: Option<usize>,
    /// Exact-declaration/currentness diagnosis.
    pub declaration_status: DiagnosticStatus,
    /// Official Codex Hook trust/review state.
    pub hook_trust: HookTrustState,
    /// Current title ownership state.
    pub title_ownership: TitleOwnershipState,
}

/// Source state of the closed presentation settings document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PresentationSettingsSource {
    /// No document exists, so built-in effective defaults are in use.
    Default,
    /// A valid user settings document was read.
    Configured,
    /// The document is malformed or unsafe and was not interpreted.
    Invalid,
    /// A safe settings location could not be inspected.
    Unavailable,
}

impl PresentationSettingsSource {
    /// Stable machine-oriented spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Configured => "configured",
            Self::Invalid => "invalid",
            Self::Unavailable => "unavailable",
        }
    }
}

/// Read-only presentation preferences.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PresentationDiagnostics {
    /// State of the settings source.
    pub source: PresentationSettingsSource,
    /// Effective title preference when settings are valid.
    pub title_mode: Option<String>,
    /// Effective tab-color preference when settings are valid.
    pub tab_color_mode: Option<String>,
    /// Effective activity preference when settings are valid.
    pub activity_mode: Option<String>,
    /// Effective spinner preset when settings are valid.
    pub spinner_preset: Option<String>,
    /// Effective presentation theme when settings are valid.
    pub theme: Option<String>,
}

/// Read-only aggregate of ephemeral activity leases.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ActivityDiagnostics {
    /// Clarifies that counts are leases, not operating-system process claims.
    pub observation: String,
    /// Safe activity-state health.
    pub worker_state_health: String,
    /// Valid non-expired active leases.
    pub active_leases: usize,
    /// Expired active leases.
    pub stale_leases: usize,
    /// Invalid, unsafe, or bounded-out lease entries.
    pub invalid_leases: usize,
}

/// Read-only workspace-identity registry aggregate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkspaceDiagnostics {
    /// Safe registry health without aliases or canonical identities.
    pub identity_subsystem_health: String,
    /// Count of aliases in the newest valid registry generation, when available.
    pub alias_registry_count: Option<usize>,
}

/// One typed operational model shared by `status`, `status --json`, and doctor output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OperationalDiagnostics {
    /// Stable schema version.
    pub schema_version: u32,
    /// Product and executable facts.
    pub tabbeacon: TabBeaconDiagnostics,
    /// Codex compatibility facts.
    pub codex: CodexDiagnostics,
    /// Owned Codex integration facts.
    pub integration: IntegrationDiagnostics,
    /// Closed presentation preferences.
    pub presentation: PresentationDiagnostics,
    /// Visible-title authority, kept distinct from Codex configuration ownership.
    pub title: TitleAuthorityDiagnostics,
    /// Ephemeral activity lease aggregate.
    pub activity: ActivityDiagnostics,
    /// Safe workspace-identity aggregate.
    pub workspace: WorkspaceDiagnostics,
    /// The doctor verdict derived from the same collection pass.
    pub doctor: DoctorDiagnostics,
}

/// Collects one read-only operational report without creating user state or locks.
#[must_use]
pub fn collect_operational_diagnostics() -> OperationalDiagnostics {
    let (codex, integration, doctor) = collect_codex_diagnostics();
    let activity = inspect_system_activity_leases();
    let (workspace_health, alias_registry_count) = match StableAliasRegistry::default_state_root() {
        Ok(root) => {
            let inspection = StableAliasRegistry::new(root).inspect_read_only();
            (
                inspection.health().as_str().to_owned(),
                inspection.assignment_count(),
            )
        }
        Err(_) => ("unavailable".to_owned(), None),
    };
    let presentation = collect_presentation_diagnostics();
    let title_policy = WindowsTerminalPolicyStore::from_environment().inspect();
    let title =
        TitleAuthorityDiagnostics::passive(integration.title_ownership.as_str(), title_policy);
    OperationalDiagnostics {
        schema_version: DIAGNOSTICS_SCHEMA_VERSION,
        tabbeacon: TabBeaconDiagnostics {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            binary_path: env::current_exe().ok().map(|path| display_path(&path)),
        },
        codex,
        integration,
        presentation,
        title: title.clone(),
        activity: ActivityDiagnostics {
            observation: "lease_based".to_owned(),
            worker_state_health: activity.health().as_str().to_owned(),
            active_leases: activity.active_leases(),
            stale_leases: activity.stale_leases(),
            invalid_leases: activity.invalid_leases(),
        },
        workspace: WorkspaceDiagnostics {
            identity_subsystem_health: workspace_health,
            alias_registry_count,
        },
        doctor: doctor.with_title(title),
    }
}

/// Collects ordinary diagnostics and explicitly runs the owned visible-title
/// probe. This is the only diagnostics path that temporarily writes a fixture
/// title, and it never changes user configuration.
#[must_use]
pub fn collect_operational_diagnostics_with_title_probe() -> OperationalDiagnostics {
    let mut report = collect_operational_diagnostics();
    let title = report
        .title
        .clone()
        .with_active_probe(crate::visual::run_title_authority_probe());
    report.title = title.clone();
    report.doctor = report.doctor.with_title(title);
    report
}

fn collect_codex_diagnostics() -> (CodexDiagnostics, IntegrationDiagnostics, DoctorDiagnostics) {
    let Ok(integration) = CodexIntegration::from_environment() else {
        return (
            CodexDiagnostics {
                version: None,
                hook_profile: None,
                profile_state: "unknown_or_unavailable".to_owned(),
                profile_supported: false,
            },
            IntegrationDiagnostics {
                installed: false,
                owned_hook_count: None,
                declaration_status: DiagnosticStatus::Fail,
                hook_trust: HookTrustState::Unavailable,
                title_ownership: TitleOwnershipState::Unavailable,
            },
            DoctorDiagnostics::unavailable(),
        );
    };
    let report = integration.doctor();
    let manifest_status = report.check_status("ownership.manifest");
    let declaration_status = report
        .check_status("hooks.declarations")
        .map_or(DiagnosticStatus::Fail, DiagnosticStatus::from_doctor);
    let trust_status = report.check_status("hooks.trust");
    let title_status = report.check_status("terminal.title");
    let trust = match (manifest_status, trust_status) {
        (Some(DoctorStatus::Pass), Some(DoctorStatus::Pass)) => HookTrustState::Active,
        (Some(DoctorStatus::Pass), Some(DoctorStatus::Warning)) => HookTrustState::ReviewRequired,
        (Some(DoctorStatus::Pass), Some(DoctorStatus::Fail)) => HookTrustState::Failed,
        _ => HookTrustState::Unavailable,
    };
    let title_ownership = match (manifest_status, title_status, report.title_owned()) {
        (Some(DoctorStatus::Pass), Some(DoctorStatus::Pass), Some(true)) => {
            TitleOwnershipState::Tabbeacon
        }
        (Some(DoctorStatus::Pass), Some(DoctorStatus::Pass), Some(false)) => {
            TitleOwnershipState::NativeOrOff
        }
        (Some(DoctorStatus::Pass), Some(DoctorStatus::Fail), _) => TitleOwnershipState::Conflict,
        _ => TitleOwnershipState::Unavailable,
    };
    (
        CodexDiagnostics {
            version: report.codex_version().map(ToOwned::to_owned),
            hook_profile: report.hook_profile().map(|profile| profile.id().to_owned()),
            profile_state: report.compatibility_state().label().to_owned(),
            profile_supported: report.profile_supported(),
        },
        IntegrationDiagnostics {
            installed: manifest_status == Some(DoctorStatus::Pass),
            owned_hook_count: report.owned_hook_count(),
            declaration_status,
            hook_trust: trust,
            title_ownership,
        },
        DoctorDiagnostics::from_codex_report(&report),
    )
}

fn collect_presentation_diagnostics() -> PresentationDiagnostics {
    let Ok(store) = PresentationSettingsStore::from_environment() else {
        return PresentationDiagnostics {
            source: PresentationSettingsSource::Unavailable,
            title_mode: None,
            tab_color_mode: None,
            activity_mode: None,
            spinner_preset: None,
            theme: None,
        };
    };
    match store.snapshot_read_only() {
        Ok(snapshot) => presentation_diagnostics(
            if snapshot.is_absent() {
                PresentationSettingsSource::Default
            } else {
                PresentationSettingsSource::Configured
            },
            snapshot.settings(),
        ),
        Err(SettingsError::Malformed | SettingsError::SymbolicLinkTarget) => {
            PresentationDiagnostics {
                source: PresentationSettingsSource::Invalid,
                title_mode: None,
                tab_color_mode: None,
                activity_mode: None,
                spinner_preset: None,
                theme: None,
            }
        }
        Err(SettingsError::StateRootUnavailable | SettingsError::Io(_)) => {
            PresentationDiagnostics {
                source: PresentationSettingsSource::Unavailable,
                title_mode: None,
                tab_color_mode: None,
                activity_mode: None,
                spinner_preset: None,
                theme: None,
            }
        }
    }
}

fn presentation_diagnostics(
    source: PresentationSettingsSource,
    settings: PresentationSettings,
) -> PresentationDiagnostics {
    PresentationDiagnostics {
        source,
        title_mode: Some(settings.title().as_str().to_owned()),
        tab_color_mode: Some(settings.tab_color().as_str().to_owned()),
        activity_mode: Some(settings.activity().as_str().to_owned()),
        spinner_preset: Some(settings.spinner().as_str().to_owned()),
        theme: Some(settings.theme().as_str().to_owned()),
    }
}

/// Renders the existing doctor semantics from the shared typed report.
#[must_use]
pub fn human_doctor_lines(report: &DoctorDiagnostics) -> Vec<String> {
    let mut lines = report
        .checks
        .iter()
        .map(|check| {
            format!(
                "CHECK={} STATUS={} SUMMARY={}",
                check.id,
                check.status.as_str(),
                check.summary
            )
        })
        .collect::<Vec<_>>();
    lines.extend(human_title_lines(&report.title));
    lines.push(format!("DOCTOR={}", report.overall.as_str()));
    lines
}

/// Renders the shared typed operational report for a human terminal.
#[must_use]
pub fn human_status_lines(report: &OperationalDiagnostics) -> Vec<String> {
    let mut lines = vec![
        format!("STATUS_SCHEMA_VERSION={}", report.schema_version),
        format!("TABBEACON_VERSION={}", report.tabbeacon.version),
        format!(
            "TABBEACON_BINARY_PATH={}",
            option_or_unavailable(report.tabbeacon.binary_path.as_deref())
        ),
        format!(
            "CODEX_VERSION={}",
            option_or_unavailable(report.codex.version.as_deref())
        ),
        format!(
            "CODEX_HOOK_PROFILE={}",
            option_or_unavailable(report.codex.hook_profile.as_deref())
        ),
        format!("CODEX_PROFILE_STATE={}", report.codex.profile_state),
        format!("CODEX_PROFILE_SUPPORTED={}", report.codex.profile_supported),
        format!("INTEGRATION_INSTALLED={}", report.integration.installed),
        format!(
            "OWNED_HOOK_COUNT={}",
            report
                .integration
                .owned_hook_count
                .map_or_else(|| "unavailable".to_owned(), |count| count.to_string())
        ),
        format!(
            "HOOK_DECLARATIONS={}",
            report.integration.declaration_status.as_str()
        ),
        format!("HOOK_TRUST={}", report.integration.hook_trust.as_str()),
        format!(
            "TITLE_OWNERSHIP={}",
            report.integration.title_ownership.as_str()
        ),
        format!("SETTINGS_SOURCE={}", report.presentation.source.as_str()),
        format!(
            "TITLE_MODE={}",
            option_or_unavailable(report.presentation.title_mode.as_deref())
        ),
        format!(
            "TAB_COLOR_MODE={}",
            option_or_unavailable(report.presentation.tab_color_mode.as_deref())
        ),
        format!(
            "ACTIVITY_MODE={}",
            option_or_unavailable(report.presentation.activity_mode.as_deref())
        ),
        format!(
            "SPINNER_PRESET={}",
            option_or_unavailable(report.presentation.spinner_preset.as_deref())
        ),
        format!(
            "THEME={}",
            option_or_unavailable(report.presentation.theme.as_deref())
        ),
        format!("WORKER_OBSERVATION={}", report.activity.observation),
        format!(
            "WORKER_STATE_HEALTH={}",
            report.activity.worker_state_health
        ),
        format!("ACTIVE_LEASE_COUNT={}", report.activity.active_leases),
        format!("STALE_LEASE_COUNT={}", report.activity.stale_leases),
        format!("INVALID_LEASE_COUNT={}", report.activity.invalid_leases),
        format!(
            "WORKSPACE_IDENTITY_HEALTH={}",
            report.workspace.identity_subsystem_health
        ),
        format!(
            "ALIAS_REGISTRY_COUNT={}",
            report
                .workspace
                .alias_registry_count
                .map_or_else(|| "unavailable".to_owned(), |count| count.to_string())
        ),
        format!("DOCTOR={}", report.doctor.overall.as_str()),
        format!("DOCTOR_WARNING_COUNT={}", report.doctor.warnings.len()),
        format!("DOCTOR_FAILURE_COUNT={}", report.doctor.failures.len()),
    ];
    lines.extend(human_title_lines(&report.title));
    lines
}

fn human_title_lines(title: &TitleAuthorityDiagnostics) -> Vec<String> {
    vec![
        format!("TITLE_DESIRED_OWNER={}", title.desired_owner),
        format!("CODEX_TITLE_WRITER_STATE={}", title.codex_writer_state),
        format!(
            "APPLICATION_TITLE_POLICY={}",
            title.application_title_policy.as_str()
        ),
        format!("TITLE_POLICY_SOURCE={}", title.policy_source.as_str()),
        format!(
            "ACTIVE_PROFILE_RESOLUTION={}",
            title.active_profile_resolution.as_str()
        ),
        format!("TITLE_REMEDIATION={}", title.remediation_available.as_str()),
        format!("TITLE_REMEDIATION_SCOPE={}", title.remediation_scope),
        format!("VISIBLE_TITLE_PROBE={}", title.visible_probe.as_str()),
        format!("TITLE_PROBE_BOUNDARY={}", title.probe_boundary),
        format!("TITLE_AUTHORITY={}", title.authority.as_str()),
        format!("TITLE_CONFLICT_CLASS={}", title.conflict_class.as_str()),
    ]
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy()
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .take(1_024)
        .collect()
}

fn option_or_unavailable(value: Option<&str>) -> &str {
    value.unwrap_or("unavailable")
}

#[cfg(test)]
mod tests {
    use super::{
        DIAGNOSTICS_SCHEMA_VERSION, DiagnosticCheck, DiagnosticStatus, DoctorDiagnostics,
        human_doctor_lines,
    };

    #[test]
    fn doctor_json_has_a_stable_schema_and_structured_verdicts() {
        let report = DoctorDiagnostics::from_checks(vec![
            DiagnosticCheck {
                id: "hooks.trust".to_owned(),
                status: DiagnosticStatus::Warning,
                summary: "official review is required".to_owned(),
            },
            DiagnosticCheck {
                id: "terminal.title".to_owned(),
                status: DiagnosticStatus::Fail,
                summary: "ownership conflicts with the preference".to_owned(),
            },
        ]);
        let json = serde_json::to_string(&report).expect("diagnostic report serializes");
        let parsed: serde_json::Value =
            serde_json::from_str(&json).expect("diagnostic JSON parses");

        assert_eq!(parsed["schema_version"], DIAGNOSTICS_SCHEMA_VERSION);
        assert_eq!(parsed["overall"], "fail");
        assert_eq!(parsed["warnings"].as_array().map(Vec::len), Some(1));
        assert_eq!(parsed["failures"].as_array().map(Vec::len), Some(1));
        assert!(report.is_failure());
    }

    #[test]
    fn human_doctor_output_uses_the_same_typed_verdict() {
        let report = DoctorDiagnostics::from_checks(vec![DiagnosticCheck {
            id: "codex.profile".to_owned(),
            status: DiagnosticStatus::Warning,
            summary: "unsupported profile".to_owned(),
        }]);

        assert_eq!(
            human_doctor_lines(&report),
            vec![
                "CHECK=codex.profile STATUS=WARNING SUMMARY=unsupported profile".to_owned(),
                "TITLE_DESIRED_OWNER=tabbeacon".to_owned(),
                "CODEX_TITLE_WRITER_STATE=unavailable".to_owned(),
                "APPLICATION_TITLE_POLICY=not_inspected".to_owned(),
                "TITLE_POLICY_SOURCE=unavailable".to_owned(),
                "ACTIVE_PROFILE_RESOLUTION=unavailable".to_owned(),
                "TITLE_REMEDIATION=unavailable".to_owned(),
                "TITLE_REMEDIATION_SCOPE=none".to_owned(),
                "VISIBLE_TITLE_PROBE=not_run".to_owned(),
                "TITLE_PROBE_BOUNDARY=not_run".to_owned(),
                "TITLE_AUTHORITY=unverified".to_owned(),
                "TITLE_CONFLICT_CLASS=unverified".to_owned(),
                "DOCTOR=WARNING".to_owned(),
            ]
        );
        assert!(!report.is_failure());
    }

    #[test]
    fn diagnostic_json_does_not_invent_sensitive_payload_fields() {
        let report = DoctorDiagnostics::unavailable();
        let json = serde_json::to_string(&report).expect("diagnostic report serializes");

        for forbidden in [
            "prompt_content",
            "assistant_content",
            "tool_input",
            "tool_output",
            "session_id",
            "turn_id",
            "authorization",
            "trusted_hash",
        ] {
            assert!(
                !json.contains(forbidden),
                "diagnostic JSON leaked {forbidden}"
            );
        }
    }
}
