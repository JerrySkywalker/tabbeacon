use std::{
    env, fs,
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};

use super::{
    CanonicalRepositoryIdentity, RepositoryDisplayName, RepositoryIdentityError,
    RepositoryIdentityResolver, StableAliasRegistry,
};

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
    /// Root of the resolved Git worktree or ordinary directory.
    pub workspace_root: PathBuf,
    /// Shared Git directory for Git workspaces; absent for ordinary directories.
    pub git_common_dir: Option<PathBuf>,
    /// Evidence class used for this resolution.
    pub kind: WorkspaceKind,
}

/// Resolves a cwd as a Git workspace or an ordinary-directory workspace.
#[derive(Debug, Clone)]
pub struct WorkspaceIdentityResolver {
    repository: RepositoryIdentityResolver,
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
        Self {
            repository: RepositoryIdentityResolver::new(state_root),
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
        let cwd = cwd.as_ref();
        match self.repository.resolve(cwd) {
            Ok(resolved) => Ok(ResolvedWorkspaceIdentity {
                identity: resolved.identity,
                display_name: resolved.display_name,
                alias: resolved.alias,
                workspace_root: resolved.worktree_root,
                git_common_dir: Some(resolved.git_common_dir),
                kind: WorkspaceKind::Git,
            }),
            Err(RepositoryIdentityError::NotRepository(_)) => self.resolve_directory(cwd),
            Err(error) => Err(error),
        }
    }

    fn resolve_directory(
        &self,
        cwd: &Path,
    ) -> Result<ResolvedWorkspaceIdentity, RepositoryIdentityError> {
        let workspace_root = fs::canonicalize(cwd)?;
        if !workspace_root.is_dir() {
            return Err(RepositoryIdentityError::InvalidIdentifier {
                kind: "directory workspace",
                detail: "cwd is not a directory".to_owned(),
            });
        }
        let identity = directory_identity(&workspace_root)?;
        let display_name = directory_display_name(&workspace_root, self.home_directory.as_deref())?;
        let alias = self.repository.registry.resolve(&identity, &display_name)?;
        Ok(ResolvedWorkspaceIdentity {
            identity,
            display_name,
            alias,
            workspace_root,
            git_common_dir: None,
            kind: WorkspaceKind::Directory,
        })
    }
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
