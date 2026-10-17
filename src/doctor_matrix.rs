//! Bounded provider/mode evidence, separate from static installation health.
//! No runtime observation or configuration mutation is performed here.

use clap::ValueEnum;
use serde::Serialize;

use crate::{
    diagnostics::{DiagnosticStatus, DoctorDiagnostics},
    providers::registry::{ProviderHookState, ProviderRegistry},
};

/// Topology query, never an instruction to launch or reconfigure a provider.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
#[value(rename_all = "snake_case")]
pub enum ExecutionMode {
    #[value(alias = "no-daemon", alias = "no_daemon")]
    Embedded,
    SharedDaemon,
    Direct,
    #[default]
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::{DiagnosticCheck, DoctorDiagnostics};

    #[test]
    fn matrix_static_and_probe_success_never_qualify_a_current_view() {
        let registry = ProviderRegistry::codex_observation(Some("0.162.0"), true, true, true);
        let doctor = DoctorDiagnostics {
            schema_version: 1,
            overall: DiagnosticStatus::Pass,
            mutation_authority: crate::providers::codex::CodexMutationAuthority::Blocked,
            runtime_continuity: crate::providers::codex::CodexRuntimeContinuity::Unproven,
            checks: ["hooks.declarations", "hooks.trust", "hooks.runtime-probe"]
                .into_iter()
                .map(|id| DiagnosticCheck {
                    id: id.to_owned(),
                    status: DiagnosticStatus::Pass,
                    summary: "fixture only".to_owned(),
                })
                .collect(),
            warnings: Vec::new(),
            failures: Vec::new(),
            title: crate::title_authority::TitleAuthorityDiagnostics::passive(
                "fixture",
                crate::windows_terminal_policy::TitlePolicyDiagnostics::not_inspected(),
            ),
        };
        let matrix = DoctorMatrix::from_diagnostics(&doctor, &registry);
        for row in &matrix.rows {
            assert_eq!(row.observed_execution_mode, ExecutionMode::Unknown);
            for fact in &row.evidence {
                if row.provider == "codex" && matches!(fact.capability, "declarations" | "trust") {
                    assert_eq!(fact.verdict, EvidenceVerdict::Proven);
                } else if row.provider == "codex"
                    && row.execution_mode == ExecutionMode::SharedDaemon
                    && matches!(fact.capability, "terminal_binding" | "presentation_route")
                {
                    assert_eq!(fact.verdict, EvidenceVerdict::BlockedUpstream);
                } else {
                    assert_ne!(fact.verdict, EvidenceVerdict::Proven);
                }
            }
        }
    }

    #[test]
    fn matrix_history_is_never_promoted_for_another_version_or_mode() {
        for version in [None, Some("0.162.0"), Some("0.149.0"), Some("99.0.0")] {
            let registry = ProviderRegistry::codex_observation(version, false, false, false);
            let matrix = DoctorMatrix::from_registry(&registry);
            let daemon = matrix.filtered(Some("codex"), Some(ExecutionMode::SharedDaemon));
            assert!(daemon.historical_evidence.is_empty());
            assert!(
                daemon
                    .rows
                    .iter()
                    .flat_map(|row| &row.evidence)
                    .all(|fact| fact.verdict != EvidenceVerdict::Proven)
            );
            let embedded = matrix.filtered(Some("codex"), Some(ExecutionMode::Embedded));
            assert_eq!(embedded.historical_evidence[0].provider_version, "0.162.0");
            assert!(
                embedded.rows[0]
                    .evidence
                    .iter()
                    .all(|fact| fact.verdict != EvidenceVerdict::Proven)
            );
        }
    }

    #[test]
    fn matrix_future_provider_preserves_only_registered_contracts() {
        let mut registry = ProviderRegistry::default();
        registry.providers[1].id =
            crate::providers::registry::ProviderId::new("future-agent").unwrap();
        let matrix = DoctorMatrix::from_registry(&registry).filtered(Some("future-agent"), None);
        assert_eq!(matrix.rows.len(), 3);
        assert!(matrix.historical_evidence.is_empty());
        assert!(
            matrix
                .rows
                .iter()
                .all(|row| row.observed_execution_mode == ExecutionMode::Unknown)
        );
        assert!(
            matrix
                .rows
                .iter()
                .flat_map(|row| &row.evidence)
                .all(|fact| fact.verdict == EvidenceVerdict::Unproven)
        );
        let json = serde_json::to_string(&matrix).unwrap();
        for forbidden in [
            "WT_SESSION",
            "thread_id",
            "turn_id",
            "payload",
            "credential",
            "pid",
        ] {
            assert!(!json.contains(forbidden));
        }
    }
}

impl ExecutionMode {
    /// Stable machine spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Embedded => "embedded",
            Self::SharedDaemon => "shared_daemon",
            Self::Direct => "direct",
            Self::Unknown => "unknown",
        }
    }
}

/// Evidence never silently becomes a legacy global failure or success.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceVerdict {
    Proven,
    Unproven,
    ObservedFailed,
    BlockedUpstream,
    Unsupported,
    NotApplicable,
}

impl EvidenceVerdict {
    /// Stable machine spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Proven => "proven",
            Self::Unproven => "unproven",
            Self::ObservedFailed => "observed_failed",
            Self::BlockedUpstream => "blocked_upstream",
            Self::Unsupported => "unsupported",
            Self::NotApplicable => "not_applicable",
        }
    }
}

/// One safe fact with explicit authority and scope; no native IDs or payloads.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MatrixEvidence {
    pub capability: &'static str,
    pub verdict: EvidenceVerdict,
    pub scope: &'static str,
    pub source: &'static str,
    pub reason: &'static str,
}

fn evidence(
    capability: &'static str,
    verdict: EvidenceVerdict,
    scope: &'static str,
    source: &'static str,
    reason: &'static str,
) -> MatrixEvidence {
    MatrixEvidence {
        capability,
        verdict,
        scope,
        source,
        reason,
    }
}

/// A queried mode is not an observed current mode.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DoctorMatrixRow {
    pub provider: String,
    pub execution_mode: ExecutionMode,
    pub observed_execution_mode: ExecutionMode,
    pub registered_capabilities: Vec<crate::providers::registry::ProviderCapabilityStatus>,
    pub evidence: Vec<MatrixEvidence>,
    pub safe_action: &'static str,
}

/// Immutable historical receipt, never used to qualify a current session.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct HistoricalModeEvidence {
    pub provider: &'static str,
    pub execution_mode: ExecutionMode,
    pub provider_version: &'static str,
    pub evidence_head: &'static str,
    pub scope: &'static str,
    pub source: &'static str,
    pub observed_at: &'static str,
    pub capabilities: Vec<MatrixEvidence>,
}

/// Shared projection for CLI and the current management screens.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DoctorMatrix {
    pub schema_version: u32,
    pub read_only: bool,
    pub rows: Vec<DoctorMatrixRow>,
    pub historical_evidence: Vec<HistoricalModeEvidence>,
}

impl DoctorMatrix {
    /// Uses only already-collected static diagnostics and registered providers.
    #[must_use]
    pub fn from_diagnostics(doctor: &DoctorDiagnostics, registry: &ProviderRegistry) -> Self {
        let mut matrix = Self::from_registry(registry);
        for row in &mut matrix.rows {
            if row.provider != "codex" {
                continue;
            }
            for (capability, check_id) in [
                ("declarations", "hooks.declarations"),
                ("trust", "hooks.trust"),
            ] {
                let verdict = doctor
                    .checks
                    .iter()
                    .find(|check| check.id == check_id)
                    .map_or(EvidenceVerdict::Unproven, |check| match check.status {
                        DiagnosticStatus::Pass => EvidenceVerdict::Proven,
                        DiagnosticStatus::Warning => EvidenceVerdict::Unproven,
                        DiagnosticStatus::Fail => EvidenceVerdict::ObservedFailed,
                    });
                if let Some(fact) = row
                    .evidence
                    .iter_mut()
                    .find(|fact| fact.capability == capability)
                {
                    fact.verdict = verdict;
                }
            }
        }
        matrix
    }

    /// Offline registry projection. Admission proves a contract, never a live view.
    #[must_use]
    pub fn from_registry(registry: &ProviderRegistry) -> Self {
        let rows = registry.providers.iter().flat_map(|provider| {
            let modes = if provider.id.as_str() == "codex" {
                [ExecutionMode::Unknown, ExecutionMode::Embedded, ExecutionMode::SharedDaemon]
            } else {
                [ExecutionMode::Unknown, ExecutionMode::Direct, ExecutionMode::SharedDaemon]
            };
            modes.into_iter().map(|mode| {
                let hook_verdict = if provider.hooks == ProviderHookState::NotApplicable {
                    EvidenceVerdict::NotApplicable
                } else { EvidenceVerdict::Unproven };
                let mut facts = vec![
                    evidence("declarations", hook_verdict, "static_inspection", "registered_provider", "Static declarations do not establish delivery or a current terminal."),
                    evidence("trust", hook_verdict, "static_inspection", "registered_provider", "Hook trust is independent from provider delivery and visual proof."),
                ];
                for capability in ["provider_delivery", "execution_mode", "terminal_binding", "presentation_route", "owner_visual"] {
                    facts.push(evidence(capability, EvidenceVerdict::Unproven, "current_view", "no_current_view_receipt", "No current mode-scoped receipt; versions, defaults, environment and isolated probes are insufficient."));
                }
                if provider.id.as_str() == "codex" && mode == ExecutionMode::SharedDaemon {
                    for fact in &mut facts {
                        if matches!(fact.capability, "terminal_binding" | "presentation_route") {
                            *fact = evidence(fact.capability, EvidenceVerdict::BlockedUpstream, "pinned_upstream_contract", "codex_c1382380de69521303b416720a52f42d51af6248", "Shared daemon lacks a trusted current-view terminal sink; delivery and the live failure branch remain unproven.");
                        }
                    }
                }
                DoctorMatrixRow {
                    provider: provider.id.as_str().to_owned(),
                    execution_mode: mode,
                    observed_execution_mode: ExecutionMode::Unknown,
                    registered_capabilities: provider.capability_profile.capabilities.clone(),
                    evidence: facts,
                    safe_action: if provider.id.as_str() == "codex" {
                        "Inspect doctor and /hooks; Owner reviews changed hashes. Embedded historical proof is scoped to --no-daemon; native daemon needs upstream capability."
                    } else {
                        "Inspect registered capability/admission; characterize the mode with Owner evidence before claiming live presentation."
                    },
                }
            }).collect::<Vec<_>>()
        }).collect();
        Self {
            schema_version: 1,
            read_only: true,
            rows,
            historical_evidence: vec![HistoricalModeEvidence {
                provider: "codex",
                execution_mode: ExecutionMode::Embedded,
                provider_version: "0.162.0",
                evidence_head: "436ea8d643a880c7b537353e85176108c17c060a",
                scope: "historical_owned_no_daemon_only",
                source: "TB-V0801/resume-20261010/owner-l4; issue_122",
                observed_at: "2026-10-10",
                capabilities: ["provider_delivery", "terminal_binding", "presentation_route", "owner_visual"].into_iter()
                    .map(|capability| evidence(capability, EvidenceVerdict::Proven, "historical_owned_no_daemon_only", "owner_attended_01620_no_daemon", "Working green, completion blue and normal exit reset; no current-session or new-trust qualification.")).collect(),
            }, HistoricalModeEvidence {
                provider: "codex",
                execution_mode: ExecutionMode::Unknown,
                provider_version: "0.162.0",
                evidence_head: "a76ed15e758debf6b1635406bd03001695b58bdd",
                scope: "owner_report_default_launch_mode_unobserved",
                source: "issue_122; TB-V082-NATIVE-DAEMON-AUDIT-001",
                observed_at: "2026-10-10",
                capabilities: vec![evidence("owner_visual", EvidenceVerdict::ObservedFailed, "owner_report_default_launch_mode_unobserved", "owner_report", "Default codex had no visible tab update; actual delivery/mode/failure subbranch unproven.")],
            }],
        }
    }

    /// Restricts display only. Historical mode evidence remains separate.
    #[must_use]
    pub fn filtered(&self, provider: Option<&str>, mode: Option<ExecutionMode>) -> Self {
        let mut result = self.clone();
        result.rows.retain(|row| {
            provider.is_none_or(|id| id == row.provider)
                && mode.is_none_or(|value| value == row.execution_mode)
        });
        result.historical_evidence.retain(|row| {
            provider.is_none_or(|id| id == row.provider)
                && mode.is_none_or(|value| value == row.execution_mode)
        });
        result
    }

    /// Stable plain projection from the same facts used by Human/TUI output.
    #[must_use]
    pub fn plain_lines(&self) -> Vec<String> {
        let mut lines = vec!["MATRIX_READ_ONLY=true".to_owned()];
        for row in &self.rows {
            for fact in &row.evidence {
                lines.push(format!(
                    "MATRIX={}|{}|{}|{}|scope={}|source={}|observed_mode={}",
                    row.provider,
                    row.execution_mode.as_str(),
                    fact.capability,
                    fact.verdict.as_str(),
                    fact.scope,
                    fact.source,
                    row.observed_execution_mode.as_str()
                ));
            }
            lines.push(format!(
                "MATRIX_ACTION={}|{}|{}",
                row.provider,
                row.execution_mode.as_str(),
                row.safe_action
            ));
        }
        for row in &self.historical_evidence {
            for fact in &row.capabilities {
                lines.push(format!(
                    "MATRIX_HISTORY={}|{}|{}|{}|version={}|head={}|scope={}",
                    row.provider,
                    row.execution_mode.as_str(),
                    fact.capability,
                    fact.verdict.as_str(),
                    row.provider_version,
                    row.evidence_head,
                    row.scope
                ));
            }
        }
        lines
    }
}
