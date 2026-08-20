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
// This state is only a bridge between one-shot Hook processes. It must never
// turn a lost `SessionEnd` into durable workspace authority for a reused native
// provider session ID.
const MAX_ANCHOR_IDLE_SECONDS: u64 = 24 * 60 * 60;

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
        observed_at_unix_seconds: u64,
        workspace_identity_sha256: &str,
        effective_alias: &RepositoryAlias,
        binding_source: RootWorkspaceBindingSource,
    ) -> io::Result<RootWorkspaceSelection> {
        self.with_state(session_sha256, observed_at_unix_seconds, |state| {
            if state
                .retired_through_generation
                .is_some_and(|retired| generation <= retired)
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "root anchor binding is stale relative to a retired generation",
                ));
            }
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
            state.last_touched_unix_seconds = observed_at_unix_seconds;
            selection_from_state(state)?.ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "root anchor was not stored")
            })
        })
    }

    /// Checks for an existing anchor without resolving or retaining an event
    /// cwd. This avoids duplicate repository discovery on first-prompt binding.
    pub(super) fn has_anchor(
        &self,
        session_sha256: &str,
        generation: u64,
        observed_at_unix_seconds: u64,
    ) -> io::Result<bool> {
        self.with_state(session_sha256, observed_at_unix_seconds, |state| {
            Ok(state
                .anchor
                .as_ref()
                .is_some_and(|anchor| anchor.bound_at_generation <= generation))
        })
    }

    /// Returns the existing alias while recording only whether this event's
    /// resolved workspace identity differs from the root anchor.
    pub(super) fn select_existing_or_observe_mismatch(
        &self,
        session_sha256: &str,
        generation: u64,
        observed_at_unix_seconds: u64,
        observed_workspace_identity_sha256: &str,
    ) -> io::Result<Option<RootWorkspaceSelection>> {
        self.with_state(session_sha256, observed_at_unix_seconds, |state| {
            let Some(anchor) = state.anchor.as_ref() else {
                return Ok(None);
            };
            if anchor.bound_at_generation > generation {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "root anchor selection is stale relative to a newer binding",
                ));
            }
            if anchor.workspace_identity_sha256 != observed_workspace_identity_sha256 {
                state.workspace_mismatch_observed = true;
            }
            state.last_touched_unix_seconds = observed_at_unix_seconds;
            selection_from_state(state)
        })
    }

    /// Reads and then retires the session anchor. The returned selection lets a
    /// `SessionEnd` render the final root title without allowing a new cwd to
    /// become title authority.
    pub(super) fn take_for_session_end(
        &self,
        session_sha256: &str,
        generation: u64,
        observed_at_unix_seconds: u64,
    ) -> io::Result<Option<RootWorkspaceSelection>> {
        self.with_state(session_sha256, observed_at_unix_seconds, |state| {
            let selection = match state.anchor.as_ref() {
                Some(anchor) if anchor.bound_at_generation <= generation => {
                    selection_from_state(state)?
                }
                Some(_) | None => None,
            };
            state.retire_through(generation);
            if state
                .anchor
                .as_ref()
                .is_some_and(|anchor| anchor.bound_at_generation <= generation)
            {
                state.anchor = None;
                state.active_subagents = 0;
                state.workspace_mismatch_observed = false;
            }
            state.last_touched_unix_seconds = observed_at_unix_seconds;
            Ok(selection)
        })
    }

    /// Projects only explicit subagent lifecycle evidence into a bounded count.
    /// Raw agent IDs are deliberately ignored after normalizer validation.
    pub(super) fn observe_subagent(
        &self,
        context: &CodexHookContext,
        observed_at_unix_seconds: u64,
    ) -> io::Result<()> {
        let session_sha256 = session_sha256(context.session_id());
        self.with_state(&session_sha256, observed_at_unix_seconds, |state| {
            if state.anchor.is_none() {
                return Ok(());
            }
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
            state.last_touched_unix_seconds = observed_at_unix_seconds;
            Ok(())
        })
    }

    fn with_state<T>(
        &self,
        session_sha256: &str,
        observed_at_unix_seconds: u64,
        mutate: impl FnOnce(&mut AnchorState) -> io::Result<T>,
    ) -> io::Result<T> {
        self.prepare_directory()?;
        let lock = self.open_lock()?;
        lock.lock()?;
        self.prune_expired_states_locked(observed_at_unix_seconds)?;
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

    fn prune_expired_states_locked(&self, observed_at_unix_seconds: u64) -> io::Result<()> {
        for entry in fs::read_dir(&self.directory)? {
            let entry = entry?;
            let path = entry.path();
            if path.file_name().is_some_and(|name| name == LOCK_FILE) {
                continue;
            }
            let Some(session_sha256) = path
                .file_stem()
                .and_then(|value| value.to_str())
                .filter(|value| is_sha256(value))
            else {
                continue;
            };
            reject_symbolic_link(&path)?;
            let Ok(Some(state)) = load_state(&path, session_sha256) else {
                // Corrupt or foreign-looking state is preserved for safe
                // diagnosis rather than being deleted by a best-effort sweep.
                continue;
            };
            if state.is_expired(observed_at_unix_seconds) {
                fs::remove_file(path)?;
            }
        }
        Ok(())
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
    #[serde(default)]
    retired_through_generation: Option<u64>,
    #[serde(default)]
    last_touched_unix_seconds: u64,
}

impl AnchorState {
    fn new(session_sha256: &str) -> Self {
        Self {
            schema: STATE_SCHEMA.to_owned(),
            session_sha256: session_sha256.to_owned(),
            anchor: None,
            active_subagents: 0,
            workspace_mismatch_observed: false,
            retired_through_generation: None,
            last_touched_unix_seconds: 0,
        }
    }

    fn retire_through(&mut self, generation: u64) {
        self.retired_through_generation = Some(
            self.retired_through_generation
                .unwrap_or_default()
                .max(generation),
        );
    }

    fn is_expired(&self, observed_at_unix_seconds: u64) -> bool {
        observed_at_unix_seconds.saturating_sub(self.last_touched_unix_seconds)
            > MAX_ANCHOR_IDLE_SECONDS
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

#[cfg(test)]
mod tests {
    use std::{
        env, fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::repo::RepositoryAlias;

    use super::{MAX_ANCHOR_IDLE_SECONDS, RootWorkspaceAnchorStore, RootWorkspaceBindingSource};

    fn test_root(label: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test clock follows Unix epoch")
            .as_nanos();
        let root = env::temp_dir().join(format!(
            "tabbeacon-root-workspace-anchor-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("owned test root is created");
        root
    }

    fn session() -> String {
        "a".repeat(64)
    }

    fn identity() -> String {
        "b".repeat(64)
    }

    #[test]
    fn session_end_tombstone_rejects_an_in_flight_older_binding() {
        let root = test_root("tombstone");
        let store = RootWorkspaceAnchorStore::new(&root);
        let session = super::session_sha256("test-session");
        let alias = RepositoryAlias::new("ROOT".to_owned()).expect("safe alias");

        assert!(
            store
                .take_for_session_end(&session, 2, 10)
                .expect("end tombstone writes safely")
                .is_none()
        );
        assert!(
            store
                .bind(
                    &session,
                    1,
                    10,
                    &identity(),
                    &alias,
                    RootWorkspaceBindingSource::SessionStartStartup,
                )
                .is_err()
        );
        assert!(
            store
                .bind(
                    &session,
                    3,
                    10,
                    &identity(),
                    &alias,
                    RootWorkspaceBindingSource::SessionStartStartup,
                )
                .is_ok()
        );

        fs::remove_dir_all(root).expect("owned test root is removed");
    }

    #[test]
    fn expired_anchor_is_pruned_before_a_reused_session_can_observe_it() {
        let root = test_root("expiry");
        let store = RootWorkspaceAnchorStore::new(&root);
        let session = session();
        let alias = RepositoryAlias::new("ROOT".to_owned()).expect("safe alias");
        store
            .bind(
                &session,
                1,
                10,
                &identity(),
                &alias,
                RootWorkspaceBindingSource::SessionStartStartup,
            )
            .expect("anchor writes safely");

        assert!(
            !store
                .has_anchor(&session, 2, 10 + MAX_ANCHOR_IDLE_SECONDS + 1)
                .expect("expired anchor is safely absent")
        );
        assert!(
            !store
                .state_path(&session)
                .expect("owned state path is valid")
                .exists()
        );

        fs::remove_dir_all(root).expect("owned test root is removed");
    }

    #[test]
    fn subagent_observation_without_a_root_anchor_creates_no_session_file() {
        let root = test_root("subagent");
        let store = RootWorkspaceAnchorStore::new(&root);
        let session = super::session_sha256("test-session");
        let context = super::CodexHookContext {
            event: super::CodexHookEvent::SubagentStart,
            session_id: "test-session".to_owned(),
            turn_id: Some("test-turn".to_owned()),
            agent_id: Some("test-agent".to_owned()),
            agent_type: Some("thread".to_owned()),
            session_start_source: None,
            cwd: root.join("alternate"),
        };

        store
            .observe_subagent(&context, 10)
            .expect("unanchored observation is ignored safely");
        assert!(
            !store
                .state_path(&session)
                .expect("owned state path is valid")
                .exists()
        );

        fs::remove_dir_all(root).expect("owned test root is removed");
    }
}
