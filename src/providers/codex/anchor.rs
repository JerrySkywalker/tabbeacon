//! Session-scoped, privacy-preserving root-workspace anchoring for Codex Hooks.

use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

use atomic_write_file::AtomicWriteFile;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{activity::SessionWorkspaceObservability, repo::RepositoryAlias};

use super::{CodexHookContext, CodexHookEvent, CodexSessionStartSource};

const STATE_DIRECTORY: &str = "codex-root-workspace-anchor-v1";
const LOCK_FILE: &str = "root-workspace-anchor.lock";
const STATE_SCHEMA: &str = "tabbeacon-codex-root-workspace-anchor-v1";
const MAX_ACTIVE_SUBAGENTS: u16 = 1_024;

/// The admitted event source that established the current root anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum RootWorkspaceBindingSource {
    SessionStartStartup,
    SessionStartResume,
    SessionStartClear,
    #[serde(rename = "first_root_event_fallback")]
    UserPromptFallback,
}

impl RootWorkspaceBindingSource {
    pub(super) const fn from_session_start(source: CodexSessionStartSource) -> Self {
        match source {
            CodexSessionStartSource::Startup => Self::SessionStartStartup,
            CodexSessionStartSource::Resume => Self::SessionStartResume,
            CodexSessionStartSource::Clear => Self::SessionStartClear,
        }
    }

    const fn resets_subagent_observation(self) -> bool {
        !matches!(self, Self::UserPromptFallback)
    }
}

/// The bounded facts a root Hook may consume for title rendering and later
/// provider-aware sessions presentation. It carries no raw path, agent, or
/// native session identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RootWorkspaceSelection {
    effective_alias: RepositoryAlias,
    binding_source: RootWorkspaceBindingSource,
    root_binding_stable: bool,
    workspace_mismatch_observed: bool,
    active_subagents: u16,
}

impl RootWorkspaceSelection {
    pub(super) fn effective_alias(&self) -> &RepositoryAlias {
        &self.effective_alias
    }

    #[allow(dead_code)]
    pub(super) const fn binding_source(&self) -> RootWorkspaceBindingSource {
        self.binding_source
    }

    pub(super) const fn root_binding_stable(&self) -> bool {
        self.root_binding_stable
    }

    pub(super) const fn workspace_mismatch_observed(&self) -> bool {
        self.workspace_mismatch_observed
    }

    pub(super) const fn active_subagents(&self) -> u16 {
        self.active_subagents
    }

    pub(super) fn workspace_observability(&self) -> SessionWorkspaceObservability {
        SessionWorkspaceObservability {
            root_binding_stable: self.root_binding_stable(),
            workspace_mismatch_observed: self.workspace_mismatch_observed(),
            active_subagents: self.active_subagents(),
            background_tasks: None,
        }
    }
}

/// Content-minimal, process-safe state for one provider session's root alias.
#[derive(Debug, Clone)]
pub(super) struct RootWorkspaceAnchorStore {
    directory: PathBuf,
}

impl RootWorkspaceAnchorStore {
    pub(super) fn new(state_root: impl Into<PathBuf>) -> Self {
        Self {
            directory: state_root.into().join(STATE_DIRECTORY),
        }
    }

    /// Establishes or replaces the root anchor at an admitted binding boundary.
    pub(super) fn bind(
        &self,
        session_sha256: &str,
        generation: u64,
        workspace_identity_sha256: &str,
        effective_alias: &RepositoryAlias,
        binding_source: RootWorkspaceBindingSource,
    ) -> io::Result<RootWorkspaceSelection> {
        self.with_state(session_sha256, |state| {
            state.anchor = Some(PersistedRootWorkspaceAnchor {
                workspace_identity_sha256: workspace_identity_sha256.to_owned(),
                effective_alias: effective_alias.as_str().to_owned(),
                binding_source,
                bound_at_generation: generation,
            });
            state.workspace_mismatch_observed = false;
            if binding_source.resets_subagent_observation() {
                state.active_subagents = 0;
            }
            selection_from_state(state)?.ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "root anchor was not stored")
            })
        })
    }

    /// Checks for an existing anchor without resolving or retaining an event
    /// cwd. This avoids duplicate repository discovery on first-prompt binding.
    pub(super) fn has_anchor(&self, session_sha256: &str) -> io::Result<bool> {
        self.with_state(session_sha256, |state| Ok(state.anchor.is_some()))
    }

    /// Returns the existing alias while recording only whether this event's
    /// resolved workspace identity differs from the root anchor.
    pub(super) fn select_existing_or_observe_mismatch(
        &self,
        session_sha256: &str,
        observed_workspace_identity_sha256: &str,
    ) -> io::Result<Option<RootWorkspaceSelection>> {
        self.with_state(session_sha256, |state| {
            let Some(anchor) = state.anchor.as_ref() else {
                return Ok(None);
            };
            if anchor.workspace_identity_sha256 != observed_workspace_identity_sha256 {
                state.workspace_mismatch_observed = true;
            }
            selection_from_state(state)
        })
    }

    /// Reads and then retires the session anchor. The returned selection lets a
    /// `SessionEnd` render the final root title without allowing a new cwd to
    /// become title authority.
    pub(super) fn take_for_session_end(
        &self,
        session_sha256: &str,
    ) -> io::Result<Option<RootWorkspaceSelection>> {
        self.prepare_directory()?;
        let lock = self.open_lock()?;
        lock.lock()?;
        let path = self.state_path(session_sha256)?;
        let result = match load_state(&path, session_sha256)? {
            Some(state) => {
                let selection = selection_from_state(&state)?;
                match fs::remove_file(&path) {
                    Ok(()) => Ok(selection),
                    Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(selection),
                    Err(error) => Err(error),
                }
            }
            None => Ok(None),
        };
        File::unlock(&lock)?;
        result
    }

    /// Projects only explicit subagent lifecycle evidence into a bounded count.
    /// Raw agent IDs are deliberately ignored after normalizer validation.
    pub(super) fn observe_subagent(&self, context: &CodexHookContext) -> io::Result<()> {
        let session_sha256 = session_sha256(context.session_id());
        self.with_state(&session_sha256, |state| {
            match context.event() {
                CodexHookEvent::SubagentStart => {
                    state.active_subagents = state
                        .active_subagents
                        .saturating_add(1)
                        .min(MAX_ACTIVE_SUBAGENTS);
                }
                CodexHookEvent::SubagentStop => {
                    state.active_subagents = state.active_subagents.saturating_sub(1);
                }
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "subagent observation requires an explicit lifecycle event",
                    ));
                }
            }
            Ok(())
        })
    }

    fn with_state<T>(
        &self,
        session_sha256: &str,
        mutate: impl FnOnce(&mut AnchorState) -> io::Result<T>,
    ) -> io::Result<T> {
        self.prepare_directory()?;
        let lock = self.open_lock()?;
        lock.lock()?;
        let path = self.state_path(session_sha256)?;
        let mut state =
            load_state(&path, session_sha256)?.unwrap_or_else(|| AnchorState::new(session_sha256));
        let before = state.clone();
        let result = mutate(&mut state);
        if result.is_ok() && state != before {
            write_state(&path, &state)?;
        }
        File::unlock(&lock)?;
        result
    }

    fn prepare_directory(&self) -> io::Result<()> {
        reject_symbolic_link(&self.directory)?;
        fs::create_dir_all(&self.directory)
    }

    fn open_lock(&self) -> io::Result<File> {
        let path = self.directory.join(LOCK_FILE);
        reject_symbolic_link(&path)?;
        OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)
    }

    fn state_path(&self, session_sha256: &str) -> io::Result<PathBuf> {
        if !is_sha256(session_sha256) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "root anchor session identity is invalid",
            ));
        }
        let path = self.directory.join(format!("{session_sha256}.json"));
        reject_symbolic_link(&path)?;
        Ok(path)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct AnchorState {
    schema: String,
    session_sha256: String,
    #[serde(default)]
    anchor: Option<PersistedRootWorkspaceAnchor>,
    #[serde(default)]
    active_subagents: u16,
    #[serde(default)]
    workspace_mismatch_observed: bool,
}

impl AnchorState {
    fn new(session_sha256: &str) -> Self {
        Self {
            schema: STATE_SCHEMA.to_owned(),
            session_sha256: session_sha256.to_owned(),
            anchor: None,
            active_subagents: 0,
            workspace_mismatch_observed: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PersistedRootWorkspaceAnchor {
    workspace_identity_sha256: String,
    effective_alias: String,
    binding_source: RootWorkspaceBindingSource,
    bound_at_generation: u64,
}

fn selection_from_state(state: &AnchorState) -> io::Result<Option<RootWorkspaceSelection>> {
    let Some(anchor) = state.anchor.as_ref() else {
        return Ok(None);
    };
    if !is_sha256(&anchor.workspace_identity_sha256) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "root anchor workspace identity is invalid",
        ));
    }
    let effective_alias = RepositoryAlias::new(anchor.effective_alias.clone()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "root anchor effective alias is invalid",
        )
    })?;
    Ok(Some(RootWorkspaceSelection {
        effective_alias,
        binding_source: anchor.binding_source,
        root_binding_stable: true,
        workspace_mismatch_observed: state.workspace_mismatch_observed,
        active_subagents: state.active_subagents,
    }))
}

fn load_state(path: &Path, session_sha256: &str) -> io::Result<Option<AnchorState>> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let state: AnchorState = serde_json::from_slice(&bytes)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if state.schema != STATE_SCHEMA
        || state.session_sha256 != session_sha256
        || !is_sha256(session_sha256)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "root workspace anchor state is incompatible",
        ));
    }
    if state.active_subagents > MAX_ACTIVE_SUBAGENTS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "root workspace subagent count is invalid",
        ));
    }
    selection_from_state(&state)?;
    Ok(Some(state))
}

fn write_state(path: &Path, state: &AnchorState) -> io::Result<()> {
    let bytes = serde_json::to_vec_pretty(state)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let mut file = AtomicWriteFile::options().open(path)?;
    file.write_all(&bytes)?;
    file.flush()?;
    file.commit()
}

fn session_sha256(session_id: &str) -> String {
    format!("{:x}", Sha256::digest(session_id.as_bytes()))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn reject_symbolic_link(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "root workspace anchor cannot use a symbolic link",
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}
