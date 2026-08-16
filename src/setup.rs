//! Draft-first coordination for the lightweight guided setup flow.
//!
//! This module deliberately owns no persistent settings format and no Codex
//! configuration format. It keeps an in-memory [`SetupPlan`] until the caller
//! explicitly applies it, then delegates to the existing typed settings and
//! ownership-aware integration primitives.

use std::{env, path::PathBuf, process::Command};

use crate::{
    providers::codex::{CodexDoctorReport, DoctorStatus, SetupOutcome},
    settings::{
        ConditionalSaveOutcome, PresentationSettings, PresentationSettingsStore, SettingsError,
    },
};

/// Read-only Windows Terminal availability classification for guided setup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsTerminalState {
    /// The current process is inside a Windows Terminal session.
    CurrentSession,
    /// Windows Terminal was found, but the current process is not inside it.
    Detected,
    /// No bounded Windows Terminal probe succeeded.
    Unavailable,
}

impl WindowsTerminalState {
    /// Human-safe summary for the compact setup view.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::CurrentSession => "detected (current session)",
            Self::Detected => "detected",
            Self::Unavailable => "unavailable",
        }
    }
}

/// Read-only classification of the existing Codex Hook integration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookSetupState {
    /// No valid TabBeacon ownership manifest was found.
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
        if declarations == Some(DoctorStatus::Pass)
            && currentness == Some(DoctorStatus::Fail)
        {
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
        let check_status = |id| report.check(id).map(|check| check.status());
        Self {
            tabbeacon_version: tabbeacon_version.into(),
            binary_path: binary_path.into(),
            windows_terminal,
            codex_version: report.codex_version().map(ToOwned::to_owned),
            hook_profile: report.hook_profile().map(|profile| profile.id()),
            profile_supported: report.profile_supported(),
            hooks: HookSetupState::from_statuses(
                check_status("ownership.manifest"),
                check_status("hooks.declarations"),
                check_status("hooks.currentness"),
                check_status("hooks.trust"),
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

    /// Current executable path, intentionally limited to TabBeacon itself.
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

    /// Original effective settings used for conflict detection and compensation.
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
    pub fn apply(
        &self,
        store: &PresentationSettingsStore,
        setup: impl FnOnce(bool) -> Result<SetupOutcome, String>,
    ) -> Result<SetupApplyResult, SettingsError> {
        match store.save_if_unchanged(self.before, self.draft)? {
            ConditionalSaveOutcome::Conflict => return Ok(SetupApplyResult::SettingsConflict),
            ConditionalSaveOutcome::Saved => {}
        }
        match setup(self.draft.title().owns_tabbeacon_title()) {
            Ok(outcome) => Ok(SetupApplyResult::Applied(outcome)),
            Err(reason) => {
                let settings_restored = matches!(
                    store.save_if_unchanged(self.draft, self.before),
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
    let detected = Command::new("wt.exe")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success());
    windows_terminal_state(current_session, detected)
}

fn windows_terminal_state(current_session: bool, detected: bool) -> WindowsTerminalState {
    if current_session {
        WindowsTerminalState::CurrentSession
    } else if detected {
        WindowsTerminalState::Detected
    } else {
        WindowsTerminalState::Unavailable
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
    use crate::{
        settings::{ActivityMode, PresentationTheme, SpinnerPreset, TabColorMode, TitleMode},
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
            tabbeacon_version: "0.1.1".to_owned(),
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
        let root = path.parent().expect("config parent").parent().expect("state root");
        let draft = PresentationSettings::preset("full").expect("known preset");
        let plan = SetupPlan::new(PresentationSettings::default(), discovery()).with_draft(draft);

        assert_eq!(plan.preview_settings(), draft);
        assert_eq!(plan.cancel(), SetupApplyResult::Cancelled);
        assert!(!root.exists(), "cancel must not create settings or a lock");
    }

    #[test]
    fn apply_persists_the_typed_draft_and_reuses_title_ownership() {
        let path = temporary_config("apply");
        let store = PresentationSettingsStore::new(&path);
        let draft = PresentationSettings::new(
            TitleMode::Native,
            TabColorMode::Native,
            ActivityMode::Both,
            SpinnerPreset::Braille,
            PresentationTheme::Classic,
        );
        let plan = SetupPlan::new(PresentationSettings::default(), discovery()).with_draft(draft);
        let requested_title_ownership = Cell::new(None);

        let result = plan
            .apply(&store, |owns_title| {
                requested_title_ownership.set(Some(owns_title));
                Ok(SetupOutcome::AlreadyInstalled)
            })
            .expect("settings save succeeds");

        assert_eq!(result, SetupApplyResult::Applied(SetupOutcome::AlreadyInstalled));
        assert_eq!(requested_title_ownership.get(), Some(false));
        assert_eq!(store.load().expect("persisted settings read"), draft);
        fs::remove_dir_all(path.parent().expect("config parent").parent().expect("state root"))
            .expect("fixture root removes");
    }

    #[test]
    fn failed_provider_setup_restores_the_pre_wizard_settings() {
        let path = temporary_config("restore");
        let store = PresentationSettingsStore::new(&path);
        let before = PresentationSettings::default();
        store.save(before).expect("baseline settings save");
        let plan = SetupPlan::new(before, discovery()).with_draft(
            PresentationSettings::preset("native").expect("known preset"),
        );

        let result = plan
            .apply(&store, |_| Err("ownership preflight failed".to_owned()))
            .expect("settings save and compensation complete");

        assert_eq!(
            result,
            SetupApplyResult::SetupFailed {
                reason: "ownership preflight failed".to_owned(),
                settings_restored: true,
            }
        );
        assert_eq!(store.load().expect("restored settings read"), before);
        fs::remove_dir_all(path.parent().expect("config parent").parent().expect("state root"))
            .expect("fixture root removes");
    }

    #[test]
    fn stale_plan_does_not_overwrite_a_concurrent_settings_change() {
        let path = temporary_config("conflict");
        let store = PresentationSettingsStore::new(&path);
        let before = PresentationSettings::default();
        store.save(before).expect("baseline settings save");
        let plan = SetupPlan::new(before, discovery()).with_draft(
            PresentationSettings::preset("native").expect("known preset"),
        );
        let concurrent = before.with_theme(PresentationTheme::Classic);
        store.save(concurrent).expect("concurrent settings save");
        let setup_called = Cell::new(false);

        let result = plan
            .apply(&store, |_| {
                setup_called.set(true);
                Ok(SetupOutcome::AlreadyInstalled)
            })
            .expect("conflict is a normal result");

        assert_eq!(result, SetupApplyResult::SettingsConflict);
        assert!(!setup_called.get());
        assert_eq!(store.load().expect("concurrent settings read"), concurrent);
        fs::remove_dir_all(path.parent().expect("config parent").parent().expect("state root"))
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
    fn terminal_detection_distinguishes_current_and_unavailable_contexts() {
        assert_eq!(
            windows_terminal_state(true, false),
            WindowsTerminalState::CurrentSession
        );
        assert_eq!(
            windows_terminal_state(false, true),
            WindowsTerminalState::Detected
        );
        assert_eq!(
            windows_terminal_state(false, false),
            WindowsTerminalState::Unavailable
        );
    }
}
