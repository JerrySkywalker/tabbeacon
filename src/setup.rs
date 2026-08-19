//! Draft-first coordination for the lightweight guided setup flow.
//!
//! This module deliberately owns no persistent settings format and no Codex
//! configuration format. It keeps an in-memory [`SetupPlan`] until the caller
//! explicitly applies it, then delegates to the existing typed settings and
//! ownership-aware integration primitives.

use std::{env, fmt, path::PathBuf};

use crate::{
    interface_preferences::{
        InterfacePreferences, InterfacePreferencesConditionalOutcome, InterfacePreferencesError,
        InterfacePreferencesSnapshot, InterfacePreferencesSnapshotSaveOutcome,
        InterfacePreferencesStore,
    },
    providers::codex::{CodexDoctorReport, CodexHookProfile, DoctorStatus, SetupOutcome},
    settings::{
        ConditionalSaveOutcome, PresentationSettings, PresentationSettingsSnapshot,
        PresentationSettingsStore, SettingsError, SnapshotSaveOutcome,
    },
};

/// Read-only Windows Terminal availability classification for guided setup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsTerminalState {
    /// The current process is inside a Windows Terminal session.
    CurrentSession,
    /// Setup is not running inside a Windows Terminal session.
    NotCurrentSession,
}

impl WindowsTerminalState {
    /// Human-safe summary for the compact setup view.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::CurrentSession => "detected (current session)",
            Self::NotCurrentSession => "not current session",
        }
    }
}

/// Read-only classification of the existing Codex Hook integration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookSetupState {
    /// No valid `TabBeacon` ownership manifest was found.
    AbsentOrInvalid,
    /// Owned declarations are exact, current, trusted, and active.
    Current,
    /// Exact owned declarations are present but must be upgraded.
    UpgradeRequired,
    /// Declarations are present but the official Codex `/hooks` review is needed.
    ReviewRequired,
    /// An owned integration exists but has a separate actionable problem.
    AttentionRequired,
}

impl HookSetupState {
    /// Human-safe summary for the compact setup view.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::AbsentOrInvalid => "absent or invalid",
            Self::Current => "current",
            Self::UpgradeRequired => "upgrade required",
            Self::ReviewRequired => "review required",
            Self::AttentionRequired => "attention required",
        }
    }

    fn from_statuses(
        manifest: Option<DoctorStatus>,
        declarations: Option<DoctorStatus>,
        currentness: Option<DoctorStatus>,
        trust: Option<DoctorStatus>,
        overall: DoctorStatus,
    ) -> Self {
        if manifest != Some(DoctorStatus::Pass) {
            return Self::AbsentOrInvalid;
        }
        if declarations == Some(DoctorStatus::Pass) && currentness == Some(DoctorStatus::Fail) {
            return Self::UpgradeRequired;
        }
        if trust == Some(DoctorStatus::Warning) {
            return Self::ReviewRequired;
        }
        if overall == DoctorStatus::Pass {
            return Self::Current;
        }
        Self::AttentionRequired
    }
}

/// Non-sensitive environment information rendered before setup choices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupDiscovery {
    tabbeacon_version: String,
    binary_path: PathBuf,
    windows_terminal: WindowsTerminalState,
    codex_version: Option<String>,
    hook_profile: Option<&'static str>,
    profile_supported: bool,
    hooks: HookSetupState,
    doctor_status: DoctorStatus,
}

impl SetupDiscovery {
    /// Builds a typed discovery snapshot from the existing read-only doctor report.
    #[must_use]
    pub fn from_doctor(
        tabbeacon_version: impl Into<String>,
        binary_path: impl Into<PathBuf>,
        windows_terminal: WindowsTerminalState,
        report: &CodexDoctorReport,
    ) -> Self {
        Self {
            tabbeacon_version: tabbeacon_version.into(),
            binary_path: binary_path.into(),
            windows_terminal,
            codex_version: report.codex_version().map(ToOwned::to_owned),
            hook_profile: report.hook_profile().map(CodexHookProfile::id),
            profile_supported: report.profile_supported(),
            hooks: HookSetupState::from_statuses(
                report.check_status("ownership.manifest"),
                report.check_status("hooks.declarations"),
                report.check_status("hooks.currentness"),
                report.check_status("hooks.trust"),
                report.overall(),
            ),
            doctor_status: report.overall(),
        }
    }

    /// Product semantic version.
    #[must_use]
    pub fn tabbeacon_version(&self) -> &str {
        &self.tabbeacon_version
    }

    /// Current executable path, intentionally limited to `TabBeacon` itself.
    #[must_use]
    pub fn binary_path(&self) -> &std::path::Path {
        &self.binary_path
    }

    /// Windows Terminal availability classification.
    #[must_use]
    pub const fn windows_terminal(&self) -> WindowsTerminalState {
        self.windows_terminal
    }

    /// Detected Codex version when the bounded probe succeeds.
    #[must_use]
    pub fn codex_version(&self) -> Option<&str> {
        self.codex_version.as_deref()
    }

    /// Exact admitted Codex Hook profile identifier when known.
    #[must_use]
    pub const fn hook_profile(&self) -> Option<&'static str> {
        self.hook_profile
    }

    /// Whether the current Codex version matches a source-audited profile.
    #[must_use]
    pub const fn profile_supported(&self) -> bool {
        self.profile_supported
    }

    /// Existing hook declaration/trust state.
    #[must_use]
    pub const fn hooks(&self) -> HookSetupState {
        self.hooks
    }

    /// Aggregate read-only doctor result.
    #[must_use]
    pub const fn doctor_status(&self) -> DoctorStatus {
        self.doctor_status
    }
}

/// Explicit user decision at the end of guided setup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupDecision {
    /// Persist the typed draft and delegate to the existing setup engine.
    Apply,
    /// Discard the in-memory draft without persistent side effects.
    Cancel,
}

/// Typed, unpersisted setup choices and their read-only discovery context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupPlan {
    before: PresentationSettings,
    draft: PresentationSettings,
    discovery: SetupDiscovery,
}

impl SetupPlan {
    /// Starts with the current effective settings and a read-only discovery snapshot.
    #[must_use]
    pub fn new(before: PresentationSettings, discovery: SetupDiscovery) -> Self {
        Self {
            before,
            draft: before,
            discovery,
        }
    }

    /// Replaces the in-memory draft with one fully typed selection.
    #[must_use]
    pub fn with_draft(mut self, draft: PresentationSettings) -> Self {
        self.draft = draft;
        self
    }

    /// Original effective settings rendered to the user and bound to the snapshot.
    #[must_use]
    pub fn before(&self) -> PresentationSettings {
        self.before
    }

    /// Current in-memory candidate; reading it never persists anything.
    #[must_use]
    pub fn draft(&self) -> PresentationSettings {
        self.draft
    }

    /// Typed environment snapshot shown by the setup UI.
    #[must_use]
    pub fn discovery(&self) -> &SetupDiscovery {
        &self.discovery
    }

    /// Returns the exact settings to use for an uncommitted preview.
    #[must_use]
    pub fn preview_settings(&self) -> PresentationSettings {
        self.draft
    }

    /// Records an explicit cancellation without touching a store or provider.
    #[must_use]
    pub fn cancel(&self) -> SetupApplyResult {
        SetupApplyResult::Cancelled
    }

    /// Applies the draft, then delegates the provider step to the existing setup engine.
    ///
    /// The settings update is atomic and guarded against a concurrent `config`
    /// change. The closure is intentionally the only provider boundary: callers
    /// must pass the existing ownership-aware setup implementation instead of
    /// duplicating Hook/config behavior in the wizard.
    ///
    /// # Errors
    ///
    /// Returns a [`SettingsError`] when the settings snapshot cannot be read or
    /// the requested atomic update cannot be persisted. Provider failures are
    /// represented by [`SetupApplyResult::SetupFailed`] after a best-effort
    /// settings rollback.
    pub fn apply(
        &self,
        store: &PresentationSettingsStore,
        snapshot: &PresentationSettingsSnapshot,
        setup: impl FnOnce(bool) -> Result<SetupOutcome, String>,
    ) -> Result<SetupApplyResult, SettingsError> {
        if snapshot.settings() != self.before {
            return Ok(SetupApplyResult::SettingsConflict);
        }
        let receipt = match store.save_snapshot_if_unchanged(snapshot, self.draft)? {
            SnapshotSaveOutcome::Conflict => return Ok(SetupApplyResult::SettingsConflict),
            SnapshotSaveOutcome::Saved(receipt) => receipt,
        };
        match setup(self.draft.title().owns_tabbeacon_title()) {
            Ok(outcome) => Ok(SetupApplyResult::Applied(outcome)),
            Err(reason) => {
                let settings_restored = matches!(
                    store.restore_snapshot_if_unchanged(&receipt, snapshot),
                    Ok(ConditionalSaveOutcome::Saved)
                );
                Ok(SetupApplyResult::SetupFailed {
                    reason,
                    settings_restored,
                })
            }
        }
    }
}

/// Error returned while composing the separately owned presentation and
/// Interface preference stores for guided setup.
#[derive(Debug)]
pub enum GuidedSetupError {
    /// The existing presentation settings store could not complete a guarded operation.
    Settings(SettingsError),
    /// The separate user-local Interface store could not complete a guarded operation.
    Interface(InterfacePreferencesError),
}

impl fmt::Display for GuidedSetupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Settings(error) => write!(formatter, "presentation settings: {error}"),
            Self::Interface(error) => write!(formatter, "Interface preferences: {error}"),
        }
    }
}

impl std::error::Error for GuidedSetupError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Settings(error) => Some(error),
            Self::Interface(error) => Some(error),
        }
    }
}

impl From<SettingsError> for GuidedSetupError {
    fn from(error: SettingsError) -> Self {
        Self::Settings(error)
    }
}

impl From<InterfacePreferencesError> for GuidedSetupError {
    fn from(error: InterfacePreferencesError) -> Self {
        Self::Interface(error)
    }
}

/// One composite, still-unpersisted guided setup draft.
///
/// Presentation and Interface preferences deliberately remain separate files.
/// This coordinator applies each only through its byte-exact snapshot API and
/// conditionally compensates the first write if a later step fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuidedSetupPlan {
    before: PresentationSettings,
    draft: PresentationSettings,
    interface_before: InterfacePreferences,
    interface_draft: InterfacePreferences,
    discovery: SetupDiscovery,
}

impl GuidedSetupPlan {
    /// Starts a composite draft from read-only snapshots.
    #[must_use]
    pub fn new(
        before: PresentationSettings,
        interface_before: InterfacePreferences,
        discovery: SetupDiscovery,
    ) -> Self {
        Self {
            before,
            draft: before,
            interface_before,
            interface_draft: interface_before,
            discovery,
        }
    }

    /// Replaces the presentation portion of the in-memory draft.
    #[must_use]
    pub fn with_presentation_draft(mut self, draft: PresentationSettings) -> Self {
        self.draft = draft;
        self
    }

    /// Replaces the Interface portion of the in-memory draft.
    #[must_use]
    pub fn with_interface_draft(mut self, draft: InterfacePreferences) -> Self {
        self.interface_draft = draft;
        self
    }

    /// Original presentation settings bound to the read-only snapshot.
    #[must_use]
    pub const fn before(&self) -> PresentationSettings {
        self.before
    }

    /// Original Interface preferences bound to the read-only snapshot.
    #[must_use]
    pub const fn interface_before(&self) -> InterfacePreferences {
        self.interface_before
    }

    /// Staged presentation settings.
    #[must_use]
    pub const fn draft(&self) -> PresentationSettings {
        self.draft
    }

    /// Staged Interface preferences.
    #[must_use]
    pub const fn interface_draft(&self) -> InterfacePreferences {
        self.interface_draft
    }

    /// Typed discovery data rendered by the guided setup surface.
    #[must_use]
    pub fn discovery(&self) -> &SetupDiscovery {
        &self.discovery
    }

    /// Returns the uncommitted presentation settings to preview.
    #[must_use]
    pub const fn preview_settings(&self) -> PresentationSettings {
        self.draft
    }

    /// Cancels without creating a lock, file, or user preference directory.
    #[must_use]
    pub const fn cancel(&self) -> GuidedSetupApplyResult {
        GuidedSetupApplyResult::Cancelled
    }

    /// Applies both drafts with per-store atomic compare-and-save semantics.
    ///
    /// There is intentionally no false claim of a cross-file transaction. If
    /// the second guarded write or provider step fails, only the exact first
    /// write is conditionally restored; concurrent edits always win.
    ///
    /// # Errors
    ///
    /// Returns a typed store error when either snapshot-guarded operation
    /// cannot be safely read, written, or restored.
    pub fn apply(
        &self,
        settings_store: &PresentationSettingsStore,
        settings_snapshot: &PresentationSettingsSnapshot,
        interface_store: &InterfacePreferencesStore,
        interface_snapshot: &InterfacePreferencesSnapshot,
        setup: impl FnOnce(bool) -> Result<SetupOutcome, String>,
    ) -> Result<GuidedSetupApplyResult, GuidedSetupError> {
        if settings_snapshot.settings() != self.before {
            return Ok(GuidedSetupApplyResult::SettingsConflict);
        }
        if interface_snapshot.preferences() != self.interface_before {
            return Ok(GuidedSetupApplyResult::InterfaceConflict);
        }

        let interface_receipt = if self.interface_draft == self.interface_before {
            None
        } else {
            match interface_store
                .save_snapshot_if_unchanged(interface_snapshot, self.interface_draft)?
            {
                InterfacePreferencesSnapshotSaveOutcome::Saved(receipt) => Some(receipt),
                InterfacePreferencesSnapshotSaveOutcome::Conflict => {
                    return Ok(GuidedSetupApplyResult::InterfaceConflict);
                }
            }
        };

        let settings_receipt = if self.draft == self.before {
            None
        } else {
            let settings_save =
                settings_store.save_snapshot_if_unchanged(settings_snapshot, self.draft);
            match settings_save {
                Ok(SnapshotSaveOutcome::Saved(receipt)) => Some(receipt),
                Ok(SnapshotSaveOutcome::Conflict) => {
                    if let Some(receipt) = interface_receipt.as_ref() {
                        let _ = interface_store
                            .restore_snapshot_if_unchanged(receipt, interface_snapshot);
                    }
                    return Ok(GuidedSetupApplyResult::SettingsConflict);
                }
                Err(error) => {
                    // The Interface write is already complete at this point.  A
                    // failed second write must not silently strand the first
                    // draft; its receipt is the only authority to compensate it.
                    if let Some(receipt) = interface_receipt.as_ref() {
                        let _ = interface_store
                            .restore_snapshot_if_unchanged(receipt, interface_snapshot);
                    }
                    return Err(error.into());
                }
            }
        };

        match setup(self.draft.title().owns_tabbeacon_title()) {
            Ok(outcome) => Ok(GuidedSetupApplyResult::Applied(outcome)),
            Err(reason) => {
                let settings_restored = settings_receipt.as_ref().is_none_or(|receipt| {
                    matches!(
                        settings_store.restore_snapshot_if_unchanged(receipt, settings_snapshot),
                        Ok(ConditionalSaveOutcome::Saved)
                    )
                });
                let interface_restored = interface_receipt.as_ref().is_none_or(|receipt| {
                    matches!(
                        interface_store.restore_snapshot_if_unchanged(receipt, interface_snapshot),
                        Ok(InterfacePreferencesConditionalOutcome::Saved)
                    )
                });
                Ok(GuidedSetupApplyResult::SetupFailed {
                    reason,
                    settings_restored,
                    interface_restored,
                })
            }
        }
    }
}

/// Result of applying or cancelling the combined guided setup draft.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuidedSetupApplyResult {
    /// The user discarded the in-memory draft.
    Cancelled,
    /// Both guarded writes (if changed) and the existing provider setup completed.
    Applied(SetupOutcome),
    /// A presentation settings document changed after its snapshot.
    SettingsConflict,
    /// An Interface preferences document changed after its snapshot.
    InterfaceConflict,
    /// Provider setup failed after any changed documents were conditionally restored.
    SetupFailed {
        /// Safe provider/setup failure reason.
        reason: String,
        /// Whether the exact original presentation document was restored.
        settings_restored: bool,
        /// Whether the exact original Interface document was restored.
        interface_restored: bool,
    },
}

/// Result of applying or cancelling a guided setup plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetupApplyResult {
    /// The draft was intentionally discarded with no persistent operation.
    Cancelled,
    /// Settings were persisted and existing setup completed.
    Applied(SetupOutcome),
    /// A concurrent settings update prevented a stale wizard overwrite.
    SettingsConflict,
    /// Provider setup failed after settings were written.
    SetupFailed {
        /// Safe, human-readable setup failure.
        reason: String,
        /// Whether the pre-wizard settings were restored atomically.
        settings_restored: bool,
    },
}

/// Performs the bounded, read-only Windows Terminal presence probe used by setup.
#[must_use]
pub fn detect_windows_terminal() -> WindowsTerminalState {
    let current_session = env::var_os("WT_SESSION").is_some_and(|value| !value.is_empty());
    windows_terminal_state(current_session)
}

fn windows_terminal_state(current_session: bool) -> WindowsTerminalState {
    if current_session {
        WindowsTerminalState::CurrentSession
    } else {
        WindowsTerminalState::NotCurrentSession
    }
}

#[cfg(test)]
mod tests {
    use std::{
        cell::Cell,
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;
    use crate::interface_preferences::{HumanColor, InterfaceLanguage, InterfacePreferencesStore};
    use crate::settings::{
        ActivityMode, PresentationTheme, SpinnerPreset, TabColorMode, TitleMode,
    };

    fn temporary_config(name: &str) -> std::path::PathBuf {
        std::env::temp_dir()
            .join(format!(
                "tabbeacon-setup-{name}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("clock is after Unix epoch")
                    .as_nanos()
            ))
            .join("state")
            .join("config.toml")
    }

    fn discovery() -> SetupDiscovery {
        SetupDiscovery {
            tabbeacon_version: env!("CARGO_PKG_VERSION").to_owned(),
            binary_path: "C:\\TabBeacon\\tabbeacon.exe".into(),
            windows_terminal: WindowsTerminalState::CurrentSession,
            codex_version: Some("0.147.0".to_owned()),
            hook_profile: Some("codex-hooks-rust-v0.147.0"),
            profile_supported: true,
            hooks: HookSetupState::ReviewRequired,
            doctor_status: DoctorStatus::Warning,
        }
    }

    #[test]
    fn preview_and_cancel_preserve_an_absent_settings_root() {
        let path = temporary_config("cancel");
        let root = path
            .parent()
            .expect("config parent")
            .parent()
            .expect("state root");
        let store = PresentationSettingsStore::new(&path);
        let snapshot = store
            .snapshot_read_only()
            .expect("absent settings snapshot");
        let draft = PresentationSettings::preset("full").expect("known preset");
        let plan = SetupPlan::new(snapshot.settings(), discovery()).with_draft(draft);

        assert_eq!(plan.preview_settings(), draft);
        assert_eq!(plan.cancel(), SetupApplyResult::Cancelled);
        assert!(!root.exists(), "cancel must not create settings or a lock");
    }

    #[test]
    fn apply_persists_the_typed_draft_and_reuses_title_ownership() {
        let path = temporary_config("apply");
        let store = PresentationSettingsStore::new(&path);
        let snapshot = store
            .snapshot_read_only()
            .expect("absent settings snapshot");
        let draft = PresentationSettings::new(
            TitleMode::Native,
            TabColorMode::Native,
            ActivityMode::Both,
            SpinnerPreset::Braille,
            PresentationTheme::Classic,
        );
        let plan = SetupPlan::new(snapshot.settings(), discovery()).with_draft(draft);
        let requested_title_ownership = Cell::new(None);

        let result = plan
            .apply(&store, &snapshot, |owns_title| {
                requested_title_ownership.set(Some(owns_title));
                Ok(SetupOutcome::AlreadyInstalled)
            })
            .expect("settings save succeeds");

        assert_eq!(
            result,
            SetupApplyResult::Applied(SetupOutcome::AlreadyInstalled)
        );
        assert_eq!(requested_title_ownership.get(), Some(false));
        assert_eq!(store.load().expect("persisted settings read"), draft);
        fs::remove_dir_all(
            path.parent()
                .expect("config parent")
                .parent()
                .expect("state root"),
        )
        .expect("fixture root removes");
    }

    #[test]
    fn failed_provider_setup_restores_the_pre_wizard_settings() {
        let path = temporary_config("restore");
        let store = PresentationSettingsStore::new(&path);
        let before = PresentationSettings::default();
        store.save(before).expect("baseline settings save");
        let snapshot = store
            .snapshot_read_only()
            .expect("baseline settings snapshot");
        let plan = SetupPlan::new(before, discovery())
            .with_draft(PresentationSettings::preset("native").expect("known preset"));

        let result = plan
            .apply(&store, &snapshot, |_| {
                Err("ownership preflight failed".to_owned())
            })
            .expect("settings save and compensation complete");

        assert_eq!(
            result,
            SetupApplyResult::SetupFailed {
                reason: "ownership preflight failed".to_owned(),
                settings_restored: true,
            }
        );
        assert_eq!(store.load().expect("restored settings read"), before);
        fs::remove_dir_all(
            path.parent()
                .expect("config parent")
                .parent()
                .expect("state root"),
        )
        .expect("fixture root removes");
    }

    #[test]
    fn failed_provider_setup_restores_an_absent_settings_document() {
        let path = temporary_config("restore-absent");
        let root = path
            .parent()
            .expect("config parent")
            .parent()
            .expect("state root");
        let store = PresentationSettingsStore::new(&path);
        let snapshot = store
            .snapshot_read_only()
            .expect("absent settings snapshot");
        assert!(snapshot.is_absent());
        let plan = SetupPlan::new(snapshot.settings(), discovery())
            .with_draft(PresentationSettings::preset("native").expect("known preset"));

        let result = plan
            .apply(&store, &snapshot, |_| {
                Err("ownership preflight failed".to_owned())
            })
            .expect("settings save and compensation complete");

        assert_eq!(
            result,
            SetupApplyResult::SetupFailed {
                reason: "ownership preflight failed".to_owned(),
                settings_restored: true,
            }
        );
        assert!(
            !path.exists(),
            "rollback must restore an originally absent settings document"
        );
        fs::remove_dir_all(root).expect("fixture root removes");
    }

    #[test]
    fn failed_provider_setup_restores_the_original_document_bytes() {
        let path = temporary_config("restore-bytes");
        let parent = path.parent().expect("config parent");
        fs::create_dir_all(parent).expect("config parent creates");
        let original = br#"# Preserve comments and unknown user settings.
[custom]
value = "keep"

[presentation]
title = "tabbeacon"
tab_color = "tabbeacon"
activity = "title-indicator"
spinner = "codex"
theme = "muted-dark"
"#;
        fs::write(&path, original).expect("baseline document writes");
        let store = PresentationSettingsStore::new(&path);
        let snapshot = store
            .snapshot_read_only()
            .expect("baseline settings snapshot");
        let plan = SetupPlan::new(snapshot.settings(), discovery())
            .with_draft(PresentationSettings::preset("native").expect("known preset"));

        let result = plan
            .apply(&store, &snapshot, |_| {
                Err("ownership preflight failed".to_owned())
            })
            .expect("settings save and compensation complete");

        assert_eq!(
            result,
            SetupApplyResult::SetupFailed {
                reason: "ownership preflight failed".to_owned(),
                settings_restored: true,
            }
        );
        assert_eq!(
            fs::read(&path).expect("restored document reads"),
            original,
            "rollback must restore comments and unknown user configuration exactly"
        );
        fs::remove_dir_all(parent.parent().expect("state root")).expect("fixture root removes");
    }

    #[test]
    fn failed_provider_setup_preserves_a_concurrent_settings_update() {
        let path = temporary_config("restore-concurrent");
        let store = PresentationSettingsStore::new(&path);
        let before = PresentationSettings::default();
        store.save(before).expect("baseline settings save");
        let snapshot = store
            .snapshot_read_only()
            .expect("baseline settings snapshot");
        let plan = SetupPlan::new(before, discovery())
            .with_draft(PresentationSettings::preset("native").expect("known preset"));
        let concurrent = before.with_theme(PresentationTheme::Classic);

        let result = plan
            .apply(&store, &snapshot, |_| {
                store.save(concurrent).map_err(|error| error.to_string())?;
                Err("ownership preflight failed".to_owned())
            })
            .expect("provider failure is a normal result");

        assert_eq!(
            result,
            SetupApplyResult::SetupFailed {
                reason: "ownership preflight failed".to_owned(),
                settings_restored: false,
            }
        );
        assert_eq!(
            store.load().expect("concurrent settings survive rollback"),
            concurrent
        );
        fs::remove_dir_all(
            path.parent()
                .expect("config parent")
                .parent()
                .expect("state root"),
        )
        .expect("fixture root removes");
    }

    #[test]
    fn failed_provider_setup_preserves_a_same_settings_raw_document_update() {
        let path = temporary_config("restore-raw-concurrent");
        let store = PresentationSettingsStore::new(&path);
        let before = PresentationSettings::default();
        store.save(before).expect("baseline settings save");
        let snapshot = store
            .snapshot_read_only()
            .expect("baseline settings snapshot");
        let plan = SetupPlan::new(before, discovery())
            .with_draft(PresentationSettings::preset("native").expect("known preset"));

        let result = plan
            .apply(&store, &snapshot, |_| {
                let mut concurrent = fs::read(&path).expect("guided document reads");
                concurrent.extend_from_slice(b"# concurrent unknown setting preservation\n");
                fs::write(&path, &concurrent).expect("concurrent document writes");
                Err("ownership preflight failed".to_owned())
            })
            .expect("provider failure is a normal result");

        assert_eq!(
            result,
            SetupApplyResult::SetupFailed {
                reason: "ownership preflight failed".to_owned(),
                settings_restored: false,
            }
        );
        assert!(
            fs::read(&path)
                .expect("concurrent document reads")
                .ends_with(b"# concurrent unknown setting preservation\n"),
            "rollback must not overwrite a same-effective document changed by another writer"
        );
        fs::remove_dir_all(
            path.parent()
                .expect("config parent")
                .parent()
                .expect("state root"),
        )
        .expect("fixture root removes");
    }

    #[test]
    fn stale_plan_does_not_overwrite_a_concurrent_settings_change() {
        let path = temporary_config("conflict");
        let store = PresentationSettingsStore::new(&path);
        let before = PresentationSettings::default();
        store.save(before).expect("baseline settings save");
        let snapshot = store
            .snapshot_read_only()
            .expect("baseline settings snapshot");
        let plan = SetupPlan::new(before, discovery())
            .with_draft(PresentationSettings::preset("native").expect("known preset"));
        let concurrent = before.with_theme(PresentationTheme::Classic);
        store.save(concurrent).expect("concurrent settings save");
        let setup_called = Cell::new(false);

        let result = plan
            .apply(&store, &snapshot, |_| {
                setup_called.set(true);
                Ok(SetupOutcome::AlreadyInstalled)
            })
            .expect("conflict is a normal result");

        assert_eq!(result, SetupApplyResult::SettingsConflict);
        assert!(!setup_called.get());
        assert_eq!(store.load().expect("concurrent settings read"), concurrent);
        fs::remove_dir_all(
            path.parent()
                .expect("config parent")
                .parent()
                .expect("state root"),
        )
        .expect("fixture root removes");
    }

    #[test]
    fn hook_state_is_derived_from_typed_check_statuses() {
        assert_eq!(
            HookSetupState::from_statuses(
                Some(DoctorStatus::Fail),
                None,
                None,
                None,
                DoctorStatus::Fail,
            ),
            HookSetupState::AbsentOrInvalid
        );
        assert_eq!(
            HookSetupState::from_statuses(
                Some(DoctorStatus::Pass),
                Some(DoctorStatus::Pass),
                Some(DoctorStatus::Fail),
                Some(DoctorStatus::Pass),
                DoctorStatus::Fail,
            ),
            HookSetupState::UpgradeRequired
        );
        assert_eq!(
            HookSetupState::from_statuses(
                Some(DoctorStatus::Pass),
                Some(DoctorStatus::Pass),
                Some(DoctorStatus::Pass),
                Some(DoctorStatus::Warning),
                DoctorStatus::Warning,
            ),
            HookSetupState::ReviewRequired
        );
        assert_eq!(
            HookSetupState::from_statuses(
                Some(DoctorStatus::Pass),
                Some(DoctorStatus::Pass),
                Some(DoctorStatus::Pass),
                Some(DoctorStatus::Pass),
                DoctorStatus::Pass,
            ),
            HookSetupState::Current
        );
    }

    #[test]
    fn terminal_detection_is_current_session_only_and_never_launches_wt() {
        assert_eq!(
            windows_terminal_state(true),
            WindowsTerminalState::CurrentSession
        );
        assert_eq!(
            windows_terminal_state(false),
            WindowsTerminalState::NotCurrentSession
        );
    }

    #[test]
    fn composite_cancel_preserves_absent_presentation_and_interface_roots() {
        let config = temporary_config("composite-cancel");
        let interface = config.with_file_name("interface.toml");
        let root = config
            .parent()
            .expect("state parent")
            .parent()
            .expect("state root");
        let settings_store = PresentationSettingsStore::new(&config);
        let interface_store = InterfacePreferencesStore::new(&interface);
        let settings_snapshot = settings_store
            .snapshot_read_only()
            .expect("settings snapshot");
        let interface_snapshot = interface_store
            .snapshot_read_only()
            .expect("Interface snapshot");
        let plan = GuidedSetupPlan::new(
            settings_snapshot.settings(),
            interface_snapshot.preferences(),
            discovery(),
        )
        .with_interface_draft(
            interface_snapshot
                .preferences()
                .with_language(InterfaceLanguage::ZhCn),
        );

        assert_eq!(plan.cancel(), GuidedSetupApplyResult::Cancelled);
        assert!(
            !root.exists(),
            "cancel must not create a state root or lock"
        );
    }

    #[test]
    fn composite_interface_only_apply_leaves_presentation_document_absent() {
        let config = temporary_config("composite-interface-only");
        let interface = config.with_file_name("interface.toml");
        let root = config
            .parent()
            .expect("state parent")
            .parent()
            .expect("state root");
        let settings_store = PresentationSettingsStore::new(&config);
        let interface_store = InterfacePreferencesStore::new(&interface);
        let settings_snapshot = settings_store
            .snapshot_read_only()
            .expect("settings snapshot");
        let interface_snapshot = interface_store
            .snapshot_read_only()
            .expect("Interface snapshot");
        let draft = interface_snapshot
            .preferences()
            .with_language(InterfaceLanguage::ZhCn)
            .with_color(HumanColor::Never);
        let plan = GuidedSetupPlan::new(
            settings_snapshot.settings(),
            interface_snapshot.preferences(),
            discovery(),
        )
        .with_interface_draft(draft);

        assert_eq!(
            plan.apply(
                &settings_store,
                &settings_snapshot,
                &interface_store,
                &interface_snapshot,
                |_| Ok(SetupOutcome::AlreadyInstalled),
            )
            .expect("Interface-only apply"),
            GuidedSetupApplyResult::Applied(SetupOutcome::AlreadyInstalled)
        );
        assert!(
            !config.exists(),
            "an Interface-only setup Apply must not create presentation config.toml"
        );
        assert_eq!(
            interface_store.load_read_only().expect("Interface reread"),
            draft
        );
        fs::remove_dir_all(root).expect("state root removes");
    }

    #[test]
    fn composite_provider_failure_restores_both_originally_absent_documents() {
        let config = temporary_config("composite-restore");
        let interface = config.with_file_name("interface.toml");
        let root = config
            .parent()
            .expect("state parent")
            .parent()
            .expect("state root");
        let settings_store = PresentationSettingsStore::new(&config);
        let interface_store = InterfacePreferencesStore::new(&interface);
        let settings_snapshot = settings_store
            .snapshot_read_only()
            .expect("settings snapshot");
        let interface_snapshot = interface_store
            .snapshot_read_only()
            .expect("Interface snapshot");
        let plan = GuidedSetupPlan::new(
            settings_snapshot.settings(),
            interface_snapshot.preferences(),
            discovery(),
        )
        .with_presentation_draft(PresentationSettings::preset("native").expect("preset"))
        .with_interface_draft(
            interface_snapshot
                .preferences()
                .with_language(InterfaceLanguage::ZhCn),
        );

        assert!(matches!(
            plan.apply(
                &settings_store,
                &settings_snapshot,
                &interface_store,
                &interface_snapshot,
                |_| Err("controlled provider failure".to_owned()),
            )
            .expect("guarded writes and rollback"),
            GuidedSetupApplyResult::SetupFailed {
                settings_restored: true,
                interface_restored: true,
                ..
            }
        ));
        assert!(!config.exists());
        assert!(!interface.exists());
        fs::remove_dir_all(root).expect("state root removes");
    }

    #[test]
    fn composite_second_store_error_restores_the_interface_write() {
        let config = temporary_config("composite-second-store-error");
        let interface = config.with_file_name("interface.toml");
        let root = config
            .parent()
            .expect("state parent")
            .parent()
            .expect("state root");
        let settings_store = PresentationSettingsStore::new(&config);
        let interface_store = InterfacePreferencesStore::new(&interface);
        let settings_snapshot = settings_store
            .snapshot_read_only()
            .expect("settings snapshot");
        let interface_snapshot = interface_store
            .snapshot_read_only()
            .expect("Interface snapshot");
        let plan = GuidedSetupPlan::new(
            settings_snapshot.settings(),
            interface_snapshot.preferences(),
            discovery(),
        )
        .with_presentation_draft(PresentationSettings::preset("native").expect("preset"))
        .with_interface_draft(
            interface_snapshot
                .preferences()
                .with_language(InterfaceLanguage::ZhCn),
        );

        // Snapshot first, then introduce a path that can be read but cannot be
        // atomically replaced as a settings document. The Interface write is
        // earlier in the prescribed order and therefore must be compensated.
        fs::create_dir_all(&config).expect("settings path becomes a directory");
        assert!(
            plan.apply(
                &settings_store,
                &settings_snapshot,
                &interface_store,
                &interface_snapshot,
                |_| Ok(SetupOutcome::AlreadyInstalled),
            )
            .is_err()
        );
        assert!(
            !interface.exists(),
            "a failed second store write must restore the Interface document"
        );
        fs::remove_dir_all(root).expect("state root removes");
    }
}
