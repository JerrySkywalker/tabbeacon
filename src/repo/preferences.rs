//! Device-local explicit workspace alias preferences.
//!
//! This store intentionally contains only user overrides. Generated aliases
//! and their policy versions remain in the repository registry, so resetting
//! an override never rewrites identity history.

use std::{
    collections::{BTreeMap, BTreeSet},
    env, fmt,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

use atomic_write_file::AtomicWriteFile;
use serde::{Deserialize, Serialize};

use super::{CanonicalRepositoryIdentity, RepositoryAlias};

const PREFERENCES_FILE: &str = "preferences-v1.json";
const LOCK_FILE: &str = "preferences.lock";
const PREFERENCES_SCHEMA: &str = "tabbeacon-workspace-preferences-v1";

/// A safe failure from device-local workspace preference storage.
#[derive(Debug)]
pub enum WorkspacePreferenceError {
    /// No safe user-local state root was available.
    StateRootUnavailable,
    /// A storage operation failed.
    Io(io::Error),
    /// Existing state was malformed or violated the preference schema.
    Malformed,
    /// A preference target was a symbolic link and was not followed.
    SymbolicLinkTarget,
}

impl fmt::Display for WorkspacePreferenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::StateRootUnavailable => {
                "a safe per-user TabBeacon workspace preference path is unavailable"
            }
            Self::Io(_) => "a TabBeacon workspace preference file operation failed",
            Self::Malformed => {
                "the TabBeacon workspace preference file is malformed or unsupported"
            }
            Self::SymbolicLinkTarget => {
                "the TabBeacon workspace preference file is a symbolic link"
            }
        })
    }
}

impl std::error::Error for WorkspacePreferenceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for WorkspacePreferenceError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for WorkspacePreferenceError {
    fn from(_error: serde_json::Error) -> Self {
        Self::Malformed
    }
}

/// Typed explicit overrides, deliberately separate from generated aliases.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WorkspacePreferences {
    overrides: BTreeMap<CanonicalRepositoryIdentity, RepositoryAlias>,
}

impl WorkspacePreferences {
    /// Returns the current explicit alias for one canonical identity.
    #[must_use]
    pub fn override_for(&self, identity: &CanonicalRepositoryIdentity) -> Option<RepositoryAlias> {
        self.overrides.get(identity).cloned()
    }

    /// Returns the aliases reserved by explicit user choice.
    #[must_use]
    pub fn override_aliases(&self) -> BTreeSet<RepositoryAlias> {
        self.overrides.values().cloned().collect()
    }

    /// Returns the internal canonical identities for coordination code. Human
    /// renderers must never display these device-local/private identities.
    #[must_use]
    pub fn identities(&self) -> BTreeSet<CanonicalRepositoryIdentity> {
        self.overrides.keys().cloned().collect()
    }

    /// Returns a copy with an identity-local override set.
    #[must_use]
    pub fn with_override(
        mut self,
        identity: CanonicalRepositoryIdentity,
        alias: RepositoryAlias,
    ) -> Self {
        self.overrides.insert(identity, alias);
        self
    }

    /// Returns a copy with only one identity-local override removed.
    #[must_use]
    pub fn without_override(mut self, identity: &CanonicalRepositoryIdentity) -> Self {
        self.overrides.remove(identity);
        self
    }

    /// Whether one explicit override is present for an identity.
    #[must_use]
    pub fn has_override(&self, identity: &CanonicalRepositoryIdentity) -> bool {
        self.overrides.contains_key(identity)
    }

    pub(crate) fn overrides(&self) -> &BTreeMap<CanonicalRepositoryIdentity, RepositoryAlias> {
        &self.overrides
    }
}

/// Opaque exact-byte snapshot used for optimistic write and restore checks.
pub struct WorkspacePreferencesSnapshot {
    preferences: WorkspacePreferences,
    contents: Option<Vec<u8>>,
}

impl WorkspacePreferencesSnapshot {
    /// Typed preferences at the snapshot point.
    #[must_use]
    pub fn preferences(&self) -> &WorkspacePreferences {
        &self.preferences
    }

    /// Whether no preference document existed at the snapshot point.
    #[must_use]
    pub const fn is_absent(&self) -> bool {
        self.contents.is_none()
    }

    fn matches(&self, other: &Self) -> bool {
        self.contents == other.contents
    }
}

/// Opaque receipt binding a conditional restoration to one exact write.
pub struct WorkspacePreferencesWriteReceipt {
    contents: Vec<u8>,
}

impl WorkspacePreferencesWriteReceipt {
    fn matches(&self, snapshot: &WorkspacePreferencesSnapshot) -> bool {
        snapshot.contents.as_deref() == Some(self.contents.as_slice())
    }
}

/// Outcome of one byte-exact conditional preference operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspacePreferencesConditionalOutcome {
    /// The expected document was still current and the operation completed.
    Saved,
    /// A concurrent writer changed the document first.
    Conflict,
}

/// Outcome of saving a draft against a read-only snapshot.
pub enum WorkspacePreferencesSnapshotSaveOutcome {
    /// The draft was saved and can be conditionally restored with this receipt.
    Saved(WorkspacePreferencesWriteReceipt),
    /// A concurrent writer changed the document first.
    Conflict,
}

/// Atomic, process-safe device-local workspace preference storage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspacePreferenceStore {
    path: PathBuf,
}

impl WorkspacePreferenceStore {
    /// Creates a store for an explicitly injected preference document path.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Derives the sibling user-local preference path for one registry root.
    #[must_use]
    pub(crate) fn for_registry_state_root(registry_root: &Path) -> Self {
        Self::new(
            registry_root
                .parent()
                .unwrap_or(registry_root)
                .join("workspace-preferences")
                .join(PREFERENCES_FILE),
        )
    }

    /// Returns the user-global workspace preference store.
    ///
    /// On Windows this is `%LOCALAPPDATA%\\TabBeacon\\workspace-preferences\\preferences-v1.json`.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspacePreferenceError::StateRootUnavailable`] when the
    /// platform does not expose a safe per-user state root.
    pub fn from_environment() -> Result<Self, WorkspacePreferenceError> {
        #[cfg(windows)]
        let root = env::var_os("LOCALAPPDATA")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .map(|path| path.join("TabBeacon").join("workspace-preferences"));
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
            .map(|path| path.join("tabbeacon").join("workspace-preferences"));
        root.map(|root| Self::new(root.join(PREFERENCES_FILE)))
            .ok_or(WorkspacePreferenceError::StateRootUnavailable)
    }

    /// Returns the preference location without reading it.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Reads typed preferences without creating a directory, file, or lock.
    ///
    /// # Errors
    ///
    /// Returns an error when the existing preference file is unsafe, malformed,
    /// or cannot be read.
    pub fn load_read_only(&self) -> Result<WorkspacePreferences, WorkspacePreferenceError> {
        Ok(self.snapshot_read_only()?.preferences)
    }

    /// Captures a byte-exact snapshot without creating state or a lock.
    ///
    /// # Errors
    ///
    /// Returns an error when the existing preference file is unsafe, malformed,
    /// or cannot be read.
    pub fn snapshot_read_only(
        &self,
    ) -> Result<WorkspacePreferencesSnapshot, WorkspacePreferenceError> {
        self.snapshot_unlocked()
    }

    /// Returns whether an opaque snapshot is still byte-exactly current.
    ///
    /// This is deliberately read-only so rollback verification cannot create
    /// state while reporting an import failure.
    ///
    /// # Errors
    ///
    /// Returns the same safe read error as [`Self::snapshot_read_only`].
    pub fn snapshot_is_current(
        &self,
        expected: &WorkspacePreferencesSnapshot,
    ) -> Result<bool, WorkspacePreferenceError> {
        Ok(self.snapshot_read_only()?.matches(expected))
    }

    /// Returns whether a guided write receipt is still byte-exactly current.
    ///
    /// # Errors
    ///
    /// Returns the same safe read error as [`Self::snapshot_read_only`].
    pub fn write_receipt_is_current(
        &self,
        receipt: &WorkspacePreferencesWriteReceipt,
    ) -> Result<bool, WorkspacePreferenceError> {
        Ok(receipt.matches(&self.snapshot_read_only()?))
    }

    /// Saves a replacement only when the original bytes are still current.
    ///
    /// # Errors
    ///
    /// Returns an error when the preference target cannot be safely locked or
    /// atomically written. A byte mismatch is returned as `Conflict`.
    pub fn save_snapshot_if_unchanged(
        &self,
        expected: &WorkspacePreferencesSnapshot,
        replacement: WorkspacePreferences,
    ) -> Result<WorkspacePreferencesSnapshotSaveOutcome, WorkspacePreferenceError> {
        self.with_lock(|| {
            let current = self.snapshot_unlocked()?;
            if !current.matches(expected) {
                return Ok(WorkspacePreferencesSnapshotSaveOutcome::Conflict);
            }
            Ok(WorkspacePreferencesSnapshotSaveOutcome::Saved(
                self.save_snapshot_unlocked(replacement)?,
            ))
        })
    }

    /// Restores an original snapshot only when the prior write remains exact.
    ///
    /// # Errors
    ///
    /// Returns an error when the preference target cannot be safely locked or
    /// restored. A byte mismatch is returned as `Conflict`.
    pub fn restore_snapshot_if_unchanged(
        &self,
        receipt: &WorkspacePreferencesWriteReceipt,
        original: &WorkspacePreferencesSnapshot,
    ) -> Result<WorkspacePreferencesConditionalOutcome, WorkspacePreferenceError> {
        self.with_lock(|| {
            let current = self.snapshot_unlocked()?;
            if !receipt.matches(&current) {
                return Ok(WorkspacePreferencesConditionalOutcome::Conflict);
            }
            self.restore_snapshot_unlocked(original)?;
            Ok(WorkspacePreferencesConditionalOutcome::Saved)
        })
    }

    fn with_lock<T>(
        &self,
        operation: impl FnOnce() -> Result<T, WorkspacePreferenceError>,
    ) -> Result<T, WorkspacePreferenceError> {
        let parent = self
            .path
            .parent()
            .ok_or(WorkspacePreferenceError::StateRootUnavailable)?;
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

    fn snapshot_unlocked(&self) -> Result<WorkspacePreferencesSnapshot, WorkspacePreferenceError> {
        self.reject_symbolic_link()?;
        let contents = read_optional_bytes(&self.path)?;
        let preferences = match contents.as_deref() {
            Some(bytes) => preferences_from_bytes(bytes)?,
            None => WorkspacePreferences::default(),
        };
        Ok(WorkspacePreferencesSnapshot {
            preferences,
            contents,
        })
    }

    fn save_snapshot_unlocked(
        &self,
        preferences: WorkspacePreferences,
    ) -> Result<WorkspacePreferencesWriteReceipt, WorkspacePreferenceError> {
        let contents = serde_json::to_vec_pretty(&SerializablePreferences::from(preferences))?;
        atomic_write(&self.path, &contents)?;
        Ok(WorkspacePreferencesWriteReceipt { contents })
    }

    fn restore_snapshot_unlocked(
        &self,
        snapshot: &WorkspacePreferencesSnapshot,
    ) -> Result<(), WorkspacePreferenceError> {
        self.reject_symbolic_link()?;
        match snapshot.contents.as_deref() {
            Some(contents) => atomic_write(&self.path, contents)?,
            None => match fs::remove_file(&self.path) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            },
        }
        Ok(())
    }

    fn reject_symbolic_link(&self) -> Result<(), WorkspacePreferenceError> {
        match fs::symlink_metadata(&self.path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                Err(WorkspacePreferenceError::SymbolicLinkTarget)
            }
            Ok(_) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SerializablePreferences {
    schema: String,
    overrides: BTreeMap<String, String>,
}

impl From<WorkspacePreferences> for SerializablePreferences {
    fn from(preferences: WorkspacePreferences) -> Self {
        Self {
            schema: PREFERENCES_SCHEMA.to_owned(),
            overrides: preferences
                .overrides
                .into_iter()
                .map(|(identity, alias)| (identity.as_str().to_owned(), alias.as_str().to_owned()))
                .collect(),
        }
    }
}

fn preferences_from_bytes(bytes: &[u8]) -> Result<WorkspacePreferences, WorkspacePreferenceError> {
    let serialized = serde_json::from_slice::<SerializablePreferences>(bytes)
        .map_err(|_| WorkspacePreferenceError::Malformed)?;
    if serialized.schema != PREFERENCES_SCHEMA {
        return Err(WorkspacePreferenceError::Malformed);
    }
    let mut overrides = BTreeMap::new();
    let mut aliases = BTreeSet::new();
    for (identity, alias) in serialized.overrides {
        let identity = CanonicalRepositoryIdentity::new(identity)
            .map_err(|_| WorkspacePreferenceError::Malformed)?;
        let alias = RepositoryAlias::new(alias).map_err(|_| WorkspacePreferenceError::Malformed)?;
        if !aliases.insert(alias.clone()) {
            return Err(WorkspacePreferenceError::Malformed);
        }
        overrides.insert(identity, alias);
    }
    Ok(WorkspacePreferences { overrides })
}

fn read_optional_bytes(path: &Path) -> Result<Option<Vec<u8>>, io::Error> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "workspace preference target has no parent",
        )
    })?;
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
        WorkspacePreferenceStore, WorkspacePreferencesConditionalOutcome,
        WorkspacePreferencesSnapshotSaveOutcome,
    };
    use crate::repo::{CanonicalRepositoryIdentity, RepositoryAlias};

    fn temporary_root(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "tabbeacon-workspace-preferences-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock is after Unix epoch")
                .as_nanos()
        ))
    }

    fn identity(value: &str) -> CanonicalRepositoryIdentity {
        CanonicalRepositoryIdentity::new(format!("remote:example/{value}"))
            .expect("identity fixture is valid")
    }

    fn alias(value: &str) -> RepositoryAlias {
        RepositoryAlias::new(value).expect("alias fixture is valid")
    }

    #[test]
    fn absent_read_is_non_mutating() {
        let root = temporary_root("absent");
        let store = WorkspacePreferenceStore::new(root.join("state/preferences-v1.json"));

        assert!(
            store
                .load_read_only()
                .expect("defaults load")
                .overrides()
                .is_empty()
        );
        assert!(
            !root.exists(),
            "passive reads must not create a root or lock"
        );
    }

    #[test]
    fn exact_byte_drift_refuses_a_stale_write_and_restore() {
        let root = temporary_root("drift");
        let path = root.join("state/preferences-v1.json");
        let store = WorkspacePreferenceStore::new(&path);
        let original = store.snapshot_read_only().expect("absent snapshot");
        let first = original
            .preferences()
            .clone()
            .with_override(identity("one"), alias("ONE"));
        let receipt = match store
            .save_snapshot_if_unchanged(&original, first)
            .expect("first save")
        {
            WorkspacePreferencesSnapshotSaveOutcome::Saved(receipt) => receipt,
            WorkspacePreferencesSnapshotSaveOutcome::Conflict => panic!("first save conflicts"),
        };
        fs::write(
            &path,
            "{\n  \"schema\": \"tabbeacon-workspace-preferences-v1\",\n  \"overrides\": {\n    \"remote:example/one\": \"ONE\"\n  }\n}\n",
        )
        .expect("same typed state with different bytes writes");

        assert!(matches!(
            store
                .save_snapshot_if_unchanged(&original, original.preferences().clone())
                .expect("stale save checks"),
            WorkspacePreferencesSnapshotSaveOutcome::Conflict
        ));
        assert_eq!(
            store
                .restore_snapshot_if_unchanged(&receipt, &original)
                .expect("stale restore checks"),
            WorkspacePreferencesConditionalOutcome::Conflict
        );
        fs::remove_dir_all(root).expect("test root removes");
    }

    #[test]
    fn reset_style_map_edit_preserves_other_overrides() {
        let root = temporary_root("preserve");
        let path = root.join("state/preferences-v1.json");
        let store = WorkspacePreferenceStore::new(&path);
        let snapshot = store.snapshot_read_only().expect("absent snapshot");
        let both = snapshot
            .preferences()
            .clone()
            .with_override(identity("one"), alias("ONE"))
            .with_override(identity("two"), alias("TWO"));
        let _ = store
            .save_snapshot_if_unchanged(&snapshot, both)
            .expect("both save");
        let snapshot = store.snapshot_read_only().expect("saved snapshot");
        let replacement = snapshot
            .preferences()
            .clone()
            .without_override(&identity("one"));
        let _ = store
            .save_snapshot_if_unchanged(&snapshot, replacement)
            .expect("one override clears");
        let final_state = store.load_read_only().expect("final preferences read");
        assert!(!final_state.has_override(&identity("one")));
        assert_eq!(
            final_state.override_for(&identity("two")),
            Some(alias("TWO"))
        );
        fs::remove_dir_all(root).expect("test root removes");
    }

    #[test]
    fn malformed_or_unknown_document_fails_closed_without_rewrite() {
        let root = temporary_root("malformed");
        let path = root.join("state/preferences-v1.json");
        fs::create_dir_all(path.parent().expect("fixture parent")).expect("fixture parent creates");
        let contents = br#"{
  "schema": "tabbeacon-workspace-preferences-v1",
  "overrides": {},
  "future_field": true
}"#;
        fs::write(&path, contents).expect("malformed fixture writes");
        let store = WorkspacePreferenceStore::new(&path);

        assert!(store.snapshot_read_only().is_err());
        assert_eq!(fs::read(&path).expect("fixture remains readable"), contents);
        fs::remove_dir_all(root).expect("test root removes");
    }

    #[test]
    fn receipt_guarded_restore_returns_an_absent_original_document() {
        let root = temporary_root("restore-absent");
        let path = root.join("state/preferences-v1.json");
        let store = WorkspacePreferenceStore::new(&path);
        let original = store.snapshot_read_only().expect("absent snapshot");
        let replacement = original
            .preferences()
            .clone()
            .with_override(identity("one"), alias("ONE"));
        let receipt = match store
            .save_snapshot_if_unchanged(&original, replacement)
            .expect("save succeeds")
        {
            WorkspacePreferencesSnapshotSaveOutcome::Saved(receipt) => receipt,
            WorkspacePreferencesSnapshotSaveOutcome::Conflict => panic!("initial save conflicts"),
        };
        assert!(path.exists(), "write creates the explicit preference file");
        assert_eq!(
            store
                .restore_snapshot_if_unchanged(&receipt, &original)
                .expect("restore returns an outcome"),
            WorkspacePreferencesConditionalOutcome::Saved
        );
        assert!(
            !path.exists(),
            "restore removes only the newly created file"
        );
        fs::remove_dir_all(root).expect("test root removes");
    }

    #[test]
    fn concurrent_snapshot_writers_leave_one_parseable_document() {
        let root = temporary_root("concurrent");
        let store = Arc::new(WorkspacePreferenceStore::new(
            root.join("state/preferences-v1.json"),
        ));
        let barrier = Arc::new(Barrier::new(2));
        let workers = (0..2)
            .map(|index| {
                let store = store.clone();
                let barrier = barrier.clone();
                thread::spawn(move || {
                    let snapshot = store.snapshot_read_only().expect("snapshot reads");
                    barrier.wait();
                    let replacement = snapshot
                        .preferences()
                        .clone()
                        .with_override(identity(&format!("worker-{index}")), alias("WORK"));
                    store
                        .save_snapshot_if_unchanged(&snapshot, replacement)
                        .expect("conditional save returns")
                })
            })
            .collect::<Vec<_>>();
        let outcomes = workers
            .into_iter()
            .map(|worker| worker.join().expect("worker joins"))
            .collect::<Vec<_>>();
        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| matches!(
                    outcome,
                    WorkspacePreferencesSnapshotSaveOutcome::Saved(_)
                ))
                .count(),
            1
        );
        assert!(store.load_read_only().is_ok(), "winning document parses");
        fs::remove_dir_all(root).expect("test root removes");
    }
}
