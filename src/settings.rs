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
};

use atomic_write_file::AtomicWriteFile;
use toml_edit::{DocumentMut, Item, Table, value};

const CONFIG_FILE: &str = "config.toml";
const LOCK_FILE: &str = "config.lock";

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

/// Typed, provider-neutral user presentation choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresentationSettings {
    title: TitleMode,
    tab_color: TabColorMode,
    activity: ActivityMode,
    spinner: SpinnerPreset,
    theme: PresentationTheme,
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
        Self {
            title,
            tab_color,
            activity,
            spinner,
            theme,
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
        lock.lock()?;
        let result = operation();
        File::unlock(&lock)?;
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

fn parse_settings_bytes(bytes: &[u8]) -> Result<PresentationSettings, SettingsError> {
    let document = std::str::from_utf8(bytes)
        .map_err(|_| SettingsError::Malformed)?
        .parse::<DocumentMut>()
        .map_err(|_| SettingsError::Malformed)?;
    settings_from_document(&document)
}

fn settings_from_document(document: &DocumentMut) -> Result<PresentationSettings, SettingsError> {
    let Some(presentation) = document.get("presentation") else {
        return Ok(PresentationSettings::default());
    };
    let table = presentation
        .as_table_like()
        .ok_or(SettingsError::Malformed)?;
    let defaults = PresentationSettings::default();
    Ok(PresentationSettings::new(
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
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{
        ActivityMode, ConditionalSaveOutcome, PresentationSettings, PresentationSettingsStore,
        PresentationTheme, SpinnerPreset, TabColorMode, TitleMode,
    };

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
