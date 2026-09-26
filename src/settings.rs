//! Persistent, provider-neutral presentation preferences.
//!
//! Settings live in the per-user `TabBeacon` state root. They never live in a
//! repository and malformed input is deliberately contained by callers that
//! use [`PresentationSettingsStore::load_or_default`].

use std::{
    env, fmt,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use atomic_write_file::AtomicWriteFile;
use toml_edit::{DocumentMut, Item, Table, value};

use crate::presentation_policy::{
    ApplicationStatus, CliTarget, PresentationCapabilities, PresentationOverride,
    ResolvedPresentation, resolve_presentation,
};

const CONFIG_FILE: &str = "config.toml";
const LOCK_FILE: &str = "config.lock";
const SETTINGS_OPERATION_LOCK_BUDGET: Duration = Duration::from_millis(750);

/// Who owns terminal title updates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleMode {
    /// `TabBeacon` emits a safe semantic title.
    TabBeacon,
    /// Codex resumes ownership of its native terminal title.
    Native,
    /// `TabBeacon` emits no title; `Codex` native titles remain restored.
    Off,
}

impl TitleMode {
    /// Stable configuration spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TabBeacon => "tabbeacon",
            Self::Native => "native",
            Self::Off => "off",
        }
    }

    /// Parses one supported configuration spelling.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "tabbeacon" => Some(Self::TabBeacon),
            "native" => Some(Self::Native),
            "off" => Some(Self::Off),
            _ => None,
        }
    }

    /// Whether `TabBeacon` must own `Codex` terminal-title suppression.
    #[must_use]
    pub const fn owns_tabbeacon_title(self) -> bool {
        matches!(self, Self::TabBeacon)
    }
}

impl fmt::Display for TitleMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Who owns dynamic Windows Terminal frame/tab color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabColorMode {
    /// `TabBeacon` emits semantic dynamic color.
    TabBeacon,
    /// `TabBeacon` clears its color and then leaves the native terminal color alone.
    Native,
    /// `TabBeacon` clears its color and emits no dynamic color afterwards.
    Off,
}

impl TabColorMode {
    /// Stable configuration spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TabBeacon => "tabbeacon",
            Self::Native => "native",
            Self::Off => "off",
        }
    }

    /// Parses one supported configuration spelling.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "tabbeacon" => Some(Self::TabBeacon),
            "native" => Some(Self::Native),
            "off" => Some(Self::Off),
            _ => None,
        }
    }
}

impl fmt::Display for TabColorMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Activity channels `TabBeacon` may own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityMode {
    /// Animate the title status slot while active work is proven.
    TitleSpinner,
    /// A static title activity marker while working.
    TitleIndicator,
    /// Windows Terminal's native progress ring only.
    WindowsTerminalRing,
    /// Animated title activity plus the Windows Terminal progress ring.
    Both,
    /// `TabBeacon` emits no activity decoration.
    Native,
    /// `TabBeacon` clears its activity output and emits no ongoing decoration.
    Off,
}

impl ActivityMode {
    /// Stable configuration spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TitleSpinner => "title-spinner",
            Self::TitleIndicator => "title-indicator",
            Self::WindowsTerminalRing => "wt-ring",
            Self::Both => "both",
            Self::Native => "native",
            Self::Off => "off",
        }
    }

    /// Parses one supported configuration spelling.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "title-spinner" => Some(Self::TitleSpinner),
            "title-indicator" => Some(Self::TitleIndicator),
            "wt-ring" => Some(Self::WindowsTerminalRing),
            "both" => Some(Self::Both),
            "native" => Some(Self::Native),
            "off" => Some(Self::Off),
            _ => None,
        }
    }

    /// Whether a title marker belongs on active work.
    #[must_use]
    pub const fn uses_title_activity(self) -> bool {
        matches!(self, Self::TitleSpinner | Self::TitleIndicator | Self::Both)
    }

    /// Whether active work should be owned by the ephemeral title worker.
    #[must_use]
    pub const fn uses_worker_animation(self) -> bool {
        matches!(self, Self::TitleSpinner | Self::Both)
    }

    /// Whether Windows Terminal progress belongs on active work.
    #[must_use]
    pub const fn uses_windows_terminal_ring(self) -> bool {
        matches!(self, Self::WindowsTerminalRing | Self::Both)
    }
}

impl fmt::Display for ActivityMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Bounded built-in title activity frame sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpinnerPreset {
    /// Codex-like reduced bullet pulse.
    Codex,
    /// Braille dot spinner.
    Braille,
    /// Quadrant rotation.
    Quadrant,
    /// Four-character line rotation.
    Line,
    /// Four-step pulse.
    Pulse,
}

impl SpinnerPreset {
    /// Stable configuration spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Braille => "braille",
            Self::Quadrant => "quadrant",
            Self::Line => "line",
            Self::Pulse => "pulse",
        }
    }

    /// Parses one supported configuration spelling.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "codex" => Some(Self::Codex),
            "braille" => Some(Self::Braille),
            "quadrant" => Some(Self::Quadrant),
            "line" => Some(Self::Line),
            "pulse" => Some(Self::Pulse),
            _ => None,
        }
    }

    /// Deterministic control-free title frames.
    #[must_use]
    pub const fn frames(self) -> &'static [&'static str] {
        match self {
            Self::Codex => &["•", "◦"],
            Self::Braille => &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"],
            Self::Quadrant => &["◐", "◓", "◑", "◒"],
            Self::Line => &["-", "\\", "|", "/"],
            Self::Pulse => &["·", "•", "●", "•"],
        }
    }

    /// The first deterministic fallback frame for one-shot hooks.
    #[must_use]
    pub const fn fallback_indicator(self) -> &'static str {
        self.frames()[0]
    }
}

impl fmt::Display for SpinnerPreset {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Terminal palette choice applied after semantic presentation resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentationTheme {
    /// The comfortable v0.1 dark terminal default.
    MutedDark,
    /// The G02 compatibility palette.
    Classic,
}

impl PresentationTheme {
    /// Stable configuration spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MutedDark => "muted-dark",
            Self::Classic => "classic",
        }
    }

    /// Parses one supported configuration spelling.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "muted-dark" => Some(Self::MutedDark),
            "classic" => Some(Self::Classic),
            _ => None,
        }
    }
}

impl fmt::Display for PresentationTheme {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Provider label policy for the visible terminal title.
///
/// The policy is provider-neutral: individual admitted providers contribute a
/// short, validated badge only when the policy selects one.  `Auto` preserves
/// the compact single-provider title and is therefore the compatibility
/// default for existing users.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderBadgePolicy {
    /// Add a badge only when multiple admitted providers need disambiguation.
    Auto,
    /// Always add the admitted provider's bounded badge.
    Always,
    /// Never add a provider badge to a terminal title.
    Off,
}

impl ProviderBadgePolicy {
    /// Stable configuration spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Always => "always",
            Self::Off => "off",
        }
    }

    /// Parses one supported configuration spelling.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "auto" => Some(Self::Auto),
            "always" => Some(Self::Always),
            "off" => Some(Self::Off),
            _ => None,
        }
    }
}

impl fmt::Display for ProviderBadgePolicy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Typed, provider-neutral user presentation choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresentationSettings {
    title: TitleMode,
    tab_color: TabColorMode,
    activity: ActivityMode,
    spinner: SpinnerPreset,
    theme: PresentationTheme,
    provider_badge: ProviderBadgePolicy,
    strict_channel_policy: bool,
}

impl PresentationSettings {
    /// Constructs fully typed settings.
    #[must_use]
    pub const fn new(
        title: TitleMode,
        tab_color: TabColorMode,
        activity: ActivityMode,
        spinner: SpinnerPreset,
        theme: PresentationTheme,
    ) -> Self {
        Self::new_with_provider_badge(
            title,
            tab_color,
            activity,
            spinner,
            theme,
            ProviderBadgePolicy::Auto,
        )
    }

    /// Constructs fully typed settings with an explicit provider-badge policy.
    #[must_use]
    pub const fn new_with_provider_badge(
        title: TitleMode,
        tab_color: TabColorMode,
        activity: ActivityMode,
        spinner: SpinnerPreset,
        theme: PresentationTheme,
        provider_badge: ProviderBadgePolicy,
    ) -> Self {
        Self {
            title,
            tab_color,
            activity,
            spinner,
            theme,
            provider_badge,
            strict_channel_policy: false,
        }
    }

    /// Title channel choice.
    #[must_use]
    pub const fn title(self) -> TitleMode {
        self.title
    }

    /// Dynamic tab color channel choice.
    #[must_use]
    pub const fn tab_color(self) -> TabColorMode {
        self.tab_color
    }

    /// Activity channel choice.
    #[must_use]
    pub const fn activity(self) -> ActivityMode {
        self.activity
    }

    /// Built-in activity frame choice.
    #[must_use]
    pub const fn spinner(self) -> SpinnerPreset {
        self.spinner
    }

    /// Semantic color palette choice.
    #[must_use]
    pub const fn theme(self) -> PresentationTheme {
        self.theme
    }

    /// Provider badge behavior for the title channel.
    #[must_use]
    pub const fn provider_badge(self) -> ProviderBadgePolicy {
        self.provider_badge
    }

    /// Whether an explicit provider mode forbids writes to unmanaged channels.
    #[must_use]
    pub const fn strict_channel_policy(self) -> bool {
        self.strict_channel_policy
    }

    /// Marks an effective provider mode without changing legacy stored values.
    #[must_use]
    pub const fn with_strict_channel_policy(mut self, strict: bool) -> Self {
        self.strict_channel_policy = strict;
        self
    }

    /// Returns a copy with one title mode.
    #[must_use]
    pub const fn with_title(mut self, title: TitleMode) -> Self {
        self.title = title;
        self
    }

    /// Returns a copy with one tab-color mode.
    #[must_use]
    pub const fn with_tab_color(mut self, tab_color: TabColorMode) -> Self {
        self.tab_color = tab_color;
        self
    }

    /// Returns a copy with one activity mode.
    #[must_use]
    pub const fn with_activity(mut self, activity: ActivityMode) -> Self {
        self.activity = activity;
        self
    }

    /// Returns a copy with one spinner preset.
    #[must_use]
    pub const fn with_spinner(mut self, spinner: SpinnerPreset) -> Self {
        self.spinner = spinner;
        self
    }

    /// Returns a copy with one theme.
    #[must_use]
    pub const fn with_theme(mut self, theme: PresentationTheme) -> Self {
        self.theme = theme;
        self
    }

    /// Returns a copy with one provider-badge policy.
    #[must_use]
    pub const fn with_provider_badge(mut self, provider_badge: ProviderBadgePolicy) -> Self {
        self.provider_badge = provider_badge;
        self
    }

    /// Named, compact configuration presets.
    #[must_use]
    pub fn preset(name: &str) -> Option<Self> {
        match name {
            "native" => Some(Self::new(
                TitleMode::Native,
                TabColorMode::Native,
                ActivityMode::Native,
                SpinnerPreset::Braille,
                PresentationTheme::MutedDark,
            )),
            "minimal" => Some(Self::new(
                TitleMode::TabBeacon,
                TabColorMode::Native,
                ActivityMode::TitleIndicator,
                SpinnerPreset::Braille,
                PresentationTheme::MutedDark,
            )),
            "balanced" => Some(Self::default()),
            "terminal-ring" | "full" => Some(Self::new(
                TitleMode::TabBeacon,
                TabColorMode::TabBeacon,
                ActivityMode::WindowsTerminalRing,
                SpinnerPreset::Braille,
                PresentationTheme::MutedDark,
            )),
            _ => None,
        }
    }
}

impl Default for PresentationSettings {
    fn default() -> Self {
        // v0.3 new/default installs use the balanced profile. Existing
        // documents are parsed as-is and are never silently rewritten merely
        // because this absent-document fallback advances.
        Self::new(
            TitleMode::TabBeacon,
            TabColorMode::TabBeacon,
            ActivityMode::TitleSpinner,
            SpinnerPreset::Braille,
            PresentationTheme::MutedDark,
        )
    }
}

/// Non-sensitive settings read/write failure.
#[derive(Debug)]
pub enum SettingsError {
    /// No safe per-user settings location was available.
    StateRootUnavailable,
    /// A filesystem operation failed.
    Io(io::Error),
    /// The TOML document is malformed or uses an unsupported value shape.
    Malformed,
    /// The target is a symbolic link and is never replaced implicitly.
    SymbolicLinkTarget,
}

impl fmt::Display for SettingsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::StateRootUnavailable => "a safe per-user TabBeacon settings path is unavailable",
            Self::Io(_) => "a TabBeacon settings file operation failed",
            Self::Malformed => "the TabBeacon settings file is malformed or unsupported",
            Self::SymbolicLinkTarget => "the TabBeacon settings file is a symbolic link",
        })
    }
}

impl std::error::Error for SettingsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for SettingsError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Result of conditionally saving a settings draft.
///
/// A guided flow can use this to avoid overwriting a setting that changed
/// after its read-only snapshot was taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionalSaveOutcome {
    /// The expected settings were still current and the replacement was saved.
    Saved,
    /// Another writer changed the settings after the caller took its snapshot.
    Conflict,
}

/// Opaque, read-only snapshot of one presentation-settings document.
///
/// The snapshot retains the original document bytes only for an ownership-safe
/// rollback; it deliberately exposes typed effective settings instead of raw
/// user configuration.
pub struct PresentationSettingsSnapshot {
    settings: PresentationSettings,
    contents: Option<Vec<u8>>,
}

impl PresentationSettingsSnapshot {
    pub(crate) fn recovery_contents(&self) -> Option<&[u8]> {
        self.contents.as_deref()
    }
    /// Effective typed settings at the time the snapshot was taken.
    #[must_use]
    pub const fn settings(&self) -> PresentationSettings {
        self.settings
    }

    /// Whether no settings document existed when the snapshot was taken.
    #[must_use]
    pub const fn is_absent(&self) -> bool {
        self.contents.is_none()
    }

    /// Parses one provider override from the same exact bytes as the global
    /// settings used by this snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed or unsupported configuration bytes.
    pub fn provider_override(
        &self,
        provider: CliTarget,
    ) -> Result<PresentationOverride, SettingsError> {
        let Some(bytes) = self.contents.as_deref() else {
            return Ok(PresentationOverride::default());
        };
        let document = std::str::from_utf8(bytes)
            .map_err(|_| SettingsError::Malformed)?
            .parse::<DocumentMut>()
            .map_err(|_| SettingsError::Malformed)?;
        provider_override_from_document(&document, provider)
    }

    fn matches(&self, other: &Self) -> bool {
        self.contents == other.contents
    }
}

/// Opaque receipt for one snapshot-guarded settings write.
///
/// The receipt is accepted only by [`PresentationSettingsStore`] to protect a
/// subsequent rollback from overwriting a concurrent configuration update.
pub struct PresentationSettingsWriteReceipt {
    contents: Vec<u8>,
}

impl PresentationSettingsWriteReceipt {
    fn matches(&self, snapshot: &PresentationSettingsSnapshot) -> bool {
        snapshot.contents.as_deref() == Some(self.contents.as_slice())
    }
}

/// Result of saving a draft against an exact read-only snapshot.
pub enum SnapshotSaveOutcome {
    /// The original document was still exact and the draft was saved.
    Saved(PresentationSettingsWriteReceipt),
    /// Another writer changed the document after the snapshot was taken.
    Conflict,
}

/// Process-safe, atomic per-user presentation settings storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresentationSettingsStore {
    path: PathBuf,
}

impl PresentationSettingsStore {
    /// Creates a store for an explicitly injected config path.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Returns the default user-global `TabBeacon` config path.
    ///
    /// On Windows this is `%LOCALAPPDATA%\\TabBeacon\\config.toml`.
    ///
    /// # Errors
    ///
    /// Returns an error when no safe per-user state root is available.
    pub fn from_environment() -> Result<Self, SettingsError> {
        #[cfg(windows)]
        let root = env::var_os("LOCALAPPDATA")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .map(|path| path.join("TabBeacon"));
        #[cfg(not(windows))]
        let root = env::var_os("XDG_STATE_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                env::var_os("HOME")
                    .filter(|value| !value.is_empty())
                    .map(PathBuf::from)
                    .map(|path| path.join(".local").join("state"))
            })
            .map(|path| path.join("tabbeacon"));
        root.map(|root| Self::new(root.join(CONFIG_FILE)))
            .ok_or(SettingsError::StateRootUnavailable)
    }

    /// Returns the config location without reading it.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Loads typed settings or reports an absent/malformed document.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed TOML, unsupported values, unsafe links,
    /// or an unreadable per-user settings path.
    pub fn load(&self) -> Result<PresentationSettings, SettingsError> {
        self.with_lock(|| self.load_unlocked())
    }

    /// Reads effective settings without creating a state directory or lock file.
    ///
    /// This is intentionally separate from [`Self::load`] for read-only setup
    /// discovery and diagnostics. It never creates a missing parent directory,
    /// lock, or settings file.
    ///
    /// # Errors
    ///
    /// Returns the same safe parse, symbolic-link, or I/O errors as
    /// [`Self::load`].
    pub fn load_read_only(&self) -> Result<PresentationSettings, SettingsError> {
        Ok(self.snapshot_read_only()?.settings())
    }

    /// Reads one partial provider override without creating a lock or file.
    /// Legacy v0.7.3 documents have no overrides and retain their global meaning.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed, unreadable, or unsafe configuration.
    pub fn load_provider_override_read_only(
        &self,
        provider: CliTarget,
    ) -> Result<PresentationOverride, SettingsError> {
        self.reject_symbolic_link()?;
        let Some(bytes) = read_optional_bytes(&self.path)? else {
            return Ok(PresentationOverride::default());
        };
        let document = std::str::from_utf8(&bytes)
            .map_err(|_| SettingsError::Malformed)?
            .parse::<DocumentMut>()
            .map_err(|_| SettingsError::Malformed)?;
        // A malformed global section must never be hidden by an override read.
        settings_from_document(&document)?;
        provider_override_from_document(&document, provider)
    }

    /// Projects one provider's saved preference through proved capabilities.
    /// The caller supplies live application status; reading the file alone can
    /// never establish installation, Hook trust, or live application.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed, unreadable, or unsafe configuration.
    pub fn resolve_provider_read_only(
        &self,
        provider: CliTarget,
        capabilities: PresentationCapabilities,
        application: ApplicationStatus,
    ) -> Result<ResolvedPresentation, SettingsError> {
        let snapshot = self.snapshot_read_only()?;
        let override_for_cli = match snapshot.contents.as_deref() {
            Some(bytes) => {
                let document = std::str::from_utf8(bytes)
                    .map_err(|_| SettingsError::Malformed)?
                    .parse::<DocumentMut>()
                    .map_err(|_| SettingsError::Malformed)?;
                provider_override_from_document(&document, provider)?
            }
            None => PresentationOverride::default(),
        };
        Ok(resolve_presentation(
            snapshot.settings(),
            override_for_cli,
            capabilities,
            application,
        ))
    }

    /// Saves only the selected CLI's override if the exact preview bytes remain
    /// current. Unknown TOML and other provider tables are preserved.
    ///
    /// # Errors
    ///
    /// Returns an error when the exact-owned configuration cannot be safely
    /// parsed, locked, or atomically written.
    pub fn save_provider_override_snapshot_if_unchanged(
        &self,
        expected: &PresentationSettingsSnapshot,
        provider: CliTarget,
        replacement: PresentationOverride,
    ) -> Result<SnapshotSaveOutcome, SettingsError> {
        self.save_provider_override_snapshot_if_unchanged_guarded(
            expected,
            provider,
            replacement,
            || Ok(()),
        )
    }

    /// Saves an exact provider draft while holding the settings lock before
    /// acquiring a caller-provided output guard. Cursor uses the route guard
    /// so a stalled settings writer cannot hold up all admitted Hook output.
    /// The guard is acquired only after the current snapshot has matched and
    /// the candidate document has rendered, and is held through the write.
    ///
    /// # Errors
    ///
    /// Rejects drift, an unsafe settings target, or a failed output guard
    /// before writing the candidate document.
    pub fn save_provider_override_snapshot_if_unchanged_guarded<G>(
        &self,
        expected: &PresentationSettingsSnapshot,
        provider: CliTarget,
        replacement: PresentationOverride,
        acquire_guard: impl FnOnce() -> Result<G, SettingsError>,
    ) -> Result<SnapshotSaveOutcome, SettingsError> {
        self.with_lock(|| {
            let current = self.snapshot_unlocked()?;
            if !current.matches(expected) {
                return Ok(SnapshotSaveOutcome::Conflict);
            }
            let mut document = match current.contents.as_deref() {
                Some(bytes) => std::str::from_utf8(bytes)
                    .map_err(|_| SettingsError::Malformed)?
                    .parse::<DocumentMut>()
                    .map_err(|_| SettingsError::Malformed)?,
                None => DocumentMut::new(),
            };
            write_provider_override(&mut document, provider, replacement)?;
            let contents = document.to_string().into_bytes();
            let _guard = acquire_guard()?;
            if current.contents.as_deref() != Some(contents.as_slice()) {
                atomic_write(&self.path, &contents)?;
            }
            Ok(SnapshotSaveOutcome::Saved(
                PresentationSettingsWriteReceipt { contents },
            ))
        })
    }

    /// Applies a portable global setting and provider overrides as one atomic
    /// presentation-document write guarded by the preview's exact bytes.
    ///
    /// # Errors
    ///
    /// Returns an error when the exact-owned configuration cannot be safely
    /// parsed, locked, or atomically written.
    pub fn save_portable_snapshot_if_unchanged(
        &self,
        expected: &PresentationSettingsSnapshot,
        global: Option<PresentationSettings>,
        overrides: &[(CliTarget, PresentationOverride)],
    ) -> Result<SnapshotSaveOutcome, SettingsError> {
        self.with_lock(|| {
            let current = self.snapshot_unlocked()?;
            if !current.matches(expected) {
                return Ok(SnapshotSaveOutcome::Conflict);
            }
            let contents = Self::render_portable_snapshot(&current, global, overrides)?;
            atomic_write(&self.path, &contents)?;
            Ok(SnapshotSaveOutcome::Saved(
                PresentationSettingsWriteReceipt { contents },
            ))
        })
    }

    pub(crate) fn render_portable_snapshot(
        snapshot: &PresentationSettingsSnapshot,
        global: Option<PresentationSettings>,
        overrides: &[(CliTarget, PresentationOverride)],
    ) -> Result<Vec<u8>, SettingsError> {
        let mut document = match snapshot.contents.as_deref() {
            Some(bytes) => std::str::from_utf8(bytes)
                .map_err(|_| SettingsError::Malformed)?
                .parse::<DocumentMut>()
                .map_err(|_| SettingsError::Malformed)?,
            None => DocumentMut::new(),
        };
        if let Some(global) = global {
            write_settings(&mut document, global)?;
        }
        for (provider, override_for_cli) in overrides {
            write_provider_override(&mut document, *provider, *override_for_cli)?;
        }
        Ok(document.to_string().into_bytes())
    }

    /// Captures the current document without creating a state directory or lock.
    ///
    /// Callers can use the opaque snapshot to ensure a later recovery restores
    /// an originally absent document as absent, without exposing raw settings.
    ///
    /// # Errors
    ///
    /// Returns the same safe parse, symbolic-link, or I/O errors as
    /// [`Self::load_read_only`].
    pub fn snapshot_read_only(&self) -> Result<PresentationSettingsSnapshot, SettingsError> {
        self.snapshot_unlocked()
    }

    /// Returns whether an opaque snapshot is still byte-exactly current.
    ///
    /// This is read-only and exposes no configuration bytes. Guided recovery
    /// uses it to verify a completed compensation before reporting success.
    ///
    /// # Errors
    ///
    /// Returns the same safe read error as [`Self::snapshot_read_only`].
    pub fn snapshot_is_current(
        &self,
        expected: &PresentationSettingsSnapshot,
    ) -> Result<bool, SettingsError> {
        Ok(self.snapshot_read_only()?.matches(expected))
    }

    /// Restores a transaction's exact prior bytes only if this store still
    /// contains its exact planned write. Repeated recovery is idempotent.
    pub(crate) fn recover_import_bytes_if_unchanged(
        &self,
        planned: &[u8],
        original: Option<&[u8]>,
    ) -> Result<bool, SettingsError> {
        if let Some(bytes) = original {
            parse_settings_bytes(bytes)?;
        }
        self.with_lock(|| {
            let current = self.snapshot_unlocked()?;
            if current.contents.as_deref() == original {
                return Ok(true);
            }
            if current.contents.as_deref() != Some(planned) {
                return Ok(false);
            }
            match original {
                Some(bytes) => atomic_write(&self.path, bytes)?,
                None => fs::remove_file(&self.path)?,
            }
            Ok(self.snapshot_unlocked()?.contents.as_deref() == original)
        })
    }

    /// Returns whether a guided write receipt is still byte-exactly current.
    ///
    /// This lets a multi-store operation perform its final drift check without
    /// exposing the serialized user configuration.
    ///
    /// # Errors
    ///
    /// Returns the same safe read error as [`Self::snapshot_read_only`].
    pub fn write_receipt_is_current(
        &self,
        receipt: &PresentationSettingsWriteReceipt,
    ) -> Result<bool, SettingsError> {
        Ok(receipt.matches(&self.snapshot_read_only()?))
    }

    /// Reads valid settings, defaulting safely for absent or malformed input.
    #[must_use]
    pub fn load_or_default(&self) -> PresentationSettings {
        self.load().unwrap_or_default()
    }

    /// Atomically saves typed settings while preserving unknown TOML keys.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed existing TOML, unsafe links, or a failed
    /// process-safe atomic write.
    pub fn save(&self, settings: PresentationSettings) -> Result<(), SettingsError> {
        self.with_lock(|| self.save_unlocked(settings))
    }

    /// Saves a draft only when the caller's read snapshot is still current.
    ///
    /// # Errors
    ///
    /// Returns a storage error without modifying settings when the current
    /// document cannot be safely read or written.
    pub fn save_if_unchanged(
        &self,
        expected: PresentationSettings,
        replacement: PresentationSettings,
    ) -> Result<ConditionalSaveOutcome, SettingsError> {
        self.with_lock(|| {
            if self.load_unlocked()? != expected {
                return Ok(ConditionalSaveOutcome::Conflict);
            }
            self.save_unlocked(replacement)?;
            Ok(ConditionalSaveOutcome::Saved)
        })
    }

    /// Saves a draft only when the exact read-only document is still current.
    ///
    /// Unlike [`Self::save_if_unchanged`], this preserves absence and unknown
    /// TOML bytes as part of the comparison, so a later rollback can avoid
    /// overwriting a concurrent change.
    ///
    /// # Errors
    ///
    /// Returns a storage error without modifying settings when the current
    /// document cannot be safely read or written.
    pub fn save_snapshot_if_unchanged(
        &self,
        expected: &PresentationSettingsSnapshot,
        replacement: PresentationSettings,
    ) -> Result<SnapshotSaveOutcome, SettingsError> {
        self.with_lock(|| {
            let current = self.snapshot_unlocked()?;
            if !current.matches(expected) {
                return Ok(SnapshotSaveOutcome::Conflict);
            }
            Ok(SnapshotSaveOutcome::Saved(
                self.save_snapshot_unlocked(&current, replacement)?,
            ))
        })
    }

    /// Restores an original snapshot only when the prior guided write remains exact.
    ///
    /// # Errors
    ///
    /// Returns a storage error without modifying settings when the current
    /// document cannot be safely read or restored.
    pub fn restore_snapshot_if_unchanged(
        &self,
        receipt: &PresentationSettingsWriteReceipt,
        original: &PresentationSettingsSnapshot,
    ) -> Result<ConditionalSaveOutcome, SettingsError> {
        self.with_lock(|| {
            let current = self.snapshot_unlocked()?;
            if !receipt.matches(&current) {
                return Ok(ConditionalSaveOutcome::Conflict);
            }
            self.restore_snapshot_unlocked(original)?;
            Ok(ConditionalSaveOutcome::Saved)
        })
    }

    /// Replaces the settings with documented v0.1 defaults.
    ///
    /// # Errors
    ///
    /// Returns an error for an unsafe link or a failed process-safe atomic write.
    pub fn reset(&self) -> Result<PresentationSettings, SettingsError> {
        let settings = PresentationSettings::default();
        self.with_lock(|| {
            self.reject_symbolic_link()?;
            let mut document = DocumentMut::new();
            write_settings(&mut document, settings)?;
            atomic_write(&self.path, document.to_string().as_bytes())?;
            Ok(settings)
        })
    }

    fn with_lock<T>(
        &self,
        operation: impl FnOnce() -> Result<T, SettingsError>,
    ) -> Result<T, SettingsError> {
        let parent = self
            .path
            .parent()
            .ok_or(SettingsError::StateRootUnavailable)?;
        fs::create_dir_all(parent)?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(parent.join(LOCK_FILE))?;
        try_lock_with_budget(&lock, SETTINGS_OPERATION_LOCK_BUDGET)?;
        let result = operation();
        File::unlock(&lock)?;
        result
    }

    /// Holds the existing settings lock across one Hook's route and output
    /// decision. TabBeacon's presentation-document writers use this lock, so an older
    /// Hook cannot finish writing after a newer global/import configuration
    /// commit. A busy writer makes the Hook skip decoration within its budget.
    pub(crate) fn with_runtime_lock_bounded<T>(
        &self,
        budget: Duration,
        operation: impl FnOnce() -> io::Result<T>,
    ) -> io::Result<T> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| io::Error::other("settings state root unavailable"))?;
        fs::create_dir_all(parent)?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(parent.join(LOCK_FILE))?;
        try_lock_with_budget(&lock, budget)?;
        let result = operation();
        // The Hook reports the output result itself. Closing this owned lock
        // releases it without turning a successful flush into an apparent
        // failure solely because a separate unlock call failed.
        drop(lock);
        result
    }

    fn load_unlocked(&self) -> Result<PresentationSettings, SettingsError> {
        Ok(self.snapshot_unlocked()?.settings())
    }

    fn snapshot_unlocked(&self) -> Result<PresentationSettingsSnapshot, SettingsError> {
        self.reject_symbolic_link()?;
        let contents = read_optional_bytes(&self.path)?;
        let settings = match contents.as_deref() {
            Some(bytes) => parse_settings_bytes(bytes)?,
            None => PresentationSettings::default(),
        };
        Ok(PresentationSettingsSnapshot { settings, contents })
    }

    fn save_unlocked(&self, settings: PresentationSettings) -> Result<(), SettingsError> {
        let snapshot = self.snapshot_unlocked()?;
        self.save_snapshot_unlocked(&snapshot, settings)?;
        Ok(())
    }

    fn save_snapshot_unlocked(
        &self,
        snapshot: &PresentationSettingsSnapshot,
        settings: PresentationSettings,
    ) -> Result<PresentationSettingsWriteReceipt, SettingsError> {
        let mut document = match snapshot.contents.as_deref() {
            Some(bytes) => std::str::from_utf8(bytes)
                .map_err(|_| SettingsError::Malformed)?
                .parse::<DocumentMut>()
                .map_err(|_| SettingsError::Malformed)?,
            None => DocumentMut::new(),
        };
        write_settings(&mut document, settings)?;
        let contents = document.to_string().into_bytes();
        atomic_write(&self.path, &contents)?;
        Ok(PresentationSettingsWriteReceipt { contents })
    }

    fn restore_snapshot_unlocked(
        &self,
        snapshot: &PresentationSettingsSnapshot,
    ) -> Result<(), SettingsError> {
        self.reject_symbolic_link()?;
        match snapshot.contents.as_deref() {
            Some(contents) => atomic_write(&self.path, contents)?,
            None => fs::remove_file(&self.path)?,
        }
        Ok(())
    }

    fn reject_symbolic_link(&self) -> Result<(), SettingsError> {
        match fs::symlink_metadata(&self.path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                Err(SettingsError::SymbolicLinkTarget)
            }
            Ok(_) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

fn try_lock_with_budget(lock: &File, budget: Duration) -> io::Result<()> {
    let deadline = Instant::now() + budget;
    loop {
        match lock.try_lock() {
            Ok(()) => return Ok(()),
            Err(fs::TryLockError::WouldBlock) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(5));
            }
            Err(fs::TryLockError::WouldBlock) => {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "TabBeacon settings lock remained busy",
                ));
            }
            Err(error) => return Err(error.into()),
        }
    }
}

fn parse_settings_bytes(bytes: &[u8]) -> Result<PresentationSettings, SettingsError> {
    let document = std::str::from_utf8(bytes)
        .map_err(|_| SettingsError::Malformed)?
        .parse::<DocumentMut>()
        .map_err(|_| SettingsError::Malformed)?;
    settings_from_document(&document)
}

fn provider_override_from_document(
    document: &DocumentMut,
    provider: CliTarget,
) -> Result<PresentationOverride, SettingsError> {
    let Some(root) = document.get("provider_presentation") else {
        return Ok(PresentationOverride::default());
    };
    let root = root.as_table_like().ok_or(SettingsError::Malformed)?;
    let Some(entry) = root.get(provider.as_str()) else {
        return Ok(PresentationOverride::default());
    };
    let entry = entry.as_table_like().ok_or(SettingsError::Malformed)?;
    Ok(PresentationOverride {
        title: parse_optional_value(entry.get("title"), TitleMode::parse)?,
        tab_color: parse_optional_value(entry.get("tab_color"), TabColorMode::parse)?,
        activity: parse_optional_value(entry.get("activity"), ActivityMode::parse)?,
    })
}

fn parse_optional_value<T>(
    item: Option<&Item>,
    parse: impl Fn(&str) -> Option<T>,
) -> Result<Option<T>, SettingsError> {
    item.map(|value| {
        value
            .as_str()
            .and_then(parse)
            .ok_or(SettingsError::Malformed)
    })
    .transpose()
}

fn write_provider_override(
    document: &mut DocumentMut,
    provider: CliTarget,
    replacement: PresentationOverride,
) -> Result<(), SettingsError> {
    if replacement == PresentationOverride::default() {
        if let Some(root) = document.get_mut("provider_presentation") {
            let root = root.as_table_like_mut().ok_or(SettingsError::Malformed)?;
            root.remove(provider.as_str());
            if root.is_empty() {
                document.as_table_mut().remove("provider_presentation");
            }
        }
        return Ok(());
    }
    if !document.as_table().contains_key("provider_presentation") {
        document["provider_presentation"] = Item::Table(Table::new());
    }
    let root = document["provider_presentation"]
        .as_table_like_mut()
        .ok_or(SettingsError::Malformed)?;
    if root.get(provider.as_str()).is_none() {
        root.insert(provider.as_str(), Item::Table(Table::new()));
    }
    let entry = root
        .get_mut(provider.as_str())
        .and_then(Item::as_table_like_mut)
        .ok_or(SettingsError::Malformed)?;
    for (key, selected) in [
        ("title", replacement.title.map(TitleMode::as_str)),
        ("tab_color", replacement.tab_color.map(TabColorMode::as_str)),
        ("activity", replacement.activity.map(ActivityMode::as_str)),
    ] {
        if let Some(selected) = selected {
            entry.insert(key, value(selected));
        } else {
            entry.remove(key);
        }
    }
    Ok(())
}

fn settings_from_document(document: &DocumentMut) -> Result<PresentationSettings, SettingsError> {
    let Some(presentation) = document.get("presentation") else {
        return Ok(PresentationSettings::default());
    };
    let table = presentation
        .as_table_like()
        .ok_or(SettingsError::Malformed)?;
    let defaults = PresentationSettings::default();
    Ok(PresentationSettings::new_with_provider_badge(
        parse_value(table.get("title"), TitleMode::parse, defaults.title())?,
        parse_value(
            table.get("tab_color"),
            TabColorMode::parse,
            defaults.tab_color(),
        )?,
        parse_value(
            table.get("activity"),
            ActivityMode::parse,
            defaults.activity(),
        )?,
        parse_value(
            table.get("spinner"),
            SpinnerPreset::parse,
            defaults.spinner(),
        )?,
        parse_value(
            table.get("theme"),
            PresentationTheme::parse,
            defaults.theme(),
        )?,
        parse_value(
            table.get("provider_badge"),
            ProviderBadgePolicy::parse,
            defaults.provider_badge(),
        )?,
    ))
}

fn parse_value<T: Copy>(
    value: Option<&Item>,
    parse: impl Fn(&str) -> Option<T>,
    default: T,
) -> Result<T, SettingsError> {
    let Some(value) = value else {
        return Ok(default);
    };
    value
        .as_str()
        .and_then(parse)
        .ok_or(SettingsError::Malformed)
}

fn write_settings(
    document: &mut DocumentMut,
    settings: PresentationSettings,
) -> Result<(), SettingsError> {
    if !document.as_table().contains_key("presentation") {
        document["presentation"] = Item::Table(Table::new());
    }
    let table = document["presentation"]
        .as_table_like_mut()
        .ok_or(SettingsError::Malformed)?;
    table.insert("title", value(settings.title().as_str()));
    table.insert("tab_color", value(settings.tab_color().as_str()));
    table.insert("activity", value(settings.activity().as_str()));
    table.insert("spinner", value(settings.spinner().as_str()));
    table.insert("theme", value(settings.theme().as_str()));
    table.insert("provider_badge", value(settings.provider_badge().as_str()));
    Ok(())
}

fn read_optional_bytes(path: &Path) -> Result<Option<Vec<u8>>, io::Error> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let Some(parent) = path.parent() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "atomic target has no parent",
        ));
    };
    fs::create_dir_all(parent)?;
    let mut file = AtomicWriteFile::options().open(path)?;
    file.write_all(bytes)?;
    file.flush()?;
    file.commit()
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::{Arc, Barrier},
        thread,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    use super::{
        ActivityMode, ConditionalSaveOutcome, PresentationSettings, PresentationSettingsStore,
        PresentationTheme, ProviderBadgePolicy, SpinnerPreset, TabColorMode, TitleMode,
    };

    use crate::presentation_policy::{CliTarget, PresentationMode, PresentationOverride};

    #[test]
    fn busy_settings_lock_refuses_a_writer_without_an_unbounded_wait_or_write() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("config.toml");
        let store = PresentationSettingsStore::new(&path);
        store.load().unwrap();
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(root.path().join("config.lock"))
            .unwrap();
        lock.lock().unwrap();
        let started = Instant::now();
        let refused = store.save(PresentationSettings::default());
        assert!(matches!(
            refused,
            Err(super::SettingsError::Io(ref error)) if error.kind() == std::io::ErrorKind::WouldBlock
        ));
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(!path.exists());
        fs::File::unlock(&lock).unwrap();
        store.save(PresentationSettings::default()).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn guarded_provider_save_refuses_output_lock_failure_before_any_write() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("config.toml");
        fs::write(&path, "[foreign]\nkey = \"preserved\"\n").unwrap();
        let store = PresentationSettingsStore::new(&path);
        let before = store.snapshot_read_only().unwrap();
        let original = fs::read(&path).unwrap();
        let draft = PresentationOverride::default().with_mode(PresentationMode::PreserveNative);
        let failed = store.save_provider_override_snapshot_if_unchanged_guarded::<()>(
            &before,
            CliTarget::Cursor,
            draft,
            || {
                Err(super::SettingsError::Io(std::io::Error::other(
                    "route lock failed",
                )))
            },
        );
        assert!(failed.is_err());
        assert_eq!(fs::read(&path).unwrap(), original);
        assert!(matches!(
            store.save_provider_override_snapshot_if_unchanged_guarded(
                &before,
                CliTarget::Cursor,
                draft,
                || Ok(()),
            ),
            Ok(super::SnapshotSaveOutcome::Saved(_))
        ));
        let after_saved = fs::read(&path).unwrap();
        assert!(matches!(
            store.save_provider_override_snapshot_if_unchanged_guarded::<()>(
                &before,
                CliTarget::Cursor,
                PresentationOverride::default(),
                || panic!("drift must reject before the route lock is acquired"),
            ),
            Ok(super::SnapshotSaveOutcome::Conflict)
        ));
        assert_eq!(fs::read(&path).unwrap(), after_saved);
        assert!(
            fs::read_to_string(path)
                .unwrap()
                .contains("key = \"preserved\"")
        );
    }

    fn temporary_config(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "tabbeacon-settings-{name}-{}-{}.toml",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock is after Unix epoch")
                .as_nanos()
        ))
    }

    fn temporary_root(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "tabbeacon-settings-root-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock is after Unix epoch")
                .as_nanos()
        ))
    }

    #[test]
    fn missing_settings_use_the_v03_balanced_defaults_without_creating_a_file() {
        let path = temporary_config("defaults");
        let store = PresentationSettingsStore::new(&path);
        assert_eq!(
            store.load().expect("missing config defaults"),
            PresentationSettings::default()
        );
        let defaults = PresentationSettings::default();
        assert_eq!(defaults.title(), TitleMode::TabBeacon);
        assert_eq!(defaults.tab_color(), TabColorMode::TabBeacon);
        assert_eq!(defaults.activity(), ActivityMode::TitleSpinner);
        assert_eq!(defaults.spinner(), SpinnerPreset::Braille);
        assert_eq!(defaults.theme(), PresentationTheme::MutedDark);
        assert_eq!(defaults.provider_badge(), ProviderBadgePolicy::Auto);
        assert!(
            !path.exists(),
            "reading absent defaults must not write config"
        );
    }

    #[test]
    fn human_presets_keep_one_primary_activity_channel() {
        let native = PresentationSettings::preset("native").expect("native preset");
        assert_eq!(native.title(), TitleMode::Native);
        assert_eq!(native.tab_color(), TabColorMode::Native);
        assert_eq!(native.activity(), ActivityMode::Native);

        let minimal = PresentationSettings::preset("minimal").expect("minimal preset");
        assert_eq!(minimal.title(), TitleMode::TabBeacon);
        assert_eq!(minimal.tab_color(), TabColorMode::Native);
        assert_eq!(minimal.activity(), ActivityMode::TitleIndicator);

        let balanced = PresentationSettings::preset("balanced").expect("balanced preset");
        assert_eq!(balanced, PresentationSettings::default());
        assert_eq!(balanced.spinner(), SpinnerPreset::Braille);

        let terminal_ring =
            PresentationSettings::preset("terminal-ring").expect("terminal ring preset");
        assert_eq!(terminal_ring.title(), TitleMode::TabBeacon);
        assert_eq!(terminal_ring.tab_color(), TabColorMode::TabBeacon);
        assert_eq!(terminal_ring.activity(), ActivityMode::WindowsTerminalRing);
        assert!(!terminal_ring.activity().uses_worker_animation());
        assert!(terminal_ring.activity().uses_windows_terminal_ring());
        assert_eq!(terminal_ring.spinner(), SpinnerPreset::Braille);

        let legacy_full_name = PresentationSettings::preset("full").expect("legacy full name");
        assert_eq!(legacy_full_name, terminal_ring);
        assert_ne!(legacy_full_name.activity(), ActivityMode::Both);
    }

    #[test]
    fn save_preserves_unknown_future_keys_and_round_trips_typed_values() {
        let path = temporary_config("preserve");
        fs::write(
            &path,
            "[presentation]\nfuture_flag = true\ntitle = \"native\"\n\n[future]\nkey = \"kept\"\n",
        )
        .expect("fixture config writes");
        let store = PresentationSettingsStore::new(&path);
        let configured = PresentationSettings::new(
            TitleMode::Off,
            TabColorMode::Native,
            ActivityMode::Both,
            SpinnerPreset::Braille,
            PresentationTheme::Classic,
        );
        store.save(configured).expect("settings save succeeds");
        assert_eq!(store.load().expect("settings reread"), configured);
        let text = fs::read_to_string(&path).expect("saved config reads");
        assert!(text.contains("future_flag = true"));
        assert!(text.contains("[future]"));
        assert!(text.contains("key = \"kept\""));
        fs::remove_file(path).expect("fixture config removes");
    }

    #[test]
    fn provider_badge_migrates_absent_settings_without_rewrite_and_round_trips_explicit_choice() {
        let path = temporary_config("provider-badge");
        let legacy = "[presentation]\ntitle = \"tabbeacon\"\n";
        fs::write(&path, legacy).expect("legacy fixture writes");
        let store = PresentationSettingsStore::new(&path);

        assert_eq!(
            store
                .load_read_only()
                .expect("legacy settings read")
                .provider_badge(),
            ProviderBadgePolicy::Auto
        );
        assert_eq!(
            fs::read_to_string(&path).expect("legacy settings reread"),
            legacy,
            "read-only migration never rewrites existing user configuration"
        );

        let selected = store
            .load_read_only()
            .expect("legacy settings reread")
            .with_provider_badge(ProviderBadgePolicy::Always);
        store.save(selected).expect("explicit selection saves");
        assert_eq!(
            store.load_read_only().expect("saved selection rereads"),
            selected
        );
        assert!(
            fs::read_to_string(&path)
                .expect("selected document reads")
                .contains("provider_badge = \"always\"")
        );
        fs::remove_file(path).expect("fixture config removes");
    }

    #[test]
    fn malformed_user_configuration_falls_back_without_breaking_hook_callers() {
        let path = temporary_config("malformed");
        let malformed = "[presentation\ntitle = \"tabbeacon\"";
        fs::write(&path, malformed).expect("malformed fixture writes");
        let store = PresentationSettingsStore::new(&path);
        assert!(store.load().is_err());
        assert!(store.load_read_only().is_err());
        assert_eq!(store.load_or_default(), PresentationSettings::default());
        assert_eq!(
            fs::read_to_string(&path).expect("malformed fixture remains readable"),
            malformed,
            "a fallback must never rewrite malformed existing user configuration"
        );
        fs::remove_file(path).expect("fixture config removes");
    }

    #[test]
    fn existing_v02_static_and_custom_settings_are_never_silently_rewritten() {
        let path = temporary_config("existing-users");
        let v02_static = concat!(
            "[presentation]\n",
            "title = \"tabbeacon\"\n",
            "tab_color = \"tabbeacon\"\n",
            "activity = \"title-indicator\"\n",
            "spinner = \"codex\"\n",
            "theme = \"muted-dark\"\n",
        );
        fs::write(&path, v02_static).expect("v0.2 fixture writes");
        let store = PresentationSettingsStore::new(&path);
        assert_eq!(
            store.load_or_default(),
            PresentationSettings::new(
                TitleMode::TabBeacon,
                TabColorMode::TabBeacon,
                ActivityMode::TitleIndicator,
                SpinnerPreset::Codex,
                PresentationTheme::MutedDark,
            )
        );
        assert_eq!(
            fs::read_to_string(&path).expect("v0.2 fixture rereads"),
            v02_static
        );

        let custom = concat!(
            "[presentation]\n",
            "title = \"native\"\n",
            "tab_color = \"off\"\n",
            "activity = \"wt-ring\"\n",
            "spinner = \"line\"\n",
            "theme = \"classic\"\n",
        );
        fs::write(&path, custom).expect("custom fixture writes");
        assert_eq!(
            store.load_or_default(),
            PresentationSettings::new(
                TitleMode::Native,
                TabColorMode::Off,
                ActivityMode::WindowsTerminalRing,
                SpinnerPreset::Line,
                PresentationTheme::Classic,
            )
        );
        assert_eq!(
            fs::read_to_string(&path).expect("custom fixture rereads"),
            custom
        );
        fs::remove_file(path).expect("existing fixture removes");
    }

    #[test]
    fn legacy_both_token_stays_readable_and_byte_exact_until_explicit_apply() {
        let path = temporary_config("legacy-both");
        let legacy_both = concat!(
            "[presentation]\n",
            "title = \"tabbeacon\"\n",
            "tab_color = \"tabbeacon\"\n",
            "activity = \"both\"\n",
            "spinner = \"braille\"\n",
            "theme = \"muted-dark\"\n",
        );
        fs::write(&path, legacy_both).expect("legacy fixture writes");

        let store = PresentationSettingsStore::new(&path);
        assert_eq!(
            store.load_or_default().activity(),
            ActivityMode::Both,
            "the legacy machine token remains an explicit dual-activity preference"
        );
        assert_eq!(
            fs::read_to_string(&path).expect("legacy fixture rereads"),
            legacy_both,
            "normal reads must never rewrite an existing user choice"
        );
        fs::remove_file(path).expect("legacy fixture removes");
    }

    #[test]
    fn read_only_load_of_absent_settings_creates_no_parent_or_lock() {
        let root = temporary_root("read-only");
        let path = root.join("state").join("config.toml");
        let store = PresentationSettingsStore::new(&path);

        assert_eq!(
            store.load_read_only().expect("read-only defaults load"),
            PresentationSettings::default()
        );
        assert!(!root.exists(), "inspection must not create a state root");
    }

    #[test]
    fn conditional_save_refuses_to_overwrite_a_newer_settings_value() {
        let root = temporary_root("conditional");
        let path = root.join("state").join("config.toml");
        let store = PresentationSettingsStore::new(&path);
        let before = PresentationSettings::default();
        let first = before.with_theme(PresentationTheme::Classic);
        let second = before.with_activity(ActivityMode::Both);

        store.save(before).expect("baseline settings save");
        assert_eq!(
            store
                .save_if_unchanged(before, first)
                .expect("first conditional save"),
            ConditionalSaveOutcome::Saved
        );
        assert_eq!(
            store
                .save_if_unchanged(before, second)
                .expect("stale conditional save"),
            ConditionalSaveOutcome::Conflict
        );
        assert_eq!(store.load().expect("current settings read"), first);
        fs::remove_dir_all(root).expect("fixture root removes");
    }

    #[test]
    fn concurrent_saves_publish_only_complete_parseable_documents() {
        let path = temporary_config("concurrent");
        let store = Arc::new(PresentationSettingsStore::new(&path));
        let count = 6_usize;
        let barrier = Arc::new(Barrier::new(count));
        let workers = (0..count)
            .map(|index| {
                let store = store.clone();
                let barrier = barrier.clone();
                thread::spawn(move || {
                    barrier.wait();
                    let theme = if index % 2 == 0 {
                        PresentationTheme::MutedDark
                    } else {
                        PresentationTheme::Classic
                    };
                    store
                        .save(PresentationSettings::default().with_theme(theme))
                        .expect("concurrent save succeeds");
                })
            })
            .collect::<Vec<_>>();
        for worker in workers {
            worker.join().expect("config writer joins");
        }
        let loaded = store.load().expect("final config parses");
        assert!(matches!(
            loaded.theme(),
            PresentationTheme::MutedDark | PresentationTheme::Classic
        ));
        fs::remove_file(path).expect("fixture config removes");
    }

    #[test]
    fn built_in_spinner_frames_are_nonempty_and_control_free() {
        for preset in [
            SpinnerPreset::Codex,
            SpinnerPreset::Braille,
            SpinnerPreset::Quadrant,
            SpinnerPreset::Line,
            SpinnerPreset::Pulse,
        ] {
            assert!(!preset.frames().is_empty());
            assert!(
                preset
                    .frames()
                    .iter()
                    .all(|frame| !frame.is_empty() && !frame.chars().any(char::is_control))
            );
        }
    }
}
