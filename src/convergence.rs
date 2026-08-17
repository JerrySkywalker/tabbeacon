//! Typed, data-driven G18 convergence scenario contract.
//!
//! The matrix is intentionally a product-visible diagnostic artifact rather
//! than an ad-hoc test checklist. It records which evidence method can prove
//! each lifecycle path, keeps unsupported Hook semantics explicit, and never
//! treats an internal render or worker as visible-title proof by itself.

use serde::{Deserialize, Serialize};

/// v0.3's maximum time from an admitted lifecycle event to its required state.
pub const CONVERGENCE_DEADLINE_MS: u16 = 1_000;

/// Evidence method required by a scenario's strongest claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConvergenceProofMethod {
    /// Deterministic typed core, renderer, or worker tests.
    DeterministicCore,
    /// Isolated admitted Codex Hook payload fixture.
    HookFixture,
    /// Positively owned Windows Terminal UIA observation.
    OwnedWindowsTerminalUia,
    /// One explicitly elevated Owner-launched validation fixture.
    OwnerElevated,
}

/// Semantic state expected after one scenario's input sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedSemanticState {
    /// A ready root session with no attention demand.
    Ready,
    /// A root turn is working.
    Working,
    /// A result is ready for the user.
    ResultReady,
    /// An approval requires user attention.
    Approval,
    /// Lifecycle end clears the presentation.
    Ended,
    /// The admitted event must preserve the current state.
    Preserved,
    /// The input is isolated and cannot alter a root session.
    Isolated,
    /// Current Hooks do not provide enough authority to claim this state.
    NotAdmitted,
}

/// Required visible state once the title channel is known healthy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedVisibleState {
    /// At least three moving spinner frames with a stationary workspace alias.
    WorkingAnimation,
    /// A stable ready title with the neutral left slot.
    ReadyStatic,
    /// A stable result-ready title with the success left slot.
    ResultReadyStatic,
    /// A stable approval title with the attention left slot.
    ApprovalStatic,
    /// Presentation must reset/stop without later worker writes.
    Cleanup,
    /// Preserve the prior visible state exactly.
    Preserved,
    /// Isolation must prevent a cross-tab/root visible write.
    NoCrossSessionWrite,
    /// A superseded or crashed writer must make no later visible write.
    NoStaleVisibleWrite,
    /// Current provider evidence cannot justify a visible claim.
    NotClaimed,
}

/// Current evidence state for a scenario row.
///
/// The checked-in matrix deliberately starts as `pending_evidence`; a run's
/// durable receipt records the bound result without serializing any Hook data
/// or foreign tab-title content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConvergenceScenarioResult {
    /// The scenario contract exists but a particular evidence run has not yet
    /// supplied its result.
    PendingEvidence,
}

/// One complete scenario row, serializable without payload or title content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ConvergenceScenario {
    /// Stable scenario identifier.
    pub scenario_id: &'static str,
    /// Bounded initial semantic state.
    pub initial_state: ExpectedSemanticState,
    /// Admitted lifecycle event sequence, never a raw Hook payload.
    pub event_sequence: &'static [&'static str],
    /// State expected after all admitted events.
    pub expected_semantic_state: ExpectedSemanticState,
    /// Visible contract for a healthy title channel.
    pub expected_visible_state: ExpectedVisibleState,
    /// Maximum allowed convergence deadline.
    pub maximum_convergence_deadline_ms: u16,
    /// Evidence needed for the claimed result.
    pub proof_method: ConvergenceProofMethod,
    /// Cleanup obligation for the owned fixture/session.
    pub cleanup_requirement: &'static str,
    /// Result placeholder, completed in a run-bound durable evidence matrix.
    pub result: ConvergenceScenarioResult,
}

macro_rules! scenario {
    ($id:literal, $initial:ident, [$($event:literal),+], $semantic:ident, $visible:ident, $proof:ident, $cleanup:literal) => {
        ConvergenceScenario {
            scenario_id: $id,
            initial_state: ExpectedSemanticState::$initial,
            event_sequence: &[$($event),+],
            expected_semantic_state: ExpectedSemanticState::$semantic,
            expected_visible_state: ExpectedVisibleState::$visible,
            maximum_convergence_deadline_ms: CONVERGENCE_DEADLINE_MS,
            proof_method: ConvergenceProofMethod::$proof,
            cleanup_requirement: $cleanup,
            result: ConvergenceScenarioResult::PendingEvidence,
        }
    };
}

const MATRIX: [ConvergenceScenario; 32] = [
    scenario!(
        "fresh_codex_launch",
        Ready,
        ["SessionStart:start"],
        Ready,
        ReadyStatic,
        HookFixture,
        "session remains isolated"
    ),
    scenario!(
        "session_start_startup",
        Ready,
        ["SessionStart:startup"],
        Ready,
        ReadyStatic,
        HookFixture,
        "no worker before work"
    ),
    scenario!(
        "session_start_resume",
        Ready,
        ["SessionStart:resume"],
        Ready,
        ReadyStatic,
        HookFixture,
        "no worker before work"
    ),
    scenario!(
        "session_start_clear",
        Ready,
        ["SessionStart:clear"],
        Ready,
        ReadyStatic,
        HookFixture,
        "no worker before work"
    ),
    scenario!(
        "user_prompt_working",
        Ready,
        ["UserPromptSubmit"],
        Working,
        WorkingAnimation,
        OwnedWindowsTerminalUia,
        "stop worker at terminal end"
    ),
    scenario!(
        "working_animation",
        Working,
        ["worker:frame"],
        Working,
        WorkingAnimation,
        OwnedWindowsTerminalUia,
        "stop worker at terminal end"
    ),
    scenario!(
        "new_turn_supersession",
        Working,
        ["UserPromptSubmit:new", "delayed:Stop:old"],
        Working,
        NoStaleVisibleWrite,
        DeterministicCore,
        "old generation cannot write"
    ),
    scenario!(
        "pre_compact",
        Working,
        ["PreCompact"],
        Preserved,
        Preserved,
        HookFixture,
        "keep current worker ownership"
    ),
    scenario!(
        "post_compact",
        Working,
        ["PostCompact"],
        Preserved,
        Preserved,
        HookFixture,
        "keep current worker ownership"
    ),
    scenario!(
        "subagent_start_isolation",
        Working,
        ["SubagentStart"],
        Isolated,
        NoCrossSessionWrite,
        HookFixture,
        "root worker remains scoped"
    ),
    scenario!(
        "subagent_stop_isolation",
        Working,
        ["SubagentStop"],
        Isolated,
        NoCrossSessionWrite,
        HookFixture,
        "root worker remains scoped"
    ),
    scenario!(
        "stop_result_ready",
        Working,
        ["Stop"],
        ResultReady,
        ResultReadyStatic,
        OwnedWindowsTerminalUia,
        "terminate working worker"
    ),
    scenario!(
        "permission_request",
        Working,
        ["PermissionRequest"],
        Approval,
        ApprovalStatic,
        OwnedWindowsTerminalUia,
        "terminate working worker"
    ),
    scenario!(
        "question_not_admitted",
        Working,
        ["Question"],
        NotAdmitted,
        NotClaimed,
        DeterministicCore,
        "no invented state"
    ),
    scenario!(
        "session_end",
        ResultReady,
        ["SessionEnd"],
        Ended,
        Cleanup,
        HookFixture,
        "reset terminal state"
    ),
    scenario!(
        "interruption_fidelity",
        Working,
        ["Ctrl+C"],
        NotAdmitted,
        NotClaimed,
        DeterministicCore,
        "no invented interruption state"
    ),
    scenario!(
        "codex_disappearance",
        Working,
        ["owner_process_exit"],
        Ended,
        Cleanup,
        DeterministicCore,
        "bounded cleanup observer"
    ),
    scenario!(
        "worker_crash",
        Working,
        ["worker_process_exit"],
        Working,
        NoStaleVisibleWrite,
        DeterministicCore,
        "fail open without stale writes"
    ),
    scenario!(
        "terminal_close",
        Working,
        ["terminal_binding_closed"],
        Ended,
        Cleanup,
        DeterministicCore,
        "bounded terminal cleanup"
    ),
    scenario!(
        "normal_powershell_visible",
        Working,
        ["UserPromptSubmit", "Stop"],
        ResultReady,
        ResultReadyStatic,
        OwnedWindowsTerminalUia,
        "owned fixture cleanup"
    ),
    scenario!(
        "actual_elevated_powershell_visible",
        Working,
        ["UserPromptSubmit", "Stop"],
        ResultReady,
        ResultReadyStatic,
        OwnerElevated,
        "owned elevated fixture cleanup"
    ),
    scenario!(
        "git_workspace",
        Ready,
        ["UserPromptSubmit:git"],
        Working,
        WorkingAnimation,
        OwnedWindowsTerminalUia,
        "owned fixture cleanup"
    ),
    scenario!(
        "linked_worktree",
        Ready,
        ["UserPromptSubmit:linked_worktree"],
        Working,
        WorkingAnimation,
        OwnedWindowsTerminalUia,
        "owned fixture cleanup"
    ),
    scenario!(
        "non_git_workspace",
        Ready,
        ["UserPromptSubmit:non_git"],
        Working,
        WorkingAnimation,
        OwnedWindowsTerminalUia,
        "owned fixture cleanup"
    ),
    scenario!(
        "home_workspace",
        Ready,
        ["UserPromptSubmit:home"],
        Working,
        WorkingAnimation,
        OwnedWindowsTerminalUia,
        "owned fixture cleanup"
    ),
    scenario!(
        "different_repositories_different_tabs",
        Working,
        ["second_session:other_repository"],
        Isolated,
        NoCrossSessionWrite,
        DeterministicCore,
        "both session workers remain independently scoped"
    ),
    scenario!(
        "same_repository_different_tabs",
        Working,
        ["second_session:same_repository"],
        Isolated,
        NoCrossSessionWrite,
        DeterministicCore,
        "both session workers remain independently scoped"
    ),
    scenario!(
        "same_workspace_parallel_sessions",
        Working,
        ["second_session:same_workspace"],
        Isolated,
        NoCrossSessionWrite,
        DeterministicCore,
        "both session workers remain independently scoped"
    ),
    scenario!(
        "binary_relocation_upgrade",
        Working,
        ["binding:binary_relocated"],
        Preserved,
        Preserved,
        DeterministicCore,
        "old binding cannot retain ownership"
    ),
    scenario!(
        "settings_animated_to_static",
        Working,
        ["settings:activity=static"],
        Ready,
        ReadyStatic,
        DeterministicCore,
        "animated worker terminates"
    ),
    scenario!(
        "settings_animated_to_native",
        Working,
        ["settings:activity=native"],
        Ended,
        Cleanup,
        DeterministicCore,
        "animated worker terminates"
    ),
    scenario!(
        "settings_animated_to_off",
        Working,
        ["settings:activity=off"],
        Ended,
        Cleanup,
        DeterministicCore,
        "animated worker terminates"
    ),
];

/// Returns the frozen G18 scenario matrix.
#[must_use]
pub const fn scenario_matrix() -> &'static [ConvergenceScenario] {
    &MATRIX
}

/// Counts only scenarios that require owned Windows Terminal UIA proof.
#[must_use]
pub fn owned_uia_scenario_count() -> usize {
    scenario_matrix()
        .iter()
        .filter(|scenario| scenario.proof_method == ConvergenceProofMethod::OwnedWindowsTerminalUia)
        .count()
}

#[cfg(test)]
mod tests {
    use super::{
        CONVERGENCE_DEADLINE_MS, ConvergenceProofMethod, ExpectedSemanticState,
        ExpectedVisibleState, owned_uia_scenario_count, scenario_matrix,
    };

    #[test]
    fn matrix_covers_every_admitted_lifecycle_and_recovery_boundary() {
        let identifiers = scenario_matrix()
            .iter()
            .map(|scenario| scenario.scenario_id)
            .collect::<Vec<_>>();
        for required in [
            "fresh_codex_launch",
            "session_start_startup",
            "session_start_resume",
            "session_start_clear",
            "user_prompt_working",
            "new_turn_supersession",
            "pre_compact",
            "post_compact",
            "subagent_start_isolation",
            "subagent_stop_isolation",
            "stop_result_ready",
            "permission_request",
            "session_end",
            "codex_disappearance",
            "worker_crash",
            "terminal_close",
            "git_workspace",
            "linked_worktree",
            "non_git_workspace",
            "home_workspace",
            "different_repositories_different_tabs",
            "same_repository_different_tabs",
            "same_workspace_parallel_sessions",
            "binary_relocation_upgrade",
            "settings_animated_to_static",
            "settings_animated_to_native",
            "settings_animated_to_off",
        ] {
            assert!(identifiers.contains(&required), "missing {required}");
        }
    }

    #[test]
    fn every_claim_is_bounded_and_working_requires_real_visible_frames() {
        for scenario in scenario_matrix() {
            assert!(
                scenario.maximum_convergence_deadline_ms <= CONVERGENCE_DEADLINE_MS,
                "{} relaxed the deadline",
                scenario.scenario_id
            );
            if scenario.expected_visible_state == ExpectedVisibleState::WorkingAnimation {
                assert!(
                    matches!(
                        scenario.proof_method,
                        ConvergenceProofMethod::OwnedWindowsTerminalUia
                            | ConvergenceProofMethod::OwnerElevated
                    ),
                    "{} cannot use an internal-only working proof",
                    scenario.scenario_id
                );
            }
        }
        assert!(owned_uia_scenario_count() >= 8);
    }

    #[test]
    fn unsupported_hook_semantics_remain_explicitly_unclaimed() {
        for scenario in scenario_matrix().iter().filter(|scenario| {
            scenario.expected_semantic_state == ExpectedSemanticState::NotAdmitted
        }) {
            assert_eq!(
                scenario.expected_visible_state,
                ExpectedVisibleState::NotClaimed
            );
        }
    }

    #[test]
    fn every_scenario_starts_with_an_explicit_evidence_result() {
        assert!(scenario_matrix().iter().all(|scenario| {
            scenario.result == super::ConvergenceScenarioResult::PendingEvidence
        }));
    }
}
