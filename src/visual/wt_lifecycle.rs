//! Exact-owned temporary Windows Terminal lifecycle for qualification tooling.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{VisualError, VisualResult};

const OWNERSHIP_SCHEMA: &str = "tabbeacon-temporary-wt-ownership-v1";
const CLEANUP_SCHEMA: &str = "tabbeacon-temporary-wt-cleanup-v1";
const MAX_CLEANUP_RECOVERY_ATTEMPTS: u8 = 32;

/// Fresh UIA correlation for one fixed, run-owned anchor tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExactWindowObservation {
    /// Number of desktop `TabItem` elements matching the complete anchor title.
    pub anchor_tab_match_count: u32,
    /// Number of distinct ancestor windows for those exact `TabItem` elements.
    pub target_window_match_count: u32,
    /// Exact ancestor HWND when cardinality is one.
    pub native_window_id: Option<isize>,
}

/// Narrow platform boundary used by the durable lifecycle.
pub trait ExactOwnedWindowBackend {
    /// Observes only the complete fixed anchor title.
    ///
    /// # Errors
    ///
    /// Returns a platform error when exact UIA observation is unavailable.
    fn observe_exact_anchor(&self, anchor_title: &str) -> VisualResult<ExactWindowObservation>;

    /// Revalidates the anchor and closes only its exact expected HWND.
    ///
    /// # Errors
    ///
    /// Returns a platform error when ownership changed or the exact close
    /// cannot be proved.
    fn close_exact_anchor(&self, anchor_title: &str, expected_hwnd: isize) -> VisualResult<()>;
}

/// Additional process proof required before a later run may recover a window.
pub trait ExactOwnedWindowRecoveryBackend: ExactOwnedWindowBackend {
    /// Returns the start time for the currently live process at one PID.
    ///
    /// `None` is positive proof that no process currently owns the PID. Any
    /// unavailable or ambiguous observation must return an error.
    ///
    /// # Errors
    ///
    /// Returns a platform error when the PID or its process start time cannot
    /// be observed unambiguously.
    fn creator_process_started_unix_ms(&self, creator_process_id: u32)
    -> VisualResult<Option<u64>>;
}

/// Immutable proof that one run owns one temporary Windows Terminal window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporaryWindowsTerminalOwnership {
    /// Stable schema identifier.
    pub schema: String,
    /// Unique qualification-run nonce.
    pub run_id: String,
    /// Complete fixed anchor `TabItem` title.
    pub anchor_title: String,
    /// Unique `wt.exe -w` routing identity.
    pub window_routing_id: String,
    /// Exact ancestor HWND admitted at registration.
    pub native_window_id: isize,
    /// Long-lived qualification supervisor process identifier.
    pub creator_process_id: u32,
    /// Start time of the exact creator process instance, used to defeat PID reuse.
    #[serde(default)]
    pub creator_process_started_unix_ms: Option<u64>,
    /// UTC Unix milliseconds at admission.
    pub created_unix_ms: u64,
}

/// Primary qualification result retained independently from cleanup outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TemporaryWindowProductDisposition {
    /// Product/qualification assertions passed.
    Pass,
    /// Product/qualification assertions failed.
    Fail,
    /// A prerequisite blocked qualification.
    Blocked,
    /// A bounded watchdog expired.
    Timeout,
    /// The qualification body raised an exception.
    Exception,
    /// The host cancelled the qualification.
    Cancelled,
}

/// Separately reported exact-owned cleanup result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporaryWindowsTerminalCleanupReceipt {
    /// Stable schema identifier.
    pub schema: String,
    /// SHA-256 of the immutable ownership record bytes.
    pub ownership_sha256: String,
    /// Original product disposition, never rewritten by cleanup.
    pub product_disposition: TemporaryWindowProductDisposition,
    /// `PASS` or `FAIL` for infrastructure cleanup.
    pub temporary_wt_cleanup: String,
    /// Number of exact-owned windows registered by this record.
    pub temporary_windows_created: u32,
    /// Number closed by this cleanup attempt.
    pub temporary_windows_closed: u32,
    /// Remaining exact anchor matches after cleanup.
    pub owned_temporary_wt_remaining: u32,
    /// Exact contract proof that no Owner window was targeted.
    pub owner_windows_closed: u32,
    /// Broad Windows Terminal process/window mutation is forbidden.
    pub broad_window_kill_used: bool,
    /// Content-minimal cleanup classification.
    pub detail: String,
}

/// Registers one already-launched exact-owned window and writes immutable proof.
///
/// # Errors
///
/// Returns a classified identity, evidence, UIA, or filesystem error. No
/// record is written unless the anchor and ancestor window cardinalities are
/// both exactly one.
pub fn register_temporary_windows_terminal<B: ExactOwnedWindowRecoveryBackend>(
    backend: &B,
    evidence_root: &Path,
    run_id: &str,
    anchor_title: &str,
    window_routing_id: &str,
    creator_process_id: u32,
) -> VisualResult<PathBuf> {
    validate_registration(evidence_root, run_id, anchor_title, window_routing_id)?;
    if creator_process_id == 0 {
        return Err(VisualError::Platform(
            "temporary Windows Terminal creator process ID must be nonzero".to_owned(),
        ));
    }
    let creator_process_started_unix_ms = backend
        .creator_process_started_unix_ms(creator_process_id)?
        .ok_or_else(|| {
            VisualError::Platform(
                "temporary Windows Terminal creator process is not active at registration"
                    .to_owned(),
            )
        })?;
    let observation = backend.observe_exact_anchor(anchor_title)?;
    if observation.anchor_tab_match_count != 1 || observation.target_window_match_count != 1 {
        return Err(VisualError::Platform(format!(
            "exact temporary Windows Terminal ownership is ambiguous: anchor_matches={} window_matches={}",
            observation.anchor_tab_match_count, observation.target_window_match_count
        )));
    }
    let native_window_id = observation.native_window_id.ok_or_else(|| {
        VisualError::Platform(
            "exact temporary Windows Terminal ownership has no ancestor HWND".to_owned(),
        )
    })?;
    if native_window_id == 0 {
        return Err(VisualError::Platform(
            "exact temporary Windows Terminal ownership has a zero ancestor HWND".to_owned(),
        ));
    }

    let created_unix_ms = unix_ms();
    if creator_process_started_unix_ms > created_unix_ms {
        return Err(VisualError::Platform(
            "temporary Windows Terminal creator process time is newer than its ownership record"
                .to_owned(),
        ));
    }
    let ownership = TemporaryWindowsTerminalOwnership {
        schema: OWNERSHIP_SCHEMA.to_owned(),
        run_id: run_id.to_owned(),
        anchor_title: anchor_title.to_owned(),
        window_routing_id: window_routing_id.to_owned(),
        native_window_id,
        creator_process_id,
        creator_process_started_unix_ms: Some(creator_process_started_unix_ms),
        created_unix_ms,
    };
    let path = evidence_root.join(format!("temporary-wt-{run_id}.ownership.json"));
    write_new_json(&path, &ownership)?;
    Ok(path)
}

/// Waits a bounded interval for an already-launched exact anchor, then writes
/// its immutable ownership record.
///
/// # Errors
///
/// Returns the final classified registration error after the deadline or an
/// immediately non-retryable evidence/identity error.
pub fn register_temporary_windows_terminal_with_retry<B: ExactOwnedWindowRecoveryBackend>(
    backend: &B,
    evidence_root: &Path,
    run_id: &str,
    anchor_title: &str,
    window_routing_id: &str,
    creator_process_id: u32,
    budget: Duration,
) -> VisualResult<PathBuf> {
    let deadline = Instant::now() + budget;
    loop {
        match register_temporary_windows_terminal(
            backend,
            evidence_root,
            run_id,
            anchor_title,
            window_routing_id,
            creator_process_id,
        ) {
            Ok(path) => return Ok(path),
            Err(VisualError::Platform(_)) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(50));
            }
            Err(error) => return Err(error),
        }
    }
}

/// Cleans one exact-owned window while preserving the primary disposition.
///
/// # Errors
///
/// Returns an evidence or filesystem error when the immutable record or
/// cleanup receipt cannot be validated or written. UIA ambiguity is recorded
/// as a cleanup failure receipt without closing anything.
pub fn cleanup_temporary_windows_terminal<B: ExactOwnedWindowBackend>(
    backend: &B,
    ownership_path: &Path,
    product_disposition: TemporaryWindowProductDisposition,
) -> VisualResult<TemporaryWindowsTerminalCleanupReceipt> {
    let (ownership, ownership_sha256) = read_validated_ownership(ownership_path)?;
    let receipt_path = cleanup_receipt_path(ownership_path)?;
    cleanup_temporary_windows_terminal_to_receipt(
        backend,
        &ownership,
        &ownership_sha256,
        &receipt_path,
        product_disposition,
    )
}

/// Retries a failed cleanup only for the exact process instance that created
/// the immutable ownership record.
///
/// This is intentionally separate from stale recovery: an active creator may
/// retry its own transient close failure, while a different run must continue
/// to wait until that exact creator process instance has exited.
///
/// # Errors
///
/// Returns a classified evidence, process-identity, UIA, or filesystem error.
/// The retry is refused unless the caller PID and process start time both
/// match the immutable creator identity and the latest cleanup receipt is a
/// failure with an exact-owned window still remaining.
pub fn retry_temporary_windows_terminal_cleanup<B: ExactOwnedWindowRecoveryBackend>(
    backend: &B,
    ownership_path: &Path,
    creator_process_id: u32,
) -> VisualResult<TemporaryWindowsTerminalCleanupReceipt> {
    let (ownership, ownership_sha256) = read_validated_ownership(ownership_path)?;
    if creator_process_id == 0 || ownership.creator_process_id != creator_process_id {
        return Err(VisualError::Platform(
            "temporary Windows Terminal cleanup retry refused wrong creator process ID".to_owned(),
        ));
    }
    let expected_creator_start = ownership.creator_process_started_unix_ms.ok_or_else(|| {
        VisualError::Platform(
            "temporary Windows Terminal cleanup retry refused unproven creator process identity"
                .to_owned(),
        )
    })?;
    if expected_creator_start > ownership.created_unix_ms {
        return Err(VisualError::Platform(
            "temporary Windows Terminal cleanup retry refused invalid creator process time"
                .to_owned(),
        ));
    }
    if backend.creator_process_started_unix_ms(creator_process_id)? != Some(expected_creator_start)
    {
        return Err(VisualError::Platform(
            "temporary Windows Terminal cleanup retry refused changed creator process identity"
                .to_owned(),
        ));
    }
    let previous = latest_cleanup_receipt(ownership_path, &ownership_sha256)?.ok_or_else(|| {
        VisualError::Platform(
            "temporary Windows Terminal cleanup retry requires a prior failure receipt".to_owned(),
        )
    })?;
    if previous.temporary_wt_cleanup != "FAIL" || previous.owned_temporary_wt_remaining == 0 {
        return Err(VisualError::Platform(
            "temporary Windows Terminal cleanup retry requires an unfinished prior failure"
                .to_owned(),
        ));
    }
    let receipt_path = next_cleanup_recovery_receipt_path(ownership_path)?;
    cleanup_temporary_windows_terminal_to_receipt(
        backend,
        &ownership,
        &ownership_sha256,
        &receipt_path,
        previous.product_disposition,
    )
}

fn cleanup_temporary_windows_terminal_to_receipt<B: ExactOwnedWindowBackend>(
    backend: &B,
    ownership: &TemporaryWindowsTerminalOwnership,
    ownership_sha256: &str,
    receipt_path: &Path,
    product_disposition: TemporaryWindowProductDisposition,
) -> VisualResult<TemporaryWindowsTerminalCleanupReceipt> {
    if receipt_path.exists() {
        let receipt: TemporaryWindowsTerminalCleanupReceipt =
            serde_json::from_slice(&fs::read(receipt_path)?)?;
        validate_cleanup_receipt(&receipt, ownership_sha256)?;
        if receipt.product_disposition != product_disposition {
            return Err(VisualError::Platform(
                "existing temporary Windows Terminal cleanup receipt does not match ownership"
                    .to_owned(),
            ));
        }
        return Ok(receipt);
    }

    let Ok(initial) = backend.observe_exact_anchor(&ownership.anchor_title) else {
        return write_cleanup_receipt(
            receipt_path,
            cleanup_failure(
                ownership_sha256.to_owned(),
                product_disposition,
                1,
                "INITIAL_OBSERVATION_FAILED",
            ),
        );
    };

    let receipt = if initial.anchor_tab_match_count == 0
        && initial.target_window_match_count == 0
        && initial.native_window_id.is_none()
    {
        cleanup_success(
            ownership_sha256.to_owned(),
            product_disposition,
            0,
            "EXACT_OWNED_WINDOW_ALREADY_ABSENT",
        )
    } else if initial.anchor_tab_match_count != 1 || initial.target_window_match_count != 1 {
        cleanup_failure(
            ownership_sha256.to_owned(),
            product_disposition,
            initial.anchor_tab_match_count.max(1),
            "AMBIGUOUS_EXACT_OWNERSHIP_REFUSED",
        )
    } else if initial.native_window_id != Some(ownership.native_window_id) {
        cleanup_failure(
            ownership_sha256.to_owned(),
            product_disposition,
            initial.anchor_tab_match_count,
            "HWND_MISMATCH_REFUSED",
        )
    } else if backend
        .close_exact_anchor(&ownership.anchor_title, ownership.native_window_id)
        .is_err()
    {
        cleanup_failure(
            ownership_sha256.to_owned(),
            product_disposition,
            1,
            "EXACT_CLOSE_FAILED",
        )
    } else {
        // The backend contract includes bounded proof that the exact admitted
        // HWND disappeared. A second UIA tree query can retain a stale element
        // after WindowPattern.Close and is therefore not the close authority.
        cleanup_success(
            ownership_sha256.to_owned(),
            product_disposition,
            1,
            "EXACT_OWNED_WINDOW_CLOSED",
        )
    };

    write_cleanup_receipt(receipt_path, receipt)
}

/// Recovers unfinished records below one admitted qualification registry.
/// Symlinks are never traversed, and every close still requires a fresh exact
/// anchor/HWND correlation through [`cleanup_temporary_windows_terminal`].
///
/// # Errors
///
/// Returns a classified evidence, filesystem, or ownership error. Recovery
/// stops on the first ambiguous record rather than risking an Owner window.
pub fn recover_stale_temporary_windows_terminals<B: ExactOwnedWindowRecoveryBackend>(
    backend: &B,
    registry_root: &Path,
) -> VisualResult<Vec<TemporaryWindowsTerminalCleanupReceipt>> {
    if !registry_root.exists() {
        return Ok(Vec::new());
    }
    let metadata = fs::symlink_metadata(registry_root)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(VisualError::Platform(
            "temporary Windows Terminal registry must be a real directory".to_owned(),
        ));
    }
    let mut ownership_paths = Vec::new();
    collect_unfinished_ownership_records(registry_root, 0, &mut ownership_paths)?;
    ownership_paths.sort();
    let mut receipts = Vec::with_capacity(ownership_paths.len());
    for ownership_path in ownership_paths {
        let (ownership, ownership_sha256) = read_validated_ownership(&ownership_path)?;
        let expected_creator_start = ownership.creator_process_started_unix_ms.ok_or_else(|| {
            VisualError::Platform(
                "stale exact-owned Windows Terminal recovery refused because creator process identity is unproven"
                    .to_owned(),
            )
        })?;
        if expected_creator_start > ownership.created_unix_ms {
            return Err(VisualError::Platform(
                "stale exact-owned Windows Terminal recovery refused invalid creator process time"
                    .to_owned(),
            ));
        }
        if backend.creator_process_started_unix_ms(ownership.creator_process_id)?
            == Some(expected_creator_start)
        {
            return Err(VisualError::Platform(
                "stale exact-owned Windows Terminal recovery refused because its creator process remains active"
                    .to_owned(),
            ));
        }
        let product_disposition = latest_cleanup_receipt(&ownership_path, &ownership_sha256)?
            .map_or(TemporaryWindowProductDisposition::Exception, |receipt| {
                receipt.product_disposition
            });
        let receipt_path = next_cleanup_recovery_receipt_path(&ownership_path)?;
        let receipt = cleanup_temporary_windows_terminal_to_receipt(
            backend,
            &ownership,
            &ownership_sha256,
            &receipt_path,
            product_disposition,
        )?;
        if receipt.temporary_wt_cleanup != "PASS" {
            return Err(VisualError::Platform(format!(
                "stale exact-owned Windows Terminal recovery refused: {}",
                receipt.detail
            )));
        }
        receipts.push(receipt);
    }
    Ok(receipts)
}

/// Safely closes a just-launched window when durable registration itself
/// failed. No action is taken unless the complete anchor resolves to exactly
/// one ancestor HWND.
///
/// # Errors
///
/// Returns a platform error when the anchor is ambiguous, lacks an HWND, or
/// the exact UIA close cannot be proved.
pub fn close_unregistered_exact_anchor<B: ExactOwnedWindowBackend>(
    backend: &B,
    anchor_title: &str,
) -> VisualResult<bool> {
    let observation = backend.observe_exact_anchor(anchor_title)?;
    if observation.anchor_tab_match_count == 0 && observation.target_window_match_count == 0 {
        return Ok(false);
    }
    if observation.anchor_tab_match_count != 1 || observation.target_window_match_count != 1 {
        return Err(VisualError::Platform(
            "unregistered temporary Windows Terminal cleanup refused ambiguous ownership"
                .to_owned(),
        ));
    }
    let hwnd = observation.native_window_id.ok_or_else(|| {
        VisualError::Platform(
            "unregistered temporary Windows Terminal cleanup refused missing HWND".to_owned(),
        )
    })?;
    backend.close_exact_anchor(anchor_title, hwnd)?;
    Ok(true)
}

fn collect_unfinished_ownership_records(
    directory: &Path,
    depth: u8,
    paths: &mut Vec<PathBuf>,
) -> VisualResult<()> {
    if depth > 6 {
        return Err(VisualError::Platform(
            "temporary Windows Terminal registry nesting exceeds the bounded scan depth".to_owned(),
        ));
    }
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let metadata = entry.file_type()?;
        if metadata.is_symlink() {
            continue;
        }
        let path = entry.path();
        if metadata.is_dir() {
            collect_unfinished_ownership_records(&path, depth + 1, paths)?;
            continue;
        }
        let Some(filename) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if !filename.starts_with("temporary-wt-") || !filename.ends_with(".ownership.json") {
            continue;
        }
        let (_, ownership_sha256) = read_validated_ownership(&path)?;
        let latest = latest_cleanup_receipt(&path, &ownership_sha256)?;
        if latest.as_ref().is_none_or(|receipt| {
            receipt.temporary_wt_cleanup != "PASS" || receipt.owned_temporary_wt_remaining != 0
        }) {
            paths.push(path);
        }
    }
    Ok(())
}

fn validate_registration(
    evidence_root: &Path,
    run_id: &str,
    anchor_title: &str,
    window_routing_id: &str,
) -> VisualResult<()> {
    if !is_safe_token(run_id, 64) {
        return Err(VisualError::InvalidIdentifier(run_id.to_owned()));
    }
    if !is_safe_token(window_routing_id, 96) {
        return Err(VisualError::InvalidIdentifier(window_routing_id.to_owned()));
    }
    if anchor_title.is_empty()
        || anchor_title.len() > 160
        || anchor_title.chars().any(char::is_control)
        || !anchor_title.contains(run_id)
    {
        return Err(VisualError::InvalidIdentifier(anchor_title.to_owned()));
    }
    let metadata = fs::symlink_metadata(evidence_root)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(VisualError::Platform(
            "temporary Windows Terminal evidence root must be an existing real directory"
                .to_owned(),
        ));
    }
    Ok(())
}

fn validate_ownership(ownership: &TemporaryWindowsTerminalOwnership) -> VisualResult<()> {
    if ownership.schema != OWNERSHIP_SCHEMA
        || ownership.native_window_id == 0
        || ownership.creator_process_id == 0
        || ownership.created_unix_ms == 0
    {
        return Err(VisualError::Platform(
            "invalid temporary Windows Terminal ownership record".to_owned(),
        ));
    }
    validate_registration_fields(
        &ownership.run_id,
        &ownership.anchor_title,
        &ownership.window_routing_id,
    )
}

fn validate_registration_fields(
    run_id: &str,
    anchor_title: &str,
    window_routing_id: &str,
) -> VisualResult<()> {
    if !is_safe_token(run_id, 64)
        || !is_safe_token(window_routing_id, 96)
        || anchor_title.is_empty()
        || anchor_title.len() > 160
        || anchor_title.chars().any(char::is_control)
        || !anchor_title.contains(run_id)
    {
        return Err(VisualError::Platform(
            "unsafe temporary Windows Terminal ownership identity".to_owned(),
        ));
    }
    Ok(())
}

fn is_safe_token(value: &str, max_len: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_len
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn cleanup_receipt_path(ownership_path: &Path) -> VisualResult<PathBuf> {
    cleanup_receipt_path_for_attempt(ownership_path, None)
}

fn cleanup_receipt_path_for_attempt(
    ownership_path: &Path,
    recovery_attempt: Option<u8>,
) -> VisualResult<PathBuf> {
    let filename = ownership_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| VisualError::InvalidIdentifier(ownership_path.display().to_string()))?;
    let stem = filename
        .strip_suffix(".ownership.json")
        .ok_or_else(|| VisualError::InvalidIdentifier(filename.to_owned()))?;
    let filename = recovery_attempt.map_or_else(
        || format!("{stem}.cleanup.json"),
        |attempt| format!("{stem}.recovery-{attempt:02}.cleanup.json"),
    );
    Ok(ownership_path.with_file_name(filename))
}

fn read_validated_ownership(
    ownership_path: &Path,
) -> VisualResult<(TemporaryWindowsTerminalOwnership, String)> {
    let ownership_bytes = fs::read(ownership_path)?;
    let ownership: TemporaryWindowsTerminalOwnership = serde_json::from_slice(&ownership_bytes)?;
    validate_ownership(&ownership)?;
    Ok((ownership, hex_sha256(&ownership_bytes)))
}

fn latest_cleanup_receipt(
    ownership_path: &Path,
    ownership_sha256: &str,
) -> VisualResult<Option<TemporaryWindowsTerminalCleanupReceipt>> {
    let mut latest = None;
    for recovery_attempt in
        std::iter::once(None).chain((1..=MAX_CLEANUP_RECOVERY_ATTEMPTS).map(Some))
    {
        let path = cleanup_receipt_path_for_attempt(ownership_path, recovery_attempt)?;
        if !path.exists() {
            break;
        }
        let receipt: TemporaryWindowsTerminalCleanupReceipt =
            serde_json::from_slice(&fs::read(path)?)?;
        validate_cleanup_receipt(&receipt, ownership_sha256)?;
        latest = Some(receipt);
    }
    Ok(latest)
}

fn next_cleanup_recovery_receipt_path(ownership_path: &Path) -> VisualResult<PathBuf> {
    let primary = cleanup_receipt_path(ownership_path)?;
    if !primary.exists() {
        return Ok(primary);
    }
    for attempt in 1..=MAX_CLEANUP_RECOVERY_ATTEMPTS {
        let candidate = cleanup_receipt_path_for_attempt(ownership_path, Some(attempt))?;
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(VisualError::Platform(
        "temporary Windows Terminal cleanup recovery attempt budget exhausted".to_owned(),
    ))
}

fn validate_cleanup_receipt(
    receipt: &TemporaryWindowsTerminalCleanupReceipt,
    ownership_sha256: &str,
) -> VisualResult<()> {
    let status_is_valid = match receipt.temporary_wt_cleanup.as_str() {
        "PASS" => receipt.owned_temporary_wt_remaining == 0,
        "FAIL" => receipt.owned_temporary_wt_remaining > 0,
        _ => false,
    };
    if receipt.schema != CLEANUP_SCHEMA
        || receipt.ownership_sha256 != ownership_sha256
        || receipt.temporary_windows_created != 1
        || receipt.temporary_windows_closed > 1
        || receipt.owner_windows_closed != 0
        || receipt.broad_window_kill_used
        || !status_is_valid
    {
        return Err(VisualError::Platform(
            "invalid temporary Windows Terminal cleanup receipt".to_owned(),
        ));
    }
    Ok(())
}

fn cleanup_success(
    ownership_sha256: String,
    product_disposition: TemporaryWindowProductDisposition,
    closed: u32,
    detail: &str,
) -> TemporaryWindowsTerminalCleanupReceipt {
    TemporaryWindowsTerminalCleanupReceipt {
        schema: CLEANUP_SCHEMA.to_owned(),
        ownership_sha256,
        product_disposition,
        temporary_wt_cleanup: "PASS".to_owned(),
        temporary_windows_created: 1,
        temporary_windows_closed: closed,
        owned_temporary_wt_remaining: 0,
        owner_windows_closed: 0,
        broad_window_kill_used: false,
        detail: detail.to_owned(),
    }
}

fn cleanup_failure(
    ownership_sha256: String,
    product_disposition: TemporaryWindowProductDisposition,
    remaining: u32,
    detail: &str,
) -> TemporaryWindowsTerminalCleanupReceipt {
    TemporaryWindowsTerminalCleanupReceipt {
        schema: CLEANUP_SCHEMA.to_owned(),
        ownership_sha256,
        product_disposition,
        temporary_wt_cleanup: "FAIL".to_owned(),
        temporary_windows_created: 1,
        temporary_windows_closed: 0,
        owned_temporary_wt_remaining: remaining,
        owner_windows_closed: 0,
        broad_window_kill_used: false,
        detail: detail.to_owned(),
    }
}

fn write_cleanup_receipt(
    path: &Path,
    receipt: TemporaryWindowsTerminalCleanupReceipt,
) -> VisualResult<TemporaryWindowsTerminalCleanupReceipt> {
    write_new_json(path, &receipt)?;
    Ok(receipt)
}

fn write_new_json(path: &Path, value: &impl Serialize) -> VisualResult<()> {
    if path.exists() {
        return Err(VisualError::EvidenceArtifactExists(path.to_path_buf()));
    }
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    let temp_path = path.with_extension(format!("{}.{}.tmp", std::process::id(), unix_ms()));
    let write_result = (|| -> VisualResult<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(&temp_path, path)?;
        Ok(())
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    write_result
}

fn hex_sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn unix_ms() -> u64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    u64::try_from(millis).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, fs, time::SystemTime};

    use super::{
        ExactOwnedWindowBackend, ExactOwnedWindowRecoveryBackend, ExactWindowObservation,
        TemporaryWindowProductDisposition, cleanup_receipt_path,
        cleanup_temporary_windows_terminal, close_unregistered_exact_anchor,
        recover_stale_temporary_windows_terminals, register_temporary_windows_terminal,
        retry_temporary_windows_terminal_cleanup,
    };
    use crate::visual::VisualResult;

    struct TestRoot(std::path::PathBuf);

    impl TestRoot {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("test clock follows Unix epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "tabbeacon-temporary-wt-lifecycle-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("owned lifecycle root creates");
            Self(path)
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    struct FakeBackend {
        anchor_matches: Cell<u32>,
        window_matches: Cell<u32>,
        native_window_id: Cell<Option<isize>>,
        close_calls: Cell<u32>,
        close_fails: Cell<bool>,
        close_leaves_window: Cell<bool>,
        creator_process_started_unix_ms: Cell<Option<u64>>,
    }

    impl FakeBackend {
        fn exact() -> Self {
            Self {
                anchor_matches: Cell::new(1),
                window_matches: Cell::new(1),
                native_window_id: Cell::new(Some(72)),
                close_calls: Cell::new(0),
                close_fails: Cell::new(false),
                close_leaves_window: Cell::new(false),
                creator_process_started_unix_ms: Cell::new(Some(41)),
            }
        }
    }

    impl ExactOwnedWindowBackend for FakeBackend {
        fn observe_exact_anchor(
            &self,
            _anchor_title: &str,
        ) -> VisualResult<ExactWindowObservation> {
            Ok(ExactWindowObservation {
                anchor_tab_match_count: self.anchor_matches.get(),
                target_window_match_count: self.window_matches.get(),
                native_window_id: self.native_window_id.get(),
            })
        }

        fn close_exact_anchor(
            &self,
            _anchor_title: &str,
            expected_hwnd: isize,
        ) -> VisualResult<()> {
            assert_eq!(expected_hwnd, 72);
            self.close_calls.set(self.close_calls.get() + 1);
            if self.close_fails.get() {
                return Err(crate::visual::VisualError::Platform(
                    "synthetic exact close failure".to_owned(),
                ));
            }
            if !self.close_leaves_window.get() {
                self.anchor_matches.set(0);
                self.window_matches.set(0);
                self.native_window_id.set(None);
            }
            Ok(())
        }
    }

    impl ExactOwnedWindowRecoveryBackend for FakeBackend {
        fn creator_process_started_unix_ms(
            &self,
            _creator_process_id: u32,
        ) -> VisualResult<Option<u64>> {
            Ok(self.creator_process_started_unix_ms.get())
        }
    }

    fn register(root: &TestRoot, backend: &FakeBackend) -> std::path::PathBuf {
        register_temporary_windows_terminal(
            backend,
            &root.0,
            "TB-G105B-72-abc",
            "TB-WT-ANCHOR-TB-G105B-72-abc",
            "tabbeacon-TB-G105B-72-abc",
            4242,
        )
        .expect("exact window registers")
    }

    fn assert_disposition_closes(disposition: TemporaryWindowProductDisposition) {
        let root = TestRoot::new();
        let backend = FakeBackend::exact();
        let ownership = register(&root, &backend);
        let receipt = cleanup_temporary_windows_terminal(&backend, &ownership, disposition)
            .expect("exact cleanup records");

        assert_eq!(receipt.product_disposition, disposition);
        assert_eq!(receipt.temporary_wt_cleanup, "PASS");
        assert_eq!(backend.close_calls.get(), 1);
    }

    #[test]
    fn normal_completion_closes_only_the_registered_exact_window() {
        let root = TestRoot::new();
        let backend = FakeBackend::exact();
        let ownership = register(&root, &backend);

        let receipt = cleanup_temporary_windows_terminal(
            &backend,
            &ownership,
            TemporaryWindowProductDisposition::Pass,
        )
        .expect("exact window cleanup records");

        assert_eq!(backend.close_calls.get(), 1);
        assert_eq!(receipt.temporary_wt_cleanup, "PASS");
        assert_eq!(receipt.temporary_windows_created, 1);
        assert_eq!(receipt.temporary_windows_closed, 1);
        assert_eq!(receipt.owned_temporary_wt_remaining, 0);
        assert_eq!(receipt.owner_windows_closed, 0);
        assert!(!receipt.broad_window_kill_used);
        assert_eq!(
            receipt.product_disposition,
            TemporaryWindowProductDisposition::Pass
        );
    }

    #[test]
    fn product_failure_stays_failure_while_exact_cleanup_succeeds() {
        assert_disposition_closes(TemporaryWindowProductDisposition::Fail);
    }

    #[test]
    fn timeout_still_closes_the_exact_owned_window() {
        assert_disposition_closes(TemporaryWindowProductDisposition::Timeout);
    }

    #[test]
    fn caught_exception_still_closes_the_exact_owned_window() {
        assert_disposition_closes(TemporaryWindowProductDisposition::Exception);
    }

    #[test]
    fn repeated_cleanup_is_idempotent() {
        let root = TestRoot::new();
        let backend = FakeBackend::exact();
        let ownership = register(&root, &backend);
        let first = cleanup_temporary_windows_terminal(
            &backend,
            &ownership,
            TemporaryWindowProductDisposition::Pass,
        )
        .expect("first cleanup records");
        let second = cleanup_temporary_windows_terminal(
            &backend,
            &ownership,
            TemporaryWindowProductDisposition::Pass,
        )
        .expect("second cleanup reuses receipt");

        assert_eq!(first, second);
        assert_eq!(backend.close_calls.get(), 1);
    }

    #[test]
    fn repeated_cleanup_rejects_every_invalid_existing_receipt_invariant() {
        for invalid_case in [
            "schema",
            "ownership_sha256",
            "product_disposition",
            "temporary_windows_created",
            "temporary_windows_closed",
            "owner_windows_closed",
            "broad_window_kill_used",
            "pass_with_remaining_window",
            "fail_without_remaining_window",
            "unknown_cleanup_status",
        ] {
            let root = TestRoot::new();
            let backend = FakeBackend::exact();
            let ownership = register(&root, &backend);
            let mut receipt = cleanup_temporary_windows_terminal(
                &backend,
                &ownership,
                TemporaryWindowProductDisposition::Pass,
            )
            .expect("first cleanup records");
            match invalid_case {
                "schema" => receipt.schema = "wrong-schema".to_owned(),
                "ownership_sha256" => receipt.ownership_sha256 = "0".repeat(64),
                "product_disposition" => {
                    receipt.product_disposition = TemporaryWindowProductDisposition::Fail;
                }
                "temporary_windows_created" => receipt.temporary_windows_created = 2,
                "temporary_windows_closed" => receipt.temporary_windows_closed = 2,
                "owner_windows_closed" => receipt.owner_windows_closed = 1,
                "broad_window_kill_used" => receipt.broad_window_kill_used = true,
                "pass_with_remaining_window" => receipt.owned_temporary_wt_remaining = 1,
                "fail_without_remaining_window" => receipt.temporary_wt_cleanup = "FAIL".to_owned(),
                "unknown_cleanup_status" => receipt.temporary_wt_cleanup = "UNKNOWN".to_owned(),
                _ => unreachable!("all invalid receipt cases are explicit"),
            }
            let receipt_path = cleanup_receipt_path(&ownership).expect("receipt path resolves");
            fs::write(
                receipt_path,
                serde_json::to_vec_pretty(&receipt).expect("tampered receipt serializes"),
            )
            .expect("tampered receipt writes");

            assert!(
                cleanup_temporary_windows_terminal(
                    &backend,
                    &ownership,
                    TemporaryWindowProductDisposition::Pass,
                )
                .is_err(),
                "invalid existing cleanup receipt must be rejected: {invalid_case}"
            );
            assert_eq!(
                backend.close_calls.get(),
                1,
                "receipt validation must not repeat or widen cleanup: {invalid_case}"
            );
        }
    }

    #[test]
    fn wrong_hwnd_is_refused_without_closing_an_owner_window() {
        let root = TestRoot::new();
        let backend = FakeBackend::exact();
        let ownership = register(&root, &backend);
        backend.native_window_id.set(Some(73));

        let receipt = cleanup_temporary_windows_terminal(
            &backend,
            &ownership,
            TemporaryWindowProductDisposition::Fail,
        )
        .expect("refusal records");

        assert_eq!(
            receipt.product_disposition,
            TemporaryWindowProductDisposition::Fail
        );
        assert_eq!(receipt.temporary_wt_cleanup, "FAIL");
        assert_eq!(receipt.detail, "HWND_MISMATCH_REFUSED");
        assert_eq!(receipt.owner_windows_closed, 0);
        assert_eq!(backend.close_calls.get(), 0);
    }

    #[test]
    fn missing_anchor_is_never_closed_or_guessed() {
        let root = TestRoot::new();
        let backend = FakeBackend::exact();
        let ownership = register(&root, &backend);
        backend.anchor_matches.set(0);
        backend.window_matches.set(0);
        backend.native_window_id.set(None);

        let receipt = cleanup_temporary_windows_terminal(
            &backend,
            &ownership,
            TemporaryWindowProductDisposition::Blocked,
        )
        .expect("already absent exact window records");

        assert_eq!(receipt.temporary_wt_cleanup, "PASS");
        assert_eq!(receipt.detail, "EXACT_OWNED_WINDOW_ALREADY_ABSENT");
        assert_eq!(receipt.temporary_windows_closed, 0);
        assert_eq!(receipt.owned_temporary_wt_remaining, 0);
        assert_eq!(backend.close_calls.get(), 0);
    }

    #[test]
    fn ambiguous_exact_anchor_is_refused() {
        let root = TestRoot::new();
        let backend = FakeBackend::exact();
        let ownership = register(&root, &backend);
        backend.anchor_matches.set(2);
        backend.window_matches.set(2);
        backend.native_window_id.set(None);

        let receipt = cleanup_temporary_windows_terminal(
            &backend,
            &ownership,
            TemporaryWindowProductDisposition::Blocked,
        )
        .expect("ambiguity records");

        assert_eq!(receipt.temporary_wt_cleanup, "FAIL");
        assert_eq!(receipt.detail, "AMBIGUOUS_EXACT_OWNERSHIP_REFUSED");
        assert_eq!(receipt.owner_windows_closed, 0);
        assert_eq!(backend.close_calls.get(), 0);
    }

    #[test]
    fn stale_exact_owned_record_recovers_only_the_same_hwnd() {
        let root = TestRoot::new();
        let creating_backend = FakeBackend::exact();
        let ownership = register(&root, &creating_backend);
        let recovery_backend = FakeBackend::exact();

        let receipt = cleanup_temporary_windows_terminal(
            &recovery_backend,
            &ownership,
            TemporaryWindowProductDisposition::Exception,
        )
        .expect("stale exact ownership recovers");

        assert_eq!(receipt.temporary_wt_cleanup, "PASS");
        assert_eq!(recovery_backend.close_calls.get(), 1);
        assert_eq!(creating_backend.close_calls.get(), 0);
    }

    #[test]
    fn cleanup_failure_does_not_overwrite_product_failure() {
        let root = TestRoot::new();
        let backend = FakeBackend::exact();
        let ownership = register(&root, &backend);
        backend.close_fails.set(true);

        let receipt = cleanup_temporary_windows_terminal(
            &backend,
            &ownership,
            TemporaryWindowProductDisposition::Fail,
        )
        .expect("dual failure records");

        assert_eq!(
            receipt.product_disposition,
            TemporaryWindowProductDisposition::Fail
        );
        assert_eq!(receipt.temporary_wt_cleanup, "FAIL");
        assert_eq!(receipt.detail, "EXACT_CLOSE_FAILED");
    }

    #[test]
    fn cleanup_writes_a_durable_receipt() {
        let root = TestRoot::new();
        let backend = FakeBackend::exact();
        let ownership = register(&root, &backend);
        let receipt = cleanup_temporary_windows_terminal(
            &backend,
            &ownership,
            TemporaryWindowProductDisposition::Pass,
        )
        .expect("cleanup records");
        let receipt_path = ownership.with_file_name(
            ownership
                .file_name()
                .and_then(|value| value.to_str())
                .expect("UTF-8 filename")
                .replace(".ownership.json", ".cleanup.json"),
        );
        let durable: super::TemporaryWindowsTerminalCleanupReceipt =
            serde_json::from_slice(&fs::read(receipt_path).expect("receipt bytes"))
                .expect("receipt JSON");

        assert_eq!(durable, receipt);
    }

    #[test]
    fn unregistered_cleanup_refuses_ambiguity_and_closes_one_exact_hwnd() {
        let backend = FakeBackend::exact();
        assert!(
            close_unregistered_exact_anchor(&backend, "TB-WT-ANCHOR-TBWT-abc")
                .expect("one exact HWND closes")
        );
        assert_eq!(backend.close_calls.get(), 1);

        let ambiguous = FakeBackend::exact();
        ambiguous.anchor_matches.set(2);
        ambiguous.window_matches.set(2);
        ambiguous.native_window_id.set(None);
        assert!(close_unregistered_exact_anchor(&ambiguous, "TB-WT-ANCHOR-TBWT-abc").is_err());
        assert_eq!(ambiguous.close_calls.get(), 0);
    }

    #[test]
    fn next_run_recovers_a_nested_stale_exact_owned_record() {
        let registry = TestRoot::new();
        let run_root = registry.0.join("prior-run");
        fs::create_dir(&run_root).expect("prior run root creates");
        let creating_backend = FakeBackend::exact();
        let run = TestRoot(run_root);
        let ownership = register(&run, &creating_backend);
        assert!(ownership.is_file());
        // Prevent the nested helper from deleting the registry-owned directory.
        std::mem::forget(run);

        let recovery_backend = FakeBackend::exact();
        recovery_backend.creator_process_started_unix_ms.set(None);
        let receipts = recover_stale_temporary_windows_terminals(&recovery_backend, &registry.0)
            .expect("stale exact-owned record recovers");

        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].temporary_wt_cleanup, "PASS");
        assert_eq!(recovery_backend.close_calls.get(), 1);
    }

    #[test]
    fn recovery_refuses_an_unfinished_record_while_its_creator_is_still_active() {
        let registry = TestRoot::new();
        let creating_backend = FakeBackend::exact();
        let ownership = register(&registry, &creating_backend);
        assert!(ownership.is_file());

        let recovery_backend = FakeBackend::exact();
        assert!(
            recover_stale_temporary_windows_terminals(&recovery_backend, &registry.0).is_err(),
            "an unfinished record is not stale while its exact creator process is active"
        );
        assert_eq!(recovery_backend.close_calls.get(), 0);
    }

    #[test]
    fn recovery_treats_a_reused_pid_as_an_exited_creator_instance() {
        let registry = TestRoot::new();
        let creating_backend = FakeBackend::exact();
        let ownership = register(&registry, &creating_backend);
        assert!(ownership.is_file());

        let recovery_backend = FakeBackend::exact();
        recovery_backend
            .creator_process_started_unix_ms
            .set(Some(42));
        let receipts = recover_stale_temporary_windows_terminals(&recovery_backend, &registry.0)
            .expect("a newer process at the reused PID is not the recorded creator");

        assert_eq!(receipts.len(), 1);
        assert_eq!(recovery_backend.close_calls.get(), 1);
    }

    #[test]
    fn recovery_refuses_a_legacy_record_without_creator_instance_proof() {
        let registry = TestRoot::new();
        let creating_backend = FakeBackend::exact();
        let ownership_path = register(&registry, &creating_backend);
        let mut ownership: serde_json::Value =
            serde_json::from_slice(&fs::read(&ownership_path).expect("ownership record reads"))
                .expect("ownership record parses");
        ownership
            .as_object_mut()
            .expect("ownership is an object")
            .remove("creator_process_started_unix_ms");
        fs::write(
            &ownership_path,
            serde_json::to_vec_pretty(&ownership).expect("legacy ownership serializes"),
        )
        .expect("legacy ownership fixture writes");

        let recovery_backend = FakeBackend::exact();
        recovery_backend.creator_process_started_unix_ms.set(None);
        assert!(
            recover_stale_temporary_windows_terminals(&recovery_backend, &registry.0).is_err(),
            "recovery requires positive creator-instance evidence"
        );
        assert_eq!(recovery_backend.close_calls.get(), 0);
    }

    #[test]
    fn next_run_retries_exact_owned_window_after_an_immutable_cleanup_failure() {
        let registry = TestRoot::new();
        let creating_backend = FakeBackend::exact();
        let ownership = register(&registry, &creating_backend);
        creating_backend.close_fails.set(true);
        let failed = cleanup_temporary_windows_terminal(
            &creating_backend,
            &ownership,
            TemporaryWindowProductDisposition::Fail,
        )
        .expect("initial exact close failure records");
        assert_eq!(failed.temporary_wt_cleanup, "FAIL");
        assert_eq!(failed.owned_temporary_wt_remaining, 1);

        let recovery_backend = FakeBackend::exact();
        recovery_backend.creator_process_started_unix_ms.set(None);
        let receipts = recover_stale_temporary_windows_terminals(&recovery_backend, &registry.0)
            .expect("later run re-proves ownership and closes the stale exact HWND");

        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].temporary_wt_cleanup, "PASS");
        assert_eq!(receipts[0].owned_temporary_wt_remaining, 0);
        assert_eq!(
            receipts[0].product_disposition,
            TemporaryWindowProductDisposition::Fail,
            "recovery preserves the original product result"
        );
        assert_eq!(recovery_backend.close_calls.get(), 1);
        assert_eq!(
            cleanup_temporary_windows_terminal(
                &creating_backend,
                &ownership,
                TemporaryWindowProductDisposition::Fail,
            )
            .expect("original failure receipt remains immutable"),
            failed
        );
        assert!(
            recover_stale_temporary_windows_terminals(&recovery_backend, &registry.0)
                .expect("successful recovery is terminal")
                .is_empty()
        );
    }

    #[test]
    fn exact_active_owner_retries_cleanup_and_preserves_product_failure() {
        let registry = TestRoot::new();
        let backend = FakeBackend::exact();
        let ownership = register(&registry, &backend);
        backend.close_fails.set(true);
        let failed = cleanup_temporary_windows_terminal(
            &backend,
            &ownership,
            TemporaryWindowProductDisposition::Fail,
        )
        .expect("initial exact close failure records");
        assert_eq!(failed.temporary_wt_cleanup, "FAIL");

        backend.close_fails.set(false);
        let retried = retry_temporary_windows_terminal_cleanup(&backend, &ownership, 4242)
            .expect("the exact active creator retries its own cleanup");

        assert_eq!(retried.temporary_wt_cleanup, "PASS");
        assert_eq!(retried.owned_temporary_wt_remaining, 0);
        assert_eq!(
            retried.product_disposition,
            TemporaryWindowProductDisposition::Fail
        );
        assert_eq!(backend.close_calls.get(), 2);
    }

    #[test]
    fn active_owner_retry_refuses_the_wrong_creator_pid() {
        let registry = TestRoot::new();
        let backend = FakeBackend::exact();
        let ownership = register(&registry, &backend);
        backend.close_fails.set(true);
        cleanup_temporary_windows_terminal(
            &backend,
            &ownership,
            TemporaryWindowProductDisposition::Pass,
        )
        .expect("initial exact close failure records");
        backend.close_fails.set(false);

        assert!(
            retry_temporary_windows_terminal_cleanup(&backend, &ownership, 4243).is_err(),
            "a different process cannot retry another run's cleanup"
        );
        assert_eq!(backend.close_calls.get(), 1);
    }

    #[test]
    fn active_owner_retry_refuses_a_reused_creator_pid() {
        let registry = TestRoot::new();
        let backend = FakeBackend::exact();
        let ownership = register(&registry, &backend);
        backend.close_fails.set(true);
        cleanup_temporary_windows_terminal(
            &backend,
            &ownership,
            TemporaryWindowProductDisposition::Pass,
        )
        .expect("initial exact close failure records");
        backend.close_fails.set(false);
        backend.creator_process_started_unix_ms.set(Some(42));

        assert!(
            retry_temporary_windows_terminal_cleanup(&backend, &ownership, 4242).is_err(),
            "PID reuse cannot inherit exact ownership"
        );
        assert_eq!(backend.close_calls.get(), 1);
    }
}
