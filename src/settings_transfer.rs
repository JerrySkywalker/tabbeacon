//! Versioned, portable user-settings document primitives for G54.
//!
//! This module serializes only typed, user-owned configuration. It also owns
//! the typed import plan and the small, snapshot-guarded transaction that
//! applies that plan across the three user-local stores.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use atomic_write_file::AtomicWriteFile;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    interface_preferences::{
        HumanColor, InterfaceLanguage, InterfacePreferences,
        InterfacePreferencesConditionalOutcome, InterfacePreferencesSnapshot,
        InterfacePreferencesSnapshotSaveOutcome, InterfacePreferencesStore,
        InterfacePreferencesWriteReceipt,
    },
    presentation_policy::{CliTarget, PresentationOverride},
    private_journal::{
        ensure_private_journal_dir, seal_private_journal_file, verify_private_journal_file,
    },
    repo::{
        CanonicalRepositoryIdentity, RepositoryAlias, WorkspacePreferenceStore,
        WorkspacePreferences, WorkspacePreferencesConditionalOutcome, WorkspacePreferencesSnapshot,
        WorkspacePreferencesSnapshotSaveOutcome, WorkspacePreferencesWriteReceipt,
    },
    settings::{
        ActivityMode, ConditionalSaveOutcome, PresentationSettings, PresentationSettingsSnapshot,
        PresentationSettingsStore, PresentationSettingsWriteReceipt, PresentationTheme,
        ProviderBadgePolicy, SnapshotSaveOutcome, SpinnerPreset, TabColorMode, TitleMode,
    },
};

/// Stable schema identifier for portable user configuration exports.
pub const EXPORT_SCHEMA_V1: &str = "tabbeacon-export-v1";
/// Portable format carrying explicit per-provider presentation overrides.
pub const EXPORT_SCHEMA_V2: &str = "tabbeacon-export-v2";
/// Hard bound before JSON parsing so an import cannot become an unbounded log
/// or arbitrary system image.
pub const MAX_EXPORT_BYTES: usize = 1024 * 1024;

static EXPORT_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[cfg(test)]
fn interrupt_import_test_at(stage: &str) {
    if std::env::var("TABBEACON_TEST_IMPORT_ABORT_STAGE")
        .ok()
        .as_deref()
        == Some(stage)
    {
        std::process::exit(77);
    }
}
const IMPORT_JOURNAL_SCHEMA: &str = "tabbeacon-import-journal-v1";
const IMPORT_JOURNAL_FILE: &str = "import-transaction-v1.json";
const IMPORT_JOURNAL_LOCK: &str = "import-transaction-v1.lock";
const PRIVATE_IMPORT_JOURNAL_DIR: &str = "private-import-journal-v1";
const MAX_IMPORT_JOURNAL_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ImportFileDelta {
    before: Option<Vec<u8>>,
    after: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ImportJournalPhase {
    Prepared,
    HookMayHaveChanged,
    Applied,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ImportJournal {
    schema: String,
    presentation_path: PathBuf,
    interface_path: PathBuf,
    workspace_path: PathBuf,
    presentation: Option<ImportFileDelta>,
    interface: Option<ImportFileDelta>,
    workspace: Option<ImportFileDelta>,
    codex_title_before: Option<bool>,
    phase: ImportJournalPhase,
}

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
    provider_overrides: Vec<(CliTarget, PresentationOverride)>,
    interface: Option<InterfacePreferences>,
    workspace_preferences: Option<WorkspacePreferences>,
    portable_matches: usize,
    unmatched_entries: usize,
    conflicts: Vec<ImportPlanConflict>,
}

impl ImportPlan {
    /// Whether applying this plan would change who owns Codex terminal titles.
    /// Callers must reconcile the Codex Hook configuration before reporting an
    /// applied import; a portable file cannot grant that external authority.
    #[must_use]
    pub fn changes_codex_title_ownership(&self, snapshot: &PresentationSettingsSnapshot) -> bool {
        self.codex_title_ownership_after(snapshot)
            != snapshot
                .provider_override(CliTarget::Codex)
                .unwrap_or_default()
                .title
                .unwrap_or(snapshot.settings().title())
                .owns_tabbeacon_title()
    }

    /// Resolved Codex title owner after this portable plan is applied.
    #[must_use]
    pub fn codex_title_ownership_after(&self, snapshot: &PresentationSettingsSnapshot) -> bool {
        let before_global = snapshot.settings();
        let after_global = self.presentation.unwrap_or(before_global);
        let before_override = snapshot
            .provider_override(CliTarget::Codex)
            .unwrap_or_default();
        let after_override = self
            .provider_overrides
            .iter()
            .find_map(|(provider, replacement)| {
                (*provider == CliTarget::Codex).then_some(*replacement)
            })
            .unwrap_or(before_override);
        let after_title = after_override.title.unwrap_or(after_global.title());
        after_title.owns_tabbeacon_title()
    }

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
            || !self.provider_overrides.is_empty()
            || self.interface.is_some()
            || self.workspace_preferences.is_some()
    }

    /// Whether Presentation settings would change on Apply.
    #[must_use]
    pub const fn changes_presentation(&self) -> bool {
        self.presentation.is_some() || !self.provider_overrides.is_empty()
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
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    provider_presentation: BTreeMap<String, ProviderOverrideExport>,
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
    #[serde(default = "default_provider_badge")]
    provider_badge: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderOverrideExport {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tab_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    activity: Option<String>,
}

impl From<PresentationOverride> for ProviderOverrideExport {
    fn from(value: PresentationOverride) -> Self {
        Self {
            title: value.title.map(|value| value.as_str().to_owned()),
            tab_color: value.tab_color.map(|value| value.as_str().to_owned()),
            activity: value.activity.map(|value| value.as_str().to_owned()),
        }
    }
}

impl ProviderOverrideExport {
    fn settings(&self) -> Result<PresentationOverride, SettingsTransferError> {
        Ok(PresentationOverride {
            title: self
                .title
                .as_deref()
                .map(TitleMode::parse)
                .transpose_option()?,
            tab_color: self
                .tab_color
                .as_deref()
                .map(TabColorMode::parse)
                .transpose_option()?,
            activity: self
                .activity
                .as_deref()
                .map(ActivityMode::parse)
                .transpose_option()?,
        })
    }
}

trait OptionOptionExt<T> {
    fn transpose_option(self) -> Result<Option<T>, SettingsTransferError>;
}

impl<T> OptionOptionExt<T> for Option<Option<T>> {
    fn transpose_option(self) -> Result<Option<T>, SettingsTransferError> {
        match self {
            Some(Some(value)) => Ok(Some(value)),
            None => Ok(None),
            Some(None) => Err(SettingsTransferError::InvalidDocument),
        }
    }
}

fn default_provider_badge() -> String {
    ProviderBadgePolicy::Auto.as_str().to_owned()
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
            provider_presentation: BTreeMap::new(),
            interface: interface.map(InterfaceExport::from),
            workspace_aliases,
            omitted_device_local_workspace_aliases,
        }
    }

    /// Adds only user-owned provider preferences. Trust and integration state
    /// are deliberately absent from the portable format.
    #[must_use]
    pub fn with_provider_overrides(
        mut self,
        overrides: BTreeMap<CliTarget, PresentationOverride>,
    ) -> Self {
        self.provider_presentation = overrides
            .into_iter()
            .filter(|(_, value)| *value != PresentationOverride::default())
            .map(|(provider, value)| (provider.as_str().to_owned(), value.into()))
            .collect();
        if !self.provider_presentation.is_empty() {
            EXPORT_SCHEMA_V2.clone_into(&mut self.schema);
        }
        self
    }

    /// Exact portable schema of this document.
    #[must_use]
    pub fn schema(&self) -> &str {
        &self.schema
    }

    /// Number of provider-specific preferences, never integration/trust state.
    #[must_use]
    pub fn provider_override_count(&self) -> usize {
        self.provider_presentation.len()
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
        if document.schema != EXPORT_SCHEMA_V1 && document.schema != EXPORT_SCHEMA_V2 {
            return Err(SettingsTransferError::InvalidDocument);
        }
        if document.schema == EXPORT_SCHEMA_V1 && !document.provider_presentation.is_empty() {
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
        let mut provider_overrides = Vec::new();
        for (provider_name, portable) in &self.provider_presentation {
            let provider =
                CliTarget::parse(provider_name).ok_or(SettingsTransferError::InvalidDocument)?;
            let replacement = portable.settings()?;
            let current = presentation_snapshot
                .provider_override(provider)
                .map_err(|_| SettingsTransferError::InvalidDocument)?;
            if replacement != current {
                provider_overrides.push((provider, replacement));
            }
        }
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
            provider_overrides,
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
        for (provider, override_for_cli) in &self.provider_presentation {
            if CliTarget::parse(provider).is_none() {
                return Err(SettingsTransferError::InvalidDocument);
            }
            let _ = override_for_cli.settings()?;
        }
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
    // Library callers without a Hook coordinator cannot silently transfer
    // title ownership by writing preferences alone.
    if plan.changes_codex_title_ownership(presentation_snapshot) {
        return ImportApplyOutcome::Conflict;
    }
    apply_import_plan_inner(
        plan,
        presentation_store,
        presentation_snapshot,
        interface_store,
        interface_snapshot,
        workspace_store,
        workspace_snapshot,
        None,
    )
}

/// Applies a title-changing import with a caller-owned Codex reconciliation.
/// The callback must itself enforce Hook ownership and trust boundaries.
#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn apply_import_plan_with_reconciliation(
    plan: &ImportPlan,
    presentation_store: &PresentationSettingsStore,
    presentation_snapshot: &PresentationSettingsSnapshot,
    interface_store: &InterfacePreferencesStore,
    interface_snapshot: &InterfacePreferencesSnapshot,
    workspace_store: &WorkspacePreferenceStore,
    workspace_snapshot: &WorkspacePreferencesSnapshot,
    mut reconcile: impl FnMut(bool) -> Result<(), String>,
) -> ImportApplyOutcome {
    apply_import_plan_inner(
        plan,
        presentation_store,
        presentation_snapshot,
        interface_store,
        interface_snapshot,
        workspace_store,
        workspace_snapshot,
        Some(&mut reconcile),
    )
}

fn import_journal_path(store: &PresentationSettingsStore) -> Option<PathBuf> {
    Some(
        store
            .path()
            .parent()?
            .join(PRIVATE_IMPORT_JOURNAL_DIR)
            .join(IMPORT_JOURNAL_FILE),
    )
}

fn import_transaction_lock(store: &PresentationSettingsStore) -> Option<File> {
    let directory = store.path().parent()?;
    reject_import_reparse(directory).ok()?;
    fs::create_dir_all(directory).ok()?;
    let path = directory.join(IMPORT_JOURNAL_LOCK);
    reject_import_reparse(&path).ok()?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)
        .ok()?;
    file.lock().ok()?;
    Some(file)
}

fn reject_import_reparse(path: &Path) -> Result<(), ()> {
    for candidate in path.ancestors() {
        match fs::symlink_metadata(candidate) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err(());
                }
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    if metadata.file_attributes() & 0x400 != 0 {
                        return Err(());
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(()),
        }
    }
    Ok(())
}

fn load_import_journal(path: &Path) -> Result<Option<ImportJournal>, ()> {
    reject_import_reparse(path)?;
    let legacy_path = path
        .parent()
        .and_then(Path::parent)
        .ok_or(())?
        .join(IMPORT_JOURNAL_FILE);
    reject_import_reparse(&legacy_path)?;
    if legacy_path.exists() {
        // Earlier development candidates could leave raw bytes beside the
        // settings file. Never silently bypass such a pending transaction.
        return Err(());
    }
    if path.exists() {
        ensure_private_journal_dir(path.parent().ok_or(())?).map_err(|_| ())?;
        verify_private_journal_file(path).map_err(|_| ())?;
    }
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(()),
    };
    let mut bytes = Vec::new();
    file.take((MAX_IMPORT_JOURNAL_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| ())?;
    if bytes.len() > MAX_IMPORT_JOURNAL_BYTES {
        return Err(());
    }
    let journal: ImportJournal = serde_json::from_slice(&bytes).map_err(|_| ())?;
    if journal.schema != IMPORT_JOURNAL_SCHEMA {
        return Err(());
    }
    Ok(Some(journal))
}

fn write_import_journal(path: &Path, journal: &ImportJournal, new: bool) -> Result<(), ()> {
    reject_import_reparse(path)?;
    ensure_private_journal_dir(path.parent().ok_or(())?).map_err(|_| ())?;
    let bytes = serde_json::to_vec(journal).map_err(|_| ())?;
    if bytes.len() > MAX_IMPORT_JOURNAL_BYTES {
        return Err(());
    }
    if new {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path)
            .map_err(|_| ())?;
        seal_private_journal_file(path).map_err(|_| ())?;
        file.write_all(&bytes).map_err(|_| ())?;
        file.sync_all().map_err(|_| ())?;
    } else {
        verify_private_journal_file(path).map_err(|_| ())?;
        let mut file = AtomicWriteFile::options().open(path).map_err(|_| ())?;
        file.write_all(&bytes).map_err(|_| ())?;
        file.flush().map_err(|_| ())?;
        file.commit().map_err(|_| ())?;
        seal_private_journal_file(path).map_err(|_| ())?;
    }
    Ok(())
}

fn remove_exact_import_journal(path: &Path, journal: &ImportJournal) -> Result<(), ()> {
    let current = load_import_journal(path)?.ok_or(())?;
    if serde_json::to_vec(&current).map_err(|_| ())?
        != serde_json::to_vec(journal).map_err(|_| ())?
    {
        return Err(());
    }
    fs::remove_file(path).map_err(|_| ())
}

fn import_delta_current(current: Option<&[u8]>, delta: &ImportFileDelta) -> bool {
    current == delta.before.as_deref() || current == Some(delta.after.as_slice())
}

fn import_journal_paths_match(
    journal: &ImportJournal,
    presentation: &PresentationSettingsStore,
    interface: &InterfacePreferencesStore,
    workspace: &WorkspacePreferenceStore,
) -> bool {
    journal.presentation_path == presentation.path()
        && journal.interface_path == interface.path()
        && journal.workspace_path == workspace.path()
}

/// Resumes an interrupted CLI import by restoring exact owned preference
/// bytes and reconciling the prior Codex title owner. External drift stops
/// recovery before any write; uncertain Hook ownership leaves the journal.
#[must_use]
pub fn recover_pending_import(
    presentation: &PresentationSettingsStore,
    interface: &InterfacePreferencesStore,
    workspace: &WorkspacePreferenceStore,
    mut reconcile: impl FnMut(bool) -> Result<(), String>,
) -> Option<ImportApplyOutcome> {
    let path = import_journal_path(presentation)?;
    match load_import_journal(&path) {
        Ok(None) => return None,
        Err(()) => return Some(ImportApplyOutcome::PartialState),
        Ok(Some(_)) => {}
    }
    let Some(_lock) = import_transaction_lock(presentation) else {
        return Some(ImportApplyOutcome::PartialState);
    };
    let journal = match load_import_journal(&path) {
        Ok(Some(journal)) => journal,
        Ok(None) => return None,
        Err(()) => return Some(ImportApplyOutcome::PartialState),
    };
    if !import_journal_paths_match(&journal, presentation, interface, workspace) {
        return Some(ImportApplyOutcome::PartialState);
    }
    let (Ok(p), Ok(i), Ok(w)) = (
        presentation.snapshot_read_only(),
        interface.snapshot_read_only(),
        workspace.snapshot_read_only(),
    ) else {
        return Some(ImportApplyOutcome::PartialState);
    };
    if journal
        .presentation
        .as_ref()
        .is_some_and(|delta| !import_delta_current(p.recovery_contents(), delta))
        || journal
            .interface
            .as_ref()
            .is_some_and(|delta| !import_delta_current(i.recovery_contents(), delta))
        || journal
            .workspace
            .as_ref()
            .is_some_and(|delta| !import_delta_current(w.recovery_contents(), delta))
    {
        return Some(ImportApplyOutcome::PartialState);
    }
    if journal.phase == ImportJournalPhase::Applied {
        let all_applied = journal
            .presentation
            .as_ref()
            .is_none_or(|delta| p.recovery_contents() == Some(delta.after.as_slice()))
            && journal
                .interface
                .as_ref()
                .is_none_or(|delta| i.recovery_contents() == Some(delta.after.as_slice()))
            && journal
                .workspace
                .as_ref()
                .is_none_or(|delta| w.recovery_contents() == Some(delta.after.as_slice()));
        return Some(
            if all_applied && remove_exact_import_journal(&path, &journal).is_ok() {
                ImportApplyOutcome::Applied
            } else {
                ImportApplyOutcome::PartialState
            },
        );
    }
    if journal.phase == ImportJournalPhase::HookMayHaveChanged
        && let Some(before) = journal.codex_title_before
        && reconcile(before).is_err()
    {
        return Some(ImportApplyOutcome::PartialState);
    }
    let restored = journal.workspace.as_ref().is_none_or(|delta| {
        workspace
            .recover_import_bytes_if_unchanged(&delta.after, delta.before.as_deref())
            .is_ok_and(|restored| restored)
    }) && journal.interface.as_ref().is_none_or(|delta| {
        interface
            .recover_import_bytes_if_unchanged(&delta.after, delta.before.as_deref())
            .is_ok_and(|restored| restored)
    }) && journal.presentation.as_ref().is_none_or(|delta| {
        presentation
            .recover_import_bytes_if_unchanged(&delta.after, delta.before.as_deref())
            .is_ok_and(|restored| restored)
    });
    Some(
        if restored && remove_exact_import_journal(&path, &journal).is_ok() {
            ImportApplyOutcome::RolledBack
        } else {
            ImportApplyOutcome::PartialState
        },
    )
}

/// Writes a bounded recovery journal before touching any store, then applies
/// the existing snapshot-guarded import and closes the journal only after all
/// three stores and the external title callback have settled.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_lines)] // The journal's ordered write boundary must stay inspectable.
#[must_use]
pub fn apply_import_plan_durable(
    plan: &ImportPlan,
    presentation: &PresentationSettingsStore,
    presentation_snapshot: &PresentationSettingsSnapshot,
    interface: &InterfacePreferencesStore,
    interface_snapshot: &InterfacePreferencesSnapshot,
    workspace: &WorkspacePreferenceStore,
    workspace_snapshot: &WorkspacePreferencesSnapshot,
    mut reconcile: impl FnMut(bool) -> Result<(), String>,
) -> ImportApplyOutcome {
    if !plan.is_applicable() {
        return ImportApplyOutcome::Conflict;
    }
    let Some(_lock) = import_transaction_lock(presentation) else {
        return ImportApplyOutcome::PartialState;
    };
    let Some(path) = import_journal_path(presentation) else {
        return ImportApplyOutcome::PartialState;
    };
    if !matches!(load_import_journal(&path), Ok(None)) {
        return ImportApplyOutcome::PartialState;
    }
    let presentation_delta = if plan.presentation.is_some() || !plan.provider_overrides.is_empty() {
        let Ok(after) = PresentationSettingsStore::render_portable_snapshot(
            presentation_snapshot,
            plan.presentation,
            &plan.provider_overrides,
        ) else {
            return ImportApplyOutcome::Conflict;
        };
        Some(ImportFileDelta {
            before: presentation_snapshot
                .recovery_contents()
                .map(ToOwned::to_owned),
            after,
        })
    } else {
        None
    };
    let interface_delta = if let Some(replacement) = plan.interface {
        let Ok(after) =
            InterfacePreferencesStore::render_replacement_bytes(interface_snapshot, replacement)
        else {
            return ImportApplyOutcome::Conflict;
        };
        Some(ImportFileDelta {
            before: interface_snapshot
                .recovery_contents()
                .map(ToOwned::to_owned),
            after,
        })
    } else {
        None
    };
    let workspace_delta = if let Some(replacement) = plan.workspace_preferences.clone() {
        let Ok(after) = WorkspacePreferenceStore::render_replacement_bytes(replacement) else {
            return ImportApplyOutcome::Conflict;
        };
        Some(ImportFileDelta {
            before: workspace_snapshot
                .recovery_contents()
                .map(ToOwned::to_owned),
            after,
        })
    } else {
        None
    };
    let title_changes = plan.changes_codex_title_ownership(presentation_snapshot);
    let mut journal = ImportJournal {
        schema: IMPORT_JOURNAL_SCHEMA.to_owned(),
        presentation_path: presentation.path().to_owned(),
        interface_path: interface.path().to_owned(),
        workspace_path: workspace.path().to_owned(),
        presentation: presentation_delta,
        interface: interface_delta,
        workspace: workspace_delta,
        codex_title_before: title_changes
            .then(|| !plan.codex_title_ownership_after(presentation_snapshot)),
        phase: ImportJournalPhase::Prepared,
    };
    if write_import_journal(&path, &journal, true).is_err() {
        return ImportApplyOutcome::PartialState;
    }
    #[cfg(test)]
    interrupt_import_test_at("journal_prepared");
    let outcome = {
        let mut journaled_reconcile = |owns_title| {
            if journal.phase == ImportJournalPhase::Prepared {
                let mut pending = journal.clone();
                pending.phase = ImportJournalPhase::HookMayHaveChanged;
                write_import_journal(&path, &pending, false)
                    .map_err(|()| "import recovery journal cannot mark Hook boundary".to_owned())?;
                journal = pending;
            }
            reconcile(owns_title)
        };
        apply_import_plan_inner(
            plan,
            presentation,
            presentation_snapshot,
            interface,
            interface_snapshot,
            workspace,
            workspace_snapshot,
            Some(&mut journaled_reconcile),
        )
    };
    match outcome {
        ImportApplyOutcome::Applied => {
            journal.phase = ImportJournalPhase::Applied;
            if write_import_journal(&path, &journal, false).is_err() {
                return ImportApplyOutcome::PartialState;
            }
            #[cfg(test)]
            interrupt_import_test_at("phase_applied");
            if remove_exact_import_journal(&path, &journal).is_ok() {
                ImportApplyOutcome::Applied
            } else {
                ImportApplyOutcome::PartialState
            }
        }
        ImportApplyOutcome::Conflict | ImportApplyOutcome::RolledBack => {
            if remove_exact_import_journal(&path, &journal).is_ok() {
                outcome
            } else {
                ImportApplyOutcome::PartialState
            }
        }
        ImportApplyOutcome::PartialState => ImportApplyOutcome::PartialState,
    }
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_lines)]
fn apply_import_plan_inner(
    plan: &ImportPlan,
    presentation_store: &PresentationSettingsStore,
    presentation_snapshot: &PresentationSettingsSnapshot,
    interface_store: &InterfacePreferencesStore,
    interface_snapshot: &InterfacePreferencesSnapshot,
    workspace_store: &WorkspacePreferenceStore,
    workspace_snapshot: &WorkspacePreferencesSnapshot,
    mut reconcile: Option<&mut dyn FnMut(bool) -> Result<(), String>>,
) -> ImportApplyOutcome {
    if !plan.is_applicable() {
        return ImportApplyOutcome::Conflict;
    }
    let title_changes = plan.changes_codex_title_ownership(presentation_snapshot);
    if title_changes && reconcile.is_none() {
        return ImportApplyOutcome::Conflict;
    }

    let mut presentation_receipt = None;
    let mut interface_receipt = None;
    let mut workspace_receipt = None;

    if plan.presentation.is_some() || !plan.provider_overrides.is_empty() {
        match presentation_store.save_portable_snapshot_if_unchanged(
            presentation_snapshot,
            plan.presentation,
            &plan.provider_overrides,
        ) {
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
        #[cfg(test)]
        interrupt_import_test_at("presentation");
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
        #[cfg(test)]
        interrupt_import_test_at("interface");
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
        #[cfg(test)]
        interrupt_import_test_at("workspace");
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

    if title_changes {
        let previous_owner = !plan.codex_title_ownership_after(presentation_snapshot);
        let next_owner = !previous_owner;
        let coordinator = reconcile
            .as_mut()
            .expect("title change requires coordinator");
        #[cfg(test)]
        interrupt_import_test_at("hook_before");
        if coordinator(next_owner).is_err() {
            // A failed external write might already have reached one owned
            // Codex file. Attempt an ownership-checked compensation, but do
            // not call the combined operation rolled back without a durable
            // external receipt.
            let _ = coordinator(previous_owner);
            let _ = rollback_import(
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
            return ImportApplyOutcome::PartialState;
        }
        #[cfg(test)]
        interrupt_import_test_at("hook_after");
        // A callback can take long enough for another writer to change one
        // of the three preference stores. Never report Applied on stale bytes.
        let receipts_still_current = presentation_receipt.as_ref().is_none_or(|receipt| {
            matches!(
                presentation_store.write_receipt_is_current(receipt),
                Ok(true)
            )
        }) && interface_receipt.as_ref().is_none_or(|receipt| {
            matches!(interface_store.write_receipt_is_current(receipt), Ok(true))
        }) && workspace_receipt.as_ref().is_none_or(|receipt| {
            matches!(workspace_store.write_receipt_is_current(receipt), Ok(true))
        });
        if !receipts_still_current {
            // Another writer may now own the preference. Do not change its
            // Codex title setting by guessing its intended ownership.
            return ImportApplyOutcome::PartialState;
        }
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
            provider_badge: value.provider_badge().as_str().to_owned(),
        }
    }
}

impl PresentationExport {
    fn settings(&self) -> Result<PresentationSettings, SettingsTransferError> {
        Ok(PresentationSettings::new_with_provider_badge(
            TitleMode::parse(&self.title).ok_or(SettingsTransferError::InvalidDocument)?,
            TabColorMode::parse(&self.tab_color).ok_or(SettingsTransferError::InvalidDocument)?,
            ActivityMode::parse(&self.activity).ok_or(SettingsTransferError::InvalidDocument)?,
            SpinnerPreset::parse(&self.spinner).ok_or(SettingsTransferError::InvalidDocument)?,
            PresentationTheme::parse(&self.theme).ok_or(SettingsTransferError::InvalidDocument)?,
            ProviderBadgePolicy::parse(&self.provider_badge)
                .ok_or(SettingsTransferError::InvalidDocument)?,
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
        env, fs,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::{
        interface_preferences::{
            HumanColor, InterfaceLanguage, InterfacePreferences, InterfacePreferencesStore,
        },
        presentation_policy::{CliTarget, PresentationMode, PresentationOverride},
        repo::{
            CanonicalRepositoryIdentity, RepositoryAlias, WorkspacePreferenceStore,
            WorkspacePreferences,
        },
        settings::{
            ActivityMode, PresentationSettings, PresentationSettingsStore, PresentationTheme,
            ProviderBadgePolicy, SpinnerPreset, TabColorMode, TitleMode,
        },
    };

    use super::{
        EXPORT_SCHEMA_V1, EXPORT_SCHEMA_V2, ImportApplyOutcome, ImportPlan, ImportPlanConflict,
        MAX_EXPORT_BYTES, SettingsExportV1, SettingsTransferError, apply_import_plan,
        apply_import_plan_durable, apply_import_plan_with_reconciliation, portable_workspace_key,
        recover_pending_import,
    };

    fn temporary_root(name: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock is after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("tabbeacon-settings-transfer-{name}-{unique}"))
    }

    fn durable_import_fixture(
        root: &std::path::Path,
    ) -> (
        PresentationSettingsStore,
        InterfacePreferencesStore,
        WorkspacePreferenceStore,
        ImportPlan,
    ) {
        let presentation = PresentationSettingsStore::new(root.join("config.toml"));
        let interface = InterfacePreferencesStore::new(root.join("interface.toml"));
        let workspace = WorkspacePreferenceStore::new(root.join("preferences.json"));
        let identity = CanonicalRepositoryIdentity::new("remote:example/durable-import").unwrap();
        let plan = ImportPlan {
            presentation: Some(
                PresentationSettings::default().with_theme(PresentationTheme::Classic),
            ),
            provider_overrides: vec![(
                CliTarget::Codex,
                PresentationOverride::default().with_mode(PresentationMode::PreserveNative),
            )],
            interface: Some(InterfacePreferences::new(
                InterfaceLanguage::ZhCn,
                HumanColor::Never,
                true,
            )),
            workspace_preferences: Some(
                WorkspacePreferences::default()
                    .with_override(identity, RepositoryAlias::new("DURABLE").unwrap()),
            ),
            portable_matches: 1,
            unmatched_entries: 0,
            conflicts: Vec::new(),
        };
        (presentation, interface, workspace, plan)
    }

    #[test]
    fn interrupted_import_child_fixture() {
        let Ok(root) = env::var("TABBEACON_TEST_IMPORT_CHILD_ROOT") else {
            return;
        };
        let (presentation, interface, workspace, plan) = durable_import_fixture(root.as_ref());
        let p = presentation.snapshot_read_only().unwrap();
        let i = interface.snapshot_read_only().unwrap();
        let w = workspace.snapshot_read_only().unwrap();
        let _ = apply_import_plan_durable(
            &plan,
            &presentation,
            &p,
            &interface,
            &i,
            &workspace,
            &w,
            |_| Ok(()),
        );
        panic!("injected process interruption did not occur");
    }

    #[test]
    fn durable_import_recovers_after_each_process_write_boundary() {
        for stage in [
            "journal_prepared",
            "presentation",
            "interface",
            "workspace",
            "hook_before",
            "hook_after",
        ] {
            let root = tempfile::tempdir().unwrap();
            let output = Command::new(env::current_exe().unwrap())
                .args([
                    "--exact",
                    "settings_transfer::tests::interrupted_import_child_fixture",
                ])
                .env("TABBEACON_TEST_IMPORT_CHILD_ROOT", root.path())
                .env("TABBEACON_TEST_IMPORT_ABORT_STAGE", stage)
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(77), "stage={stage}");
            let journal = root
                .path()
                .join("private-import-journal-v1/import-transaction-v1.json");
            crate::private_journal::ensure_private_journal_dir(journal.parent().unwrap()).unwrap();
            crate::private_journal::verify_private_journal_file(&journal).unwrap();
            let (presentation, interface, workspace, _) = durable_import_fixture(root.path());
            let mut reconciled = Vec::new();
            assert_eq!(
                recover_pending_import(&presentation, &interface, &workspace, |owner| {
                    reconciled.push(owner);
                    Ok(())
                }),
                Some(ImportApplyOutcome::RolledBack),
                "stage={stage}"
            );
            if stage == "hook_after" {
                assert_eq!(reconciled, [true], "stage={stage}");
            } else {
                assert!(reconciled.is_empty(), "stage={stage}");
            }
            assert!(!presentation.path().exists(), "stage={stage}");
            assert!(!interface.path().exists(), "stage={stage}");
            assert!(!workspace.path().exists(), "stage={stage}");
            assert_eq!(
                recover_pending_import(&presentation, &interface, &workspace, |_| Ok(())),
                None,
                "recovery is idempotent at {stage}"
            );
        }
    }

    #[test]
    fn durable_import_recognizes_committed_journal_after_process_exit() {
        let root = tempfile::tempdir().unwrap();
        let output = Command::new(env::current_exe().unwrap())
            .args([
                "--exact",
                "settings_transfer::tests::interrupted_import_child_fixture",
            ])
            .env("TABBEACON_TEST_IMPORT_CHILD_ROOT", root.path())
            .env("TABBEACON_TEST_IMPORT_ABORT_STAGE", "phase_applied")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(77));
        let (presentation, interface, workspace, _) = durable_import_fixture(root.path());
        let mut called = false;
        assert_eq!(
            recover_pending_import(&presentation, &interface, &workspace, |_| {
                called = true;
                Ok(())
            }),
            Some(ImportApplyOutcome::Applied)
        );
        assert!(!called, "committed recovery never reverts Hook ownership");
        assert!(presentation.path().exists());
        assert!(interface.path().exists());
        assert!(workspace.path().exists());
        assert_eq!(
            recover_pending_import(&presentation, &interface, &workspace, |_| Ok(())),
            None
        );
    }

    #[test]
    fn durable_import_refuses_external_drift_after_process_interruption() {
        let root = tempfile::tempdir().unwrap();
        let output = Command::new(env::current_exe().unwrap())
            .args([
                "--exact",
                "settings_transfer::tests::interrupted_import_child_fixture",
            ])
            .env("TABBEACON_TEST_IMPORT_CHILD_ROOT", root.path())
            .env("TABBEACON_TEST_IMPORT_ABORT_STAGE", "workspace")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(77));
        let (presentation, interface, workspace, _) = durable_import_fixture(root.path());
        fs::write(interface.path(), "[foreign]\nkeep = true\n").unwrap();
        let before_presentation = fs::read(presentation.path()).unwrap();
        let before_workspace = fs::read(workspace.path()).unwrap();
        let mut called = false;
        assert_eq!(
            recover_pending_import(&presentation, &interface, &workspace, |_| {
                called = true;
                Ok(())
            }),
            Some(ImportApplyOutcome::PartialState)
        );
        assert!(!called);
        assert_eq!(fs::read(presentation.path()).unwrap(), before_presentation);
        assert_eq!(fs::read(workspace.path()).unwrap(), before_workspace);
        assert_eq!(
            fs::read_to_string(interface.path()).unwrap(),
            "[foreign]\nkeep = true\n"
        );
    }

    #[test]
    fn durable_import_commits_and_removes_owned_journal() {
        let root = tempfile::tempdir().unwrap();
        let (presentation, interface, workspace, plan) = durable_import_fixture(root.path());
        let p = presentation.snapshot_read_only().unwrap();
        let i = interface.snapshot_read_only().unwrap();
        let w = workspace.snapshot_read_only().unwrap();
        let mut calls = Vec::new();
        assert_eq!(
            apply_import_plan_durable(
                &plan,
                &presentation,
                &p,
                &interface,
                &i,
                &workspace,
                &w,
                |owner| {
                    calls.push(owner);
                    Ok(())
                },
            ),
            ImportApplyOutcome::Applied
        );
        assert_eq!(calls, [false]);
        assert!(presentation.path().exists());
        assert!(interface.path().exists());
        assert!(workspace.path().exists());
        assert_eq!(
            recover_pending_import(&presentation, &interface, &workspace, |_| Ok(())),
            None
        );
        assert!(
            !root
                .path()
                .join("private-import-journal-v1/import-transaction-v1.json")
                .exists()
        );
    }

    #[test]
    fn absent_recovery_journal_is_read_only() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("fresh");
        let (presentation, interface, workspace, _) = durable_import_fixture(&directory);
        assert_eq!(
            recover_pending_import(&presentation, &interface, &workspace, |_| Ok(())),
            None
        );
        assert!(!directory.exists());
    }

    #[test]
    fn legacy_unprotected_journal_blocks_new_import_without_reading_it() {
        let root = tempfile::tempdir().unwrap();
        let (presentation, interface, workspace, plan) = durable_import_fixture(root.path());
        let legacy = root.path().join("import-transaction-v1.json");
        fs::write(&legacy, b"unprotected legacy journal bytes").unwrap();
        assert_eq!(
            recover_pending_import(&presentation, &interface, &workspace, |_| Ok(())),
            Some(ImportApplyOutcome::PartialState)
        );
        assert_eq!(
            apply_import_plan_durable(
                &plan,
                &presentation,
                &presentation.snapshot_read_only().unwrap(),
                &interface,
                &interface.snapshot_read_only().unwrap(),
                &workspace,
                &workspace.snapshot_read_only().unwrap(),
                |_| Ok(()),
            ),
            ImportApplyOutcome::PartialState
        );
        assert_eq!(
            fs::read(legacy).unwrap(),
            b"unprotected legacy journal bytes"
        );
    }

    #[test]
    fn durable_import_retries_uncertain_hook_boundary_only_with_owned_callback() {
        let root = tempfile::tempdir().unwrap();
        let (presentation, interface, workspace, plan) = durable_import_fixture(root.path());
        let p = presentation.snapshot_read_only().unwrap();
        let i = interface.snapshot_read_only().unwrap();
        let w = workspace.snapshot_read_only().unwrap();
        let mut attempts = Vec::new();
        assert_eq!(
            apply_import_plan_durable(
                &plan,
                &presentation,
                &p,
                &interface,
                &i,
                &workspace,
                &w,
                |owner| {
                    attempts.push(owner);
                    if owner {
                        Ok(())
                    } else {
                        Err("isolated Hook uncertainty".into())
                    }
                },
            ),
            ImportApplyOutcome::PartialState
        );
        assert_eq!(attempts, [false, true]);
        let mut recovery = Vec::new();
        assert_eq!(
            recover_pending_import(&presentation, &interface, &workspace, |owner| {
                recovery.push(owner);
                Ok(())
            }),
            Some(ImportApplyOutcome::RolledBack)
        );
        assert_eq!(recovery, [true]);
        assert!(!presentation.path().exists());
        assert!(!interface.path().exists());
        assert!(!workspace.path().exists());
    }

    #[test]
    fn canonical_export_round_trips_typed_preferences_without_private_identity() {
        let git = CanonicalRepositoryIdentity::new("remote:example/tabbeacon").unwrap();
        let directory = CanonicalRepositoryIdentity::new("dir-v1:private-local-path-hash").unwrap();
        let preferences = WorkspacePreferences::default()
            .with_override(git.clone(), RepositoryAlias::new("TB").unwrap())
            .with_override(directory, RepositoryAlias::new("LOCAL").unwrap());
        let settings = PresentationSettings::new_with_provider_badge(
            TitleMode::Native,
            TabColorMode::Off,
            ActivityMode::Both,
            SpinnerPreset::Braille,
            PresentationTheme::Classic,
            ProviderBadgePolicy::Always,
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
        assert!(text.contains("provider_badge"));
        assert!(!text.contains(git.as_str()));
        assert!(!text.contains("dir-v1:"));
    }

    #[test]
    fn portable_v2_round_trips_provider_preferences_without_trust_or_machine_state() {
        let root = tempfile::tempdir().unwrap();
        let presentation_store = PresentationSettingsStore::new(root.path().join("config.toml"));
        let interface_store = InterfacePreferencesStore::new(root.path().join("interface.toml"));
        let workspace_store = WorkspacePreferenceStore::new(root.path().join("preferences.json"));
        let cursor = PresentationOverride::default().with_mode(PresentationMode::ColorOnly);
        let document = SettingsExportV1::new(
            Some(PresentationSettings::default()),
            None,
            &WorkspacePreferences::default(),
        )
        .with_provider_overrides(BTreeMap::from([(CliTarget::Cursor, cursor)]));
        let bytes = document.to_canonical_json().unwrap();
        let parsed = SettingsExportV1::parse(&bytes).unwrap();
        assert_eq!(parsed.schema(), EXPORT_SCHEMA_V2);
        assert_eq!(parsed.provider_override_count(), 1);
        let portable = String::from_utf8(bytes).unwrap();
        assert!(!portable.contains("trusted_hash"));
        assert!(!portable.contains("WT_SESSION"));
        let presentation_snapshot = presentation_store.snapshot_read_only().unwrap();
        let interface_snapshot = interface_store.snapshot_read_only().unwrap();
        let workspace_snapshot = workspace_store.snapshot_read_only().unwrap();
        let plan = parsed
            .import_plan(
                &presentation_snapshot,
                &interface_snapshot,
                &workspace_snapshot,
                &BTreeSet::new(),
                &BTreeMap::new(),
            )
            .unwrap();
        assert!(plan.changes_presentation());
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
        assert_eq!(
            presentation_store
                .load_provider_override_read_only(CliTarget::Cursor)
                .unwrap(),
            cursor
        );
        assert_eq!(
            presentation_store
                .load_provider_override_read_only(CliTarget::Codex)
                .unwrap(),
            PresentationOverride::default()
        );
    }

    #[test]
    fn portable_v2_provider_write_rolls_back_after_interface_drift() {
        let root = tempfile::tempdir().unwrap();
        let presentation_store = PresentationSettingsStore::new(root.path().join("config.toml"));
        let interface_store = InterfacePreferencesStore::new(root.path().join("interface.toml"));
        let workspace_store = WorkspacePreferenceStore::new(root.path().join("preferences.json"));
        let document = SettingsExportV1::new(
            None,
            Some(InterfacePreferences::new(
                InterfaceLanguage::ZhCn,
                HumanColor::Never,
                true,
            )),
            &WorkspacePreferences::default(),
        )
        .with_provider_overrides(BTreeMap::from([(
            CliTarget::Cursor,
            PresentationOverride::default().with_mode(PresentationMode::ColorOnly),
        )]));
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
    }

    #[test]
    fn import_identifies_codex_title_ownership_change_before_writes() {
        let root = tempfile::tempdir().unwrap();
        let presentation_store = PresentationSettingsStore::new(root.path().join("config.toml"));
        let interface_store = InterfacePreferencesStore::new(root.path().join("interface.toml"));
        let workspace_store = WorkspacePreferenceStore::new(root.path().join("preferences.json"));
        let presentation_snapshot = presentation_store.snapshot_read_only().unwrap();
        let interface_snapshot = interface_store.snapshot_read_only().unwrap();
        let workspace_snapshot = workspace_store.snapshot_read_only().unwrap();
        let document = SettingsExportV1::new(None, None, &WorkspacePreferences::default())
            .with_provider_overrides(BTreeMap::from([(
                CliTarget::Codex,
                PresentationOverride::default().with_mode(PresentationMode::PreserveNative),
            )]));
        let plan = document
            .import_plan(
                &presentation_snapshot,
                &interface_snapshot,
                &workspace_snapshot,
                &BTreeSet::new(),
                &BTreeMap::new(),
            )
            .unwrap();
        assert!(plan.changes_codex_title_ownership(&presentation_snapshot));
        assert!(!presentation_store.path().exists());
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
            ImportApplyOutcome::Conflict,
            "uncoordinated library use cannot change Codex title ownership"
        );
        assert!(!presentation_store.path().exists());
        let mut reconciled = Vec::new();
        assert_eq!(
            apply_import_plan_with_reconciliation(
                &plan,
                &presentation_store,
                &presentation_snapshot,
                &interface_store,
                &interface_snapshot,
                &workspace_store,
                &workspace_snapshot,
                |owned| {
                    reconciled.push(owned);
                    Ok(())
                },
            ),
            ImportApplyOutcome::Applied
        );
        assert_eq!(reconciled, [false]);
        assert_eq!(
            presentation_store
                .load_provider_override_read_only(CliTarget::Codex)
                .unwrap()
                .title,
            Some(TitleMode::Native)
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One isolated plan exercises prewrite, callback, and postwrite drift.
    fn title_changing_import_rejects_drift_and_reports_external_failure_as_partial() {
        let root = tempfile::tempdir().unwrap();
        let presentation_store = PresentationSettingsStore::new(root.path().join("config.toml"));
        let interface_store = InterfacePreferencesStore::new(root.path().join("interface.toml"));
        let workspace_store = WorkspacePreferenceStore::new(root.path().join("preferences.json"));
        let presentation_snapshot = presentation_store.snapshot_read_only().unwrap();
        let interface_snapshot = interface_store.snapshot_read_only().unwrap();
        let workspace_snapshot = workspace_store.snapshot_read_only().unwrap();
        let document = SettingsExportV1::new(None, None, &WorkspacePreferences::default())
            .with_provider_overrides(BTreeMap::from([(
                CliTarget::Codex,
                PresentationOverride::default().with_mode(PresentationMode::PreserveNative),
            )]));
        let plan = document
            .import_plan(
                &presentation_snapshot,
                &interface_snapshot,
                &workspace_snapshot,
                &BTreeSet::new(),
                &BTreeMap::new(),
            )
            .unwrap();
        presentation_store
            .save(PresentationSettings::default().with_theme(PresentationTheme::Classic))
            .unwrap();
        let mut called = false;
        assert_eq!(
            apply_import_plan_with_reconciliation(
                &plan,
                &presentation_store,
                &presentation_snapshot,
                &interface_store,
                &interface_snapshot,
                &workspace_store,
                &workspace_snapshot,
                |_| {
                    called = true;
                    Ok(())
                },
            ),
            ImportApplyOutcome::Conflict
        );
        assert!(!called);
        assert_eq!(
            presentation_store.load().unwrap().theme(),
            PresentationTheme::Classic
        );

        let current = presentation_store.snapshot_read_only().unwrap();
        let plan = document
            .import_plan(
                &current,
                &interface_snapshot,
                &workspace_snapshot,
                &BTreeSet::new(),
                &BTreeMap::new(),
            )
            .unwrap();
        let mut attempts = Vec::new();
        assert_eq!(
            apply_import_plan_with_reconciliation(
                &plan,
                &presentation_store,
                &current,
                &interface_store,
                &interface_snapshot,
                &workspace_store,
                &workspace_snapshot,
                |owned| {
                    attempts.push(owned);
                    Err("synthetic external failure".into())
                },
            ),
            ImportApplyOutcome::PartialState
        );
        assert_eq!(attempts, [false, true]);
        assert!(presentation_store.snapshot_is_current(&current).unwrap());

        let plan = document
            .import_plan(
                &current,
                &interface_snapshot,
                &workspace_snapshot,
                &BTreeSet::new(),
                &BTreeMap::new(),
            )
            .unwrap();
        assert_eq!(
            apply_import_plan_with_reconciliation(
                &plan,
                &presentation_store,
                &current,
                &interface_store,
                &interface_snapshot,
                &workspace_store,
                &workspace_snapshot,
                |_| {
                    presentation_store
                        .save(PresentationSettings::default())
                        .unwrap();
                    Ok(())
                },
            ),
            ImportApplyOutcome::PartialState,
            "drift during external coordination cannot be reported as Applied"
        );
    }

    #[test]
    fn older_export_without_provider_badge_imports_as_compact_auto() {
        let settings =
            PresentationSettings::default().with_provider_badge(ProviderBadgePolicy::Always);
        let bytes = SettingsExportV1::new(Some(settings), None, &WorkspacePreferences::default())
            .to_canonical_json()
            .expect("new export serializes");
        let mut legacy: serde_json::Value =
            serde_json::from_slice(&bytes).expect("new export is valid JSON");
        legacy["presentation"]
            .as_object_mut()
            .expect("presentation object exists")
            .remove("provider_badge");
        let legacy = serde_json::to_vec(&legacy).expect("legacy fixture serializes");

        let parsed = SettingsExportV1::parse(&legacy).expect("older export remains supported");
        assert_eq!(
            parsed
                .presentation()
                .expect("presentation remains valid")
                .expect("presentation remains present")
                .provider_badge(),
            ProviderBadgePolicy::Auto
        );
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
            apply_import_plan_with_reconciliation(
                &plan,
                &presentation_store,
                &presentation_snapshot,
                &interface_store,
                &interface_snapshot,
                &workspace_store,
                &workspace_snapshot,
                |_| Ok(()),
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
            apply_import_plan_with_reconciliation(
                &plan,
                &presentation_store,
                &presentation_snapshot,
                &interface_store,
                &interface_snapshot,
                &workspace_store,
                &workspace_snapshot,
                |_| panic!("drift must reject before Hook reconciliation"),
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
