//! Versioned, portable user-settings document primitives for G54.
//!
//! This module serializes only typed, user-owned configuration. It also owns
//! the typed import plan and the small, snapshot-guarded transaction that
//! applies that plan across the three user-local stores.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    interface_preferences::{
        HumanColor, InterfaceLanguage, InterfacePreferences,
        InterfacePreferencesConditionalOutcome, InterfacePreferencesSnapshot,
        InterfacePreferencesSnapshotSaveOutcome, InterfacePreferencesStore,
        InterfacePreferencesWriteReceipt,
    },
    repo::{
        CanonicalRepositoryIdentity, RepositoryAlias, WorkspacePreferenceStore,
        WorkspacePreferences, WorkspacePreferencesConditionalOutcome, WorkspacePreferencesSnapshot,
        WorkspacePreferencesSnapshotSaveOutcome, WorkspacePreferencesWriteReceipt,
    },
    settings::{
        ActivityMode, ConditionalSaveOutcome, PresentationSettings, PresentationSettingsSnapshot,
        PresentationSettingsStore, PresentationSettingsWriteReceipt, PresentationTheme,
        SnapshotSaveOutcome, SpinnerPreset, TabColorMode, TitleMode,
    },
};

/// Stable schema identifier for portable user configuration exports.
pub const EXPORT_SCHEMA_V1: &str = "tabbeacon-export-v1";
/// Hard bound before JSON parsing so an import cannot become an unbounded log
/// or arbitrary system image.
pub const MAX_EXPORT_BYTES: usize = 1024 * 1024;

static EXPORT_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Safe pure-document failure. Store and CLI layers map their own I/O errors.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SettingsTransferError {
    /// Input exceeds the fixed bounded import size.
    Oversize,
    /// The document is malformed, unsupported, or violates this schema.
    InvalidDocument,
}

impl fmt::Display for SettingsTransferError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Oversize => "the TabBeacon export document exceeds the supported size",
            Self::InvalidDocument => "the TabBeacon export document is malformed or unsupported",
        })
    }
}

impl std::error::Error for SettingsTransferError {}

/// Safe result from writing a requested export file.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ExportFileError {
    /// A file already exists and the caller did not explicitly allow replacement.
    AlreadyExists,
    /// The requested final target is a symbolic link and is never followed.
    SymbolicLinkTarget,
    /// The requested output could not be written safely.
    Io,
    /// The final target was protected, but an owned temporary artifact could
    /// not be cleaned or a compensated replacement could not be verified.
    PartialState,
}

impl fmt::Display for ExportFileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::AlreadyExists => "the export file already exists; use --force to replace it",
            Self::SymbolicLinkTarget => "the export target is a symbolic link",
            Self::Io => "the export file could not be written safely",
            Self::PartialState => {
                "the export target may be safe, but a temporary replacement artifact needs review"
            }
        })
    }
}

impl std::error::Error for ExportFileError {}

/// A conflict that makes an otherwise valid import unsafe to apply.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ImportPlanConflict {
    /// An imported alias would duplicate another effective local alias.
    AliasCollision,
}

/// One immutable, validated import preview. It carries no raw external path
/// and exposes only aggregate matching information to presentation code.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportPlan {
    presentation: Option<PresentationSettings>,
    interface: Option<InterfacePreferences>,
    workspace_preferences: Option<WorkspacePreferences>,
    portable_matches: usize,
    unmatched_entries: usize,
    conflicts: Vec<ImportPlanConflict>,
}

impl ImportPlan {
    /// Whether the plan has no conflicts and may be applied explicitly.
    #[must_use]
    pub const fn is_applicable(&self) -> bool {
        self.conflicts.is_empty()
    }

    /// Number of source aliases matched to already-known portable identities.
    #[must_use]
    pub const fn portable_matches(&self) -> usize {
        self.portable_matches
    }

    /// Number of source aliases that were not bound locally.
    #[must_use]
    pub const fn unmatched_entries(&self) -> usize {
        self.unmatched_entries
    }

    /// Conflicts shown before any Apply attempt.
    #[must_use]
    pub fn conflicts(&self) -> &[ImportPlanConflict] {
        &self.conflicts
    }

    /// Whether the plan would change at least one local user-owned store.
    #[must_use]
    pub const fn has_changes(&self) -> bool {
        self.presentation.is_some()
            || self.interface.is_some()
            || self.workspace_preferences.is_some()
    }

    /// Whether Presentation settings would change on Apply.
    #[must_use]
    pub const fn changes_presentation(&self) -> bool {
        self.presentation.is_some()
    }

    /// Whether Interface preferences would change on Apply.
    #[must_use]
    pub const fn changes_interface(&self) -> bool {
        self.interface.is_some()
    }

    /// Whether Workspace alias preferences would change on Apply.
    #[must_use]
    pub const fn changes_workspace_preferences(&self) -> bool {
        self.workspace_preferences.is_some()
    }
}

/// Outcome of the explicit multi-store Apply operation.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ImportApplyOutcome {
    /// Every planned store write completed and was read back successfully.
    Applied,
    /// A concurrent change or store failure occurred, and all prior writes
    /// were compensated back to their exact snapshots.
    RolledBack,
    /// At least one prior write could not be proved restored; callers must
    /// surface this as a hard partial-state failure.
    PartialState,
    /// The preview contained a conflict, so no store was touched.
    Conflict,
}

/// Canonical locale-independent portable configuration document.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingsExportV1 {
    schema: String,
    presentation: Option<PresentationExport>,
    interface: Option<InterfaceExport>,
    workspace_aliases: BTreeMap<String, String>,
    omitted_device_local_workspace_aliases: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PresentationExport {
    title: String,
    tab_color: String,
    activity: String,
    spinner: String,
    theme: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct InterfaceExport {
    language: String,
    color: String,
    reduced_motion: bool,
}

impl SettingsExportV1 {
    /// Builds a portable document. Ordinary-directory aliases are deliberately
    /// omitted: their identities are local absolute-path derivatives and must
    /// never be represented as cross-device portable state.
    #[must_use]
    pub fn new(
        presentation: Option<PresentationSettings>,
        interface: Option<InterfacePreferences>,
        workspace_preferences: &WorkspacePreferences,
    ) -> Self {
        let mut workspace_aliases = BTreeMap::new();
        let mut omitted_device_local_workspace_aliases = 0;
        for (identity, alias) in workspace_preferences.overrides() {
            if identity.as_str().starts_with("dir-v1:") {
                omitted_device_local_workspace_aliases += 1;
            } else {
                workspace_aliases.insert(
                    portable_workspace_key(identity.as_str()),
                    alias.as_str().to_owned(),
                );
            }
        }
        Self {
            schema: EXPORT_SCHEMA_V1.to_owned(),
            presentation: presentation.map(PresentationExport::from),
            interface: interface.map(InterfaceExport::from),
            workspace_aliases,
            omitted_device_local_workspace_aliases,
        }
    }

    /// Produces deterministic, locale-independent canonical JSON bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization cannot produce a valid bounded document.
    pub fn to_canonical_json(&self) -> Result<Vec<u8>, SettingsTransferError> {
        let bytes =
            serde_json::to_vec_pretty(self).map_err(|_| SettingsTransferError::InvalidDocument)?;
        if bytes.len() > MAX_EXPORT_BYTES {
            return Err(SettingsTransferError::Oversize);
        }
        Ok(bytes)
    }

    /// Parses a bounded document and refuses a future/unknown schema.
    ///
    /// # Errors
    ///
    /// Returns an error for oversize, malformed, unknown-schema, or invalid typed values.
    pub fn parse(bytes: &[u8]) -> Result<Self, SettingsTransferError> {
        if bytes.len() > MAX_EXPORT_BYTES {
            return Err(SettingsTransferError::Oversize);
        }
        let document: Self =
            serde_json::from_slice(bytes).map_err(|_| SettingsTransferError::InvalidDocument)?;
        if document.schema != EXPORT_SCHEMA_V1 {
            return Err(SettingsTransferError::InvalidDocument);
        }
        document.validate()?;
        Ok(document)
    }

    /// Typed presentation draft, when the portable document carries one.
    ///
    /// # Errors
    ///
    /// Returns an error if stored presentation tokens are unsupported.
    pub fn presentation(&self) -> Result<Option<PresentationSettings>, SettingsTransferError> {
        self.presentation
            .as_ref()
            .map(PresentationExport::settings)
            .transpose()
    }

    /// Typed interface draft, when the portable document carries one.
    ///
    /// # Errors
    ///
    /// Returns an error if stored Interface tokens are unsupported.
    pub fn interface(&self) -> Result<Option<InterfacePreferences>, SettingsTransferError> {
        self.interface
            .as_ref()
            .map(InterfaceExport::preferences)
            .transpose()
    }

    /// Portable Git identity-digest override map. The map has no raw identity,
    /// workspace root, or display path.
    #[must_use]
    pub fn workspace_aliases(&self) -> &BTreeMap<String, String> {
        &self.workspace_aliases
    }

    /// Whether a portable Presentation snapshot is present.
    #[must_use]
    pub const fn has_presentation(&self) -> bool {
        self.presentation.is_some()
    }

    /// Whether a portable Interface snapshot is present.
    #[must_use]
    pub const fn has_interface(&self) -> bool {
        self.interface.is_some()
    }

    /// Number of portable Git workspace aliases in the document.
    #[must_use]
    pub fn portable_workspace_alias_count(&self) -> usize {
        self.workspace_aliases.len()
    }

    /// Number of truthful ordinary-directory omissions.
    #[must_use]
    pub const fn omitted_device_local_workspace_aliases(&self) -> usize {
        self.omitted_device_local_workspace_aliases
    }

    /// Builds the complete preview before any user-local store is opened for
    /// mutation. Only identities already known to the local registry can be
    /// matched; an unknown digest is reported as unmatched, never guessed.
    ///
    /// # Errors
    ///
    /// Returns an error when a typed value in an otherwise parsed document is
    /// unsupported.
    pub fn import_plan(
        &self,
        presentation_snapshot: &PresentationSettingsSnapshot,
        interface_snapshot: &InterfacePreferencesSnapshot,
        workspace_snapshot: &WorkspacePreferencesSnapshot,
        known_identities: &BTreeSet<CanonicalRepositoryIdentity>,
        generated_aliases: &BTreeMap<CanonicalRepositoryIdentity, RepositoryAlias>,
    ) -> Result<ImportPlan, SettingsTransferError> {
        let presentation = self
            .presentation()?
            .filter(|candidate| *candidate != presentation_snapshot.settings());
        let interface = self
            .interface()?
            .filter(|candidate| *candidate != interface_snapshot.preferences());

        let mut matched = BTreeMap::<CanonicalRepositoryIdentity, RepositoryAlias>::new();
        let mut unmatched_entries = 0;
        for (portable_key, alias) in &self.workspace_aliases {
            let Some(identity) = known_identities
                .iter()
                .find(|identity| portable_workspace_key(identity.as_str()) == *portable_key)
            else {
                unmatched_entries += 1;
                continue;
            };
            let alias = RepositoryAlias::new(alias.clone())
                .map_err(|_| SettingsTransferError::InvalidDocument)?;
            matched.insert(identity.clone(), alias);
        }

        let mut conflicts = Vec::new();
        let mut imported_aliases = BTreeSet::new();
        for alias in matched.values() {
            if !imported_aliases.insert(alias.clone()) {
                conflicts.push(ImportPlanConflict::AliasCollision);
                break;
            }
        }
        if conflicts.is_empty() {
            for (identity, alias) in &matched {
                let collides_with_effective_alias =
                    known_identities.iter().any(|existing_identity| {
                        if existing_identity == identity {
                            return false;
                        }
                        let effective_alias =
                            matched.get(existing_identity).cloned().or_else(|| {
                                workspace_snapshot
                                    .preferences()
                                    .override_for(existing_identity)
                                    .or_else(|| generated_aliases.get(existing_identity).cloned())
                            });
                        effective_alias.as_ref() == Some(alias)
                    });
                if collides_with_effective_alias {
                    conflicts.push(ImportPlanConflict::AliasCollision);
                    break;
                }
            }
        }

        let workspace_preferences = if conflicts.is_empty() && !matched.is_empty() {
            let mut replacement = workspace_snapshot.preferences().clone();
            for (identity, alias) in matched {
                replacement = replacement.with_override(identity, alias);
            }
            (replacement != *workspace_snapshot.preferences()).then_some(replacement)
        } else {
            None
        };

        Ok(ImportPlan {
            presentation,
            interface,
            workspace_preferences,
            portable_matches: self.workspace_aliases.len() - unmatched_entries,
            unmatched_entries,
            conflicts,
        })
    }

    fn validate(&self) -> Result<(), SettingsTransferError> {
        let _ = self.presentation()?;
        let _ = self.interface()?;
        if self.workspace_aliases.iter().any(|(key, alias)| {
            key.len() != 64
                || !key.bytes().all(|byte| byte.is_ascii_hexdigit())
                || crate::repo::RepositoryAlias::new(alias.clone()).is_err()
        }) {
            return Err(SettingsTransferError::InvalidDocument);
        }
        Ok(())
    }
}

/// Writes canonical export bytes to a requested file without ever streaming
/// directly into the final target. Existing files are never overwritten unless
/// the caller explicitly requests replacement.
///
/// # Errors
///
/// Returns a safe classification without exposing filesystem internals.
pub fn write_export_file(
    path: &Path,
    bytes: &[u8],
    replace_existing: bool,
) -> Result<(), ExportFileError> {
    if bytes.len() > MAX_EXPORT_BYTES {
        return Err(ExportFileError::Io);
    }
    let parent = path.parent().ok_or(ExportFileError::Io)?;
    fs::create_dir_all(parent).map_err(|_| ExportFileError::Io)?;
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(ExportFileError::SymbolicLinkTarget);
        }
        Ok(metadata) if !metadata.file_type().is_file() => return Err(ExportFileError::Io),
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err(ExportFileError::Io),
    }

    let temporary = write_export_temp(parent, bytes)?;

    if !replace_existing {
        return install_new_export(&temporary, path);
    }

    replace_export_atomically(path, &temporary)
}

fn write_export_temp(parent: &Path, bytes: &[u8]) -> Result<PathBuf, ExportFileError> {
    let (temporary, mut file) = create_export_sidecar(parent, "write")?;
    if file.write_all(bytes).is_err() || file.sync_all().is_err() {
        return if remove_owned_export_sidecar(&temporary) {
            Err(ExportFileError::Io)
        } else {
            Err(ExportFileError::PartialState)
        };
    }
    Ok(temporary)
}

fn install_new_export(temporary: &Path, path: &Path) -> Result<(), ExportFileError> {
    match fs::hard_link(temporary, path) {
        Ok(()) => {
            if remove_owned_export_sidecar(temporary) {
                Ok(())
            } else {
                Err(ExportFileError::PartialState)
            }
        }
        Err(error) => {
            let cleanup_ok = remove_owned_export_sidecar(temporary);
            if !cleanup_ok {
                return Err(ExportFileError::PartialState);
            }
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                Err(ExportFileError::AlreadyExists)
            } else {
                Err(ExportFileError::Io)
            }
        }
    }
}

/// Replaces an existing final file with a complete same-directory sidecar in
/// one Windows filesystem operation. This avoids the observable missing-file
/// interval that a backup-and-rename sequence would create.
#[cfg(windows)]
fn replace_export_atomically(path: &Path, temporary: &Path) -> Result<(), ExportFileError> {
    use std::{ffi::OsStr, iter, os::windows::ffi::OsStrExt};
    use windows::{
        Win32::Storage::FileSystem::{REPLACE_FILE_FLAGS, ReplaceFileW},
        core::PCWSTR,
    };

    fn wide_null(path: &OsStr) -> Vec<u16> {
        path.encode_wide().chain(iter::once(0)).collect()
    }

    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return cleanup_export_temporary(temporary, ExportFileError::SymbolicLinkTarget);
        }
        Ok(metadata) if !metadata.file_type().is_file() => {
            return cleanup_export_temporary(temporary, ExportFileError::Io);
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return install_new_export(temporary, path);
        }
        Err(_) => return cleanup_export_temporary(temporary, ExportFileError::Io),
    }

    let destination = wide_null(path.as_os_str());
    let replacement = wide_null(temporary.as_os_str());
    // SAFETY: both buffers are NUL-terminated UTF-16 paths and remain alive
    // for the call; the optional backup and reserved pointers are null.
    #[allow(unsafe_code)]
    let replaced = unsafe {
        ReplaceFileW(
            PCWSTR(destination.as_ptr()),
            PCWSTR(replacement.as_ptr()),
            PCWSTR::null(),
            REPLACE_FILE_FLAGS(0),
            None,
            None,
        )
    };
    match replaced {
        Ok(()) => Ok(()),
        Err(_) => cleanup_export_temporary(temporary, ExportFileError::Io),
    }
}

#[cfg(not(windows))]
fn replace_export_atomically(path: &Path, temporary: &Path) -> Result<(), ExportFileError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            cleanup_export_temporary(temporary, ExportFileError::SymbolicLinkTarget)
        }
        Ok(metadata) if !metadata.file_type().is_file() => {
            cleanup_export_temporary(temporary, ExportFileError::Io)
        }
        Ok(_) => match fs::rename(temporary, path) {
            Ok(()) => Ok(()),
            Err(_) => cleanup_export_temporary(temporary, ExportFileError::Io),
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            install_new_export(temporary, path)
        }
        Err(_) => cleanup_export_temporary(temporary, ExportFileError::Io),
    }
}

fn create_export_sidecar(parent: &Path, purpose: &str) -> Result<(PathBuf, File), ExportFileError> {
    for _ in 0..16 {
        let sequence = EXPORT_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(
            ".tabbeacon-export-{purpose}-{}-{sequence}.tmp",
            std::process::id()
        ));
        match OpenOptions::new().create_new(true).write(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err(ExportFileError::Io),
        }
    }
    Err(ExportFileError::Io)
}

fn remove_owned_export_sidecar(path: &Path) -> bool {
    match fs::remove_file(path) {
        Ok(()) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
        Err(_) => false,
    }
}

fn cleanup_export_temporary(
    temporary: &Path,
    error: ExportFileError,
) -> Result<(), ExportFileError> {
    if remove_owned_export_sidecar(temporary) {
        Err(error)
    } else {
        Err(ExportFileError::PartialState)
    }
}

/// Applies one conflict-free plan under each store's exact snapshot guard.
/// A later failure restores every earlier store only when the write receipt is
/// still exact. Any unverified restoration is returned as `PartialState`.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_lines)] // The three stores intentionally mirror one bounded transaction.
#[must_use]
pub fn apply_import_plan(
    plan: &ImportPlan,
    presentation_store: &PresentationSettingsStore,
    presentation_snapshot: &PresentationSettingsSnapshot,
    interface_store: &InterfacePreferencesStore,
    interface_snapshot: &InterfacePreferencesSnapshot,
    workspace_store: &WorkspacePreferenceStore,
    workspace_snapshot: &WorkspacePreferencesSnapshot,
) -> ImportApplyOutcome {
    if !plan.is_applicable() {
        return ImportApplyOutcome::Conflict;
    }

    let mut presentation_receipt = None;
    let mut interface_receipt = None;
    let mut workspace_receipt = None;

    if let Some(replacement) = plan.presentation {
        match presentation_store.save_snapshot_if_unchanged(presentation_snapshot, replacement) {
            Ok(SnapshotSaveOutcome::Saved(receipt)) => presentation_receipt = Some(receipt),
            Ok(SnapshotSaveOutcome::Conflict) => return ImportApplyOutcome::Conflict,
            // A storage error can arrive after an atomic commit reached disk.
            // There is no receipt to prove or restore that store, so the
            // result must remain a hard partial-state outcome.
            Err(_) => {
                return rollback_import(
                    presentation_store,
                    presentation_snapshot,
                    presentation_receipt.as_ref(),
                    interface_store,
                    interface_snapshot,
                    interface_receipt.as_ref(),
                    workspace_store,
                    workspace_snapshot,
                    workspace_receipt.as_ref(),
                    true,
                );
            }
        }
        if !presentation_receipt.as_ref().is_some_and(|receipt| {
            matches!(
                presentation_store.write_receipt_is_current(receipt),
                Ok(true)
            )
        }) {
            return rollback_import(
                presentation_store,
                presentation_snapshot,
                presentation_receipt.as_ref(),
                interface_store,
                interface_snapshot,
                interface_receipt.as_ref(),
                workspace_store,
                workspace_snapshot,
                workspace_receipt.as_ref(),
                false,
            );
        }
    }

    if let Some(replacement) = plan.interface {
        match interface_store.save_snapshot_if_unchanged(interface_snapshot, replacement) {
            Ok(InterfacePreferencesSnapshotSaveOutcome::Saved(receipt)) => {
                interface_receipt = Some(receipt);
            }
            Ok(InterfacePreferencesSnapshotSaveOutcome::Conflict) => {
                return rollback_import(
                    presentation_store,
                    presentation_snapshot,
                    presentation_receipt.as_ref(),
                    interface_store,
                    interface_snapshot,
                    interface_receipt.as_ref(),
                    workspace_store,
                    workspace_snapshot,
                    workspace_receipt.as_ref(),
                    false,
                );
            }
            // A failed commit has no receipt and may have reached disk.
            Err(_) => {
                return rollback_import(
                    presentation_store,
                    presentation_snapshot,
                    presentation_receipt.as_ref(),
                    interface_store,
                    interface_snapshot,
                    interface_receipt.as_ref(),
                    workspace_store,
                    workspace_snapshot,
                    workspace_receipt.as_ref(),
                    true,
                );
            }
        }
        if !interface_receipt.as_ref().is_some_and(|receipt| {
            matches!(interface_store.write_receipt_is_current(receipt), Ok(true))
        }) {
            return rollback_import(
                presentation_store,
                presentation_snapshot,
                presentation_receipt.as_ref(),
                interface_store,
                interface_snapshot,
                interface_receipt.as_ref(),
                workspace_store,
                workspace_snapshot,
                workspace_receipt.as_ref(),
                false,
            );
        }
    }

    if let Some(replacement) = plan.workspace_preferences.as_ref() {
        match workspace_store.save_snapshot_if_unchanged(workspace_snapshot, replacement.clone()) {
            Ok(WorkspacePreferencesSnapshotSaveOutcome::Saved(receipt)) => {
                workspace_receipt = Some(receipt);
            }
            Ok(WorkspacePreferencesSnapshotSaveOutcome::Conflict) => {
                return rollback_import(
                    presentation_store,
                    presentation_snapshot,
                    presentation_receipt.as_ref(),
                    interface_store,
                    interface_snapshot,
                    interface_receipt.as_ref(),
                    workspace_store,
                    workspace_snapshot,
                    workspace_receipt.as_ref(),
                    false,
                );
            }
            // A failed commit has no receipt and may have reached disk.
            Err(_) => {
                return rollback_import(
                    presentation_store,
                    presentation_snapshot,
                    presentation_receipt.as_ref(),
                    interface_store,
                    interface_snapshot,
                    interface_receipt.as_ref(),
                    workspace_store,
                    workspace_snapshot,
                    workspace_receipt.as_ref(),
                    true,
                );
            }
        }
        if !workspace_receipt.as_ref().is_some_and(|receipt| {
            matches!(workspace_store.write_receipt_is_current(receipt), Ok(true))
        }) {
            return rollback_import(
                presentation_store,
                presentation_snapshot,
                presentation_receipt.as_ref(),
                interface_store,
                interface_snapshot,
                interface_receipt.as_ref(),
                workspace_store,
                workspace_snapshot,
                workspace_receipt.as_ref(),
                false,
            );
        }
    }

    // The individual immediate checks only prove each write at its own point
    // in time. Re-check every receipt after the final write so an earlier
    // store drifting during a later store operation cannot be reported as a
    // successful multi-store Apply.
    let all_receipts_current = presentation_receipt.as_ref().is_none_or(|receipt| {
        matches!(
            presentation_store.write_receipt_is_current(receipt),
            Ok(true)
        )
    }) && interface_receipt.as_ref().is_none_or(|receipt| {
        matches!(interface_store.write_receipt_is_current(receipt), Ok(true))
    }) && workspace_receipt.as_ref().is_none_or(|receipt| {
        matches!(workspace_store.write_receipt_is_current(receipt), Ok(true))
    });
    if !all_receipts_current {
        return rollback_import(
            presentation_store,
            presentation_snapshot,
            presentation_receipt.as_ref(),
            interface_store,
            interface_snapshot,
            interface_receipt.as_ref(),
            workspace_store,
            workspace_snapshot,
            workspace_receipt.as_ref(),
            false,
        );
    }

    ImportApplyOutcome::Applied
}

#[allow(clippy::too_many_arguments)]
fn rollback_import(
    presentation_store: &PresentationSettingsStore,
    presentation_snapshot: &PresentationSettingsSnapshot,
    presentation_receipt: Option<&PresentationSettingsWriteReceipt>,
    interface_store: &InterfacePreferencesStore,
    interface_snapshot: &InterfacePreferencesSnapshot,
    interface_receipt: Option<&InterfacePreferencesWriteReceipt>,
    workspace_store: &WorkspacePreferenceStore,
    workspace_snapshot: &WorkspacePreferencesSnapshot,
    workspace_receipt: Option<&WorkspacePreferencesWriteReceipt>,
    failed_write_may_be_unreceipted: bool,
) -> ImportApplyOutcome {
    let workspace_restored = workspace_receipt.is_none_or(|receipt| {
        matches!(
            workspace_store.restore_snapshot_if_unchanged(receipt, workspace_snapshot),
            Ok(WorkspacePreferencesConditionalOutcome::Saved)
        ) && matches!(
            workspace_store.snapshot_is_current(workspace_snapshot),
            Ok(true)
        )
    });
    let interface_restored = interface_receipt.is_none_or(|receipt| {
        matches!(
            interface_store.restore_snapshot_if_unchanged(receipt, interface_snapshot),
            Ok(InterfacePreferencesConditionalOutcome::Saved)
        ) && matches!(
            interface_store.snapshot_is_current(interface_snapshot),
            Ok(true)
        )
    });
    let presentation_restored = presentation_receipt.is_none_or(|receipt| {
        matches!(
            presentation_store.restore_snapshot_if_unchanged(receipt, presentation_snapshot),
            Ok(ConditionalSaveOutcome::Saved)
        ) && matches!(
            presentation_store.snapshot_is_current(presentation_snapshot),
            Ok(true)
        )
    });
    if !failed_write_may_be_unreceipted
        && workspace_restored
        && interface_restored
        && presentation_restored
    {
        ImportApplyOutcome::RolledBack
    } else {
        ImportApplyOutcome::PartialState
    }
}

impl From<PresentationSettings> for PresentationExport {
    fn from(value: PresentationSettings) -> Self {
        Self {
            title: value.title().as_str().to_owned(),
            tab_color: value.tab_color().as_str().to_owned(),
            activity: value.activity().as_str().to_owned(),
            spinner: value.spinner().as_str().to_owned(),
            theme: value.theme().as_str().to_owned(),
        }
    }
}

impl PresentationExport {
    fn settings(&self) -> Result<PresentationSettings, SettingsTransferError> {
        Ok(PresentationSettings::new(
            TitleMode::parse(&self.title).ok_or(SettingsTransferError::InvalidDocument)?,
            TabColorMode::parse(&self.tab_color).ok_or(SettingsTransferError::InvalidDocument)?,
            ActivityMode::parse(&self.activity).ok_or(SettingsTransferError::InvalidDocument)?,
            SpinnerPreset::parse(&self.spinner).ok_or(SettingsTransferError::InvalidDocument)?,
            PresentationTheme::parse(&self.theme).ok_or(SettingsTransferError::InvalidDocument)?,
        ))
    }
}

impl From<InterfacePreferences> for InterfaceExport {
    fn from(value: InterfacePreferences) -> Self {
        Self {
            language: value.language().as_str().to_owned(),
            color: value.color().as_str().to_owned(),
            reduced_motion: value.reduced_motion(),
        }
    }
}

impl InterfaceExport {
    fn preferences(&self) -> Result<InterfacePreferences, SettingsTransferError> {
        Ok(InterfacePreferences::new(
            InterfaceLanguage::parse(&self.language)
                .ok_or(SettingsTransferError::InvalidDocument)?,
            HumanColor::parse(&self.color).ok_or(SettingsTransferError::InvalidDocument)?,
            self.reduced_motion,
        ))
    }
}

/// Computes the portable matching authority without serializing its input.
#[must_use]
pub fn portable_workspace_key(canonical_identity: &str) -> String {
    format!("{:x}", Sha256::digest(canonical_identity.as_bytes()))
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, BTreeSet},
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::{
        interface_preferences::{
            HumanColor, InterfaceLanguage, InterfacePreferences, InterfacePreferencesStore,
        },
        repo::{
            CanonicalRepositoryIdentity, RepositoryAlias, WorkspacePreferenceStore,
            WorkspacePreferences,
        },
        settings::{
            ActivityMode, PresentationSettings, PresentationSettingsStore, PresentationTheme,
            SpinnerPreset, TabColorMode, TitleMode,
        },
    };

    use super::{
        EXPORT_SCHEMA_V1, ImportApplyOutcome, ImportPlanConflict, MAX_EXPORT_BYTES,
        SettingsExportV1, SettingsTransferError, apply_import_plan, portable_workspace_key,
    };

    fn temporary_root(name: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock is after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("tabbeacon-settings-transfer-{name}-{unique}"))
    }

    #[test]
    fn canonical_export_round_trips_typed_preferences_without_private_identity() {
        let git = CanonicalRepositoryIdentity::new("remote:example/tabbeacon").unwrap();
        let directory = CanonicalRepositoryIdentity::new("dir-v1:private-local-path-hash").unwrap();
        let preferences = WorkspacePreferences::default()
            .with_override(git.clone(), RepositoryAlias::new("TB").unwrap())
            .with_override(directory, RepositoryAlias::new("LOCAL").unwrap());
        let settings = PresentationSettings::new(
            TitleMode::Native,
            TabColorMode::Off,
            ActivityMode::Both,
            SpinnerPreset::Braille,
            PresentationTheme::Classic,
        );
        let interface = InterfacePreferences::new(InterfaceLanguage::ZhCn, HumanColor::Never, true);
        let document = SettingsExportV1::new(Some(settings), Some(interface), &preferences);
        let first = document.to_canonical_json().unwrap();
        let parsed = SettingsExportV1::parse(&first).unwrap();
        assert_eq!(parsed.to_canonical_json().unwrap(), first);
        assert_eq!(parsed.presentation().unwrap(), Some(settings));
        assert_eq!(parsed.interface().unwrap(), Some(interface));
        assert_eq!(
            parsed
                .workspace_aliases()
                .get(&portable_workspace_key(git.as_str()))
                .map(String::as_str),
            Some("TB")
        );
        assert_eq!(parsed.omitted_device_local_workspace_aliases(), 1);
        let text = String::from_utf8(first).unwrap();
        assert!(text.contains(EXPORT_SCHEMA_V1));
        assert!(!text.contains(git.as_str()));
        assert!(!text.contains("dir-v1:"));
    }

    #[test]
    fn malformed_oversize_and_unknown_versions_fail_closed() {
        assert_eq!(
            SettingsExportV1::parse(br#"{"schema":"tabbeacon-export-v2"}"#),
            Err(SettingsTransferError::InvalidDocument)
        );
        assert_eq!(
            SettingsExportV1::parse(&vec![b' '; MAX_EXPORT_BYTES + 1]),
            Err(SettingsTransferError::Oversize)
        );
    }

    #[test]
    fn import_plan_applies_all_three_stores_after_a_preview() {
        let root = temporary_root("round-trip");
        let presentation_store = PresentationSettingsStore::new(root.join("config.toml"));
        let interface_store = InterfacePreferencesStore::new(root.join("interface.toml"));
        let workspace_store = WorkspacePreferenceStore::new(root.join("preferences.json"));
        let identity = CanonicalRepositoryIdentity::new("remote:example/tabbeacon").unwrap();
        let presentation = PresentationSettings::new(
            TitleMode::Native,
            TabColorMode::Off,
            ActivityMode::Both,
            SpinnerPreset::Braille,
            PresentationTheme::Classic,
        );
        let interface = InterfacePreferences::new(InterfaceLanguage::ZhCn, HumanColor::Never, true);
        let source_preferences = WorkspacePreferences::default()
            .with_override(identity.clone(), RepositoryAlias::new("TB").unwrap());
        let bytes = SettingsExportV1::new(Some(presentation), Some(interface), &source_preferences)
            .to_canonical_json()
            .unwrap();
        let document = SettingsExportV1::parse(&bytes).unwrap();

        let presentation_snapshot = presentation_store.snapshot_read_only().unwrap();
        let interface_snapshot = interface_store.snapshot_read_only().unwrap();
        let workspace_snapshot = workspace_store.snapshot_read_only().unwrap();
        let plan = document
            .import_plan(
                &presentation_snapshot,
                &interface_snapshot,
                &workspace_snapshot,
                &BTreeSet::from([identity.clone()]),
                &BTreeMap::new(),
            )
            .unwrap();

        assert!(plan.is_applicable());
        assert!(plan.has_changes());
        assert_eq!(plan.portable_matches(), 1);
        assert_eq!(plan.unmatched_entries(), 0);
        assert_eq!(
            apply_import_plan(
                &plan,
                &presentation_store,
                &presentation_snapshot,
                &interface_store,
                &interface_snapshot,
                &workspace_store,
                &workspace_snapshot,
            ),
            ImportApplyOutcome::Applied
        );
        assert_eq!(presentation_store.load_read_only().unwrap(), presentation);
        assert_eq!(interface_store.load_read_only().unwrap(), interface);
        assert_eq!(
            workspace_store
                .load_read_only()
                .unwrap()
                .override_for(&identity)
                .unwrap()
                .as_str(),
            "TB"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn concurrent_later_store_refuses_apply_and_compensates_earlier_store() {
        let root = temporary_root("concurrent-drift");
        let presentation_store = PresentationSettingsStore::new(root.join("config.toml"));
        let interface_store = InterfacePreferencesStore::new(root.join("interface.toml"));
        let workspace_store = WorkspacePreferenceStore::new(root.join("preferences.json"));
        let presentation = PresentationSettings::new(
            TitleMode::Native,
            TabColorMode::Off,
            ActivityMode::Both,
            SpinnerPreset::Braille,
            PresentationTheme::Classic,
        );
        let interface = InterfacePreferences::new(InterfaceLanguage::ZhCn, HumanColor::Never, true);
        let document = SettingsExportV1::new(
            Some(presentation),
            Some(interface),
            &WorkspacePreferences::default(),
        );
        let presentation_snapshot = presentation_store.snapshot_read_only().unwrap();
        let interface_snapshot = interface_store.snapshot_read_only().unwrap();
        let workspace_snapshot = workspace_store.snapshot_read_only().unwrap();
        let plan = document
            .import_plan(
                &presentation_snapshot,
                &interface_snapshot,
                &workspace_snapshot,
                &BTreeSet::new(),
                &BTreeMap::new(),
            )
            .unwrap();
        interface_store
            .save(InterfacePreferences::default().with_color(HumanColor::Always))
            .unwrap();

        assert_eq!(
            apply_import_plan(
                &plan,
                &presentation_store,
                &presentation_snapshot,
                &interface_store,
                &interface_snapshot,
                &workspace_store,
                &workspace_snapshot,
            ),
            ImportApplyOutcome::RolledBack
        );
        assert!(!presentation_store.path().exists());
        assert_eq!(
            interface_store.load_read_only().unwrap().color(),
            HumanColor::Always
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn alias_collision_is_visible_before_apply_and_touches_nothing() {
        let root = temporary_root("alias-conflict");
        let presentation_store = PresentationSettingsStore::new(root.join("config.toml"));
        let interface_store = InterfacePreferencesStore::new(root.join("interface.toml"));
        let workspace_store = WorkspacePreferenceStore::new(root.join("preferences.json"));
        let source_identity = CanonicalRepositoryIdentity::new("remote:example/source").unwrap();
        let target_identity = CanonicalRepositoryIdentity::new("remote:example/target").unwrap();
        workspace_store
            .save_snapshot_if_unchanged(
                &workspace_store.snapshot_read_only().unwrap(),
                WorkspacePreferences::default().with_override(
                    target_identity.clone(),
                    RepositoryAlias::new("SAME").unwrap(),
                ),
            )
            .unwrap();
        let document = SettingsExportV1::new(
            None,
            None,
            &WorkspacePreferences::default().with_override(
                source_identity.clone(),
                RepositoryAlias::new("SAME").unwrap(),
            ),
        );
        let presentation_snapshot = presentation_store.snapshot_read_only().unwrap();
        let interface_snapshot = interface_store.snapshot_read_only().unwrap();
        let workspace_snapshot = workspace_store.snapshot_read_only().unwrap();
        let plan = document
            .import_plan(
                &presentation_snapshot,
                &interface_snapshot,
                &workspace_snapshot,
                &BTreeSet::from([source_identity, target_identity]),
                &BTreeMap::new(),
            )
            .unwrap();

        assert_eq!(plan.conflicts(), &[ImportPlanConflict::AliasCollision]);
        assert_eq!(
            apply_import_plan(
                &plan,
                &presentation_store,
                &presentation_snapshot,
                &interface_store,
                &interface_snapshot,
                &workspace_store,
                &workspace_snapshot,
            ),
            ImportApplyOutcome::Conflict
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn alias_collision_with_an_unchanged_generated_alias_is_refused() {
        let root = temporary_root("generated-alias-conflict");
        let presentation_store = PresentationSettingsStore::new(root.join("config.toml"));
        let interface_store = InterfacePreferencesStore::new(root.join("interface.toml"));
        let workspace_store = WorkspacePreferenceStore::new(root.join("preferences.json"));
        let source_identity = CanonicalRepositoryIdentity::new("remote:example/source").unwrap();
        let existing_identity =
            CanonicalRepositoryIdentity::new("remote:example/existing").unwrap();
        let document = SettingsExportV1::new(
            None,
            None,
            &WorkspacePreferences::default().with_override(
                source_identity.clone(),
                RepositoryAlias::new("TAKEN").unwrap(),
            ),
        );
        let presentation_snapshot = presentation_store.snapshot_read_only().unwrap();
        let interface_snapshot = interface_store.snapshot_read_only().unwrap();
        let workspace_snapshot = workspace_store.snapshot_read_only().unwrap();
        let plan = document
            .import_plan(
                &presentation_snapshot,
                &interface_snapshot,
                &workspace_snapshot,
                &BTreeSet::from([source_identity, existing_identity.clone()]),
                &BTreeMap::from([(existing_identity, RepositoryAlias::new("TAKEN").unwrap())]),
            )
            .unwrap();

        assert_eq!(plan.conflicts(), &[ImportPlanConflict::AliasCollision]);
        assert_eq!(
            apply_import_plan(
                &plan,
                &presentation_store,
                &presentation_snapshot,
                &interface_store,
                &interface_snapshot,
                &workspace_store,
                &workspace_snapshot,
            ),
            ImportApplyOutcome::Conflict
        );
        let _ = fs::remove_dir_all(root);
    }
}
