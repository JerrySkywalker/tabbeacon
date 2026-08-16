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
    AbbreviationPolicy, CanonicalRepositoryIdentity, RepositoryAlias, RepositoryDisplayName,
    RepositoryIdentityError,
};

const REGISTRY_SCHEMA: &str = "tabbeacon-repository-alias-registry-v1";
const SNAPSHOT_PREFIX: &str = "registry-v1-";
const SNAPSHOT_SUFFIX: &str = ".json";
const LOCK_FILE: &str = "registry.lock";
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
            Ok(_) => match self.load_latest() {
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
        fs::create_dir_all(&self.root)?;
        let lock = self.open_lock()?;
        lock.lock()?;
        let result = self.resolve_locked(identity, display_name);
        File::unlock(&lock)?;
        result
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
        fs::create_dir_all(&self.root)?;
        let lock = self.open_lock()?;
        lock.lock()?;
        let result = self.load_latest().and_then(|snapshot| {
            snapshot
                .assignments
                .get(identity.as_str())
                .map(|value| RepositoryAlias::new(value.clone()))
                .transpose()
        });
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

    fn resolve_locked(
        &self,
        identity: &CanonicalRepositoryIdentity,
        display_name: &RepositoryDisplayName,
    ) -> Result<RepositoryAlias, RepositoryIdentityError> {
        let mut snapshot = self.load_latest()?;
        if let Some(existing) = snapshot.assignments.get(identity.as_str()) {
            return RepositoryAlias::new(existing.clone());
        }
        let used = snapshot
            .assignments
            .values()
            .cloned()
            .collect::<BTreeSet<_>>();
        let alias = AbbreviationPolicy::candidates(display_name, identity)
            .into_iter()
            .find(|candidate| !used.contains(candidate.as_str()))
            .ok_or(RepositoryIdentityError::AliasExhausted)?;
        snapshot
            .assignments
            .insert(identity.as_str().to_owned(), alias.as_str().to_owned());
        snapshot.generation = self.next_generation(snapshot.generation)?;
        self.publish(&snapshot)?;
        Ok(alias)
    }

    fn next_generation(&self, current: u64) -> Result<u64, RepositoryIdentityError> {
        let mut highest = current;
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if let Some((generation, _)) = parse_snapshot_name(&name) {
                highest = highest.max(generation);
            }
        }
        highest.checked_add(1).ok_or_else(|| {
            RepositoryIdentityError::CorruptRegistry("generation overflow".to_owned())
        })
    }

    fn load_latest(&self) -> Result<RegistrySnapshot, RepositoryIdentityError> {
        let mut valid = Vec::<RegistrySnapshot>::new();
        let mut published_count = 0_usize;
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let Some((generation, expected_digest)) = parse_snapshot_name(&name) else {
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
            if snapshot.generation != generation || validate_snapshot(&snapshot).is_err() {
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

    fn publish(&self, snapshot: &RegistrySnapshot) -> Result<(), RepositoryIdentityError> {
        validate_snapshot(snapshot)?;
        let bytes = serde_json::to_vec_pretty(snapshot)?;
        let digest = hex_sha256(&bytes);
        let final_name = format!(
            "{SNAPSHOT_PREFIX}{:020}-{digest}{SNAPSHOT_SUFFIX}",
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
            ".registry-v1-{}-{counter}-{}.tmp",
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
            schema: REGISTRY_SCHEMA.to_owned(),
            generation: 0,
            assignments: BTreeMap::new(),
        }
    }
}

fn validate_snapshot(snapshot: &RegistrySnapshot) -> Result<(), RepositoryIdentityError> {
    if snapshot.schema != REGISTRY_SCHEMA
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

fn parse_snapshot_name(name: &str) -> Option<(u64, String)> {
    let body = name
        .strip_prefix(SNAPSHOT_PREFIX)?
        .strip_suffix(SNAPSHOT_SUFFIX)?;
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
    use std::{fs, time::SystemTime};

    use super::{AliasRegistryHealth, StableAliasRegistry};
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
            "R"
        );
        fs::remove_dir_all(root).expect("test cleanup");
    }
}
