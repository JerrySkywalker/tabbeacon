//! Run-bound, content-minimal evidence for the frozen G18 scenario contract.
//!
//! The static matrix declares what must be proved. This module records a
//! particular run without retaining Hook bodies, workspace paths, aliases, or
//! foreign Windows Terminal title text, then rejects evidence that does not
//! exactly bind to that contract and candidate head.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use serde::{Deserialize, Serialize};

use crate::convergence::{ConvergenceProofMethod, ConvergenceScenario, ExpectedVisibleState};

/// Schema for one complete, run-bound convergence evidence matrix.
pub const CONVERGENCE_EVIDENCE_SCHEMA_VERSION: u32 = 1;
const MAX_EVIDENCE_BYTES: u64 = 1024 * 1024;
const MAX_ARTIFACT_IDS: usize = 16;
const MAX_ARTIFACT_ID_BYTES: usize = 128;

/// One explicit evidence outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConvergenceEvidenceStatus {
    /// The scenario satisfied every required proof invariant.
    Pass,
    /// The scenario contradicted its required invariant.
    Fail,
    /// A required external/precondition boundary prevented execution.
    Blocked,
    /// Execution completed without enough proof to classify a pass or failure.
    Unproven,
    /// The contract explicitly declares the row outside this run.
    NotApplicable,
}

/// Cleanup classification reduced to a bounded, typed fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConvergenceCleanupStatus {
    /// Every owned fixture/worker retired as required.
    Pass,
    /// Cleanup violated the scenario contract.
    Fail,
    /// A bounded external condition prevented cleanup observation.
    Blocked,
    /// Cleanup was not sufficiently observed.
    Unproven,
    /// Cleanup is not meaningful for this scenario.
    NotApplicable,
}

/// One content-minimal result for a frozen scenario row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConvergenceScenarioEvidence {
    /// Immutable scenario identifier from the frozen contract.
    pub scenario_id: String,
    /// Candidate SHA the executor was instructed to validate.
    pub expected_head: String,
    /// SHA the executor observed before it started the scenario.
    pub observed_head: String,
    /// Method frozen by the scenario contract and repeated for local audit.
    pub required_proof_method: ConvergenceProofMethod,
    /// Method actually used for this evidence record.
    pub actual_proof_method: ConvergenceProofMethod,
    /// Explicit proof disposition.
    pub status: ConvergenceEvidenceStatus,
    /// Bounded observation time where title convergence is meaningful.
    pub convergence_elapsed_ms: Option<u16>,
    /// Owned fixture/worker cleanup result.
    pub cleanup_status: ConvergenceCleanupStatus,
    /// Number of distinct valid title frames, never the title strings.
    pub distinct_working_frames: Option<u8>,
    /// Whether the renderer-verified workspace alias position remained stable.
    pub workspace_alias_stable: Option<bool>,
    /// Whether an owned UIA probe proved a healthy visible title channel.
    pub title_authority_healthy: Option<bool>,
    /// Actual Windows token proof, meaningful only for Owner-elevated PASS.
    pub actual_elevated_token: Option<bool>,
    /// Whether the elevated validation reported the real Admin PowerShell path.
    pub admin_powershell: Option<bool>,
    /// Opaque, bounded identifiers of owned artifacts; never filesystem paths.
    pub artifact_ids: Vec<String>,
}

/// One complete run-bound matrix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConvergenceRun {
    /// Versioned evidence shape.
    pub schema_version: u32,
    /// Candidate head all evidence must bind to.
    pub expected_head: String,
    /// Head observed by the aggregate executor before it began.
    pub observed_head: String,
    /// One result for every frozen scenario, in deterministic contract order.
    pub scenarios: Vec<ConvergenceScenarioEvidence>,
}

/// Safe verifier outcome without serializing any evidence payloads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConvergenceVerification {
    /// Whether this exact run satisfies the complete frozen contract.
    pub valid: bool,
    /// Whether the evidence has exactly the frozen scenario identifier set.
    pub exact_scenario_set: bool,
    /// Number of evidence rows whose proof method was not the frozen method.
    pub proof_method_mismatch_count: usize,
    /// Number of distinct observed heads that differed from the expected head.
    pub mixed_head_count: usize,
    /// Total frozen scenario count.
    pub total_scenarios: usize,
    /// Count of PASS rows.
    pub pass_scenarios: usize,
    /// Count of FAIL rows.
    pub fail_scenarios: usize,
    /// Count of UNPROVEN rows.
    pub unproven_scenarios: usize,
    /// Count of BLOCKED rows.
    pub blocked_scenarios: usize,
    /// Count of permitted Owner-elevated blocked rows.
    pub blocked_owner_scenarios: usize,
    /// Stable, content-minimal verifier failure codes.
    pub violations: Vec<String>,
}

/// Loads one bounded evidence document without following a symlink/reparse
/// point chosen by an untrusted evidence path.
///
/// # Errors
///
/// Returns a stable, content-minimal error code; raw JSON and file content are
/// intentionally not included in the error.
pub fn load_convergence_run(path: &Path) -> Result<ConvergenceRun, &'static str> {
    let metadata = fs::symlink_metadata(path).map_err(|_| "evidence_metadata_unavailable")?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
        return Err("evidence_target_unsafe");
    }
    if metadata.len() > MAX_EVIDENCE_BYTES {
        return Err("evidence_oversized");
    }
    let bytes = fs::read(path).map_err(|_| "evidence_read_failed")?;
    serde_json::from_slice(&bytes).map_err(|_| "evidence_malformed")
}

/// Verifies every row against the immutable G18 contract.
#[must_use]
pub fn verify_convergence_run(
    run: &ConvergenceRun,
    frozen_matrix: &[ConvergenceScenario],
) -> ConvergenceVerification {
    let mut violations = Vec::new();
    if run.schema_version != CONVERGENCE_EVIDENCE_SCHEMA_VERSION {
        violations.push("unsupported_schema".to_owned());
    }
    if !is_sha(&run.expected_head) || !is_sha(&run.observed_head) {
        violations.push("missing_or_invalid_run_head".to_owned());
    } else if run.expected_head != run.observed_head {
        violations.push("run_head_mismatch".to_owned());
    }

    let expected = frozen_matrix
        .iter()
        .map(|scenario| (scenario.scenario_id, scenario))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    let mut by_id = BTreeMap::new();
    for evidence in &run.scenarios {
        if !seen.insert(evidence.scenario_id.as_str()) {
            violations.push("duplicate_scenario_id".to_owned());
            continue;
        }
        if !expected.contains_key(evidence.scenario_id.as_str()) {
            violations.push("unknown_scenario_id".to_owned());
            continue;
        }
        by_id.insert(evidence.scenario_id.as_str(), evidence);
    }
    if by_id.len() != expected.len() {
        violations.push("missing_required_scenario".to_owned());
    }

    let mut proof_method_mismatch_count = 0_usize;
    let mut mixed_heads = BTreeSet::new();
    let mut pass_scenarios = 0_usize;
    let mut fail_scenarios = 0_usize;
    let mut unproven_scenarios = 0_usize;
    let mut blocked_scenarios = 0_usize;
    let mut blocked_owner_scenarios = 0_usize;
    for (scenario_id, scenario) in &expected {
        let Some(evidence) = by_id.get(scenario_id) else {
            continue;
        };
        if evidence.expected_head != run.expected_head
            || evidence.observed_head != run.expected_head
        {
            mixed_heads.insert(evidence.observed_head.as_str());
            violations.push("scenario_head_mismatch".to_owned());
        }
        if evidence.required_proof_method != scenario.proof_method
            || evidence.actual_proof_method != scenario.proof_method
        {
            proof_method_mismatch_count = proof_method_mismatch_count.saturating_add(1);
            violations.push("proof_method_mismatch".to_owned());
        }
        if !valid_artifact_ids(&evidence.artifact_ids) {
            violations.push("invalid_artifact_ids".to_owned());
        }
        if evidence
            .convergence_elapsed_ms
            .is_some_and(|elapsed| elapsed > scenario.maximum_convergence_deadline_ms)
        {
            violations.push("deadline_exceeded".to_owned());
        }

        match evidence.status {
            ConvergenceEvidenceStatus::Pass => {
                pass_scenarios = pass_scenarios.saturating_add(1);
                validate_pass(scenario, evidence, &mut violations);
            }
            ConvergenceEvidenceStatus::Fail => fail_scenarios = fail_scenarios.saturating_add(1),
            ConvergenceEvidenceStatus::Unproven => {
                unproven_scenarios = unproven_scenarios.saturating_add(1)
            }
            ConvergenceEvidenceStatus::Blocked => {
                blocked_scenarios = blocked_scenarios.saturating_add(1);
                if scenario.proof_method == ConvergenceProofMethod::OwnerElevated
                    && scenario.scenario_id == "actual_elevated_powershell_visible"
                {
                    blocked_owner_scenarios = blocked_owner_scenarios.saturating_add(1);
                } else {
                    violations.push("non_owner_scenario_blocked".to_owned());
                }
            }
            ConvergenceEvidenceStatus::NotApplicable => {
                violations.push("required_scenario_not_applicable".to_owned());
            }
        }
    }

    let exact_scenario_set = by_id.len() == expected.len()
        && seen.len() == expected.len()
        && !violations.iter().any(|code| {
            matches!(
                code.as_str(),
                "duplicate_scenario_id" | "unknown_scenario_id" | "missing_required_scenario"
            )
        });
    let valid = exact_scenario_set
        && proof_method_mismatch_count == 0
        && mixed_heads.is_empty()
        && fail_scenarios == 0
        && unproven_scenarios == 0
        && blocked_scenarios == blocked_owner_scenarios
        && blocked_owner_scenarios == 1
        && violations.is_empty();
    violations.sort();
    violations.dedup();
    ConvergenceVerification {
        valid,
        exact_scenario_set,
        proof_method_mismatch_count,
        mixed_head_count: mixed_heads.len(),
        total_scenarios: expected.len(),
        pass_scenarios,
        fail_scenarios,
        unproven_scenarios,
        blocked_scenarios,
        blocked_owner_scenarios,
        violations,
    }
}

fn validate_pass(
    scenario: &ConvergenceScenario,
    evidence: &ConvergenceScenarioEvidence,
    violations: &mut Vec<String>,
) {
    if evidence.cleanup_status != ConvergenceCleanupStatus::Pass {
        violations.push("pass_without_cleanup".to_owned());
    }
    if evidence.artifact_ids.is_empty() {
        violations.push("pass_without_artifact".to_owned());
    }
    if matches!(
        scenario.proof_method,
        ConvergenceProofMethod::OwnedWindowsTerminalUia | ConvergenceProofMethod::OwnerElevated
    ) && evidence.title_authority_healthy != Some(true)
    {
        violations.push("visible_pass_without_healthy_title_authority".to_owned());
    }
    if matches!(
        scenario.expected_visible_state,
        ExpectedVisibleState::WorkingAnimation
    ) && (evidence.distinct_working_frames.unwrap_or_default() < 3
        || evidence.workspace_alias_stable != Some(true)
        || evidence.convergence_elapsed_ms.is_none())
    {
        violations.push("working_animation_incomplete".to_owned());
    }
    if matches!(
        scenario.expected_visible_state,
        ExpectedVisibleState::ResultReadyStatic | ExpectedVisibleState::ApprovalStatic
    ) && (evidence.workspace_alias_stable != Some(true)
        || evidence.convergence_elapsed_ms.is_none())
    {
        violations.push("static_visible_state_incomplete".to_owned());
    }
    if scenario.proof_method == ConvergenceProofMethod::OwnerElevated
        && (evidence.actual_elevated_token != Some(true) || evidence.admin_powershell != Some(true))
    {
        violations.push("synthetic_or_missing_elevated_proof".to_owned());
    }
}

fn valid_artifact_ids(ids: &[String]) -> bool {
    !ids.is_empty()
        && ids.len() <= MAX_ARTIFACT_IDS
        && ids.iter().all(|identifier| {
            !identifier.is_empty()
                && identifier.len() <= MAX_ARTIFACT_ID_BYTES
                && identifier
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        })
}

fn is_sha(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::{
        CONVERGENCE_EVIDENCE_SCHEMA_VERSION, ConvergenceCleanupStatus, ConvergenceEvidenceStatus,
        ConvergenceRun, ConvergenceScenarioEvidence, verify_convergence_run,
    };
    use crate::convergence::{ConvergenceProofMethod, scenario_matrix};

    const HEAD: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn evidence(scenario: &crate::convergence::ConvergenceScenario) -> ConvergenceScenarioEvidence {
        let working = matches!(
            scenario.expected_visible_state,
            crate::convergence::ExpectedVisibleState::WorkingAnimation
        );
        ConvergenceScenarioEvidence {
            scenario_id: scenario.scenario_id.to_owned(),
            expected_head: HEAD.to_owned(),
            observed_head: HEAD.to_owned(),
            required_proof_method: scenario.proof_method,
            actual_proof_method: scenario.proof_method,
            status: if scenario.proof_method == ConvergenceProofMethod::OwnerElevated {
                ConvergenceEvidenceStatus::Blocked
            } else {
                ConvergenceEvidenceStatus::Pass
            },
            convergence_elapsed_ms: (working
                || matches!(
                    scenario.expected_visible_state,
                    crate::convergence::ExpectedVisibleState::ResultReadyStatic
                        | crate::convergence::ExpectedVisibleState::ApprovalStatic
                ))
            .then_some(750),
            cleanup_status: ConvergenceCleanupStatus::Pass,
            distinct_working_frames: working.then_some(3),
            workspace_alias_stable: working.then_some(true).or_else(|| {
                matches!(
                    scenario.expected_visible_state,
                    crate::convergence::ExpectedVisibleState::ResultReadyStatic
                        | crate::convergence::ExpectedVisibleState::ApprovalStatic
                )
                .then_some(true)
            }),
            title_authority_healthy: matches!(
                scenario.proof_method,
                ConvergenceProofMethod::OwnedWindowsTerminalUia
            )
            .then_some(true),
            actual_elevated_token: None,
            admin_powershell: None,
            artifact_ids: vec![format!("test-{}", scenario.scenario_id)],
        }
    }

    fn valid_run() -> ConvergenceRun {
        ConvergenceRun {
            schema_version: CONVERGENCE_EVIDENCE_SCHEMA_VERSION,
            expected_head: HEAD.to_owned(),
            observed_head: HEAD.to_owned(),
            scenarios: scenario_matrix().iter().map(evidence).collect(),
        }
    }

    #[test]
    fn complete_non_owner_run_with_one_owner_block_is_accepted() {
        let verification = verify_convergence_run(&valid_run(), scenario_matrix());
        assert!(verification.valid, "{:?}", verification.violations);
        assert_eq!(verification.pass_scenarios, 31);
        assert_eq!(verification.blocked_owner_scenarios, 1);
    }

    #[test]
    fn wrong_or_mixed_heads_are_rejected() {
        let mut run = valid_run();
        run.scenarios[0].observed_head = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned();
        let verification = verify_convergence_run(&run, scenario_matrix());
        assert!(!verification.valid);
        assert!(verification.mixed_head_count > 0);
        assert!(
            verification
                .violations
                .contains(&"scenario_head_mismatch".to_owned())
        );
    }

    #[test]
    fn proof_method_downgrade_and_synthetic_elevation_are_rejected() {
        let mut run = valid_run();
        let uia = run
            .scenarios
            .iter_mut()
            .find(|row| {
                row.required_proof_method == ConvergenceProofMethod::OwnedWindowsTerminalUia
            })
            .expect("uia row exists");
        uia.actual_proof_method = ConvergenceProofMethod::DeterministicCore;
        let owner = run
            .scenarios
            .iter_mut()
            .find(|row| row.required_proof_method == ConvergenceProofMethod::OwnerElevated)
            .expect("owner row exists");
        owner.status = ConvergenceEvidenceStatus::Pass;
        owner.actual_elevated_token = Some(false);
        owner.admin_powershell = Some(true);
        owner.title_authority_healthy = Some(true);
        owner.convergence_elapsed_ms = Some(750);
        owner.workspace_alias_stable = Some(true);
        let verification = verify_convergence_run(&run, scenario_matrix());
        assert!(!verification.valid);
        assert!(verification.proof_method_mismatch_count > 0);
        assert!(
            verification
                .violations
                .contains(&"synthetic_or_missing_elevated_proof".to_owned())
        );
    }

    #[test]
    fn missing_duplicate_and_unknown_rows_fail_closed() {
        let mut run = valid_run();
        let duplicate = run.scenarios[0].clone();
        run.scenarios.push(duplicate);
        run.scenarios.pop();
        run.scenarios[0].scenario_id = "unknown-row".to_owned();
        let verification = verify_convergence_run(&run, scenario_matrix());
        assert!(!verification.valid);
        assert!(!verification.exact_scenario_set);
        assert!(
            verification
                .violations
                .contains(&"unknown_scenario_id".to_owned())
        );
    }

    #[test]
    fn deadline_frames_and_cleanup_are_required_for_a_visible_pass() {
        let mut run = valid_run();
        let working = run
            .scenarios
            .iter_mut()
            .find(|row| row.scenario_id == "working_animation")
            .expect("working row exists");
        working.convergence_elapsed_ms = Some(1_001);
        working.distinct_working_frames = Some(2);
        working.cleanup_status = ConvergenceCleanupStatus::Unproven;
        let verification = verify_convergence_run(&run, scenario_matrix());
        assert!(!verification.valid);
        assert!(
            verification
                .violations
                .contains(&"deadline_exceeded".to_owned())
        );
        assert!(
            verification
                .violations
                .contains(&"working_animation_incomplete".to_owned())
        );
        assert!(
            verification
                .violations
                .contains(&"pass_without_cleanup".to_owned())
        );
    }
}
