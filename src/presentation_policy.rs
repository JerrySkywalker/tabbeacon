//! Pure per-provider presentation resolution. A saved preference is not proof
//! that an integration is installed, trusted, or applied to a running session.

use crate::settings::{ActivityMode, PresentationSettings, TabColorMode, TitleMode};

/// Admitted CLI target. This enum selects product policy, never a launcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CliTarget {
    Codex,
    Agy,
    Cursor,
}

impl CliTarget {
    /// Stable, bounded configuration key.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Agy => "agy",
            Self::Cursor => "cursor",
        }
    }

    /// Parses only the three providers admitted to this release train.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "codex" => Some(Self::Codex),
            "agy" => Some(Self::Agy),
            "cursor" => Some(Self::Cursor),
            _ => None,
        }
    }
}

/// A partial provider preference; absent fields inherit global defaults.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PresentationOverride {
    pub title: Option<TitleMode>,
    pub tab_color: Option<TabColorMode>,
    pub activity: Option<ActivityMode>,
}

/// User-facing channel combination. `Custom` leaves the partial fields alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentationMode {
    FullTakeover,
    TitleOnly,
    ColorOnly,
    PreserveNative,
    Custom,
}

impl PresentationOverride {
    /// Applies a mode without copying any preference from another CLI.
    #[must_use]
    pub const fn with_mode(mut self, mode: PresentationMode) -> Self {
        match mode {
            PresentationMode::FullTakeover => {
                self.title = Some(TitleMode::TabBeacon);
                self.tab_color = Some(TabColorMode::TabBeacon);
                // Retain the user's existing activity preference.
            }
            PresentationMode::TitleOnly => {
                self.title = Some(TitleMode::TabBeacon);
                self.tab_color = Some(TabColorMode::Off);
                self.activity = Some(ActivityMode::TitleIndicator);
            }
            PresentationMode::ColorOnly => {
                self.title = Some(TitleMode::Native);
                self.tab_color = Some(TabColorMode::TabBeacon);
                self.activity = Some(ActivityMode::Off);
            }
            PresentationMode::PreserveNative => {
                self.title = Some(TitleMode::Native);
                self.tab_color = Some(TabColorMode::Native);
                self.activity = Some(ActivityMode::Native);
            }
            PresentationMode::Custom => {}
        }
        self
    }
}

/// Origin of one requested channel value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreferenceOrigin {
    GlobalDefault,
    ProviderOverride,
}

/// Positively established presentation channels for one provider and host.
#[allow(clippy::struct_excessive_bools)] // Four independent output channels are intentional.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresentationCapabilities {
    pub title: bool,
    pub color: bool,
    pub title_activity: bool,
    pub progress: bool,
}

impl PresentationCapabilities {
    /// Existing Codex channel boundary. Actual application still needs trust.
    pub const CODEX: Self = Self {
        title: true,
        color: true,
        title_activity: true,
        progress: true,
    };
    /// Agy's admitted profile is title-only.
    pub const AGY_TITLE_ONLY: Self = Self {
        title: true,
        color: false,
        title_activity: true,
        progress: false,
    };
    /// Cursor color is conditional on a proved exact terminal route.
    pub const CURSOR_COLOR_ONLY: Self = Self {
        title: false,
        color: true,
        title_activity: false,
        progress: false,
    };
    /// Unknown or unqualified channels never become effective by preference.
    pub const NONE: Self = Self {
        title: false,
        color: false,
        title_activity: false,
        progress: false,
    };
}

/// Whether an effective preference can be claimed for a live CLI session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplicationStatus {
    NotInstalled,
    InstalledUntrusted,
    RestartRequired,
    Applied,
    Unproven,
}

/// One resolved provider preference and the reason for every restriction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedPresentation {
    pub requested: PresentationSettings,
    pub effective: PresentationSettings,
    pub title_origin: PreferenceOrigin,
    pub color_origin: PreferenceOrigin,
    pub activity_origin: PreferenceOrigin,
    pub title_limited: bool,
    pub color_limited: bool,
    pub activity_limited: bool,
    pub application: ApplicationStatus,
}

/// Resolves global defaults, then a partial override, then proved capabilities.
///
/// An unsupported managed title or color falls back to native control. Activity
/// falls back to off so it cannot secretly emit a title marker or progress ring.
#[must_use]
pub fn resolve_presentation(
    global: PresentationSettings,
    override_for_cli: PresentationOverride,
    capabilities: PresentationCapabilities,
    application: ApplicationStatus,
) -> ResolvedPresentation {
    let requested = global
        .with_title(override_for_cli.title.unwrap_or(global.title()))
        .with_tab_color(override_for_cli.tab_color.unwrap_or(global.tab_color()))
        .with_activity(override_for_cli.activity.unwrap_or(global.activity()));
    let title_limited = requested.title() == TitleMode::TabBeacon && !capabilities.title;
    let color_limited = requested.tab_color() == TabColorMode::TabBeacon && !capabilities.color;
    let effective_activity = match requested.activity() {
        ActivityMode::TitleSpinner | ActivityMode::TitleIndicator
            if capabilities.title && capabilities.title_activity =>
        {
            requested.activity()
        }
        ActivityMode::WindowsTerminalRing if capabilities.progress => requested.activity(),
        ActivityMode::Both
            if capabilities.title && capabilities.title_activity && capabilities.progress =>
        {
            ActivityMode::Both
        }
        ActivityMode::Both if capabilities.title && capabilities.title_activity => {
            ActivityMode::TitleSpinner
        }
        ActivityMode::Both if capabilities.progress => ActivityMode::WindowsTerminalRing,
        ActivityMode::Native | ActivityMode::Off => requested.activity(),
        _ => ActivityMode::Off,
    };
    let activity_limited = effective_activity != requested.activity();
    let explicit_strict_mode = matches!(
        (
            override_for_cli.title,
            override_for_cli.tab_color,
            override_for_cli.activity
        ),
        (
            Some(TitleMode::Native),
            Some(TabColorMode::TabBeacon),
            Some(ActivityMode::Off)
        ) | (
            Some(TitleMode::Native),
            Some(TabColorMode::Native),
            Some(ActivityMode::Native)
        ) | (
            Some(TitleMode::TabBeacon),
            Some(TabColorMode::Off),
            Some(ActivityMode::TitleIndicator)
        )
    );
    let effective = requested
        .with_title(if title_limited {
            TitleMode::Native
        } else {
            requested.title()
        })
        .with_tab_color(if color_limited {
            TabColorMode::Native
        } else {
            requested.tab_color()
        })
        .with_activity(effective_activity)
        .with_strict_channel_policy(explicit_strict_mode);
    ResolvedPresentation {
        requested,
        effective,
        title_origin: if override_for_cli.title.is_some() {
            PreferenceOrigin::ProviderOverride
        } else {
            PreferenceOrigin::GlobalDefault
        },
        color_origin: if override_for_cli.tab_color.is_some() {
            PreferenceOrigin::ProviderOverride
        } else {
            PreferenceOrigin::GlobalDefault
        },
        activity_origin: if override_for_cli.activity.is_some() {
            PreferenceOrigin::ProviderOverride
        } else {
            PreferenceOrigin::GlobalDefault
        },
        title_limited,
        color_limited,
        activity_limited,
        application,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{ConditionalSaveOutcome, PresentationSettingsStore, SnapshotSaveOutcome};
    use std::fs;

    #[test]
    fn cursor_color_only_has_no_title_or_activity_channel() {
        let result = resolve_presentation(
            PresentationSettings::default(),
            PresentationOverride::default().with_mode(PresentationMode::ColorOnly),
            PresentationCapabilities::CURSOR_COLOR_ONLY,
            ApplicationStatus::RestartRequired,
        );
        assert_eq!(result.effective.title(), TitleMode::Native);
        assert_eq!(result.effective.tab_color(), TabColorMode::TabBeacon);
        assert_eq!(result.effective.activity(), ActivityMode::Off);
        assert!(!result.title_limited && !result.activity_limited);
        assert_eq!(result.application, ApplicationStatus::RestartRequired);
    }

    #[test]
    fn unsupported_agy_channels_do_not_become_effective() {
        let result = resolve_presentation(
            PresentationSettings::default(),
            PresentationOverride::default(),
            PresentationCapabilities::AGY_TITLE_ONLY,
            ApplicationStatus::Unproven,
        );
        assert_eq!(result.effective.title(), TitleMode::TabBeacon);
        assert_eq!(result.effective.tab_color(), TabColorMode::Native);
        assert_eq!(result.effective.activity(), ActivityMode::TitleSpinner);
        assert!(result.color_limited && !result.activity_limited);
    }

    #[test]
    fn partial_override_retains_global_fields_and_provenance() {
        let result = resolve_presentation(
            PresentationSettings::default(),
            PresentationOverride {
                tab_color: Some(TabColorMode::Off),
                ..PresentationOverride::default()
            },
            PresentationCapabilities::CODEX,
            ApplicationStatus::Applied,
        );
        assert_eq!(result.effective.title(), TitleMode::TabBeacon);
        assert_eq!(result.effective.tab_color(), TabColorMode::Off);
        assert_eq!(result.title_origin, PreferenceOrigin::GlobalDefault);
        assert_eq!(result.color_origin, PreferenceOrigin::ProviderOverride);
    }

    #[test]
    fn provider_override_is_partial_read_only_and_rollback_safe() {
        let root = tempfile::tempdir().expect("isolated root");
        let path = root.path().join("config.toml");
        let store = PresentationSettingsStore::new(&path);
        assert_eq!(
            store
                .load_provider_override_read_only(CliTarget::Cursor)
                .unwrap(),
            PresentationOverride::default()
        );
        assert!(!path.exists());
        let legacy = "[presentation]\ntitle = 'tabbeacon'\n\n[unrelated]\nkeep = true\n";
        fs::write(&path, legacy).unwrap();
        let before = store.snapshot_read_only().unwrap();
        let cursor = PresentationOverride::default().with_mode(PresentationMode::ColorOnly);
        let receipt = match store
            .save_provider_override_snapshot_if_unchanged(&before, CliTarget::Cursor, cursor)
            .unwrap()
        {
            SnapshotSaveOutcome::Saved(receipt) => receipt,
            SnapshotSaveOutcome::Conflict => panic!("unexpected conflict"),
        };
        assert_eq!(
            store
                .load_provider_override_read_only(CliTarget::Cursor)
                .unwrap(),
            cursor
        );
        assert_eq!(
            store
                .load_provider_override_read_only(CliTarget::Codex)
                .unwrap(),
            PresentationOverride::default()
        );
        assert!(fs::read_to_string(&path).unwrap().contains("keep = true"));
        assert_eq!(
            store
                .restore_snapshot_if_unchanged(&receipt, &before)
                .unwrap(),
            ConditionalSaveOutcome::Saved
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), legacy);
    }

    #[test]
    fn provider_override_refuses_concurrent_drift() {
        let root = tempfile::tempdir().expect("isolated root");
        let path = root.path().join("config.toml");
        let store = PresentationSettingsStore::new(&path);
        let before = store.snapshot_read_only().unwrap();
        fs::write(&path, "[foreign]\nkeep = 1\n").unwrap();
        let result = store
            .save_provider_override_snapshot_if_unchanged(
                &before,
                CliTarget::Agy,
                PresentationOverride::default().with_mode(PresentationMode::TitleOnly),
            )
            .unwrap();
        assert!(matches!(result, SnapshotSaveOutcome::Conflict));
        assert_eq!(fs::read_to_string(&path).unwrap(), "[foreign]\nkeep = 1\n");
    }
}
