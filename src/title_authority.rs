//! Typed, content-minimal visible-title authority classification.
//!
//! The model intentionally separates configured Codex title ownership from
//! proof that an owned Windows Terminal tab visibly accepted and retained a
//! `TabBeacon` title. It stores only classifications, never observed title text.

use serde::Serialize;

use crate::windows_terminal_policy::{
    ActiveProfileResolution, ApplicationTitlePolicy, PolicySource, TitlePolicyDiagnostics,
    TitleRemediationState,
};

/// Result of an explicit visible-title probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VisibleTitleProbe {
    /// No active presentation write was requested.
    NotRun,
    /// The owned desired title became visible and remained visible.
    Healthy,
    /// The owned desired title was written but never admitted visibly.
    Suppressed,
    /// The desired title was admitted and another title replaced it later.
    Contended,
    /// UIA, Windows Terminal, or safe target correlation was unavailable.
    Unavailable,
}

impl VisibleTitleProbe {
    /// Stable machine-readable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotRun => "not_run",
            Self::Healthy => "healthy",
            Self::Suppressed => "suppressed",
            Self::Contended => "contended",
            Self::Unavailable => "unavailable",
        }
    }
}

/// Deepest safe boundary reached by an explicit active probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TitleProbeBoundary {
    /// No active probe was requested.
    NotRun,
    /// The platform cannot execute the Windows-specific probe.
    PlatformUnavailable,
    /// The current executable or production fixture could not be prepared.
    FixturePreparation,
    /// Windows Terminal could not launch the owned static anchor.
    AnchorLaunch,
    /// UIA could not correlate the static anchor window/tab.
    AnchorCorrelation,
    /// Windows Terminal could not launch the owned probe sibling tab.
    ProbeTabLaunch,
    /// UIA could not correlate the sole non-anchor tab.
    ProbeTabCorrelation,
    /// UIA could not sample the retained probe tab over the schedule.
    VisibleObservation,
    /// The owned fixture tab did not retire within its bounded cleanup window.
    FixtureCleanup,
    /// The complete active probe and cleanup path completed.
    Complete,
}

impl TitleProbeBoundary {
    /// Stable machine-readable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotRun => "not_run",
            Self::PlatformUnavailable => "platform_unavailable",
            Self::FixturePreparation => "fixture_preparation",
            Self::AnchorLaunch => "anchor_launch",
            Self::AnchorCorrelation => "anchor_correlation",
            Self::ProbeTabLaunch => "probe_tab_launch",
            Self::ProbeTabCorrelation => "probe_tab_correlation",
            Self::VisibleObservation => "visible_observation",
            Self::FixtureCleanup => "fixture_cleanup",
            Self::Complete => "complete",
        }
    }
}

/// Classified result of one explicit active title probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveTitleProbeResult {
    /// End-to-end visible title outcome.
    pub visible_probe: VisibleTitleProbe,
    /// Deepest safe boundary reached without retaining UI text.
    pub boundary: TitleProbeBoundary,
}

impl ActiveTitleProbeResult {
    /// Creates a safe unavailable result at one bounded probe boundary.
    #[must_use]
    pub const fn unavailable(boundary: TitleProbeBoundary) -> Self {
        Self {
            visible_probe: VisibleTitleProbe::Unavailable,
            boundary,
        }
    }

    /// Creates a completed classification after owned fixture cleanup.
    #[must_use]
    pub const fn complete(visible_probe: VisibleTitleProbe) -> Self {
        Self {
            visible_probe,
            boundary: TitleProbeBoundary::Complete,
        }
    }
}

/// End-to-end visible title authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TitleAuthority {
    /// The probe proved durable visible admission.
    Healthy,
    /// The desired title was not visibly admitted.
    Suppressed,
    /// A later writer replaced an admitted title.
    Contended,
    /// The system could not safely conduct the active observation.
    Unavailable,
    /// No active visible observation was requested.
    Unverified,
}

/// Safest root-cause class combining visible evidence with Terminal policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TitleConflictClass {
    /// An active visible probe was not requested.
    Unverified,
    /// The policy itself safely explains a suppressed visible title.
    WindowsTerminalPolicy,
    /// A visible title was admitted, then another application/channel won.
    ExternalOrApplicationWriter,
    /// Both policy and visible probe prove a healthy title channel.
    Healthy,
    /// Evidence is insufficient to name the deepest source safely.
    Inconclusive,
}

impl TitleConflictClass {
    /// Stable machine-readable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unverified => "unverified",
            Self::WindowsTerminalPolicy => "windows_terminal_policy",
            Self::ExternalOrApplicationWriter => "external_or_application_writer",
            Self::Healthy => "healthy",
            Self::Inconclusive => "inconclusive",
        }
    }
}

impl TitleAuthority {
    /// Stable machine-readable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Healthy => "healthy",
            Self::Suppressed => "suppressed",
            Self::Contended => "contended",
            Self::Unavailable => "unavailable",
            Self::Unverified => "unverified",
        }
    }

    const fn from_probe(probe: VisibleTitleProbe) -> Self {
        match probe {
            VisibleTitleProbe::NotRun => Self::Unverified,
            VisibleTitleProbe::Healthy => Self::Healthy,
            VisibleTitleProbe::Suppressed => Self::Suppressed,
            VisibleTitleProbe::Contended => Self::Contended,
            VisibleTitleProbe::Unavailable => Self::Unavailable,
        }
    }
}

/// One title sample reduced before classification so raw UI text never escapes
/// the active probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleProbeSample {
    /// The exact bounded `TabBeacon` probe title was visible.
    Desired,
    /// A different title was visible; its contents are intentionally discarded.
    Other,
}

/// Content-minimal title facts shared by status and doctor diagnostics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TitleAuthorityDiagnostics {
    /// The product writer the observatory is assessing.
    pub desired_owner: String,
    /// Existing Codex terminal-title ownership diagnosis.
    pub codex_writer_state: String,
    /// Windows Terminal policy state when it has been safely inspected.
    pub application_title_policy: ApplicationTitlePolicy,
    /// Effective policy origin, never a settings path or document content.
    pub policy_source: PolicySource,
    /// Exact-profile resolution status, never a profile name or GUID.
    pub active_profile_resolution: ActiveProfileResolution,
    /// Whether a narrow explicit repair is safely available.
    pub remediation_available: TitleRemediationState,
    /// Scope that an explicit repair would touch, if any.
    pub remediation_scope: &'static str,
    /// Whether an explicit owned visible probe was run.
    pub visible_probe: VisibleTitleProbe,
    /// Deepest non-sensitive active-probe boundary.
    pub probe_boundary: String,
    /// The end-to-end authority classification.
    pub authority: TitleAuthority,
    /// Combined policy/probe root-cause classification.
    pub conflict_class: TitleConflictClass,
}

impl TitleAuthorityDiagnostics {
    /// Creates the normal passive, read-only diagnosis.
    #[must_use]
    pub fn passive(codex_writer_state: impl Into<String>, policy: TitlePolicyDiagnostics) -> Self {
        Self {
            desired_owner: "tabbeacon".to_owned(),
            codex_writer_state: codex_writer_state.into(),
            application_title_policy: policy.application_title_policy,
            policy_source: policy.policy_source,
            active_profile_resolution: policy.active_profile_resolution,
            remediation_available: policy.remediation,
            remediation_scope: policy.remediation_scope,
            visible_probe: VisibleTitleProbe::NotRun,
            probe_boundary: TitleProbeBoundary::NotRun.as_str().to_owned(),
            authority: TitleAuthority::Unverified,
            conflict_class: TitleConflictClass::Unverified,
        }
    }

    /// Attaches one explicit active-probe result to the shared model.
    #[must_use]
    pub fn with_active_probe(mut self, result: ActiveTitleProbeResult) -> Self {
        self.visible_probe = result.visible_probe;
        result
            .boundary
            .as_str()
            .clone_into(&mut self.probe_boundary);
        self.authority = TitleAuthority::from_probe(result.visible_probe);
        self.conflict_class =
            classify_conflict(result.visible_probe, self.application_title_policy);
        self
    }
}

const fn classify_conflict(
    probe: VisibleTitleProbe,
    policy: ApplicationTitlePolicy,
) -> TitleConflictClass {
    match (probe, policy) {
        (VisibleTitleProbe::NotRun, _) => TitleConflictClass::Unverified,
        (
            VisibleTitleProbe::Suppressed,
            ApplicationTitlePolicy::SuppressedByProfile
            | ApplicationTitlePolicy::SuppressedByInheritedDefault,
        ) => TitleConflictClass::WindowsTerminalPolicy,
        (VisibleTitleProbe::Contended, policy) if policy.permits_application_titles() => {
            TitleConflictClass::ExternalOrApplicationWriter
        }
        (VisibleTitleProbe::Healthy, policy) if policy.permits_application_titles() => {
            TitleConflictClass::Healthy
        }
        (
            VisibleTitleProbe::Healthy
            | VisibleTitleProbe::Suppressed
            | VisibleTitleProbe::Contended
            | VisibleTitleProbe::Unavailable,
            _,
        ) => TitleConflictClass::Inconclusive,
    }
}

/// Classifies an owned probe's ordered observation timeline.
///
/// `Other` before the first `Desired` does not establish contention: startup
/// may briefly expose an unrelated label before the desired title is admitted.
/// An `Other` sample after first visible admission is a proven later overwrite.
#[must_use]
pub fn classify_visible_title_samples(samples: &[TitleProbeSample]) -> VisibleTitleProbe {
    let mut admitted = false;
    for sample in samples {
        match sample {
            TitleProbeSample::Desired => admitted = true,
            TitleProbeSample::Other if admitted => return VisibleTitleProbe::Contended,
            TitleProbeSample::Other => {}
        }
    }
    if admitted {
        VisibleTitleProbe::Healthy
    } else {
        VisibleTitleProbe::Suppressed
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ActiveTitleProbeResult, TitleAuthority, TitleAuthorityDiagnostics, TitleProbeBoundary,
        TitleProbeSample, VisibleTitleProbe, classify_visible_title_samples,
    };
    use crate::windows_terminal_policy::TitlePolicyDiagnostics;
    use crate::windows_terminal_policy::{
        ActiveProfileResolution, ApplicationTitlePolicy, PolicySource, SettingsSourceResolution,
        TitleRemediationState,
    };

    #[test]
    fn classifier_proves_healthy_when_the_desired_title_remains_visible() {
        assert_eq!(
            classify_visible_title_samples(&[
                TitleProbeSample::Other,
                TitleProbeSample::Desired,
                TitleProbeSample::Desired,
            ]),
            VisibleTitleProbe::Healthy
        );
    }

    #[test]
    fn classifier_proves_suppression_when_the_desired_title_never_appears() {
        assert_eq!(
            classify_visible_title_samples(&[TitleProbeSample::Other, TitleProbeSample::Other]),
            VisibleTitleProbe::Suppressed
        );
    }

    #[test]
    fn classifier_proves_contention_only_after_visible_admission() {
        assert_eq!(
            classify_visible_title_samples(&[TitleProbeSample::Desired, TitleProbeSample::Other,]),
            VisibleTitleProbe::Contended
        );
    }

    #[test]
    fn tb_reg_title_ownership_001_color_success_cannot_mask_a_suppressed_title() {
        // Color evidence is intentionally not an input to title authority.
        // A presentation that never admits the desired title remains
        // suppressed even when another presentation channel is healthy.
        assert_eq!(
            classify_visible_title_samples(&[
                TitleProbeSample::Other,
                TitleProbeSample::Other,
                TitleProbeSample::Other,
            ]),
            VisibleTitleProbe::Suppressed
        );
    }

    #[test]
    fn tb_reg_title_ownership_001_admin_label_is_not_a_claim_of_actual_elevation() {
        // The classifier receives only the reduced `Other` category. This
        // covers an administrator-like native label without claiming an
        // elevated Windows Terminal fixture was exercised.
        assert_eq!(
            classify_visible_title_samples(&[TitleProbeSample::Desired, TitleProbeSample::Other]),
            VisibleTitleProbe::Contended
        );
    }

    #[test]
    fn passive_diagnostics_are_unverified_not_failed() {
        let title = TitleAuthorityDiagnostics::passive(
            "tabbeacon",
            TitlePolicyDiagnostics::not_inspected(),
        );
        assert_eq!(title.visible_probe, VisibleTitleProbe::NotRun);
        assert_eq!(title.authority, TitleAuthority::Unverified);
    }

    #[test]
    fn title_diagnostics_serialize_only_classifications() {
        let title = TitleAuthorityDiagnostics::passive(
            "tabbeacon",
            TitlePolicyDiagnostics::not_inspected(),
        )
        .with_active_probe(ActiveTitleProbeResult::complete(
            VisibleTitleProbe::Contended,
        ));
        let json = serde_json::to_string(&title).expect("title diagnostics serialize");
        assert!(json.contains("contended"));
        for forbidden in ["PowerShell", "prompt", "tool", "model", "session_id"] {
            assert!(
                !json.contains(forbidden),
                "diagnostic JSON leaked {forbidden}"
            );
        }
    }

    #[test]
    fn unavailable_probe_retains_only_a_safe_deepest_boundary() {
        let title = TitleAuthorityDiagnostics::passive(
            "tabbeacon",
            TitlePolicyDiagnostics::not_inspected(),
        )
        .with_active_probe(ActiveTitleProbeResult::unavailable(
            TitleProbeBoundary::AnchorCorrelation,
        ));
        assert_eq!(title.visible_probe, VisibleTitleProbe::Unavailable);
        assert_eq!(title.probe_boundary, "anchor_correlation");
    }

    #[test]
    fn visible_evidence_and_effective_policy_produce_conservative_root_causes() {
        let suppressed_policy = TitlePolicyDiagnostics {
            settings_source: SettingsSourceResolution::Unavailable,
            active_profile_resolution: ActiveProfileResolution::Resolved,
            application_title_policy: ApplicationTitlePolicy::SuppressedByProfile,
            policy_source: PolicySource::Profile,
            remediation: TitleRemediationState::Available,
            remediation_scope: "active_profile",
        };
        let suppressed = TitleAuthorityDiagnostics::passive("tabbeacon", suppressed_policy)
            .with_active_probe(ActiveTitleProbeResult::complete(
                VisibleTitleProbe::Suppressed,
            ));
        assert_eq!(
            suppressed.conflict_class.as_str(),
            "windows_terminal_policy"
        );

        let allowed_policy = TitlePolicyDiagnostics {
            settings_source: SettingsSourceResolution::Unavailable,
            active_profile_resolution: ActiveProfileResolution::Resolved,
            application_title_policy: ApplicationTitlePolicy::ApplicationTitlesAllowed,
            policy_source: PolicySource::Profile,
            remediation: TitleRemediationState::NotNeeded,
            remediation_scope: "none",
        };
        let contended = TitleAuthorityDiagnostics::passive("tabbeacon", allowed_policy)
            .with_active_probe(ActiveTitleProbeResult::complete(
                VisibleTitleProbe::Contended,
            ));
        assert_eq!(
            contended.conflict_class.as_str(),
            "external_or_application_writer"
        );
    }
}
