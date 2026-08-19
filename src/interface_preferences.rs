//! User-local Human interface preferences.
//!
//! Interface preferences deliberately live beside, but separately from, the
//! existing presentation settings. Reading defaults is side-effect free; an
//! explicit write is serialized, atomic, and preserves future TOML fields.

use std::{
    env, fmt,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

use atomic_write_file::AtomicWriteFile;
use toml_edit::{DocumentMut, Item, Table, value};

const INTERFACE_FILE: &str = "interface.toml";
const LOCK_FILE: &str = "interface.lock";

/// The persisted language choice for Human-only presentation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum InterfaceLanguage {
    /// Resolve from the admitted environment, local preference, and OS locale.
    #[default]
    Auto,
    /// Render Human surfaces in English.
    EnUs,
    /// Render Human surfaces in Simplified Chinese.
    ZhCn,
}

impl InterfaceLanguage {
    /// Stable persisted spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::EnUs => "en-US",
            Self::ZhCn => "zh-CN",
        }
    }

    /// Parses an admitted BCP-47-style locale selection.
    ///
    /// Environment and OS sources often vary only in case, underscore use, or
    /// a POSIX encoding suffix, so those equivalent spellings normalize to the
    /// two supported concrete locales. All other values are refused.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        let normalized = value
            .trim()
            .split(['.', '@'])
            .next()
            .unwrap_or_default()
            .replace('_', "-")
            .to_ascii_lowercase();
        match normalized.as_str() {
            "auto" => Some(Self::Auto),
            "en-us" => Some(Self::EnUs),
            "zh-cn" => Some(Self::ZhCn),
            _ => None,
        }
    }
}

impl fmt::Display for InterfaceLanguage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// The persisted color policy for Human terminal output.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum HumanColor {
    /// Color is used only for an interactive terminal that has not opted out.
    #[default]
    Auto,
    /// Semantic color is emitted even when output is redirected.
    Always,
    /// Human output is always monochrome.
    Never,
}

impl HumanColor {
    /// Stable persisted spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Always => "always",
            Self::Never => "never",
        }
    }

    /// Parses one supported persisted policy spelling.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "auto" => Some(Self::Auto),
            "always" => Some(Self::Always),
            "never" => Some(Self::Never),
            _ => None,
        }
    }
}

impl fmt::Display for HumanColor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Typed user-local Human interface preferences.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InterfacePreferences {
    language: InterfaceLanguage,
    color: HumanColor,
    reduced_motion: bool,
}

impl InterfacePreferences {
    /// Creates a typed preference set.
    #[must_use]
    pub const fn new(language: InterfaceLanguage, color: HumanColor, reduced_motion: bool) -> Self {
        Self {
            language,
            color,
            reduced_motion,
        }
    }

    /// Effective Human locale preference.
    #[must_use]
    pub const fn language(self) -> InterfaceLanguage {
        self.language
    }

    /// Effective Human color preference.
    #[must_use]
    pub const fn color(self) -> HumanColor {
        self.color
    }

    /// Whether future Human animations should be reduced.
    #[must_use]
    pub const fn reduced_motion(self) -> bool {
        self.reduced_motion
    }

    /// Returns a copy with one language choice changed.
    #[must_use]
    pub const fn with_language(self, language: InterfaceLanguage) -> Self {
        Self { language, ..self }
    }

    /// Returns a copy with one color policy changed.
    #[must_use]
    pub const fn with_color(self, color: HumanColor) -> Self {
        Self { color, ..self }
    }

    /// Returns a copy with reduced-motion changed.
    #[must_use]
    pub const fn with_reduced_motion(self, reduced_motion: bool) -> Self {
        Self {
            reduced_motion,
            ..self
        }
    }
}

/// A safe Interface preference storage failure.
#[derive(Debug)]
pub enum InterfacePreferencesError {
    /// No per-user state root can be determined safely.
    StateRootUnavailable,
    /// An underlying file operation failed.
    Io(io::Error),
    /// The document does not match the Interface schema.
    Malformed,
    /// The target is a symbolic link and is never followed for mutation.
    SymbolicLinkTarget,
}

impl fmt::Display for InterfacePreferencesError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::StateRootUnavailable => "a safe per-user TabBeacon Interface path is unavailable",
            Self::Io(_) => "a TabBeacon Interface preference file operation failed",
            Self::Malformed => {
                "the TabBeacon Interface preference file is malformed or unsupported"
            }
            Self::SymbolicLinkTarget => {
                "the TabBeacon Interface preference file is a symbolic link"
            }
        })
    }
}

impl std::error::Error for InterfacePreferencesError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for InterfacePreferencesError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Opaque, byte-exact read-only snapshot of Interface preferences.
pub struct InterfacePreferencesSnapshot {
    preferences: InterfacePreferences,
    contents: Option<Vec<u8>>,
}

impl InterfacePreferencesSnapshot {
    /// Effective typed preferences at the snapshot point.
    #[must_use]
    pub const fn preferences(&self) -> InterfacePreferences {
        self.preferences
    }

    /// Whether the Interface preference file was absent at the snapshot point.
    #[must_use]
    pub const fn is_absent(&self) -> bool {
        self.contents.is_none()
    }

    fn matches(&self, other: &Self) -> bool {
        self.contents == other.contents
    }
}

/// Opaque receipt for a snapshot-guarded Interface preference write.
pub struct InterfacePreferencesWriteReceipt {
    contents: Vec<u8>,
}

impl InterfacePreferencesWriteReceipt {
    fn matches(&self, snapshot: &InterfacePreferencesSnapshot) -> bool {
        snapshot.contents.as_deref() == Some(self.contents.as_slice())
    }
}

/// Result of a snapshot-guarded Interface preference write or restoration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InterfacePreferencesConditionalOutcome {
    /// The exact expected document was current and the operation completed.
    Saved,
    /// Another writer changed the document before the operation.
    Conflict,
}

/// Result of saving an Interface draft against a read-only snapshot.
pub enum InterfacePreferencesSnapshotSaveOutcome {
    /// The draft was saved and can later be restored with the returned receipt.
    Saved(InterfacePreferencesWriteReceipt),
    /// Another writer changed the document before the draft was saved.
    Conflict,
}

/// Process-safe, atomic per-user Interface preference storage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InterfacePreferencesStore {
    path: PathBuf,
}

impl InterfacePreferencesStore {
    /// Creates a store for an explicitly injected Interface path.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Returns the default user-global Interface preference path.
    ///
    /// On Windows this is `%LOCALAPPDATA%\\TabBeacon\\interface.toml`.
    ///
    /// # Errors
    ///
    /// Returns an error when no safe per-user state root is available.
    pub fn from_environment() -> Result<Self, InterfacePreferencesError> {
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
        root.map(|root| Self::new(root.join(INTERFACE_FILE)))
            .ok_or(InterfacePreferencesError::StateRootUnavailable)
    }

    /// Returns the Interface preference location without reading it.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Reads effective preferences without creating a directory, lock, or file.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed TOML, unsafe links, or unreadable state.
    pub fn load_read_only(&self) -> Result<InterfacePreferences, InterfacePreferencesError> {
        Ok(self.snapshot_read_only()?.preferences())
    }

    /// Captures the current document without creating state or a lock.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed TOML, unsafe links, or unreadable state.
    pub fn snapshot_read_only(
        &self,
    ) -> Result<InterfacePreferencesSnapshot, InterfacePreferencesError> {
        self.snapshot_unlocked()
    }

    /// Reads effective preferences, defaulting safely without rewriting malformed input.
    #[must_use]
    pub fn load_or_default(&self) -> InterfacePreferences {
        self.load_read_only().unwrap_or_default()
    }

    /// Atomically saves typed preferences while preserving unknown TOML fields.
    ///
    /// # Errors
    ///
    /// Returns a storage error without modifying an unsafe or malformed document.
    pub fn save(&self, preferences: InterfacePreferences) -> Result<(), InterfacePreferencesError> {
        self.with_lock(|| {
            let snapshot = self.snapshot_unlocked()?;
            self.save_snapshot_unlocked(&snapshot, preferences)?;
            Ok(())
        })
    }

    /// Saves a draft only when the exact read-only document is still current.
    ///
    /// # Errors
    ///
    /// Returns a storage error without modifying an unsafe or malformed document.
    pub fn save_snapshot_if_unchanged(
        &self,
        expected: &InterfacePreferencesSnapshot,
        replacement: InterfacePreferences,
    ) -> Result<InterfacePreferencesSnapshotSaveOutcome, InterfacePreferencesError> {
        self.with_lock(|| {
            let current = self.snapshot_unlocked()?;
            if !current.matches(expected) {
                return Ok(InterfacePreferencesSnapshotSaveOutcome::Conflict);
            }
            Ok(InterfacePreferencesSnapshotSaveOutcome::Saved(
                self.save_snapshot_unlocked(&current, replacement)?,
            ))
        })
    }

    /// Restores an original snapshot only when the prior write remains exact.
    ///
    /// # Errors
    ///
    /// Returns a storage error without modifying an unsafe or malformed document.
    pub fn restore_snapshot_if_unchanged(
        &self,
        receipt: &InterfacePreferencesWriteReceipt,
        original: &InterfacePreferencesSnapshot,
    ) -> Result<InterfacePreferencesConditionalOutcome, InterfacePreferencesError> {
        self.with_lock(|| {
            let current = self.snapshot_unlocked()?;
            if !receipt.matches(&current) {
                return Ok(InterfacePreferencesConditionalOutcome::Conflict);
            }
            self.restore_snapshot_unlocked(original)?;
            Ok(InterfacePreferencesConditionalOutcome::Saved)
        })
    }

    fn with_lock<T>(
        &self,
        operation: impl FnOnce() -> Result<T, InterfacePreferencesError>,
    ) -> Result<T, InterfacePreferencesError> {
        let parent = self
            .path
            .parent()
            .ok_or(InterfacePreferencesError::StateRootUnavailable)?;
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

    fn snapshot_unlocked(&self) -> Result<InterfacePreferencesSnapshot, InterfacePreferencesError> {
        self.reject_symbolic_link()?;
        let contents = read_optional_bytes(&self.path)?;
        let preferences = match contents.as_deref() {
            Some(bytes) => preferences_from_bytes(bytes)?,
            None => InterfacePreferences::default(),
        };
        Ok(InterfacePreferencesSnapshot {
            preferences,
            contents,
        })
    }

    fn save_snapshot_unlocked(
        &self,
        snapshot: &InterfacePreferencesSnapshot,
        preferences: InterfacePreferences,
    ) -> Result<InterfacePreferencesWriteReceipt, InterfacePreferencesError> {
        let mut document = match snapshot.contents.as_deref() {
            Some(bytes) => std::str::from_utf8(bytes)
                .map_err(|_| InterfacePreferencesError::Malformed)?
                .parse::<DocumentMut>()
                .map_err(|_| InterfacePreferencesError::Malformed)?,
            None => DocumentMut::new(),
        };
        write_preferences(&mut document, preferences)?;
        let contents = document.to_string().into_bytes();
        atomic_write(&self.path, &contents)?;
        Ok(InterfacePreferencesWriteReceipt { contents })
    }

    fn restore_snapshot_unlocked(
        &self,
        snapshot: &InterfacePreferencesSnapshot,
    ) -> Result<(), InterfacePreferencesError> {
        self.reject_symbolic_link()?;
        match snapshot.contents.as_deref() {
            Some(contents) => atomic_write(&self.path, contents)?,
            None => fs::remove_file(&self.path)?,
        }
        Ok(())
    }

    fn reject_symbolic_link(&self) -> Result<(), InterfacePreferencesError> {
        match fs::symlink_metadata(&self.path) {
            Ok(metadata) => ensure_not_symbolic_link(metadata.file_type().is_symlink()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

fn ensure_not_symbolic_link(is_symbolic_link: bool) -> Result<(), InterfacePreferencesError> {
    if is_symbolic_link {
        Err(InterfacePreferencesError::SymbolicLinkTarget)
    } else {
        Ok(())
    }
}

fn preferences_from_bytes(bytes: &[u8]) -> Result<InterfacePreferences, InterfacePreferencesError> {
    let document = std::str::from_utf8(bytes)
        .map_err(|_| InterfacePreferencesError::Malformed)?
        .parse::<DocumentMut>()
        .map_err(|_| InterfacePreferencesError::Malformed)?;
    preferences_from_document(&document)
}

fn preferences_from_document(
    document: &DocumentMut,
) -> Result<InterfacePreferences, InterfacePreferencesError> {
    let Some(interface) = document.get("interface") else {
        return Ok(InterfacePreferences::default());
    };
    let table = interface
        .as_table_like()
        .ok_or(InterfacePreferencesError::Malformed)?;
    let defaults = InterfacePreferences::default();
    Ok(InterfacePreferences::new(
        parse_value(
            table.get("language"),
            InterfaceLanguage::parse,
            defaults.language(),
        )?,
        parse_value(table.get("color"), HumanColor::parse, defaults.color())?,
        parse_boolean(table.get("reduced_motion"), defaults.reduced_motion())?,
    ))
}

fn parse_value<T: Copy>(
    value: Option<&Item>,
    parse: impl Fn(&str) -> Option<T>,
    default: T,
) -> Result<T, InterfacePreferencesError> {
    let Some(value) = value else {
        return Ok(default);
    };
    value
        .as_str()
        .and_then(parse)
        .ok_or(InterfacePreferencesError::Malformed)
}

fn parse_boolean(value: Option<&Item>, default: bool) -> Result<bool, InterfacePreferencesError> {
    let Some(value) = value else {
        return Ok(default);
    };
    value.as_bool().ok_or(InterfacePreferencesError::Malformed)
}

fn write_preferences(
    document: &mut DocumentMut,
    preferences: InterfacePreferences,
) -> Result<(), InterfacePreferencesError> {
    if !document.as_table().contains_key("interface") {
        document["interface"] = Item::Table(Table::new());
    }
    let table = document["interface"]
        .as_table_like_mut()
        .ok_or(InterfacePreferencesError::Malformed)?;
    table.insert("language", value(preferences.language().as_str()));
    table.insert("color", value(preferences.color().as_str()));
    table.insert("reduced_motion", value(preferences.reduced_motion()));
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
        HumanColor, InterfaceLanguage, InterfacePreferences,
        InterfacePreferencesConditionalOutcome, InterfacePreferencesSnapshotSaveOutcome,
        InterfacePreferencesStore, ensure_not_symbolic_link,
    };

    fn temporary_root(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "tabbeacon-interface-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock is after Unix epoch")
                .as_nanos()
        ))
    }

    #[test]
    fn absent_read_is_non_mutating_and_uses_documented_defaults() {
        let root = temporary_root("absent");
        let path = root.join("state").join("interface.toml");
        let store = InterfacePreferencesStore::new(&path);

        assert_eq!(
            store.load_read_only().expect("defaults load"),
            InterfacePreferences::default()
        );
        assert!(!root.exists(), "a passive read creates no state or lock");
    }

    #[test]
    fn save_preserves_unknown_root_and_interface_fields() {
        let root = temporary_root("unknown");
        let path = root.join("state").join("interface.toml");
        fs::create_dir_all(path.parent().expect("state parent")).expect("state root creates");
        fs::write(
            &path,
            "future_root = \"kept\"\n\n[interface]\nfuture_flag = true\nlanguage = \"auto\"\n",
        )
        .expect("fixture writes");
        let store = InterfacePreferencesStore::new(&path);
        let preferences =
            InterfacePreferences::new(InterfaceLanguage::ZhCn, HumanColor::Never, true);

        store.save(preferences).expect("atomic save");
        assert_eq!(store.load_read_only().expect("typed reread"), preferences);
        let text = fs::read_to_string(&path).expect("saved text reads");
        assert!(text.contains("future_root = \"kept\""));
        assert!(text.contains("future_flag = true"));
        fs::remove_dir_all(root).expect("fixture root removes");
    }

    #[test]
    fn malformed_document_falls_back_only_in_load_or_default_without_rewrite() {
        let root = temporary_root("malformed");
        let path = root.join("state").join("interface.toml");
        fs::create_dir_all(path.parent().expect("state parent")).expect("state root creates");
        let original = b"[interface]\nlanguage = [\"invalid\"\n";
        fs::write(&path, original).expect("malformed fixture writes");
        let store = InterfacePreferencesStore::new(&path);

        assert!(
            store.load_read_only().is_err(),
            "strict read exposes malformed state"
        );
        assert_eq!(store.load_or_default(), InterfacePreferences::default());
        assert_eq!(fs::read(&path).expect("original bytes read"), original);
        fs::remove_dir_all(root).expect("fixture root removes");
    }

    #[test]
    fn snapshot_restore_recovers_absence_without_overwriting_drift() {
        let root = temporary_root("restore");
        let path = root.join("state").join("interface.toml");
        let store = InterfacePreferencesStore::new(&path);
        let original = store.snapshot_read_only().expect("absent snapshot");
        let draft = InterfacePreferences::default().with_language(InterfaceLanguage::ZhCn);
        let InterfacePreferencesSnapshotSaveOutcome::Saved(receipt) = store
            .save_snapshot_if_unchanged(&original, draft)
            .expect("snapshot write")
        else {
            panic!("absent snapshot writes")
        };
        assert!(
            path.exists(),
            "explicit save creates the Interface document"
        );
        assert_eq!(
            store
                .restore_snapshot_if_unchanged(&receipt, &original)
                .expect("snapshot restore"),
            InterfacePreferencesConditionalOutcome::Saved
        );
        assert!(
            !path.exists(),
            "restoring an absent snapshot removes only this file"
        );

        let before = store.snapshot_read_only().expect("second absent snapshot");
        let InterfacePreferencesSnapshotSaveOutcome::Saved(receipt) = store
            .save_snapshot_if_unchanged(&before, draft)
            .expect("second snapshot write")
        else {
            panic!("second absent snapshot writes")
        };
        store
            .save(InterfacePreferences::default().with_color(HumanColor::Always))
            .expect("concurrent writer saves");
        assert_eq!(
            store
                .restore_snapshot_if_unchanged(&receipt, &before)
                .expect("stale rollback evaluates"),
            InterfacePreferencesConditionalOutcome::Conflict
        );
        assert_eq!(
            store.load_read_only().expect("concurrent value remains"),
            InterfacePreferences::default().with_color(HumanColor::Always)
        );
        fs::remove_dir_all(root).expect("fixture root removes");
    }

    #[test]
    fn snapshot_restore_recovers_the_original_present_bytes() {
        let root = temporary_root("restore-present");
        let path = root.join("state").join("interface.toml");
        fs::create_dir_all(path.parent().expect("state parent")).expect("state root creates");
        let original =
            b"future_root = \"keep\"\n\n[interface]\nlanguage = \"en-US\"\nfuture = true\n";
        fs::write(&path, original).expect("original fixture writes");
        let store = InterfacePreferencesStore::new(&path);
        let snapshot = store.snapshot_read_only().expect("original snapshot");
        let replacement = snapshot
            .preferences()
            .with_language(InterfaceLanguage::ZhCn);
        let InterfacePreferencesSnapshotSaveOutcome::Saved(receipt) = store
            .save_snapshot_if_unchanged(&snapshot, replacement)
            .expect("snapshot write")
        else {
            panic!("present snapshot writes")
        };

        assert_eq!(
            store
                .restore_snapshot_if_unchanged(&receipt, &snapshot)
                .expect("present snapshot restores"),
            InterfacePreferencesConditionalOutcome::Saved
        );
        assert_eq!(fs::read(&path).expect("restored bytes read"), original);
        fs::remove_dir_all(root).expect("fixture root removes");
    }

    #[test]
    fn byte_exact_snapshot_refuses_unknown_field_drift() {
        let root = temporary_root("drift");
        let path = root.join("state").join("interface.toml");
        let store = InterfacePreferencesStore::new(&path);
        store
            .save(InterfacePreferences::default())
            .expect("baseline save");
        let snapshot = store.snapshot_read_only().expect("baseline snapshot");
        fs::write(
            &path,
            "[interface]\nlanguage = \"auto\"\ncolor = \"auto\"\nreduced_motion = false\nfuture = \"changed\"\n",
        )
        .expect("same typed values but altered bytes write");

        assert!(matches!(
            store
                .save_snapshot_if_unchanged(
                    &snapshot,
                    InterfacePreferences::default().with_color(HumanColor::Never)
                )
                .expect("drift evaluates"),
            InterfacePreferencesSnapshotSaveOutcome::Conflict
        ));
        fs::remove_dir_all(root).expect("fixture root removes");
    }

    #[test]
    fn concurrent_saves_leave_one_complete_parseable_document() {
        let root = temporary_root("concurrent");
        let path = root.join("state").join("interface.toml");
        let store = Arc::new(InterfacePreferencesStore::new(&path));
        let count = 4_usize;
        let barrier = Arc::new(Barrier::new(count));
        let workers = (0..count)
            .map(|index| {
                let store = store.clone();
                let barrier = barrier.clone();
                thread::spawn(move || {
                    barrier.wait();
                    let language = if index % 2 == 0 {
                        InterfaceLanguage::EnUs
                    } else {
                        InterfaceLanguage::ZhCn
                    };
                    store
                        .save(InterfacePreferences::default().with_language(language))
                        .expect("serialized write succeeds");
                })
            })
            .collect::<Vec<_>>();
        for worker in workers {
            worker.join().expect("writer joins");
        }
        assert!(matches!(
            store
                .load_read_only()
                .expect("final document parses")
                .language(),
            InterfaceLanguage::EnUs | InterfaceLanguage::ZhCn
        ));
        fs::remove_dir_all(root).expect("fixture root removes");
    }

    #[test]
    fn symbolic_link_targets_are_refused_before_mutation() {
        assert!(ensure_not_symbolic_link(false).is_ok());
        assert!(matches!(
            ensure_not_symbolic_link(true),
            Err(super::InterfacePreferencesError::SymbolicLinkTarget)
        ));
    }
}
