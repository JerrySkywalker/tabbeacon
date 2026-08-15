//! Process-safe turn generation admission for one-shot Codex Hook processes.

use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

use atomic_write_file::AtomicWriteFile;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{CodexHookContext, CodexHookEvent};

const STATE_SCHEMA: &str = "tabbeacon-codex-turn-state-v1";
const STATE_DIRECTORY: &str = "codex-turn-state-v1";
const LOCK_FILE: &str = "turn-state.lock";
const RETIRED_TURN_LIMIT: usize = 64;

/// Semantic handling requested after a Hook payload has been normalized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RequestedHandling {
    Apply,
    Preserve,
}

/// Result of admitting one event against durable session generation state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum GenerationAdmission {
    Apply,
    Preserve,
    RejectStale,
}

/// Minimal process-safe state used to reject cross-turn one-shot Hook writes.
#[derive(Debug, Clone)]
pub(super) struct CodexGenerationStore {
    directory: PathBuf,
}

impl CodexGenerationStore {
    pub(super) fn new(state_root: impl Into<PathBuf>) -> Self {
        Self {
            directory: state_root.into().join(STATE_DIRECTORY),
        }
    }

    pub(super) fn admit(
        &self,
        context: &CodexHookContext,
        requested: RequestedHandling,
    ) -> io::Result<GenerationAdmission> {
        reject_symbolic_link(&self.directory)?;
        fs::create_dir_all(&self.directory)?;
        reject_symbolic_link(&self.directory.join(LOCK_FILE))?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(self.directory.join(LOCK_FILE))?;
        lock.lock()?;
        let result = self.admit_locked(context, requested);
        File::unlock(&lock)?;
        result
    }

    fn admit_locked(
        &self,
        context: &CodexHookContext,
        requested: RequestedHandling,
    ) -> io::Result<GenerationAdmission> {
        let session_digest = identifier_digest(context.session_id());
        let path = self.directory.join(format!("{session_digest}.json"));
        reject_symbolic_link(&path)?;
        let mut state = load_state(&path, &session_digest)?;
        let before = state.clone();
        let admission = match context.event() {
            CodexHookEvent::SessionStart => {
                if requested == RequestedHandling::Preserve {
                    GenerationAdmission::Preserve
                } else {
                    state.retire_current();
                    state.generation = state.generation.saturating_add(1);
                    GenerationAdmission::Apply
                }
            }
            CodexHookEvent::SessionEnd => {
                state.retire_current();
                state.generation = state.generation.saturating_add(1);
                GenerationAdmission::Apply
            }
            CodexHookEvent::UserPromptSubmit => {
                let turn = turn_digest(context)?;
                if state.current_turn.as_deref() == Some(&turn) {
                    GenerationAdmission::Apply
                } else if state.retired_turns.contains(&turn) {
                    GenerationAdmission::RejectStale
                } else {
                    state.retire_current();
                    state.generation = state.generation.saturating_add(1);
                    state.current_turn = Some(turn);
                    GenerationAdmission::Apply
                }
            }
            CodexHookEvent::PreToolUse
            | CodexHookEvent::PermissionRequest
            | CodexHookEvent::PostToolUse
            | CodexHookEvent::PreCompact
            | CodexHookEvent::PostCompact
            | CodexHookEvent::Stop => {
                let turn = turn_digest(context)?;
                let matches_current = state.current_turn.as_deref() == Some(&turn);
                if matches_current {
                    requested.admission()
                } else if state.current_turn.is_none() && !state.retired_turns.contains(&turn) {
                    // A mid-session install or a prior fail-open Hook loss can omit the
                    // prompt observation. Admit exactly one untracked current turn; once
                    // a current generation exists, different turn IDs are always stale.
                    state.generation = state.generation.saturating_add(1);
                    state.current_turn = Some(turn);
                    requested.admission()
                } else {
                    GenerationAdmission::RejectStale
                }
            }
            CodexHookEvent::SubagentStart | CodexHookEvent::SubagentStop => {
                unreachable!("subagent lifecycle is filtered before generation admission")
            }
        };
        if state != before {
            write_state(&path, &state)?;
        }
        Ok(admission)
    }
}

impl RequestedHandling {
    const fn admission(self) -> GenerationAdmission {
        match self {
            Self::Apply => GenerationAdmission::Apply,
            Self::Preserve => GenerationAdmission::Preserve,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct GenerationState {
    schema: String,
    session_digest: String,
    generation: u64,
    current_turn: Option<String>,
    retired_turns: Vec<String>,
}

impl GenerationState {
    fn new(session_digest: &str) -> Self {
        Self {
            schema: STATE_SCHEMA.to_owned(),
            session_digest: session_digest.to_owned(),
            generation: 0,
            current_turn: None,
            retired_turns: Vec::new(),
        }
    }

    fn retire_current(&mut self) {
        if let Some(turn) = self.current_turn.take()
            && !self.retired_turns.contains(&turn)
        {
            self.retired_turns.push(turn);
            if self.retired_turns.len() > RETIRED_TURN_LIMIT {
                self.retired_turns.remove(0);
            }
        }
    }
}

fn load_state(path: &Path, session_digest: &str) -> io::Result<GenerationState> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(GenerationState::new(session_digest));
        }
        Err(error) => return Err(error),
    };
    let state: GenerationState = serde_json::from_slice(&bytes)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if state.schema != STATE_SCHEMA || state.session_digest != session_digest {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Codex turn state identity is incompatible",
        ));
    }
    Ok(state)
}

fn write_state(path: &Path, state: &GenerationState) -> io::Result<()> {
    let bytes = serde_json::to_vec_pretty(state)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let mut file = AtomicWriteFile::options().open(path)?;
    file.write_all(&bytes)?;
    file.flush()?;
    file.commit()
}

fn turn_digest(context: &CodexHookContext) -> io::Result<String> {
    context.turn_id().map(identifier_digest).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "turn-scoped Hook event lacks a turn identity",
        )
    })
}

fn identifier_digest(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn reject_symbolic_link(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Codex turn state cannot use a symbolic link",
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}
