use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{
    ADAPTIVE_NAMING_POLICY_ID, AdaptiveNamingPolicy, CanonicalRepositoryIdentity, RepositoryAlias,
    RepositoryDisplayName, RepositoryIdentityError,
};

const REGISTRY_V1_SCHEMA: &str = "tabbeacon-repository-alias-registry-v1";
const REGISTRY_V2_SCHEMA: &str = "tabbeacon-repository-alias-registry-v2";
const V1_SNAPSHOT_PREFIX: &str = "registry-v1-";
const V2_SNAPSHOT_PREFIX: &str = "registry-v2-";
const SNAPSHOT_SUFFIX: &str = ".json";
const LOCK_FILE: &str = "registry.lock";
const LEGACY_POLICY_VERSION: &str = "legacy-preserved-v1";
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Complete resolved identity returned to higher layers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRepositoryIdentity {
    /// Canonical repository key, never an agent session key.
    pub identity: CanonicalRepositoryIdentity,
    /// Safe human name used for alias policy.
    pub display_name: RepositoryDisplayName,
    /// Stable locally assigned short alias.
    pub alias: RepositoryAlias,
    /// Root of the current worktree.
    pub worktree_root: PathBuf,
    /// Common Git directory shared by linked worktrees.
    pub git_common_dir: PathBuf,
}

/// One generated alias retained independently from an optional user override.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryAssignment {
    generated_alias: RepositoryAlias,
    policy_version: String,
}

impl RegistryAssignment {
    /// Alias allocated by the deterministic registry policy.
    #[must_use]
    pub fn generated_alias(&self) -> &RepositoryAlias {
        &self.generated_alias
    }

    /// Stable policy marker explaining the generation source.
    #[must_use]
    pub fn policy_version(&self) -> &str {
        &self.policy_version
    }
}

/// Process-safe registry backed by immutable, atomically published generations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StableAliasRegistry {
    root: PathBuf,
}

/// Safe health classification for a read-only alias-registry inspection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AliasRegistryHealth {
    /// No registry has been created for this user yet.
    Absent,
    /// The latest immutable registry generation is valid.
    Healthy,
    /// Published generations exist but none can be validated safely.
    Corrupt,
    /// The registry location cannot be inspected safely.
    Unavailable,
}

impl AliasRegistryHealth {
    /// Stable machine-oriented spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Absent => "absent",
            Self::Healthy => "healthy",
            Self::Corrupt => "corrupt",
            Self::Unavailable => "unavailable",
        }
    }
}

/// Content-minimal aggregate of the alias registry.
///
/// It intentionally exposes only health and assignment count, never aliases,
/// canonical identities, workspace roots, or registry paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AliasRegistryDiagnostics {
    health: AliasRegistryHealth,
    assignment_count: Option<usize>,
}

impl AliasRegistryDiagnostics {
    /// Overall read-only inspection health.
    #[must_use]
    pub const fn health(self) -> AliasRegistryHealth {
        self.health
    }

    /// Number of aliases in the newest valid generation, when available.
    #[must_use]
    pub const fn assignment_count(self) -> Option<usize> {
        self.assignment_count
    }
}

impl StableAliasRegistry {
    /// Creates a registry rooted at an explicitly injected local directory.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Returns the platform-appropriate per-user `TabBeacon` state directory.
    ///
    /// # Errors
    ///
    /// Returns [`RepositoryIdentityError::StateRootUnavailable`] when the
    /// platform environment does not expose a safe per-user state location.
    pub fn default_state_root() -> Result<PathBuf, RepositoryIdentityError> {
        #[cfg(windows)]
        {
            env::var_os("LOCALAPPDATA")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
                .map(|path| path.join("TabBeacon").join("repository-identity"))
                .ok_or(RepositoryIdentityError::StateRootUnavailable)
        }
        #[cfg(not(windows))]
        {
            if let Some(state) = env::var_os("XDG_STATE_HOME").filter(|value| !value.is_empty()) {
                return Ok(PathBuf::from(state)
                    .join("tabbeacon")
                    .join("repository-identity"));
            }
            env::var_os("HOME")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
                .map(|path| {
                    path.join(".local")
                        .join("state")
                        .join("tabbeacon")
                        .join("repository-identity")
                })
                .ok_or(RepositoryIdentityError::StateRootUnavailable)
        }
    }

    /// Returns the configured state root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Inspects the latest immutable generation without creating a lock or state.
    #[must_use]
    pub fn inspect_read_only(&self) -> AliasRegistryDiagnostics {
        match fs::symlink_metadata(&self.root) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                AliasRegistryDiagnostics {
                    health: AliasRegistryHealth::Unavailable,
                    assignment_count: None,
                }
            }
            Ok(_) => match self.load_v2_read_only() {
                Ok(snapshot) => AliasRegistryDiagnostics {
                    health: AliasRegistryHealth::Healthy,
                    assignment_count: Some(snapshot.assignments.len()),
                },
                Err(RepositoryIdentityError::CorruptRegistry(_)) => AliasRegistryDiagnostics {
                    health: AliasRegistryHealth::Corrupt,
                    assignment_count: None,
                },
                Err(_) => AliasRegistryDiagnostics {
                    health: AliasRegistryHealth::Unavailable,
                    assignment_count: None,
                },
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                AliasRegistryDiagnostics {
                    health: AliasRegistryHealth::Absent,
                    assignment_count: Some(0),
                }
            }
            Err(_) => AliasRegistryDiagnostics {
                health: AliasRegistryHealth::Unavailable,
                assignment_count: None,
            },
        }
    }

    /// Returns an existing alias or atomically assigns a collision-free one.
    ///
    /// # Errors
    ///
    /// Returns a typed I/O, corruption, or alias-exhaustion error. A failure
    /// never publishes a partial registry generation.
    pub fn resolve(
        &self,
        identity: &CanonicalRepositoryIdentity,
        display_name: &RepositoryDisplayName,
    ) -> Result<RepositoryAlias, RepositoryIdentityError> {
        Ok(self
            .resolve_assignment(identity, display_name)?
            .generated_alias)
    }

    /// Returns an existing generated assignment or allocates one with Adaptive
    /// Naming v2 while reserving explicit aliases held by another local layer.
    ///
    /// This is the mutation half of the v2 registry protocol. Callers that
    /// coordinate user overrides pass their reserved namespace here so a new
    /// automatic alias can never collide with a pending explicit choice.
    ///
    /// # Errors
    ///
    /// Returns an error when registry state cannot be safely validated,
    /// migrated, locked, or atomically published.
    pub fn resolve_assignment(
        &self,
        identity: &CanonicalRepositoryIdentity,
        display_name: &RepositoryDisplayName,
    ) -> Result<RegistryAssignment, RepositoryIdentityError> {
        self.resolve_assignment_with_reserved(identity, display_name, &BTreeSet::new())
    }

    /// As [`Self::resolve_assignment`], with aliases reserved by an explicit
    /// device-local preference layer.
    pub(crate) fn resolve_assignment_with_reserved(
        &self,
        identity: &CanonicalRepositoryIdentity,
        display_name: &RepositoryDisplayName,
        reserved_aliases: &BTreeSet<RepositoryAlias>,
    ) -> Result<RegistryAssignment, RepositoryIdentityError> {
        self.with_exclusive_lock(|registry| {
            registry.resolve_assignment_locked(identity, display_name, reserved_aliases)
        })
    }

    /// Looks up a previously assigned alias without creating an assignment.
    ///
    /// # Errors
    ///
    /// Returns a typed I/O or corruption error.
    pub fn lookup(
        &self,
        identity: &CanonicalRepositoryIdentity,
    ) -> Result<Option<RepositoryAlias>, RepositoryIdentityError> {
        self.with_exclusive_lock(|registry| {
            registry.load_or_migrate_v2_locked().map(|snapshot| {
                snapshot
                    .assignment(identity)
                    .map(|assignment| assignment.generated_alias)
            })
        })
    }

    /// Looks up one generated assignment without creating a root, lock,
    /// migration generation, or alias assignment.
    ///
    /// # Errors
    ///
    /// Returns an error when a published registry generation is invalid,
    /// unsafe, or unreadable.
    pub fn lookup_assignment_read_only(
        &self,
        identity: &CanonicalRepositoryIdentity,
    ) -> Result<Option<RegistryAssignment>, RepositoryIdentityError> {
        Ok(self.load_v2_read_only()?.assignment(identity))
    }

    /// Returns generated assignments without creating state. This typed API is
    /// intentionally for local alias coordination, never for Human rendering.
    ///
    /// # Errors
    ///
    /// Returns an error when a published registry generation is invalid,
    /// unsafe, or unreadable.
    pub fn assignments_read_only(
        &self,
    ) -> Result<BTreeMap<CanonicalRepositoryIdentity, RegistryAssignment>, RepositoryIdentityError>
    {
        self.load_v2_read_only()?.assignments()
    }

    /// Produces bounded v2 candidates without publishing an assignment.
    ///
    /// # Errors
    ///
    /// Returns an error when the current registry state cannot be safely read.
    pub fn preview_candidates_read_only(
        &self,
        identity: &CanonicalRepositoryIdentity,
        display_name: &RepositoryDisplayName,
        reserved_aliases: &BTreeSet<RepositoryAlias>,
    ) -> Result<Vec<super::AliasCandidate>, RepositoryIdentityError> {
        let snapshot = self.load_v2_read_only()?;
        let mut used = snapshot.aliases();
        used.extend(reserved_aliases.iter().cloned());
        Ok(AdaptiveNamingPolicy::candidates(
            display_name,
            identity,
            &used,
        ))
    }

    /// Runs one mutation while holding the registry namespace lock.
    ///
    /// The workspace preference coordinator uses this to serialize automatic
    /// allocation with explicit override changes. It is deliberately crate
    /// private: external callers cannot accidentally depend on lock layout.
    pub(crate) fn with_exclusive_lock<T>(
        &self,
        operation: impl FnOnce(&Self) -> Result<T, RepositoryIdentityError>,
    ) -> Result<T, RepositoryIdentityError> {
        self.ensure_safe_root_for_mutation()?;
        fs::create_dir_all(&self.root)?;
        self.reject_root_symlink()?;
        let lock = self.open_lock()?;
        lock.lock()?;
        let result = operation(self);
        File::unlock(&lock)?;
        result
    }

    fn open_lock(&self) -> Result<File, RepositoryIdentityError> {
        Ok(OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(self.root.join(LOCK_FILE))?)
    }

    fn ensure_safe_root_for_mutation(&self) -> Result<(), RepositoryIdentityError> {
        match fs::symlink_metadata(&self.root) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                Err(RepositoryIdentityError::CorruptRegistry(
                    "registry root must not be a symbolic link".to_owned(),
                ))
            }
            Ok(metadata) if !metadata.is_dir() => Err(RepositoryIdentityError::CorruptRegistry(
                "registry root is not a directory".to_owned(),
            )),
            Ok(_) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    fn reject_root_symlink(&self) -> Result<(), RepositoryIdentityError> {
        let metadata = fs::symlink_metadata(&self.root)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(RepositoryIdentityError::CorruptRegistry(
                "registry root is unsafe".to_owned(),
            ));
        }
        Ok(())
    }

    pub(crate) fn resolve_assignment_locked(
        &self,
        identity: &CanonicalRepositoryIdentity,
        display_name: &RepositoryDisplayName,
        reserved_aliases: &BTreeSet<RepositoryAlias>,
    ) -> Result<RegistryAssignment, RepositoryIdentityError> {
        let mut snapshot = self.load_or_migrate_v2_locked()?;
        if let Some(existing) = snapshot.assignment(identity) {
            return Ok(existing);
        }
        let mut used = snapshot.aliases();
        used.extend(reserved_aliases.iter().cloned());
        let alias = AdaptiveNamingPolicy::select(display_name, identity, &used)
            .map(|candidate| candidate.alias().clone())
            .ok_or(RepositoryIdentityError::AliasExhausted)?;
        let assignment = RegistryAssignment {
            generated_alias: alias,
            policy_version: ADAPTIVE_NAMING_POLICY_ID.to_owned(),
        };
        snapshot
            .assignments
            .insert(identity.as_str().to_owned(), assignment.clone().into());
        snapshot.generation = self.next_v2_generation(snapshot.generation)?;
        self.publish_v2(&snapshot)?;
        Ok(assignment)
    }

    fn next_v2_generation(&self, current: u64) -> Result<u64, RepositoryIdentityError> {
        let mut highest = current;
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if let Some((generation, _)) = parse_snapshot_name(&name, V2_SNAPSHOT_PREFIX) {
                highest = highest.max(generation);
            }
        }
        highest.checked_add(1).ok_or_else(|| {
            RepositoryIdentityError::CorruptRegistry("generation overflow".to_owned())
        })
    }

    fn load_legacy_v1(&self) -> Result<RegistrySnapshot, RepositoryIdentityError> {
        let mut valid = Vec::<RegistrySnapshot>::new();
        let mut published_count = 0_usize;
        self.ensure_safe_root_for_read()?;
        let entries = match fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(RegistrySnapshot::empty());
            }
            Err(error) => return Err(error.into()),
        };
        for entry in entries {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let Some((generation, expected_digest)) =
                parse_snapshot_name(&name, V1_SNAPSHOT_PREFIX)
            else {
                continue;
            };
            published_count += 1;
            let bytes = fs::read(entry.path())?;
            if hex_sha256(&bytes) != expected_digest {
                continue;
            }
            let Ok(snapshot) = serde_json::from_slice::<RegistrySnapshot>(&bytes) else {
                continue;
            };
            if snapshot.generation != generation || validate_v1_snapshot(&snapshot).is_err() {
                continue;
            }
            valid.push(snapshot);
        }
        valid.sort_unstable_by_key(|snapshot| snapshot.generation);
        if valid
            .windows(2)
            .any(|pair| pair[0].generation == pair[1].generation)
        {
            return Err(RepositoryIdentityError::CorruptRegistry(
                "multiple valid snapshots claim one generation".to_owned(),
            ));
        }
        if let Some(snapshot) = valid.pop() {
            Ok(snapshot)
        } else if published_count == 0 {
            Ok(RegistrySnapshot::empty())
        } else {
            Err(RepositoryIdentityError::CorruptRegistry(
                "published snapshots exist but none are valid".to_owned(),
            ))
        }
    }

    fn load_or_migrate_v2_locked(&self) -> Result<RegistrySnapshotV2, RepositoryIdentityError> {
        if let Some(mut snapshot) = self.load_v2()? {
            let legacy = match self.load_legacy_v1() {
                Ok(snapshot) => snapshot,
                // A completed valid v2 history remains authoritative if an
                // unrelated corrupt v1 artifact appears later. This matches
                // v1's existing valid-older-generation recovery behavior;
                // no corrupt v1 bytes are reinterpreted or overwritten.
                Err(RepositoryIdentityError::CorruptRegistry(_)) => return Ok(snapshot),
                Err(error) => return Err(error),
            };
            if snapshot.reconcile_legacy_assignments(&legacy)? {
                snapshot.generation = self.next_v2_generation(snapshot.generation)?;
                self.publish_v2(&snapshot)?;
            }
            return Ok(snapshot);
        }
        let legacy = self.load_legacy_v1()?;
        let migrated = RegistrySnapshotV2::from_legacy(&legacy)?;
        if legacy.generation != 0 || !legacy.assignments.is_empty() {
            self.publish_v2(&migrated)?;
        }
        Ok(migrated)
    }

    fn load_v2_read_only(&self) -> Result<RegistrySnapshotV2, RepositoryIdentityError> {
        if let Some(mut snapshot) = self.load_v2()? {
            let legacy = match self.load_legacy_v1() {
                Ok(snapshot) => snapshot,
                Err(RepositoryIdentityError::CorruptRegistry(_)) => return Ok(snapshot),
                Err(error) => return Err(error),
            };
            // An older v0.4/v0.4.1 process may append a v1 generation after
            // the first v2 migration. Read-only callers must still see that
            // authoritative legacy history, but must not publish a v2 repair.
            snapshot.reconcile_legacy_assignments(&legacy)?;
            return Ok(snapshot);
        }
        let legacy = self.load_legacy_v1()?;
        RegistrySnapshotV2::from_legacy(&legacy)
    }

    fn load_v2(&self) -> Result<Option<RegistrySnapshotV2>, RepositoryIdentityError> {
        let mut valid = Vec::<RegistrySnapshotV2>::new();
        let mut published_count = 0_usize;
        self.ensure_safe_root_for_read()?;
        let entries = match fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        for entry in entries {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let Some((generation, expected_digest)) =
                parse_snapshot_name(&name, V2_SNAPSHOT_PREFIX)
            else {
                continue;
            };
            published_count += 1;
            let bytes = fs::read(entry.path())?;
            if hex_sha256(&bytes) != expected_digest {
                continue;
            }
            let Ok(snapshot) = serde_json::from_slice::<RegistrySnapshotV2>(&bytes) else {
                continue;
            };
            if snapshot.generation != generation || validate_v2_snapshot(&snapshot).is_err() {
                continue;
            }
            valid.push(snapshot);
        }
        valid.sort_unstable_by_key(|snapshot| snapshot.generation);
        if valid
            .windows(2)
            .any(|pair| pair[0].generation == pair[1].generation)
        {
            return Err(RepositoryIdentityError::CorruptRegistry(
                "multiple valid v2 snapshots claim one generation".to_owned(),
            ));
        }
        if let Some(snapshot) = valid.pop() {
            Ok(Some(snapshot))
        } else if published_count == 0 {
            Ok(None)
        } else {
            Err(RepositoryIdentityError::CorruptRegistry(
                "published v2 snapshots exist but none are valid".to_owned(),
            ))
        }
    }

    fn publish_v2(&self, snapshot: &RegistrySnapshotV2) -> Result<(), RepositoryIdentityError> {
        validate_v2_snapshot(snapshot)?;
        let bytes = serde_json::to_vec_pretty(snapshot)?;
        let digest = hex_sha256(&bytes);
        let final_name = format!(
            "{V2_SNAPSHOT_PREFIX}{:020}-{digest}{SNAPSHOT_SUFFIX}",
            snapshot.generation
        );
        let final_path = self.root.join(final_name);
        if final_path.exists() {
            return Err(RepositoryIdentityError::CorruptRegistry(
                "target snapshot generation already exists".to_owned(),
            ));
        }
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let temporary = self.root.join(format!(
            ".registry-v2-{}-{counter}-{}.tmp",
            std::process::id(),
            snapshot.generation
        ));
        let publish_result = (|| -> Result<(), RepositoryIdentityError> {
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temporary)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temporary, &final_path)?;
            OpenOptions::new()
                .read(true)
                .write(true)
                .open(&final_path)?
                .sync_all()?;
            Ok(())
        })();
        if publish_result.is_err() && temporary.exists() {
            let _ = fs::remove_file(&temporary);
        }
        publish_result
    }

    fn ensure_safe_root_for_read(&self) -> Result<(), RepositoryIdentityError> {
        match fs::symlink_metadata(&self.root) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => Err(
                RepositoryIdentityError::CorruptRegistry("registry root is unsafe".to_owned()),
            ),
            Ok(_) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RegistrySnapshot {
    schema: String,
    generation: u64,
    assignments: BTreeMap<String, String>,
}

impl RegistrySnapshot {
    fn empty() -> Self {
        Self {
            schema: REGISTRY_V1_SCHEMA.to_owned(),
            generation: 0,
            assignments: BTreeMap::new(),
        }
    }
}

fn validate_v1_snapshot(snapshot: &RegistrySnapshot) -> Result<(), RepositoryIdentityError> {
    if snapshot.schema != REGISTRY_V1_SCHEMA
        || snapshot.generation == 0 && !snapshot.assignments.is_empty()
    {
        return Err(RepositoryIdentityError::CorruptRegistry(
            "snapshot schema or generation is invalid".to_owned(),
        ));
    }
    let mut aliases = BTreeSet::new();
    for (identity, alias) in &snapshot.assignments {
        CanonicalRepositoryIdentity::new(identity.clone()).map_err(|_| {
            RepositoryIdentityError::CorruptRegistry(
                "snapshot contains an invalid canonical identity".to_owned(),
            )
        })?;
        RepositoryAlias::new(alias.clone()).map_err(|_| {
            RepositoryIdentityError::CorruptRegistry(
                "snapshot contains an invalid alias".to_owned(),
            )
        })?;
        if !aliases.insert(alias) {
            return Err(RepositoryIdentityError::CorruptRegistry(
                "snapshot assigns one alias to multiple identities".to_owned(),
            ));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct StoredRegistryAssignment {
    generated_alias: String,
    policy_version: String,
}

impl From<RegistryAssignment> for StoredRegistryAssignment {
    fn from(assignment: RegistryAssignment) -> Self {
        Self {
            generated_alias: assignment.generated_alias.as_str().to_owned(),
            policy_version: assignment.policy_version,
        }
    }
}

impl StoredRegistryAssignment {
    fn assignment(&self) -> Result<RegistryAssignment, RepositoryIdentityError> {
        let generated_alias = RepositoryAlias::new(self.generated_alias.clone()).map_err(|_| {
            RepositoryIdentityError::CorruptRegistry(
                "v2 snapshot contains an invalid generated alias".to_owned(),
            )
        })?;
        if self.policy_version.is_empty() || self.policy_version.len() > 64 {
            return Err(RepositoryIdentityError::CorruptRegistry(
                "v2 snapshot contains an invalid policy version".to_owned(),
            ));
        }
        Ok(RegistryAssignment {
            generated_alias,
            policy_version: self.policy_version.clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RegistrySnapshotV2 {
    schema: String,
    generation: u64,
    assignments: BTreeMap<String, StoredRegistryAssignment>,
}

impl RegistrySnapshotV2 {
    fn from_legacy(legacy: &RegistrySnapshot) -> Result<Self, RepositoryIdentityError> {
        validate_v1_snapshot(legacy)?;
        let assignments = legacy
            .assignments
            .iter()
            .map(|(identity, alias)| {
                Ok((
                    identity.clone(),
                    StoredRegistryAssignment {
                        generated_alias: RepositoryAlias::new(alias.clone())?.as_str().to_owned(),
                        policy_version: LEGACY_POLICY_VERSION.to_owned(),
                    },
                ))
            })
            .collect::<Result<BTreeMap<_, _>, RepositoryIdentityError>>()?;
        let snapshot = Self {
            schema: REGISTRY_V2_SCHEMA.to_owned(),
            generation: legacy.generation,
            assignments,
        };
        validate_v2_snapshot(&snapshot)?;
        Ok(snapshot)
    }

    fn assignment(&self, identity: &CanonicalRepositoryIdentity) -> Option<RegistryAssignment> {
        self.assignments
            .get(identity.as_str())
            .and_then(|assignment| assignment.assignment().ok())
    }

    fn assignments(
        &self,
    ) -> Result<BTreeMap<CanonicalRepositoryIdentity, RegistryAssignment>, RepositoryIdentityError>
    {
        self.assignments
            .iter()
            .map(|(identity, assignment)| {
                Ok((
                    CanonicalRepositoryIdentity::new(identity.clone()).map_err(|_| {
                        RepositoryIdentityError::CorruptRegistry(
                            "v2 snapshot contains an invalid canonical identity".to_owned(),
                        )
                    })?,
                    assignment.assignment()?,
                ))
            })
            .collect()
    }

    fn aliases(&self) -> BTreeSet<RepositoryAlias> {
        self.assignments
            .values()
            .filter_map(|assignment| assignment.assignment().ok())
            .map(|assignment| assignment.generated_alias)
            .collect()
    }

    /// Reconciles the full latest legacy snapshot into this v2 projection.
    ///
    /// The registry lock serializes ordinary writers, but an already-running
    /// v0.4/v0.4.1 binary can still publish a newer v1 generation after this
    /// process has written v2. Those aliases remain authoritative history: we
    /// project additions with the legacy policy marker and fail closed on a
    /// mismatched identity or cross-identity alias collision.
    fn reconcile_legacy_assignments(
        &mut self,
        legacy: &RegistrySnapshot,
    ) -> Result<bool, RepositoryIdentityError> {
        validate_v1_snapshot(legacy)?;
        validate_v2_snapshot(self)?;

        let mut changed = false;
        for (identity, alias) in &legacy.assignments {
            if let Some(existing) = self.assignments.get_mut(identity) {
                if existing.generated_alias != *alias {
                    return Err(RepositoryIdentityError::CorruptRegistry(
                        "legacy assignment conflicts with v2 history".to_owned(),
                    ));
                }
                if existing.policy_version != LEGACY_POLICY_VERSION {
                    LEGACY_POLICY_VERSION.clone_into(&mut existing.policy_version);
                    changed = true;
                }
            } else {
                let alias_is_reserved =
                    self.assignments.iter().any(|(other_identity, assignment)| {
                        other_identity != identity && assignment.generated_alias == *alias
                    });
                if alias_is_reserved {
                    return Err(RepositoryIdentityError::CorruptRegistry(
                        "legacy alias conflicts with v2 history".to_owned(),
                    ));
                }
                self.assignments.insert(
                    identity.clone(),
                    StoredRegistryAssignment {
                        generated_alias: alias.clone(),
                        policy_version: LEGACY_POLICY_VERSION.to_owned(),
                    },
                );
                changed = true;
            }
        }
        validate_v2_snapshot(self)?;
        Ok(changed)
    }
}

fn validate_v2_snapshot(snapshot: &RegistrySnapshotV2) -> Result<(), RepositoryIdentityError> {
    if snapshot.schema != REGISTRY_V2_SCHEMA
        || snapshot.generation == 0 && !snapshot.assignments.is_empty()
    {
        return Err(RepositoryIdentityError::CorruptRegistry(
            "v2 snapshot schema or generation is invalid".to_owned(),
        ));
    }
    let mut aliases = BTreeSet::new();
    for (identity, assignment) in &snapshot.assignments {
        CanonicalRepositoryIdentity::new(identity.clone()).map_err(|_| {
            RepositoryIdentityError::CorruptRegistry(
                "v2 snapshot contains an invalid canonical identity".to_owned(),
            )
        })?;
        let assignment = assignment.assignment()?;
        if !aliases.insert(assignment.generated_alias) {
            return Err(RepositoryIdentityError::CorruptRegistry(
                "v2 snapshot assigns one alias to multiple identities".to_owned(),
            ));
        }
    }
    Ok(())
}

fn parse_snapshot_name(name: &str, prefix: &str) -> Option<(u64, String)> {
    let body = name.strip_prefix(prefix)?.strip_suffix(SNAPSHOT_SUFFIX)?;
    let (generation, digest) = body.split_once('-')?;
    if generation.len() != 20
        || digest.len() != 64
        || !generation.bytes().all(|byte| byte.is_ascii_digit())
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return None;
    }
    Some((generation.parse().ok()?, digest.to_owned()))
}

fn hex_sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, fs, time::SystemTime};

    use super::{
        AliasRegistryHealth, LEGACY_POLICY_VERSION, REGISTRY_V1_SCHEMA, REGISTRY_V2_SCHEMA,
        RegistrySnapshot, StableAliasRegistry, V1_SNAPSHOT_PREFIX, V2_SNAPSHOT_PREFIX, hex_sha256,
    };
    use crate::repo::{CanonicalRepositoryIdentity, RepositoryDisplayName};

    fn temporary_root(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "tabbeacon-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("clock after Unix epoch")
                .as_nanos()
        ))
    }

    fn write_snapshot(root: &std::path::Path, prefix: &str, generation: u64, bytes: &[u8]) {
        let name = format!("{prefix}{generation:020}-{}.json", hex_sha256(bytes));
        fs::write(root.join(name), bytes).expect("fixture snapshot writes");
    }

    fn write_legacy_snapshot(
        root: &std::path::Path,
        generation: u64,
        assignments: BTreeMap<String, String>,
    ) {
        let snapshot = RegistrySnapshot {
            schema: REGISTRY_V1_SCHEMA.to_owned(),
            generation,
            assignments,
        };
        let bytes = serde_json::to_vec_pretty(&snapshot).expect("legacy fixture serializes");
        write_snapshot(root, V1_SNAPSHOT_PREFIX, generation, &bytes);
    }

    #[test]
    fn read_only_inspection_preserves_an_absent_registry_root() {
        let root = temporary_root("diagnostic-absent");
        let registry = StableAliasRegistry::new(&root);

        let diagnostics = registry.inspect_read_only();

        assert_eq!(diagnostics.health(), AliasRegistryHealth::Absent);
        assert_eq!(diagnostics.assignment_count(), Some(0));
        assert!(
            !root.exists(),
            "read-only diagnostics must not create a registry directory or lock"
        );
    }

    #[test]
    fn read_only_inspection_reports_only_a_safe_assignment_count() {
        let root = temporary_root("diagnostic-count");
        let registry = StableAliasRegistry::new(&root);
        let first = CanonicalRepositoryIdentity::new("remote:example/one").expect("valid identity");
        let second =
            CanonicalRepositoryIdentity::new("remote:example/two").expect("valid identity");
        let display = RepositoryDisplayName::new("example").expect("valid display name");
        registry
            .resolve(&first, &display)
            .expect("first assignment");
        registry
            .resolve(&second, &display)
            .expect("second assignment");

        let diagnostics = registry.inspect_read_only();

        assert_eq!(diagnostics.health(), AliasRegistryHealth::Healthy);
        assert_eq!(diagnostics.assignment_count(), Some(2));
        fs::remove_dir_all(root).expect("test cleanup");
    }

    #[test]
    fn read_only_inspection_classifies_unverifiable_generations_as_corrupt() {
        let root = temporary_root("diagnostic-corrupt");
        fs::create_dir_all(&root).expect("test state root");
        fs::write(
            root.join(format!("registry-v1-{:020}-{}.json", 1, "a".repeat(64))),
            b"not a registry snapshot",
        )
        .expect("corrupt fixture writes");
        let registry = StableAliasRegistry::new(&root);

        let diagnostics = registry.inspect_read_only();

        assert_eq!(diagnostics.health(), AliasRegistryHealth::Corrupt);
        assert_eq!(diagnostics.assignment_count(), None);
        fs::remove_dir_all(root).expect("test cleanup");
    }

    #[test]
    fn existing_alias_remains_stable_when_a_new_collision_arrives() {
        let root = temporary_root("stable-alias");
        let registry = StableAliasRegistry::new(&root);
        let first = CanonicalRepositoryIdentity::new("remote:example/jerry-proxy-control")
            .expect("valid identity");
        let second = CanonicalRepositoryIdentity::new("remote:example/java-platform-core")
            .expect("valid identity");
        let display = RepositoryDisplayName::new("jerry-proxy-control").expect("valid name");
        let colliding = RepositoryDisplayName::new("java-platform-core").expect("valid name");
        let first_alias = registry
            .resolve(&first, &display)
            .expect("first assignment");
        let second_alias = registry
            .resolve(&second, &colliding)
            .expect("colliding assignment");
        assert_eq!(first_alias.as_str(), "JPC");
        assert_ne!(second_alias, first_alias);
        assert_eq!(
            registry.resolve(&first, &display).expect("stable lookup"),
            first_alias
        );
        fs::remove_dir_all(root).expect("test cleanup");
    }

    #[test]
    fn abandoned_temporary_file_is_ignored() {
        let root = temporary_root("partial");
        fs::create_dir_all(&root).expect("test state root");
        fs::write(root.join(".registry-v1-abandoned.tmp"), b"partial").expect("test partial file");
        let registry = StableAliasRegistry::new(&root);
        let identity =
            CanonicalRepositoryIdentity::new("remote:example/repo").expect("valid identity");
        let display = RepositoryDisplayName::new("repo").expect("valid name");
        assert_eq!(
            registry
                .resolve(&identity, &display)
                .expect("partial file is ignored")
                .as_str(),
            "REPO"
        );
        fs::remove_dir_all(root).expect("test cleanup");
    }

    #[test]
    fn v1_migration_preserves_assignments_then_uses_adaptive_v2_for_new_identities() {
        let root = temporary_root("v1-migration");
        fs::create_dir_all(&root).expect("test state root");
        let legacy_identity = "remote:example/legacy".to_owned();
        let legacy_alias = "LEGACY".to_owned();
        write_legacy_snapshot(
            &root,
            7,
            BTreeMap::from([(legacy_identity.clone(), legacy_alias.clone())]),
        );
        let original_v1 = fs::read_dir(&root)
            .expect("state lists")
            .find_map(|entry| {
                let entry = entry.expect("entry reads");
                entry
                    .file_name()
                    .to_str()
                    .filter(|name| name.starts_with(V1_SNAPSHOT_PREFIX))
                    .map(|_| fs::read(entry.path()).expect("v1 reads"))
            })
            .expect("v1 fixture exists");
        let registry = StableAliasRegistry::new(&root);
        let legacy = CanonicalRepositoryIdentity::new(legacy_identity).expect("valid identity");

        let passive = registry
            .lookup_assignment_read_only(&legacy)
            .expect("legacy projection reads");
        assert_eq!(
            passive.expect("legacy assignment exists").policy_version(),
            LEGACY_POLICY_VERSION
        );
        assert!(
            fs::read_dir(&root).expect("state lists").all(|entry| !entry
                .expect("entry reads")
                .file_name()
                .to_string_lossy()
                .starts_with(V2_SNAPSHOT_PREFIX)),
            "passive v1 projection must not publish v2"
        );

        let legacy_assignment = registry
            .resolve_assignment(
                &legacy,
                &RepositoryDisplayName::new("legacy").expect("valid display"),
            )
            .expect("migration resolves legacy assignment");
        assert_eq!(legacy_assignment.generated_alias().as_str(), legacy_alias);
        assert_eq!(legacy_assignment.policy_version(), LEGACY_POLICY_VERSION);
        let new_identity =
            CanonicalRepositoryIdentity::new("remote:example/adaptive").expect("valid identity");
        let new_assignment = registry
            .resolve_assignment(
                &new_identity,
                &RepositoryDisplayName::new("adaptive naming").expect("valid display"),
            )
            .expect("adaptive assignment resolves");
        assert_eq!(new_assignment.policy_version(), "adaptive-v2");
        assert_eq!(
            fs::read_dir(&root)
                .expect("state lists")
                .filter_map(Result::ok)
                .filter(|entry| entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(V2_SNAPSHOT_PREFIX))
                .count(),
            2,
            "migration and new assignment each publish one immutable v2 generation"
        );
        let v1_after = fs::read_dir(&root)
            .expect("state lists")
            .find_map(|entry| {
                let entry = entry.expect("entry reads");
                entry
                    .file_name()
                    .to_str()
                    .filter(|name| name.starts_with(V1_SNAPSHOT_PREFIX))
                    .map(|_| fs::read(entry.path()).expect("v1 reads"))
            })
            .expect("v1 persists");
        assert_eq!(v1_after, original_v1, "migration never rewrites v1 history");
        fs::remove_dir_all(root).expect("test cleanup");
    }

    #[test]
    fn late_v1_history_is_projected_then_durably_reconciled_into_v2() {
        let root = temporary_root("late-v1-history");
        fs::create_dir_all(&root).expect("test state root");
        let initial_identity = "remote:example/initial".to_owned();
        let late_identity = "remote:example/late".to_owned();
        write_legacy_snapshot(
            &root,
            1,
            BTreeMap::from([(initial_identity.clone(), "INITIAL".to_owned())]),
        );
        let registry = StableAliasRegistry::new(&root);
        let initial = CanonicalRepositoryIdentity::new(initial_identity).expect("valid identity");
        registry
            .resolve_assignment(
                &initial,
                &RepositoryDisplayName::new("initial").expect("valid display"),
            )
            .expect("initial v1 migration succeeds");

        // Simulate a v0.4/v0.4.1 process that acquired the shared lock after
        // migration and then published its next immutable v1 generation.
        write_legacy_snapshot(
            &root,
            2,
            BTreeMap::from([
                ("remote:example/initial".to_owned(), "INITIAL".to_owned()),
                (late_identity.clone(), "LATE".to_owned()),
            ]),
        );
        let late = CanonicalRepositoryIdentity::new(late_identity).expect("valid identity");

        let passive = registry
            .lookup_assignment_read_only(&late)
            .expect("read-only projection accepts late legacy history")
            .expect("late legacy assignment remains visible");
        assert_eq!(passive.generated_alias().as_str(), "LATE");
        assert_eq!(passive.policy_version(), LEGACY_POLICY_VERSION);
        let v2_before_reconcile = fs::read_dir(&root)
            .expect("state lists")
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(V2_SNAPSHOT_PREFIX)
            })
            .count();
        assert_eq!(
            v2_before_reconcile, 1,
            "passive projection must not publish"
        );

        let reconciled = registry
            .resolve_assignment(
                &late,
                &RepositoryDisplayName::new("late").expect("valid display"),
            )
            .expect("late legacy assignment reconciles");
        assert_eq!(reconciled, passive);
        let v2_after_reconcile = fs::read_dir(&root)
            .expect("state lists")
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(V2_SNAPSHOT_PREFIX)
            })
            .count();
        assert_eq!(v2_after_reconcile, 2);
        assert_eq!(
            registry
                .resolve_assignment(
                    &late,
                    &RepositoryDisplayName::new("late").expect("valid display"),
                )
                .expect("reconciled history remains idempotent"),
            reconciled
        );
        let v2_after_repeat = fs::read_dir(&root)
            .expect("state lists")
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(V2_SNAPSHOT_PREFIX)
            })
            .count();
        assert_eq!(v2_after_repeat, v2_after_reconcile);
        fs::remove_dir_all(root).expect("test cleanup");
    }

    #[test]
    fn late_v1_alias_conflict_with_v2_history_fails_closed() {
        let root = temporary_root("late-v1-conflict");
        fs::create_dir_all(&root).expect("test state root");
        write_legacy_snapshot(
            &root,
            1,
            BTreeMap::from([("remote:example/legacy".to_owned(), "LEGACY".to_owned())]),
        );
        let registry = StableAliasRegistry::new(&root);
        let legacy =
            CanonicalRepositoryIdentity::new("remote:example/legacy").expect("valid identity");
        registry
            .resolve_assignment(
                &legacy,
                &RepositoryDisplayName::new("legacy").expect("valid display"),
            )
            .expect("initial v1 migration succeeds");
        let v2_only =
            CanonicalRepositoryIdentity::new("remote:example/v2-only").expect("valid identity");
        let v2_only_assignment = registry
            .resolve_assignment(
                &v2_only,
                &RepositoryDisplayName::new("v2 only").expect("valid display"),
            )
            .expect("v2 assignment succeeds");

        write_legacy_snapshot(
            &root,
            2,
            BTreeMap::from([
                ("remote:example/legacy".to_owned(), "LEGACY".to_owned()),
                (
                    "remote:example/late".to_owned(),
                    v2_only_assignment.generated_alias().as_str().to_owned(),
                ),
            ]),
        );
        let late = CanonicalRepositoryIdentity::new("remote:example/late").expect("valid identity");
        assert!(matches!(
            registry.lookup_assignment_read_only(&late),
            Err(crate::repo::RepositoryIdentityError::CorruptRegistry(_))
        ));
        assert!(matches!(
            registry.resolve_assignment(
                &late,
                &RepositoryDisplayName::new("late").expect("valid display"),
            ),
            Err(crate::repo::RepositoryIdentityError::CorruptRegistry(_))
        ));
        fs::remove_dir_all(root).expect("test cleanup");
    }

    #[test]
    fn published_invalid_v2_fails_closed_instead_of_reinterpreting_v1() {
        let root = temporary_root("invalid-v2");
        fs::create_dir_all(&root).expect("test state root");
        write_legacy_snapshot(
            &root,
            1,
            BTreeMap::from([("remote:example/legacy".to_owned(), "LEGACY".to_owned())]),
        );
        let corrupt_v2 = serde_json::to_vec_pretty(&serde_json::json!({
            "schema": "unexpected-schema",
            "generation": 2,
            "assignments": {}
        }))
        .expect("fixture serializes");
        write_snapshot(&root, V2_SNAPSHOT_PREFIX, 2, &corrupt_v2);
        let registry = StableAliasRegistry::new(&root);
        let identity =
            CanonicalRepositoryIdentity::new("remote:example/legacy").expect("valid identity");

        assert!(matches!(
            registry.lookup_assignment_read_only(&identity),
            Err(crate::repo::RepositoryIdentityError::CorruptRegistry(_))
        ));
        assert!(matches!(
            registry.resolve_assignment(
                &identity,
                &RepositoryDisplayName::new("legacy").expect("valid display"),
            ),
            Err(crate::repo::RepositoryIdentityError::CorruptRegistry(_))
        ));
        fs::remove_dir_all(root).expect("test cleanup");
    }

    #[test]
    fn v2_schema_constant_is_distinct_from_v1() {
        assert_ne!(REGISTRY_V1_SCHEMA, REGISTRY_V2_SCHEMA);
    }
}
