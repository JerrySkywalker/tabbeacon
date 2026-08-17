//! Read-only Windows Terminal title-policy diagnosis and ownership-safe repair.
//!
//! This module deliberately treats Windows Terminal settings as a user-owned
//! JSONC document.  It parses the small, typed policy surface that `TabBeacon`
//! needs, preserves all unrelated bytes, and only offers mutation when both
//! the settings document and the active profile GUID are unambiguous.

use std::{
    collections::HashSet,
    env, fmt, fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

use atomic_write_file::AtomicWriteFile;
use serde::{Deserialize, Serialize, Serializer};
use sha2::{Digest, Sha256};

const MAX_SETTINGS_BYTES: usize = 2 * 1024 * 1024;
const RECEIPT_FILE: &str = "windows-terminal-title-policy-v1.json";

/// A supported Windows Terminal settings-document family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowsTerminalInstallation {
    /// Packaged Stable Windows Terminal.
    Stable,
    /// Packaged Preview Windows Terminal.
    Preview,
    /// Packaged Canary Windows Terminal.
    Canary,
    /// An unpackaged Windows Terminal installation.
    Unpackaged,
}

impl WindowsTerminalInstallation {
    /// Stable diagnostic spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Preview => "preview",
            Self::Canary => "canary",
            Self::Unpackaged => "unpackaged",
        }
    }
}

/// Safe classification of the current Windows Terminal settings source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsSourceResolution {
    /// One bounded supported settings document was found.
    Resolved(WindowsTerminalInstallation),
    /// More than one candidate exists, so current-session source is not proven.
    Ambiguous,
    /// No supported settings document was available for a Terminal session.
    Unavailable,
    /// The process was not positively identified as a Windows Terminal session.
    NotCurrentTerminal,
    /// A candidate was malformed, oversized, or an unsafe link/reparse target.
    MalformedOrUnsafe,
}

impl Serialize for SettingsSourceResolution {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl SettingsSourceResolution {
    /// Stable diagnostic spelling without a path.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Resolved(installation) => installation.as_str(),
            Self::Ambiguous => "ambiguous_settings_source",
            Self::Unavailable => "unavailable",
            Self::NotCurrentTerminal => "not_current_terminal",
            Self::MalformedOrUnsafe => "malformed_or_unsafe",
        }
    }
}

/// Safe classification of the active Windows Terminal profile identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveProfileResolution {
    /// A current-session GUID matched exactly one profile object.
    Resolved,
    /// The session did not expose a valid GUID or multiple objects matched it.
    Ambiguous,
    /// A settings document could not be inspected first.
    Unavailable,
}

impl Serialize for ActiveProfileResolution {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl ActiveProfileResolution {
    /// Stable diagnostic spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Resolved => "resolved_guid",
            Self::Ambiguous => "ambiguous_profile",
            Self::Unavailable => "unavailable",
        }
    }
}

/// Effective origin of the application-title behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicySource {
    /// The exact active profile supplied the applicable property.
    Profile,
    /// `profiles.defaults` supplied the applicable property.
    InheritedDefault,
    /// Neither scope supplied a suppression property; Terminal defaults apply.
    TerminalDefault,
    /// A source could not be safely resolved.
    Unavailable,
}

impl PolicySource {
    /// Stable diagnostic spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Profile => "active_profile",
            Self::InheritedDefault => "profiles_defaults",
            Self::TerminalDefault => "terminal_default",
            Self::Unavailable => "unavailable",
        }
    }
}

/// Effective ability of `TabBeacon` application title sequences to reach a tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationTitlePolicy {
    /// Inspection was intentionally not attempted.
    NotInspected,
    /// The effective profile permits application title updates.
    ApplicationTitlesAllowed,
    /// An active-profile override suppresses application title updates.
    SuppressedByProfile,
    /// An inherited `profiles.defaults` setting suppresses title updates.
    SuppressedByInheritedDefault,
    /// Titles are allowed, but a user supplied a static `tabTitle` context.
    StaticTitleContext,
    /// The profile could not be identified without display-name guessing.
    AmbiguousProfile,
    /// Multiple settings sources exist and none was proven current.
    AmbiguousSettingsSource,
    /// No supported source is available for the current Terminal session.
    Unavailable,
    /// The candidate document was malformed, oversized, or unsafe.
    MalformedOrUnsafe,
}

impl ApplicationTitlePolicy {
    /// Stable diagnostic spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotInspected => "not_inspected",
            Self::ApplicationTitlesAllowed => "application_titles_allowed",
            Self::SuppressedByProfile => "suppressed_by_profile",
            Self::SuppressedByInheritedDefault => "suppressed_by_inherited_default",
            Self::StaticTitleContext => "static_title_context",
            Self::AmbiguousProfile => "ambiguous_profile",
            Self::AmbiguousSettingsSource => "ambiguous_settings_source",
            Self::Unavailable => "unavailable",
            Self::MalformedOrUnsafe => "malformed_or_unsafe",
        }
    }

    /// Whether this policy still permits a `TabBeacon` application title write.
    #[must_use]
    pub const fn permits_application_titles(self) -> bool {
        matches!(
            self,
            Self::ApplicationTitlesAllowed | Self::StaticTitleContext
        )
    }

    #[must_use]
    const fn is_suppressed(self) -> bool {
        matches!(
            self,
            Self::SuppressedByProfile | Self::SuppressedByInheritedDefault
        )
    }
}

/// Whether a persistent, explicit repair is safe and useful.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TitleRemediationState {
    /// Effective policy already permits application titles.
    NotNeeded,
    /// An exact active-profile override can safely clear suppression.
    Available,
    /// Inspection can report facts but mutation cannot be safely targeted.
    DiagnoseOnly,
    /// A previously recorded `TabBeacon` change remains exactly owned.
    AlreadyOwned,
    /// A source/profile ambiguity blocks mutation.
    BlockedAmbiguous,
    /// The user changed an owned target after a repair.
    BlockedDrift,
    /// No supported repair surface is available.
    Unavailable,
}

impl TitleRemediationState {
    /// Stable diagnostic spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotNeeded => "not_needed",
            Self::Available => "available",
            Self::DiagnoseOnly => "diagnose_only",
            Self::AlreadyOwned => "already_owned",
            Self::BlockedAmbiguous => "blocked_ambiguous",
            Self::BlockedDrift => "blocked_drift",
            Self::Unavailable => "unavailable",
        }
    }
}

/// Content-minimal Windows Terminal title-policy facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct TitlePolicyDiagnostics {
    /// Safe settings source classification, never a filesystem path.
    pub settings_source: SettingsSourceResolution,
    /// Whether the active profile was resolved by a current-session GUID.
    pub active_profile_resolution: ActiveProfileResolution,
    /// Effective application-title policy after inheritance.
    pub application_title_policy: ApplicationTitlePolicy,
    /// Scope from which the effective policy came.
    pub policy_source: PolicySource,
    /// Whether a narrow explicit repair is currently possible.
    pub remediation: TitleRemediationState,
    /// Repair scope, fixed to the active profile when available.
    pub remediation_scope: &'static str,
}

impl TitlePolicyDiagnostics {
    /// A passive report for a system on which Terminal policy was not inspected.
    #[must_use]
    pub const fn not_inspected() -> Self {
        Self {
            settings_source: SettingsSourceResolution::NotCurrentTerminal,
            active_profile_resolution: ActiveProfileResolution::Unavailable,
            application_title_policy: ApplicationTitlePolicy::NotInspected,
            policy_source: PolicySource::Unavailable,
            remediation: TitleRemediationState::Unavailable,
            remediation_scope: "none",
        }
    }
}

/// Result of an explicit persistent repair or restore request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct TitleRemediationResult {
    /// Safe remediation outcome.
    pub state: TitleRemediationState,
    /// Whether the settings document was modified by this invocation.
    pub document_modified: bool,
    /// Whether user configuration was preserved without a mutation.
    pub user_config_preserved: bool,
}

impl TitleRemediationResult {
    const fn untouched(state: TitleRemediationState) -> Self {
        Self {
            state,
            document_modified: false,
            user_config_preserved: true,
        }
    }
}

/// A bounded settings candidate supplied by the platform discovery layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsCandidate {
    installation: WindowsTerminalInstallation,
    path: PathBuf,
}

impl SettingsCandidate {
    /// Creates one known Windows Terminal settings candidate.
    #[must_use]
    pub fn new(installation: WindowsTerminalInstallation, path: impl Into<PathBuf>) -> Self {
        Self {
            installation,
            path: path.into(),
        }
    }
}

/// One injected environment for deterministic policy diagnosis and fixtures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsTerminalPolicyStore {
    candidates: Vec<SettingsCandidate>,
    state_root: Option<PathBuf>,
    terminal_session: bool,
    active_profile_id: Option<String>,
}

impl WindowsTerminalPolicyStore {
    /// Discovers only the documented per-user Windows Terminal locations.
    #[must_use]
    pub fn from_environment() -> Self {
        let local_app_data = env::var_os("LOCALAPPDATA").map(PathBuf::from);
        let candidates = local_app_data.as_ref().map_or_else(Vec::new, |root| {
            vec![
                SettingsCandidate::new(
                    WindowsTerminalInstallation::Stable,
                    root.join("Packages")
                        .join("Microsoft.WindowsTerminal_8wekyb3d8bbwe")
                        .join("LocalState")
                        .join("settings.json"),
                ),
                SettingsCandidate::new(
                    WindowsTerminalInstallation::Preview,
                    root.join("Packages")
                        .join("Microsoft.WindowsTerminalPreview_8wekyb3d8bbwe")
                        .join("LocalState")
                        .join("settings.json"),
                ),
                SettingsCandidate::new(
                    WindowsTerminalInstallation::Canary,
                    root.join("Packages")
                        .join("Microsoft.WindowsTerminalCanary_8wekyb3d8bbwe")
                        .join("LocalState")
                        .join("settings.json"),
                ),
                SettingsCandidate::new(
                    WindowsTerminalInstallation::Unpackaged,
                    root.join("Microsoft")
                        .join("Windows Terminal")
                        .join("settings.json"),
                ),
            ]
        });
        Self {
            candidates,
            state_root: local_app_data.map(|root| root.join("TabBeacon")),
            terminal_session: env::var_os("WT_SESSION").is_some_and(|value| !value.is_empty()),
            active_profile_id: env::var("WT_PROFILE_ID").ok(),
        }
    }

    /// Creates an injected store for isolated tests or wholly owned fixtures.
    #[must_use]
    pub fn new_for_testing(
        candidates: Vec<SettingsCandidate>,
        state_root: impl Into<PathBuf>,
        terminal_session: bool,
        active_profile_id: Option<String>,
    ) -> Self {
        Self {
            candidates,
            state_root: Some(state_root.into()),
            terminal_session,
            active_profile_id,
        }
    }

    /// Collects passive policy diagnostics without creating state or writing.
    #[must_use]
    pub fn inspect(&self) -> TitlePolicyDiagnostics {
        let Ok(resolved) = self.resolve_document() else {
            return self.diagnose_resolution_failure();
        };
        let Ok(document) = JsoncDocument::parse(&resolved.bytes) else {
            return TitlePolicyDiagnostics {
                settings_source: SettingsSourceResolution::MalformedOrUnsafe,
                active_profile_resolution: ActiveProfileResolution::Unavailable,
                application_title_policy: ApplicationTitlePolicy::MalformedOrUnsafe,
                policy_source: PolicySource::Unavailable,
                remediation: TitleRemediationState::Unavailable,
                remediation_scope: "none",
            };
        };
        let Ok(profile) = resolve_active_profile(&document, self.active_profile_id.as_deref())
        else {
            return TitlePolicyDiagnostics {
                settings_source: SettingsSourceResolution::Resolved(resolved.installation),
                active_profile_resolution: ActiveProfileResolution::Ambiguous,
                application_title_policy: ApplicationTitlePolicy::AmbiguousProfile,
                policy_source: PolicySource::Unavailable,
                remediation: TitleRemediationState::BlockedAmbiguous,
                remediation_scope: "none",
            };
        };
        let Ok(effective) = effective_policy(profile.profile, profile.defaults) else {
            return TitlePolicyDiagnostics {
                settings_source: SettingsSourceResolution::MalformedOrUnsafe,
                active_profile_resolution: ActiveProfileResolution::Unavailable,
                application_title_policy: ApplicationTitlePolicy::MalformedOrUnsafe,
                policy_source: PolicySource::Unavailable,
                remediation: TitleRemediationState::Unavailable,
                remediation_scope: "none",
            };
        };
        let policy = effective.classification();
        let mut remediation = if policy.is_suppressed() {
            TitleRemediationState::Available
        } else {
            TitleRemediationState::NotNeeded
        };
        if remediation == TitleRemediationState::NotNeeded
            && self.receipt_matches_profile(resolved.installation, &resolved.bytes, profile.profile)
        {
            remediation = TitleRemediationState::AlreadyOwned;
        }
        TitlePolicyDiagnostics {
            settings_source: SettingsSourceResolution::Resolved(resolved.installation),
            active_profile_resolution: ActiveProfileResolution::Resolved,
            application_title_policy: policy,
            policy_source: effective.source,
            remediation,
            remediation_scope: if remediation == TitleRemediationState::Available {
                "active_profile"
            } else {
                "none"
            },
        }
    }

    /// Applies the smallest sufficient profile-only title-policy repair.
    ///
    /// This is intentionally an explicit operation. It re-reads the exact
    /// document immediately before its atomic write and refuses drift.
    ///
    /// # Errors
    ///
    /// Returns an error only when a bounded settings or receipt filesystem
    /// operation cannot safely complete. Policy ambiguity is a successful
    /// no-op result rather than an error.
    pub fn repair(&self) -> Result<TitleRemediationResult, PolicyError> {
        self.repair_with_before_revalidate(|_| {})
    }

    fn repair_with_before_revalidate(
        &self,
        before_revalidate: impl FnOnce(&Path),
    ) -> Result<TitleRemediationResult, PolicyError> {
        let resolved = match self.resolve_document() {
            Ok(resolved) => resolved,
            Err(error) => return Ok(TitleRemediationResult::untouched(error.remediation_state())),
        };
        let Ok(document) = JsoncDocument::parse(&resolved.bytes) else {
            return Ok(TitleRemediationResult::untouched(
                TitleRemediationState::Unavailable,
            ));
        };
        let Ok(profile) = resolve_active_profile(&document, self.active_profile_id.as_deref())
        else {
            return Ok(TitleRemediationResult::untouched(
                TitleRemediationState::BlockedAmbiguous,
            ));
        };
        let Ok(effective) = effective_policy(profile.profile, profile.defaults) else {
            return Ok(TitleRemediationResult::untouched(
                TitleRemediationState::Unavailable,
            ));
        };
        if !effective.classification().is_suppressed() {
            let state = if self.receipt_matches_profile(
                resolved.installation,
                &resolved.bytes,
                profile.profile,
            ) {
                TitleRemediationState::AlreadyOwned
            } else {
                TitleRemediationState::NotNeeded
            };
            return Ok(TitleRemediationResult::untouched(state));
        }

        let edit = profile_false_edit(&resolved.bytes, profile.profile)?;
        before_revalidate(&resolved.path);
        let current = read_safe_bytes(&resolved.path)?;
        if current != resolved.bytes {
            return Ok(TitleRemediationResult::untouched(
                TitleRemediationState::BlockedDrift,
            ));
        }
        let inserted_fragment = if edit.prior_target == PriorTargetState::Absent {
            Some(InsertedFragment {
                offset_in_profile: edit.range.start - profile.profile.start,
                bytes: String::from_utf8(edit.replacement.clone())
                    .map_err(|_| PolicyError::MalformedOrUnsafe)?,
            })
        } else {
            None
        };
        let mut updated = resolved.bytes.clone();
        updated.splice(edit.range, edit.replacement);
        let verified = JsoncDocument::parse(&updated)?;
        let verified_profile =
            resolve_active_profile(&verified, self.active_profile_id.as_deref())?;
        let verified_effective =
            effective_policy(verified_profile.profile, verified_profile.defaults)?;
        if verified_effective.classification() != ApplicationTitlePolicy::ApplicationTitlesAllowed
            && verified_effective.classification() != ApplicationTitlePolicy::StaticTitleContext
        {
            return Err(PolicyError::MalformedOrUnsafe);
        }
        let receipt = OwnershipReceipt {
            schema_version: 1,
            installation: resolved.installation,
            document_sha256_before: sha256_hex(&resolved.bytes),
            profile_id_sha256: sha256_hex(profile.profile_id.as_bytes()),
            owned_json_path: "profiles.list[].suppressApplicationTitle".to_owned(),
            prior_target: edit.prior_target,
            post_profile_sha256: sha256_hex(profile_bytes(&updated, verified_profile.profile)),
            inserted_fragment,
        };
        self.write_receipt(&receipt)?;
        if let Err(error) = atomic_write(&resolved.path, &updated) {
            let _ = self.remove_receipt();
            return Err(error);
        }
        let final_bytes = read_safe_bytes(&resolved.path)?;
        if final_bytes != updated {
            return Ok(TitleRemediationResult::untouched(
                TitleRemediationState::BlockedDrift,
            ));
        }
        Ok(TitleRemediationResult {
            state: TitleRemediationState::Available,
            document_modified: true,
            user_config_preserved: true,
        })
    }

    /// Restores only an exact `TabBeacon`-owned target that did not drift.
    ///
    /// # Errors
    ///
    /// Returns an error only when bounded settings or owned-receipt storage
    /// cannot be read or written safely. Ambiguity and drift are no-op results.
    pub fn restore(&self) -> Result<TitleRemediationResult, PolicyError> {
        let Some(receipt) = self.read_receipt()? else {
            return Ok(TitleRemediationResult::untouched(
                TitleRemediationState::NotNeeded,
            ));
        };
        let resolved = match self.resolve_document() {
            Ok(resolved) => resolved,
            Err(error) => return Ok(TitleRemediationResult::untouched(error.remediation_state())),
        };
        if resolved.installation != receipt.installation {
            return Ok(TitleRemediationResult::untouched(
                TitleRemediationState::BlockedAmbiguous,
            ));
        }
        let document = JsoncDocument::parse(&resolved.bytes)?;
        let Ok(profile) = resolve_active_profile(&document, self.active_profile_id.as_deref())
        else {
            return Ok(TitleRemediationResult::untouched(
                TitleRemediationState::BlockedAmbiguous,
            ));
        };
        if receipt.schema_version != 1
            || receipt.owned_json_path != "profiles.list[].suppressApplicationTitle"
            || sha256_hex(profile.profile_id.as_bytes()) != receipt.profile_id_sha256
            || sha256_hex(profile_bytes(&resolved.bytes, profile.profile))
                != receipt.post_profile_sha256
            || !matches!(
                target_bool(profile.profile, "suppressApplicationTitle"),
                Ok(Some(false))
            )
        {
            return Ok(TitleRemediationResult::untouched(
                TitleRemediationState::BlockedDrift,
            ));
        }
        let edit = match receipt.prior_target {
            PriorTargetState::Absent => {
                let inserted = receipt
                    .inserted_fragment
                    .as_ref()
                    .ok_or(PolicyError::MalformedOrUnsafe)?;
                let start = profile.profile.start + inserted.offset_in_profile;
                let end = start + inserted.bytes.len();
                if resolved.bytes.get(start..end) != Some(inserted.bytes.as_bytes()) {
                    return Ok(TitleRemediationResult::untouched(
                        TitleRemediationState::BlockedDrift,
                    ));
                }
                ByteEdit {
                    range: start..end,
                    replacement: Vec::new(),
                    prior_target: PriorTargetState::Absent,
                }
            }
            PriorTargetState::True => {
                restore_edit(&resolved.bytes, profile.profile, PriorTargetState::True)?
            }
        };
        let current = read_safe_bytes(&resolved.path)?;
        if current != resolved.bytes {
            return Ok(TitleRemediationResult::untouched(
                TitleRemediationState::BlockedDrift,
            ));
        }
        let mut updated = resolved.bytes.clone();
        updated.splice(edit.range, edit.replacement);
        JsoncDocument::parse(&updated)?;
        atomic_write(&resolved.path, &updated)?;
        self.remove_receipt()?;
        Ok(TitleRemediationResult {
            state: TitleRemediationState::AlreadyOwned,
            document_modified: true,
            user_config_preserved: true,
        })
    }

    fn diagnose_resolution_failure(&self) -> TitlePolicyDiagnostics {
        let source = match self.resolve_document() {
            Err(PolicyError::AmbiguousSettingsSource | PolicyError::AmbiguousProfile) => {
                SettingsSourceResolution::Ambiguous
            }
            Err(PolicyError::NotCurrentTerminal) => SettingsSourceResolution::NotCurrentTerminal,
            Err(PolicyError::Unavailable) => SettingsSourceResolution::Unavailable,
            Err(PolicyError::MalformedOrUnsafe | PolicyError::Io(_)) => {
                SettingsSourceResolution::MalformedOrUnsafe
            }
            Ok(_) => unreachable!("only called after source resolution failed"),
        };
        let (policy, remediation) = match source {
            SettingsSourceResolution::Ambiguous => (
                ApplicationTitlePolicy::AmbiguousSettingsSource,
                TitleRemediationState::BlockedAmbiguous,
            ),
            SettingsSourceResolution::MalformedOrUnsafe => (
                ApplicationTitlePolicy::MalformedOrUnsafe,
                TitleRemediationState::Unavailable,
            ),
            SettingsSourceResolution::Unavailable
            | SettingsSourceResolution::NotCurrentTerminal => (
                ApplicationTitlePolicy::Unavailable,
                TitleRemediationState::Unavailable,
            ),
            SettingsSourceResolution::Resolved(_) => {
                unreachable!("resolved document is not failure")
            }
        };
        TitlePolicyDiagnostics {
            settings_source: source,
            active_profile_resolution: ActiveProfileResolution::Unavailable,
            application_title_policy: policy,
            policy_source: PolicySource::Unavailable,
            remediation,
            remediation_scope: "none",
        }
    }

    fn resolve_document(&self) -> Result<ResolvedDocument, PolicyError> {
        if !self.terminal_session {
            return Err(PolicyError::NotCurrentTerminal);
        }
        let mut present = Vec::new();
        for candidate in &self.candidates {
            match fs::symlink_metadata(&candidate.path) {
                Ok(metadata) => {
                    if is_unsafe_target(&metadata) {
                        return Err(PolicyError::MalformedOrUnsafe);
                    }
                    present.push(candidate);
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(PolicyError::Io(error)),
            }
        }
        let [candidate] = present.as_slice() else {
            return if present.is_empty() {
                Err(PolicyError::Unavailable)
            } else {
                Err(PolicyError::AmbiguousSettingsSource)
            };
        };
        let bytes = read_safe_bytes(&candidate.path)?;
        if bytes.len() > MAX_SETTINGS_BYTES {
            return Err(PolicyError::MalformedOrUnsafe);
        }
        Ok(ResolvedDocument {
            installation: candidate.installation,
            path: candidate.path.clone(),
            bytes,
        })
    }

    fn receipt_path(&self) -> Result<PathBuf, PolicyError> {
        self.state_root
            .as_ref()
            .map(|root| root.join(RECEIPT_FILE))
            .ok_or(PolicyError::Unavailable)
    }

    fn read_receipt(&self) -> Result<Option<OwnershipReceipt>, PolicyError> {
        let path = self.receipt_path()?;
        match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|_| PolicyError::MalformedOrUnsafe),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(PolicyError::Io(error)),
        }
    }

    fn write_receipt(&self, receipt: &OwnershipReceipt) -> Result<(), PolicyError> {
        let bytes = serde_json::to_vec(receipt).map_err(|_| PolicyError::MalformedOrUnsafe)?;
        atomic_write(&self.receipt_path()?, &bytes)?;
        Ok(())
    }

    fn remove_receipt(&self) -> Result<(), PolicyError> {
        let path = self.receipt_path()?;
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(PolicyError::Io(error)),
        }
    }

    fn receipt_matches_profile(
        &self,
        installation: WindowsTerminalInstallation,
        bytes: &[u8],
        profile: &JsonNode,
    ) -> bool {
        self.read_receipt().ok().flatten().is_some_and(|receipt| {
            receipt.schema_version == 1
                && receipt.owned_json_path == "profiles.list[].suppressApplicationTitle"
                && receipt.installation == installation
                && matches!(
                    target_bool(profile, "suppressApplicationTitle"),
                    Ok(Some(false))
                )
                && sha256_hex(profile_bytes(bytes, profile)) == receipt.post_profile_sha256
        })
    }
}

/// Error retained internally; diagnostics always reduce it to safe classes.
#[derive(Debug)]
pub enum PolicyError {
    /// More than one source exists.
    AmbiguousSettingsSource,
    /// The profile cannot be identified exactly.
    AmbiguousProfile,
    /// The process is not a known Terminal session.
    NotCurrentTerminal,
    /// No source or state root is available.
    Unavailable,
    /// Input is malformed, too large, or unsafe for mutation.
    MalformedOrUnsafe,
    /// A filesystem operation failed.
    Io(io::Error),
}

impl PolicyError {
    const fn remediation_state(&self) -> TitleRemediationState {
        match self {
            Self::AmbiguousSettingsSource | Self::AmbiguousProfile => {
                TitleRemediationState::BlockedAmbiguous
            }
            Self::NotCurrentTerminal
            | Self::Unavailable
            | Self::MalformedOrUnsafe
            | Self::Io(_) => TitleRemediationState::Unavailable,
        }
    }
}

impl fmt::Display for PolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Self::AmbiguousSettingsSource => "Windows Terminal settings source is ambiguous",
            Self::AmbiguousProfile => "active Windows Terminal profile is ambiguous",
            Self::NotCurrentTerminal => "this process is not in a Windows Terminal session",
            Self::Unavailable => "Windows Terminal title policy is unavailable",
            Self::MalformedOrUnsafe => "Windows Terminal settings are malformed or unsafe",
            Self::Io(_) => "Windows Terminal title policy storage is unavailable",
        };
        formatter.write_str(text)
    }
}

impl std::error::Error for PolicyError {}

impl From<io::Error> for PolicyError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Debug, Clone, Copy)]
struct EffectivePolicy {
    source: PolicySource,
    suppress: bool,
    static_title: bool,
}

impl EffectivePolicy {
    const fn classification(self) -> ApplicationTitlePolicy {
        if self.suppress {
            return match self.source {
                PolicySource::Profile => ApplicationTitlePolicy::SuppressedByProfile,
                PolicySource::InheritedDefault => {
                    ApplicationTitlePolicy::SuppressedByInheritedDefault
                }
                PolicySource::TerminalDefault | PolicySource::Unavailable => {
                    ApplicationTitlePolicy::MalformedOrUnsafe
                }
            };
        }
        if self.static_title {
            ApplicationTitlePolicy::StaticTitleContext
        } else {
            ApplicationTitlePolicy::ApplicationTitlesAllowed
        }
    }
}

fn effective_policy(
    profile: &JsonNode,
    defaults: Option<&JsonNode>,
) -> Result<EffectivePolicy, PolicyError> {
    let profile_suppress = target_bool(profile, "suppressApplicationTitle")?;
    let default_suppress = defaults
        .map(|defaults| target_bool(defaults, "suppressApplicationTitle"))
        .transpose()?
        .flatten();
    let static_title = target_string(profile, "tabTitle")?.is_some_and(|value| !value.is_empty())
        || defaults
            .map(|defaults| target_string(defaults, "tabTitle"))
            .transpose()?
            .flatten()
            .is_some_and(|value| !value.is_empty());
    Ok(match profile_suppress {
        Some(value) => EffectivePolicy {
            source: PolicySource::Profile,
            suppress: value,
            static_title,
        },
        None => match default_suppress {
            Some(value) => EffectivePolicy {
                source: PolicySource::InheritedDefault,
                suppress: value,
                static_title,
            },
            None => EffectivePolicy {
                source: PolicySource::TerminalDefault,
                suppress: false,
                static_title,
            },
        },
    })
}

struct ResolvedProfile<'a> {
    profile: &'a JsonNode,
    defaults: Option<&'a JsonNode>,
    profile_id: String,
}

struct ResolvedDocument {
    installation: WindowsTerminalInstallation,
    path: PathBuf,
    bytes: Vec<u8>,
}

fn resolve_active_profile<'a>(
    document: &'a JsoncDocument,
    active_profile_id: Option<&str>,
) -> Result<ResolvedProfile<'a>, PolicyError> {
    let profile_id = active_profile_id
        .filter(|value| is_guid(value))
        .ok_or(PolicyError::AmbiguousProfile)?;
    let root = document
        .root
        .object()
        .ok_or(PolicyError::MalformedOrUnsafe)?;
    let profiles = unique_property(root, "profiles")
        .map_err(|()| PolicyError::MalformedOrUnsafe)?
        .ok_or(PolicyError::AmbiguousProfile)?
        .value
        .object()
        .ok_or(PolicyError::MalformedOrUnsafe)?;
    let defaults = unique_property(profiles, "defaults")
        .map_err(|()| PolicyError::MalformedOrUnsafe)?
        .map(|property| {
            property
                .value
                .object()
                .map(|_| &property.value)
                .ok_or(PolicyError::MalformedOrUnsafe)
        })
        .transpose()?;
    let list = unique_property(profiles, "list")
        .map_err(|()| PolicyError::MalformedOrUnsafe)?
        .ok_or(PolicyError::AmbiguousProfile)?
        .value
        .array()
        .ok_or(PolicyError::MalformedOrUnsafe)?;
    let mut matching = Vec::new();
    for entry in list {
        let Some(object) = entry.object() else {
            continue;
        };
        let guid = unique_property(object, "guid")
            .map_err(|()| PolicyError::MalformedOrUnsafe)?
            .and_then(|property| property.value.string());
        if guid.is_some_and(|value| value.eq_ignore_ascii_case(profile_id)) {
            matching.push(entry);
        }
    }
    let [profile] = matching.as_slice() else {
        return Err(PolicyError::AmbiguousProfile);
    };
    Ok(ResolvedProfile {
        profile,
        defaults,
        profile_id: unique_property(
            profile.object().ok_or(PolicyError::MalformedOrUnsafe)?,
            "guid",
        )
        .map_err(|()| PolicyError::MalformedOrUnsafe)?
        .and_then(|property| property.value.string())
        .ok_or(PolicyError::MalformedOrUnsafe)?
        .to_owned(),
    })
}

fn is_guid(value: &str) -> bool {
    let trimmed = match (value.starts_with('{'), value.ends_with('}')) {
        (true, true) if value.len() == 38 => &value[1..value.len() - 1],
        (false, false) => value,
        _ => return false,
    };
    trimmed.len() == 36
        && trimmed.chars().enumerate().all(|(index, character)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                character == '-'
            } else {
                character.is_ascii_hexdigit()
            }
        })
}

fn target_bool(node: &JsonNode, key: &str) -> Result<Option<bool>, PolicyError> {
    let Some(object) = node.object() else {
        return Err(PolicyError::MalformedOrUnsafe);
    };
    unique_property(object, key)
        .map_err(|()| PolicyError::MalformedOrUnsafe)?
        .map(|property| {
            property
                .value
                .boolean()
                .ok_or(PolicyError::MalformedOrUnsafe)
        })
        .transpose()
}

fn target_string<'a>(node: &'a JsonNode, key: &str) -> Result<Option<&'a str>, PolicyError> {
    let Some(object) = node.object() else {
        return Err(PolicyError::MalformedOrUnsafe);
    };
    unique_property(object, key)
        .map_err(|()| PolicyError::MalformedOrUnsafe)?
        .map(|property| {
            property
                .value
                .string()
                .ok_or(PolicyError::MalformedOrUnsafe)
        })
        .transpose()
}

fn unique_property<'a>(
    properties: &'a [JsonProperty],
    key: &str,
) -> Result<Option<&'a JsonProperty>, ()> {
    let mut found = None;
    for property in properties {
        if property.key == key && found.replace(property).is_some() {
            return Err(());
        }
    }
    Ok(found)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PriorTargetState {
    Absent,
    True,
}

#[derive(Debug, Serialize, Deserialize)]
struct OwnershipReceipt {
    schema_version: u8,
    installation: WindowsTerminalInstallation,
    document_sha256_before: String,
    profile_id_sha256: String,
    owned_json_path: String,
    prior_target: PriorTargetState,
    post_profile_sha256: String,
    inserted_fragment: Option<InsertedFragment>,
}

#[derive(Debug, Serialize, Deserialize)]
struct InsertedFragment {
    offset_in_profile: usize,
    bytes: String,
}

struct ByteEdit {
    range: std::ops::Range<usize>,
    replacement: Vec<u8>,
    prior_target: PriorTargetState,
}

fn profile_false_edit(bytes: &[u8], profile: &JsonNode) -> Result<ByteEdit, PolicyError> {
    let object = profile.object().ok_or(PolicyError::MalformedOrUnsafe)?;
    if let Some(property) = unique_property(object, "suppressApplicationTitle")
        .map_err(|()| PolicyError::MalformedOrUnsafe)?
    {
        match property.value.boolean() {
            Some(true) => Ok(ByteEdit {
                range: property.value.start..property.value.end,
                replacement: b"false".to_vec(),
                prior_target: PriorTargetState::True,
            }),
            Some(false) | None => Err(PolicyError::MalformedOrUnsafe),
        }
    } else {
        let insertion = object_insertion(bytes, profile)?;
        Ok(ByteEdit {
            range: insertion..insertion,
            replacement: insertion_text(bytes, profile)?,
            prior_target: PriorTargetState::Absent,
        })
    }
}

fn restore_edit(
    _bytes: &[u8],
    profile: &JsonNode,
    prior_target: PriorTargetState,
) -> Result<ByteEdit, PolicyError> {
    let object = profile.object().ok_or(PolicyError::MalformedOrUnsafe)?;
    let property = unique_property(object, "suppressApplicationTitle")
        .map_err(|()| PolicyError::MalformedOrUnsafe)?
        .ok_or(PolicyError::MalformedOrUnsafe)?;
    match prior_target {
        PriorTargetState::True => Ok(ByteEdit {
            range: property.value.start..property.value.end,
            replacement: b"true".to_vec(),
            prior_target,
        }),
        PriorTargetState::Absent => {
            let index = object
                .iter()
                .position(|candidate| candidate.start == property.start)
                .ok_or(PolicyError::MalformedOrUnsafe)?;
            let range = if let Some(comma) = property.comma_after {
                property.start..comma + 1
            } else if index > 0 {
                object[index - 1]
                    .comma_after
                    .ok_or(PolicyError::MalformedOrUnsafe)?..property.end
            } else {
                property.start..property.end
            };
            Ok(ByteEdit {
                range,
                replacement: Vec::new(),
                prior_target,
            })
        }
    }
}

fn object_insertion(_bytes: &[u8], profile: &JsonNode) -> Result<usize, PolicyError> {
    profile
        .object_end_before_close()
        .ok_or(PolicyError::MalformedOrUnsafe)
}

fn insertion_text(bytes: &[u8], profile: &JsonNode) -> Result<Vec<u8>, PolicyError> {
    let object = profile.object().ok_or(PolicyError::MalformedOrUnsafe)?;
    let newline = if bytes.windows(2).any(|window| window == b"\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let indent = object
        .last()
        .and_then(|property| line_indent(bytes, property.start))
        .unwrap_or_else(|| {
            format!(
                "{}  ",
                line_indent(bytes, profile.start).unwrap_or_default()
            )
        });
    let prefix = if object.is_empty() { newline } else { "," };
    let closing_indent = line_indent(bytes, profile.end - 1).unwrap_or_default();
    Ok(format!(
        "{prefix}{newline}{indent}\"suppressApplicationTitle\": false{newline}{closing_indent}"
    )
    .into_bytes())
}

fn line_indent(bytes: &[u8], offset: usize) -> Option<String> {
    let line_start = bytes[..offset]
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |index| index + 1);
    let indentation = &bytes[line_start..offset];
    if indentation
        .iter()
        .all(|byte| matches!(byte, b' ' | b'\t' | b'\r'))
    {
        Some(
            String::from_utf8_lossy(indentation)
                .trim_end_matches('\r')
                .to_owned(),
        )
    } else {
        None
    }
}

fn profile_bytes<'a>(bytes: &'a [u8], profile: &JsonNode) -> &'a [u8] {
    &bytes[profile.start..profile.end]
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn read_safe_bytes(path: &Path) -> Result<Vec<u8>, PolicyError> {
    let metadata = fs::symlink_metadata(path)?;
    if is_unsafe_target(&metadata) {
        return Err(PolicyError::MalformedOrUnsafe);
    }
    let bytes = fs::read(path)?;
    if bytes.len() > MAX_SETTINGS_BYTES {
        return Err(PolicyError::MalformedOrUnsafe);
    }
    Ok(bytes)
}

fn is_unsafe_target(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), PolicyError> {
    let parent = path.parent().ok_or(PolicyError::MalformedOrUnsafe)?;
    fs::create_dir_all(parent)?;
    let mut file = AtomicWriteFile::options().open(path)?;
    file.write_all(bytes)?;
    file.flush()?;
    file.commit()?;
    Ok(())
}

#[derive(Debug)]
struct JsoncDocument {
    root: JsonNode,
}

#[derive(Debug)]
struct JsonNode {
    start: usize,
    end: usize,
    kind: JsonKind,
}

impl JsonNode {
    fn object(&self) -> Option<&[JsonProperty]> {
        match &self.kind {
            JsonKind::Object(properties) => Some(properties),
            JsonKind::Array(_) | JsonKind::String(_) | JsonKind::Boolean(_) | JsonKind::Other => {
                None
            }
        }
    }

    fn array(&self) -> Option<&[JsonNode]> {
        match &self.kind {
            JsonKind::Array(values) => Some(values),
            JsonKind::Object(_) | JsonKind::String(_) | JsonKind::Boolean(_) | JsonKind::Other => {
                None
            }
        }
    }

    fn string(&self) -> Option<&str> {
        match &self.kind {
            JsonKind::String(value) => Some(value),
            JsonKind::Object(_) | JsonKind::Array(_) | JsonKind::Boolean(_) | JsonKind::Other => {
                None
            }
        }
    }

    const fn boolean(&self) -> Option<bool> {
        match self.kind {
            JsonKind::Boolean(value) => Some(value),
            JsonKind::Object(_) | JsonKind::Array(_) | JsonKind::String(_) | JsonKind::Other => {
                None
            }
        }
    }

    fn object_end_before_close(&self) -> Option<usize> {
        matches!(self.kind, JsonKind::Object(_)).then_some(self.end - 1)
    }
}

#[derive(Debug)]
enum JsonKind {
    Object(Vec<JsonProperty>),
    Array(Vec<JsonNode>),
    String(String),
    Boolean(bool),
    Other,
}

#[derive(Debug)]
struct JsonProperty {
    key: String,
    start: usize,
    end: usize,
    value: JsonNode,
    comma_after: Option<usize>,
}

impl JsoncDocument {
    fn parse(bytes: &[u8]) -> Result<Self, PolicyError> {
        let text = std::str::from_utf8(bytes).map_err(|_| PolicyError::MalformedOrUnsafe)?;
        let mut parser = JsoncParser::new(text);
        parser.skip_trivia()?;
        if parser.peek() == Some('\u{feff}') {
            parser.bump();
            parser.skip_trivia()?;
        }
        let root = parser.parse_value()?;
        parser.skip_trivia()?;
        if parser.peek().is_some() {
            return Err(PolicyError::MalformedOrUnsafe);
        }
        Ok(Self { root })
    }
}

struct JsoncParser<'a> {
    text: &'a str,
    index: usize,
}

impl<'a> JsoncParser<'a> {
    const fn new(text: &'a str) -> Self {
        Self { text, index: 0 }
    }

    fn parse_value(&mut self) -> Result<JsonNode, PolicyError> {
        self.skip_trivia()?;
        let start = self.index;
        match self.peek() {
            Some('{') => self.parse_object(start),
            Some('[') => self.parse_array(start),
            Some('"') => {
                let value = self.parse_string()?;
                Ok(JsonNode {
                    start,
                    end: self.index,
                    kind: JsonKind::String(value),
                })
            }
            Some('t') => self.parse_literal(start, "true", JsonKind::Boolean(true)),
            Some('f') => self.parse_literal(start, "false", JsonKind::Boolean(false)),
            Some('n') => self.parse_literal(start, "null", JsonKind::Other),
            Some('-' | '0'..='9') => self.parse_number(start),
            _ => Err(PolicyError::MalformedOrUnsafe),
        }
    }

    fn parse_object(&mut self, start: usize) -> Result<JsonNode, PolicyError> {
        self.expect('{')?;
        self.skip_trivia()?;
        let mut properties = Vec::new();
        let mut keys = HashSet::new();
        if self.peek() == Some('}') {
            self.bump();
            return Ok(JsonNode {
                start,
                end: self.index,
                kind: JsonKind::Object(properties),
            });
        }
        loop {
            self.skip_trivia()?;
            let property_start = self.index;
            let key = self.parse_string()?;
            self.skip_trivia()?;
            self.expect(':')?;
            let value = self.parse_value()?;
            let end = value.end;
            self.skip_trivia()?;
            let comma_after = if self.peek() == Some(',') {
                let comma = self.index;
                self.bump();
                Some(comma)
            } else {
                None
            };
            if !keys.insert(key.clone()) {
                // Keep parsing, but retain duplicate keys as structural ambiguity.
            }
            properties.push(JsonProperty {
                key,
                start: property_start,
                end,
                value,
                comma_after,
            });
            self.skip_trivia()?;
            match self.peek() {
                Some('}') => {
                    self.bump();
                    return Ok(JsonNode {
                        start,
                        end: self.index,
                        kind: JsonKind::Object(properties),
                    });
                }
                Some(_) if comma_after.is_some() => {}
                _ => return Err(PolicyError::MalformedOrUnsafe),
            }
        }
    }

    fn parse_array(&mut self, start: usize) -> Result<JsonNode, PolicyError> {
        self.expect('[')?;
        self.skip_trivia()?;
        let mut values = Vec::new();
        if self.peek() == Some(']') {
            self.bump();
            return Ok(JsonNode {
                start,
                end: self.index,
                kind: JsonKind::Array(values),
            });
        }
        loop {
            values.push(self.parse_value()?);
            self.skip_trivia()?;
            let comma = if self.peek() == Some(',') {
                self.bump();
                true
            } else {
                false
            };
            self.skip_trivia()?;
            match self.peek() {
                Some(']') => {
                    self.bump();
                    return Ok(JsonNode {
                        start,
                        end: self.index,
                        kind: JsonKind::Array(values),
                    });
                }
                Some(_) if comma => {}
                _ => return Err(PolicyError::MalformedOrUnsafe),
            }
        }
    }

    fn parse_string(&mut self) -> Result<String, PolicyError> {
        let start = self.index;
        self.expect('"')?;
        let mut escaped = false;
        while let Some(character) = self.peek() {
            self.bump();
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                let literal = &self.text[start..self.index];
                return serde_json::from_str(literal).map_err(|_| PolicyError::MalformedOrUnsafe);
            } else if character.is_control() {
                return Err(PolicyError::MalformedOrUnsafe);
            }
        }
        Err(PolicyError::MalformedOrUnsafe)
    }

    fn parse_literal(
        &mut self,
        start: usize,
        literal: &str,
        kind: JsonKind,
    ) -> Result<JsonNode, PolicyError> {
        if self.text[self.index..].starts_with(literal) {
            self.index += literal.len();
            Ok(JsonNode {
                start,
                end: self.index,
                kind,
            })
        } else {
            Err(PolicyError::MalformedOrUnsafe)
        }
    }

    fn parse_number(&mut self, start: usize) -> Result<JsonNode, PolicyError> {
        while self.peek().is_some_and(|character| {
            !character.is_whitespace() && !matches!(character, ',' | ']' | '}' | '/')
        }) {
            self.bump();
        }
        let literal = &self.text[start..self.index];
        match serde_json::from_str::<serde_json::Value>(literal) {
            Ok(serde_json::Value::Number(_)) => Ok(JsonNode {
                start,
                end: self.index,
                kind: JsonKind::Other,
            }),
            Ok(_) | Err(_) => Err(PolicyError::MalformedOrUnsafe),
        }
    }

    fn skip_trivia(&mut self) -> Result<(), PolicyError> {
        loop {
            while self.peek().is_some_and(char::is_whitespace) {
                self.bump();
            }
            if !self.text[self.index..].starts_with('/') {
                return Ok(());
            }
            if self.text[self.index..].starts_with("//") {
                self.index += 2;
                while self.peek().is_some_and(|character| character != '\n') {
                    self.bump();
                }
            } else if self.text[self.index..].starts_with("/*") {
                self.index += 2;
                let Some(end) = self.text[self.index..].find("*/") else {
                    return Err(PolicyError::MalformedOrUnsafe);
                };
                self.index += end + 2;
            } else {
                return Err(PolicyError::MalformedOrUnsafe);
            }
        }
    }

    fn expect(&mut self, expected: char) -> Result<(), PolicyError> {
        if self.peek() == Some(expected) {
            self.bump();
            Ok(())
        } else {
            Err(PolicyError::MalformedOrUnsafe)
        }
    }

    fn peek(&self) -> Option<char> {
        self.text[self.index..].chars().next()
    }

    fn bump(&mut self) {
        if let Some(character) = self.peek() {
            self.index += character.len_utf8();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{
        ActiveProfileResolution, ApplicationTitlePolicy, SettingsCandidate,
        SettingsSourceResolution, TitleRemediationState, WindowsTerminalInstallation,
        WindowsTerminalPolicyStore,
    };

    const GUID: &str = "{11111111-1111-1111-1111-111111111111}";

    fn temporary_root(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "tabbeacon-g17-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ))
    }

    fn fixture_store(
        name: &str,
        contents: &str,
    ) -> (WindowsTerminalPolicyStore, std::path::PathBuf) {
        let root = temporary_root(name);
        let path = root.join("fixture-settings.json");
        fs::create_dir_all(&root).expect("fixture root");
        fs::write(&path, contents).expect("fixture settings");
        let store = WindowsTerminalPolicyStore::new_for_testing(
            vec![SettingsCandidate::new(
                WindowsTerminalInstallation::Stable,
                &path,
            )],
            root.join("state"),
            true,
            Some(GUID.to_owned()),
        );
        (store, path)
    }

    fn settings(defaults: &str, profile: &str) -> String {
        format!(
            "{{\r\n  // user comment\r\n  \"profiles\": {{\r\n    \"defaults\": {{ {defaults} }},\r\n    \"list\": [\r\n      {{ \"guid\": \"{GUID}\", \"name\": \"PowerShell\", {profile} }},\r\n      {{ \"guid\": \"{{22222222-2222-2222-2222-222222222222}}\", \"name\": \"PowerShell\" }},\r\n    ],\r\n  }},\r\n  \"unknown\": {{ \"kept\": true }},\r\n}}\r\n"
        )
    }

    #[test]
    fn effective_policy_distinguishes_profile_and_inherited_suppression() {
        let (profile_store, _) = fixture_store(
            "profile-suppressed",
            &settings("", "\"suppressApplicationTitle\": true"),
        );
        assert_eq!(
            profile_store.inspect().application_title_policy,
            ApplicationTitlePolicy::SuppressedByProfile
        );
        let (default_store, _) = fixture_store(
            "default-suppressed",
            &settings(
                "\"suppressApplicationTitle\": true",
                "\"commandline\": \"pwsh\"",
            ),
        );
        let report = default_store.inspect();
        assert_eq!(
            report.application_title_policy,
            ApplicationTitlePolicy::SuppressedByInheritedDefault
        );
        assert_eq!(report.remediation, TitleRemediationState::Available);
    }

    #[test]
    fn static_tab_title_is_context_not_grounds_for_deletion() {
        let (store, _) = fixture_store("static", &settings("", "\"tabTitle\": \"keep me\""));
        let report = store.inspect();
        assert_eq!(
            report.application_title_policy,
            ApplicationTitlePolicy::StaticTitleContext
        );
        assert_eq!(report.remediation, TitleRemediationState::NotNeeded);
    }

    #[test]
    fn stable_preview_and_unpackaged_sources_are_individually_supported() {
        for (label, installation, expected) in [
            ("stable", WindowsTerminalInstallation::Stable, "stable"),
            ("preview", WindowsTerminalInstallation::Preview, "preview"),
            (
                "unpackaged",
                WindowsTerminalInstallation::Unpackaged,
                "unpackaged",
            ),
        ] {
            let root = temporary_root(label);
            let path = root.join("settings.json");
            fs::create_dir_all(&root).expect("fixture root");
            fs::write(&path, settings("", "\"suppressApplicationTitle\": false"))
                .expect("fixture settings");
            let store = WindowsTerminalPolicyStore::new_for_testing(
                vec![SettingsCandidate::new(installation, &path)],
                root.join("state"),
                true,
                Some(GUID.to_owned()),
            );
            let report = store.inspect();
            assert_eq!(report.settings_source.as_str(), expected);
            assert_eq!(
                report.application_title_policy,
                ApplicationTitlePolicy::ApplicationTitlesAllowed
            );
        }
    }

    #[test]
    fn dynamic_profile_override_and_missing_defaults_remain_guid_scoped() {
        let document = format!(
            "{{\n  \"profiles\": {{\n    \"list\": [\n      {{ \"guid\": \"{GUID}\", \"source\": \"Windows.Terminal.Wsl\", \"suppressApplicationTitle\": true }},\n    ],\n  }},\n}}\n"
        );
        let (store, _) = fixture_store("dynamic", &document);
        let report = store.inspect();
        assert_eq!(
            report.active_profile_resolution,
            ActiveProfileResolution::Resolved
        );
        assert_eq!(
            report.application_title_policy,
            ApplicationTitlePolicy::SuppressedByProfile
        );
        assert_eq!(report.remediation_scope, "active_profile");
    }

    #[test]
    fn repair_is_minimal_idempotent_and_restore_is_owned() {
        let original = settings(
            "\"suppressApplicationTitle\": true",
            "\"tabTitle\": \"keep me\"",
        );
        let (store, path) = fixture_store("repair", &original);
        let first = store.repair().expect("first repair");
        assert!(first.document_modified);
        let repaired = fs::read(&path).expect("repaired bytes");
        let repaired_text = String::from_utf8_lossy(&repaired);
        assert!(repaired_text.contains("\"tabTitle\": \"keep me\""));
        assert!(repaired_text.contains("\"unknown\": { \"kept\": true }"));
        assert!(repaired_text.contains("\r\n"));
        let second = store.repair().expect("second repair");
        assert_eq!(second.state, TitleRemediationState::AlreadyOwned);
        assert_eq!(fs::read(&path).expect("stable second bytes"), repaired);
        let restored = store.restore().expect("restore");
        assert!(restored.document_modified);
        assert_eq!(fs::read_to_string(&path).expect("restore bytes"), original);
        assert_eq!(
            store.restore().expect("repeat restore").state,
            TitleRemediationState::NotNeeded
        );
    }

    #[test]
    fn owned_target_drift_refuses_restore_but_unrelated_drift_does_not() {
        let (store, path) =
            fixture_store("drift", &settings("", "\"suppressApplicationTitle\": true"));
        store.repair().expect("repair");
        let unrelated = fs::read_to_string(&path)
            .expect("bytes")
            .replace("\"kept\": true", "\"kept\": false");
        fs::write(&path, unrelated).expect("unrelated drift");
        assert!(
            store
                .restore()
                .expect("unrelated restore")
                .document_modified
        );
        store.repair().expect("repair again");
        let target_drift = fs::read_to_string(&path).expect("bytes").replace(
            "\"suppressApplicationTitle\": false",
            "\"suppressApplicationTitle\": true",
        );
        fs::write(&path, target_drift).expect("target drift");
        assert_eq!(
            store.restore().expect("target restore").state,
            TitleRemediationState::BlockedDrift
        );
    }

    #[test]
    fn pre_write_document_drift_is_refused_without_overwriting_user_change() {
        let (store, path) = fixture_store(
            "apply-drift",
            &settings("", "\"suppressApplicationTitle\": true"),
        );
        let user_change = settings(
            "",
            "\"suppressApplicationTitle\": true, \"tabTitle\": \"user choice\"",
        );
        let result = store
            .repair_with_before_revalidate(|target| {
                fs::write(target, &user_change).expect("user drift");
            })
            .expect("drift result");
        assert_eq!(result.state, TitleRemediationState::BlockedDrift);
        assert_eq!(
            fs::read_to_string(&path).expect("preserved user bytes"),
            user_change
        );
    }

    #[test]
    fn ambiguous_sources_and_profiles_fail_closed() {
        let root = temporary_root("ambiguous");
        fs::create_dir_all(&root).expect("root");
        let first = root.join("first.json");
        let second = root.join("second.json");
        fs::write(&first, settings("", "\"suppressApplicationTitle\": true")).expect("first");
        fs::write(&second, settings("", "\"suppressApplicationTitle\": true")).expect("second");
        let store = WindowsTerminalPolicyStore::new_for_testing(
            vec![
                SettingsCandidate::new(WindowsTerminalInstallation::Stable, first),
                SettingsCandidate::new(WindowsTerminalInstallation::Preview, second),
            ],
            root.join("state"),
            true,
            Some(GUID.to_owned()),
        );
        assert_eq!(
            store.inspect().settings_source,
            SettingsSourceResolution::Ambiguous
        );
        assert_eq!(
            store.repair().expect("ambiguous repair").state,
            TitleRemediationState::BlockedAmbiguous
        );
        let (profile_store, _) = fixture_store(
            "ambiguous-profile",
            &settings("", "\"suppressApplicationTitle\": true"),
        );
        let profile_store = WindowsTerminalPolicyStore::new_for_testing(
            profile_store.candidates.clone(),
            temporary_root("ambiguous-state"),
            true,
            Some("PowerShell".to_owned()),
        );
        assert_eq!(
            profile_store.inspect().active_profile_resolution,
            ActiveProfileResolution::Ambiguous
        );
        assert_eq!(
            profile_store
                .repair()
                .expect("ambiguous profile repair")
                .state,
            TitleRemediationState::BlockedAmbiguous
        );
    }

    #[test]
    fn jsonc_comments_trailing_commas_and_utf8_bom_are_preserved() {
        let source = format!(
            "\u{feff}{}",
            settings("", "\"suppressApplicationTitle\": true")
        );
        let (store, path) = fixture_store("jsonc", &source);
        store.repair().expect("repair");
        let bytes = fs::read(&path).expect("bytes");
        assert!(bytes.starts_with(&[0xef, 0xbb, 0xbf]));
        let output = String::from_utf8_lossy(&bytes);
        assert!(output.contains("// user comment"));
        assert!(output.contains("\"unknown\": { \"kept\": true }"));
        assert!(output.contains("\"suppressApplicationTitle\": false"));
    }

    #[test]
    fn malformed_and_oversized_inputs_never_offer_repair() {
        let (malformed, _) = fixture_store("malformed", "{ not jsonc }");
        assert_eq!(
            malformed.inspect().application_title_policy,
            ApplicationTitlePolicy::MalformedOrUnsafe
        );
        assert_eq!(
            malformed.repair().expect("malformed repair").state,
            TitleRemediationState::Unavailable
        );
        let oversized = " ".repeat(2 * 1024 * 1024 + 1);
        let (oversized, _) = fixture_store("oversized", &oversized);
        assert_eq!(
            oversized.inspect().application_title_policy,
            ApplicationTitlePolicy::MalformedOrUnsafe
        );
    }

    #[test]
    fn duplicate_target_properties_fail_closed_without_guessing_an_occurrence() {
        let duplicate = settings(
            "",
            "\"suppressApplicationTitle\": true, \"suppressApplicationTitle\": false",
        );
        let (store, _) = fixture_store("duplicate-target", &duplicate);
        let report = store.inspect();
        assert_eq!(
            report.application_title_policy,
            ApplicationTitlePolicy::MalformedOrUnsafe
        );
        assert_eq!(
            store.repair().expect("duplicate repair").state,
            TitleRemediationState::Unavailable
        );
    }
}
