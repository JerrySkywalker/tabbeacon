//! Offline-first repository discovery, canonical identity, and stable aliases.
//!
//! This module consumes only local Git/filesystem metadata. It deliberately has
//! no provider, agent-session, presentation, terminal, or network dependency.

mod abbreviation;
mod discovery;
mod error;
mod identity;
mod registry;
mod workspace;

pub use abbreviation::{AbbreviationPolicy, RepositoryAlias};
pub use discovery::{DiscoveredRepository, RepositoryDiscovery, RepositoryRemote};
pub use error::RepositoryIdentityError;
pub use identity::{
    CanonicalRepositoryIdentity, CanonicalizedRepository, RepositoryDisplayName,
    canonicalize_repository, normalize_remote_url,
};
pub use registry::{
    AliasRegistryDiagnostics, AliasRegistryHealth, ResolvedRepositoryIdentity, StableAliasRegistry,
};
pub use workspace::{ResolvedWorkspaceIdentity, WorkspaceIdentityResolver, WorkspaceKind};

use std::path::Path;

/// Resolves one cwd through discovery, canonicalization, and stable assignment.
#[derive(Debug, Clone)]
pub struct RepositoryIdentityResolver {
    discovery: RepositoryDiscovery,
    registry: StableAliasRegistry,
}

impl RepositoryIdentityResolver {
    /// Creates a resolver using an explicitly injected local state root.
    #[must_use]
    pub fn new(state_root: impl Into<std::path::PathBuf>) -> Self {
        Self {
            discovery: RepositoryDiscovery::default(),
            registry: StableAliasRegistry::new(state_root),
        }
    }

    /// Creates a resolver below the platform's per-user `TabBeacon` state root.
    ///
    /// # Errors
    ///
    /// Returns [`RepositoryIdentityError::StateRootUnavailable`] when no safe
    /// per-user application-data location is available.
    pub fn with_default_state_root() -> Result<Self, RepositoryIdentityError> {
        Ok(Self::new(StableAliasRegistry::default_state_root()?))
    }

    /// Resolves and stably assigns an identity for `cwd` without network I/O.
    ///
    /// # Errors
    ///
    /// Returns a typed discovery, canonicalization, or registry error.
    pub fn resolve(
        &self,
        cwd: impl AsRef<Path>,
    ) -> Result<ResolvedRepositoryIdentity, RepositoryIdentityError> {
        let discovered = self.discovery.discover(cwd)?;
        let canonical = canonicalize_repository(&discovered)?;
        let alias = self
            .registry
            .resolve(&canonical.identity, &canonical.display_name)?;
        Ok(ResolvedRepositoryIdentity {
            identity: canonical.identity,
            display_name: canonical.display_name,
            alias,
            worktree_root: discovered.worktree_root,
            git_common_dir: discovered.git_common_dir,
        })
    }
}
