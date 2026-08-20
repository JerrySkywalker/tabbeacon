use std::{
    env, fs,
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use super::{
    AdaptiveNamingPolicy, AliasCandidate, CanonicalRepositoryIdentity, NameAnalysis,
    RepositoryAlias, RepositoryDiscovery, RepositoryDisplayName, RepositoryIdentityError,
    StableAliasRegistry, WorkspacePreferenceError, WorkspacePreferenceStore, WorkspacePreferences,
    canonicalize_repository,
};

const MAX_CUSTOM_ALIAS_DISPLAY_WIDTH: usize = 20;
const MAX_CUSTOM_ALIAS_GRAPHEMES: usize = 20;

/// The local evidence class that produced a workspace identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceKind {
    /// Existing Git repository identity and compatibility semantics.
    Git,
    /// Opaque identity derived from an ordinary directory's normalized path.
    Directory,
}

/// Presentation-facing workspace identity resolved in the shared alias namespace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedWorkspaceIdentity {
    /// Opaque canonical key. Git keys retain their existing exact value.
    pub identity: CanonicalRepositoryIdentity,
    /// Safe local hint used only by alias allocation.
    pub display_name: RepositoryDisplayName,
    /// Stable alias assigned by the existing shared registry.
    pub alias: super::RepositoryAlias,
    /// Generated registry alias, retained separately from a user override.
    pub generated_alias: super::RepositoryAlias,
    /// Optional device-local explicit alias choice.
    pub override_alias: Option<super::RepositoryAlias>,
    /// Alias that must reach Human presentation and activity leases.
    pub effective_alias: super::RepositoryAlias,
    /// Stable policy marker for the generated alias.
    pub policy_version: String,
    /// Root of the resolved Git worktree or ordinary directory.
    pub workspace_root: PathBuf,
    /// Shared Git directory for Git workspaces; absent for ordinary directories.
    pub git_common_dir: Option<PathBuf>,
    /// Evidence class used for this resolution.
    pub kind: WorkspaceKind,
}

/// Safe, non-path-bearing facts used by the `alias` command surface.
///
/// This intentionally omits canonical identities and filesystem paths, so a
/// caller can pass it directly to Human, plain, or JSON presentation without
/// accidentally disclosing local workspace internals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceAliasInspection {
    workspace: RepositoryDisplayName,
    automatic_alias: RepositoryAlias,
    custom_alias: Option<RepositoryAlias>,
    effective_alias: RepositoryAlias,
    policy_version: String,
    assigned: bool,
    analysis: NameAnalysis,
    candidates: Vec<AliasCandidate>,
}

impl WorkspaceAliasInspection {
    /// Sanitized display hint for the current workspace.
    #[must_use]
    pub fn workspace(&self) -> &RepositoryDisplayName {
        &self.workspace
    }

    /// Stable generated alias or an unpersisted prospective Adaptive v2 alias.
    #[must_use]
    pub fn automatic_alias(&self) -> &RepositoryAlias {
        &self.automatic_alias
    }

    /// Explicit device-local override, when present.
    #[must_use]
    pub fn custom_alias(&self) -> Option<&RepositoryAlias> {
        self.custom_alias.as_ref()
    }

    /// Alias effective for presentation on this device.
    #[must_use]
    pub fn effective_alias(&self) -> &RepositoryAlias {
        &self.effective_alias
    }

    /// Policy marker retained with the generated assignment.
    #[must_use]
    pub fn policy_version(&self) -> &str {
        &self.policy_version
    }

    /// Whether the automatic alias is already persisted in registry v2.
    #[must_use]
    pub const fn is_assigned(&self) -> bool {
        self.assigned
    }

    /// Pure Adaptive v2 analysis for a safe explain view.
    #[must_use]
    pub fn analysis(&self) -> &NameAnalysis {
        &self.analysis
    }

    /// Ordered, non-persisting Adaptive v2 suggestions.
    #[must_use]
    pub fn candidates(&self) -> &[AliasCandidate] {
        &self.candidates
    }
}

/// Public, privacy-safe result category for alias commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceAliasError {
    /// The supplied custom alias violated the bounded local alias grammar.
    InvalidAlias,
    /// Another local workspace already reserves the requested alias.
    Collision,
    /// A concurrently modified preference document rejected the update.
    Conflict,
    /// Local workspace evidence or state could not be used safely.
    Unavailable,
}

impl std::fmt::Display for WorkspaceAliasError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidAlias => "the requested alias is not a safe bounded workspace alias",
            Self::Collision => "the requested alias is already used by another local workspace",
            Self::Conflict => "workspace preferences changed before the requested update",
            Self::Unavailable => "the workspace alias operation could not be completed safely",
        })
    }
}

impl std::error::Error for WorkspaceAliasError {}

#[derive(Debug, Clone)]
struct WorkspaceFacts {
    identity: CanonicalRepositoryIdentity,
    display_name: RepositoryDisplayName,
    workspace_root: PathBuf,
    git_common_dir: Option<PathBuf>,
    kind: WorkspaceKind,
}

/// Resolves a cwd as a Git workspace or an ordinary-directory workspace.
#[derive(Debug, Clone)]
pub struct WorkspaceIdentityResolver {
    discovery: RepositoryDiscovery,
    registry: StableAliasRegistry,
    preferences: WorkspacePreferenceStore,
    home_directory: Option<PathBuf>,
}

impl WorkspaceIdentityResolver {
    /// Creates a resolver using the unchanged repository alias registry root.
    #[must_use]
    pub fn new(state_root: impl Into<PathBuf>) -> Self {
        Self::with_home_directory(state_root, platform_home_directory())
    }

    /// Creates a resolver with an explicit home directory for deterministic callers/tests.
    #[must_use]
    pub fn with_home_directory(
        state_root: impl Into<PathBuf>,
        home_directory: Option<PathBuf>,
    ) -> Self {
        let state_root = state_root.into();
        Self {
            discovery: RepositoryDiscovery::default(),
            preferences: WorkspacePreferenceStore::for_registry_state_root(&state_root),
            registry: StableAliasRegistry::new(state_root),
            home_directory,
        }
    }

    /// Creates a resolver with an injected preference path for focused tests
    /// and future device-local workspace preference integrations.
    #[must_use]
    pub fn with_preference_path(
        state_root: impl Into<PathBuf>,
        preference_path: impl Into<PathBuf>,
        home_directory: Option<PathBuf>,
    ) -> Self {
        Self {
            discovery: RepositoryDiscovery::default(),
            registry: StableAliasRegistry::new(state_root),
            preferences: WorkspacePreferenceStore::new(preference_path),
            home_directory,
        }
    }

    /// Creates a resolver below the existing per-user identity state root.
    ///
    /// # Errors
    ///
    /// Returns [`RepositoryIdentityError::StateRootUnavailable`] when no safe
    /// per-user application-data location is available.
    pub fn with_default_state_root() -> Result<Self, RepositoryIdentityError> {
        Ok(Self::new(StableAliasRegistry::default_state_root()?))
    }

    /// Returns the device-local preference store used by this resolver.
    #[must_use]
    pub fn preference_store(&self) -> &WorkspacePreferenceStore {
        &self.preferences
    }

    /// Resolves one existing cwd without network I/O or repository-local writes.
    ///
    /// Git workspaces preserve the existing canonical identity, alias, and
    /// linked-worktree behavior. Only an explicit non-repository result enters
    /// the ordinary-directory fallback.
    ///
    /// # Errors
    ///
    /// Returns a typed discovery, filesystem, canonicalization, or registry error.
    pub fn resolve(
        &self,
        cwd: impl AsRef<Path>,
    ) -> Result<ResolvedWorkspaceIdentity, RepositoryIdentityError> {
        let facts = self.workspace_facts(cwd.as_ref())?;
        self.registry.with_exclusive_lock(|registry| {
            let preferences = self.preference_snapshot_as_repository_error()?;
            let reserved = preferences.preferences().override_aliases();
            let assignment = registry.resolve_assignment_locked(
                &facts.identity,
                &facts.display_name,
                &reserved,
            )?;
            let override_alias = preferences.preferences().override_for(&facts.identity);
            let generated_alias = assignment.generated_alias().clone();
            let effective_alias = override_alias
                .clone()
                .unwrap_or_else(|| generated_alias.clone());
            Ok(ResolvedWorkspaceIdentity {
                identity: facts.identity.clone(),
                display_name: facts.display_name.clone(),
                alias: generated_alias.clone(),
                generated_alias,
                override_alias,
                effective_alias,
                policy_version: assignment.policy_version().to_owned(),
                workspace_root: facts.workspace_root.clone(),
                git_common_dir: facts.git_common_dir.clone(),
                kind: facts.kind,
            })
        })
    }

    /// Inspects the current workspace without creating a registry generation,
    /// preference document, directory, or lock.
    ///
    /// # Errors
    ///
    /// Returns a privacy-safe error category when local workspace evidence or
    /// existing device-local state cannot be read safely.
    pub fn inspect_alias(
        &self,
        cwd: impl AsRef<Path>,
    ) -> Result<WorkspaceAliasInspection, WorkspaceAliasError> {
        let facts = self
            .workspace_facts(cwd.as_ref())
            .map_err(|error| WorkspaceAliasError::from_repository_error(&error))?;
        let snapshot = self
            .preferences
            .snapshot_read_only()
            .map_err(|_| WorkspaceAliasError::Unavailable)?;
        self.inspect_from_snapshot(&facts, snapshot.preferences())
            .map_err(|error| WorkspaceAliasError::from_repository_error(&error))
    }

    /// Stores one explicit local alias only after a namespace collision check
    /// and a byte-exact conditional preference write.
    ///
    /// # Errors
    ///
    /// Returns `InvalidAlias` for an unsafe value, `Collision` for an alias
    /// reserved by another workspace, `Conflict` for byte drift, or
    /// `Unavailable` for unsafe local state.
    pub fn set_alias_override(
        &self,
        cwd: impl AsRef<Path>,
        alias: impl AsRef<str>,
    ) -> Result<WorkspaceAliasInspection, WorkspaceAliasError> {
        let alias = validate_custom_alias(alias.as_ref())?;
        let facts = self
            .workspace_facts(cwd.as_ref())
            .map_err(|error| WorkspaceAliasError::from_repository_error(&error))?;
        self.registry
            .with_exclusive_lock(|registry| {
                let snapshot = self.preference_snapshot_as_repository_error()?;
                let reserved = snapshot.preferences().override_aliases();
                let assignment = registry.resolve_assignment_locked(
                    &facts.identity,
                    &facts.display_name,
                    &reserved,
                )?;
                let assignments = registry.assignments_read_only()?;
                let generated_collision = assignments.iter().any(|(identity, assignment)| {
                    identity != &facts.identity && assignment.generated_alias() == &alias
                });
                let override_collision = snapshot
                    .preferences()
                    .overrides()
                    .iter()
                    .any(|(identity, existing)| identity != &facts.identity && existing == &alias);
                if generated_collision || override_collision {
                    return Err(RepositoryIdentityError::AliasConflict);
                }
                let replacement = snapshot
                    .preferences()
                    .clone()
                    .with_override(facts.identity.clone(), alias);
                let saved_preferences = replacement.clone();
                match self
                    .preferences
                    .save_snapshot_if_unchanged(&snapshot, replacement)
                    .map_err(|error| preference_error_as_repository_error(&error))?
                {
                    super::WorkspacePreferencesSnapshotSaveOutcome::Saved(_) => {}
                    super::WorkspacePreferencesSnapshotSaveOutcome::Conflict => {
                        return Err(RepositoryIdentityError::PreferenceConflict);
                    }
                }
                self.inspection_from_parts(&facts, Some(assignment), &saved_preferences)
            })
            .map_err(|error| WorkspaceAliasError::from_repository_error(&error))
    }

    /// Removes only this workspace's explicit override; generated registry
    /// history remains unchanged.
    ///
    /// # Errors
    ///
    /// Returns `Conflict` for byte drift or `Unavailable` when local workspace
    /// or preference state cannot be safely used.
    pub fn reset_alias_override(
        &self,
        cwd: impl AsRef<Path>,
    ) -> Result<WorkspaceAliasInspection, WorkspaceAliasError> {
        let facts = self
            .workspace_facts(cwd.as_ref())
            .map_err(|error| WorkspaceAliasError::from_repository_error(&error))?;
        self.registry
            .with_exclusive_lock(|registry| {
                let snapshot = self.preference_snapshot_as_repository_error()?;
                let reserved = snapshot.preferences().override_aliases();
                let assignment = registry.resolve_assignment_locked(
                    &facts.identity,
                    &facts.display_name,
                    &reserved,
                )?;
                let generated_alias = assignment.generated_alias().clone();
                let reset_collision =
                    snapshot
                        .preferences()
                        .overrides()
                        .iter()
                        .any(|(identity, existing)| {
                            identity != &facts.identity && existing == &generated_alias
                        });
                if reset_collision {
                    return Err(RepositoryIdentityError::AliasConflict);
                }
                let reset_preferences = if snapshot.preferences().has_override(&facts.identity) {
                    let replacement = snapshot
                        .preferences()
                        .clone()
                        .without_override(&facts.identity);
                    let reset_preferences = replacement.clone();
                    match self
                        .preferences
                        .save_snapshot_if_unchanged(&snapshot, replacement)
                        .map_err(|error| preference_error_as_repository_error(&error))?
                    {
                        super::WorkspacePreferencesSnapshotSaveOutcome::Saved(_) => {}
                        super::WorkspacePreferencesSnapshotSaveOutcome::Conflict => {
                            return Err(RepositoryIdentityError::PreferenceConflict);
                        }
                    }
                    reset_preferences
                } else {
                    snapshot.preferences().clone()
                };
                self.inspection_from_parts(&facts, Some(assignment), &reset_preferences)
            })
            .map_err(|error| WorkspaceAliasError::from_repository_error(&error))
    }

    fn workspace_facts(&self, cwd: &Path) -> Result<WorkspaceFacts, RepositoryIdentityError> {
        match self.discovery.discover(cwd) {
            Ok(discovered) => {
                let canonical = canonicalize_repository(&discovered)?;
                Ok(WorkspaceFacts {
                    identity: canonical.identity,
                    display_name: canonical.display_name,
                    workspace_root: discovered.worktree_root,
                    git_common_dir: Some(discovered.git_common_dir),
                    kind: WorkspaceKind::Git,
                })
            }
            Err(RepositoryIdentityError::NotRepository(_)) => self.directory_facts(cwd),
            Err(error) => Err(error),
        }
    }

    fn directory_facts(&self, cwd: &Path) -> Result<WorkspaceFacts, RepositoryIdentityError> {
        let workspace_root = fs::canonicalize(cwd)?;
        if !workspace_root.is_dir() {
            return Err(RepositoryIdentityError::InvalidIdentifier {
                kind: "directory workspace",
                detail: "cwd is not a directory".to_owned(),
            });
        }
        let identity = directory_identity(&workspace_root)?;
        let display_name = directory_display_name(&workspace_root, self.home_directory.as_deref())?;
        Ok(WorkspaceFacts {
            identity,
            display_name,
            workspace_root,
            git_common_dir: None,
            kind: WorkspaceKind::Directory,
        })
    }

    fn preference_snapshot_as_repository_error(
        &self,
    ) -> Result<super::WorkspacePreferencesSnapshot, RepositoryIdentityError> {
        self.preferences
            .snapshot_read_only()
            .map_err(|error| preference_error_as_repository_error(&error))
    }

    fn inspect_from_snapshot(
        &self,
        facts: &WorkspaceFacts,
        preferences: &WorkspacePreferences,
    ) -> Result<WorkspaceAliasInspection, RepositoryIdentityError> {
        let assignment = self.registry.lookup_assignment_read_only(&facts.identity)?;
        self.inspection_from_parts(facts, assignment, preferences)
    }

    fn inspection_from_parts(
        &self,
        facts: &WorkspaceFacts,
        assignment: Option<super::RegistryAssignment>,
        preferences: &WorkspacePreferences,
    ) -> Result<WorkspaceAliasInspection, RepositoryIdentityError> {
        let reserved = preferences.override_aliases();
        let candidates = self.registry.preview_candidates_read_only(
            &facts.identity,
            &facts.display_name,
            &reserved,
        )?;
        let assigned = assignment.is_some();
        let (automatic_alias, policy_version) = match assignment {
            Some(assignment) => (
                assignment.generated_alias().clone(),
                assignment.policy_version().to_owned(),
            ),
            None => (
                candidates
                    .first()
                    .map(|candidate| candidate.alias().clone())
                    .ok_or(RepositoryIdentityError::AliasExhausted)?,
                AdaptiveNamingPolicy::policy_id().to_owned(),
            ),
        };
        let custom_alias = preferences.override_for(&facts.identity);
        let effective_alias = custom_alias
            .clone()
            .unwrap_or_else(|| automatic_alias.clone());
        Ok(WorkspaceAliasInspection {
            workspace: facts.display_name.clone(),
            automatic_alias,
            custom_alias,
            effective_alias,
            policy_version,
            assigned,
            analysis: AdaptiveNamingPolicy::analyze(&facts.display_name),
            candidates,
        })
    }
}

impl WorkspaceAliasError {
    fn from_repository_error(error: &RepositoryIdentityError) -> Self {
        match error {
            RepositoryIdentityError::AliasConflict => Self::Collision,
            RepositoryIdentityError::PreferenceConflict => Self::Conflict,
            _ => Self::Unavailable,
        }
    }
}

fn preference_error_as_repository_error(
    error: &WorkspacePreferenceError,
) -> RepositoryIdentityError {
    let detail = match error {
        WorkspacePreferenceError::Malformed | WorkspacePreferenceError::SymbolicLinkTarget => {
            "workspace preference state is unsafe"
        }
        WorkspacePreferenceError::StateRootUnavailable | WorkspacePreferenceError::Io(_) => {
            "workspace preference state is unavailable"
        }
    };
    RepositoryIdentityError::CorruptRegistry(detail.to_owned())
}

fn validate_custom_alias(value: &str) -> Result<RepositoryAlias, WorkspaceAliasError> {
    let normalized = value.nfc().collect::<String>();
    if normalized.graphemes(true).count() > MAX_CUSTOM_ALIAS_GRAPHEMES
        || UnicodeWidthStr::width(normalized.as_str()) > MAX_CUSTOM_ALIAS_DISPLAY_WIDTH
    {
        return Err(WorkspaceAliasError::InvalidAlias);
    }
    RepositoryAlias::new(normalized).map_err(|_| WorkspaceAliasError::InvalidAlias)
}

fn directory_identity(path: &Path) -> Result<CanonicalRepositoryIdentity, RepositoryIdentityError> {
    let normalized = normalize_absolute_path(path);
    let digest = Sha256::digest(normalized.as_bytes());
    CanonicalRepositoryIdentity::new(format!("dir-v1:{digest:x}"))
}

fn directory_display_name(
    path: &Path,
    home_directory: Option<&Path>,
) -> Result<RepositoryDisplayName, RepositoryIdentityError> {
    let is_home = home_directory
        .and_then(|home| fs::canonicalize(home).ok())
        .is_some_and(|home| equivalent_paths(&home, path));
    if is_home {
        return RepositoryDisplayName::new("HOME");
    }
    if path.file_name().is_none() {
        return RepositoryDisplayName::new(root_display_hint(path));
    }
    let display = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("WORKSPACE");
    RepositoryDisplayName::new(display)
}

fn equivalent_paths(left: &Path, right: &Path) -> bool {
    normalize_absolute_path(left) == normalize_absolute_path(right)
}

fn normalize_absolute_path(path: &Path) -> String {
    let mut value = path.to_string_lossy().replace('\\', "/");
    if let Some(stripped) = value.strip_prefix("//?/UNC/") {
        value = format!("//{stripped}");
    } else if let Some(stripped) = value.strip_prefix("//?/") {
        value = stripped.to_owned();
    }
    while value.len() > 1 && value.ends_with('/') {
        value.pop();
    }
    if cfg!(windows) {
        value.make_ascii_lowercase();
    }
    value
}

#[cfg(windows)]
fn root_display_hint(path: &Path) -> String {
    use std::path::{Component, Prefix};

    match path.components().next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => {
                format!("{}-ROOT", char::from(letter).to_ascii_uppercase())
            }
            Prefix::UNC(_, _) | Prefix::VerbatimUNC(_, _) => "NETWORK-ROOT".to_owned(),
            _ => "ROOT".to_owned(),
        },
        _ => "ROOT".to_owned(),
    }
}

#[cfg(not(windows))]
fn root_display_hint(_path: &Path) -> String {
    "ROOT".to_owned()
}

fn platform_home_directory() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        env::var_os("USERPROFILE")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    }
    #[cfg(not(windows))]
    {
        env::var_os("HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    }
}
