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
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum GenerationAdmission {
    Apply(AdmittedGeneration),
    Preserve,
    RejectStale,
}

/// Content-minimal ordering ticket for one admitted root Hook event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AdmittedGeneration {
    session_sha256: String,
    turn_sha256: Option<String>,
    generation: u64,
    event_sequence: u64,
}

impl AdmittedGeneration {
    pub(super) fn session_sha256(&self) -> &str {
        &self.session_sha256
    }

    pub(super) fn turn_sha256(&self) -> Option<&str> {
        self.turn_sha256.as_deref()
    }

    pub(super) const fn generation(&self) -> u64 {
        self.generation
    }

    pub(super) const fn event_sequence(&self) -> u64 {
        self.event_sequence
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AdmissionDecision {
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
        let decision = match context.event() {
            CodexHookEvent::SessionStart => {
                if requested == RequestedHandling::Preserve {
                    AdmissionDecision::Preserve
                } else {
                    state.retire_current();
                    state.generation = state.generation.saturating_add(1);
                    AdmissionDecision::Apply
                }
            }
            CodexHookEvent::SessionEnd => {
                state.retire_current();
                state.generation = state.generation.saturating_add(1);
                AdmissionDecision::Apply
            }
            CodexHookEvent::UserPromptSubmit => {
                let turn = turn_digest(context)?;
                if state.current_turn.as_deref() == Some(&turn) {
                    AdmissionDecision::Apply
                } else if state.retired_turns.contains(&turn) {
                    AdmissionDecision::RejectStale
                } else {
                    state.retire_current();
                    state.generation = state.generation.saturating_add(1);
                    state.current_turn = Some(turn);
                    AdmissionDecision::Apply
                }
            }
            CodexHookEvent::PreToolUse
            | CodexHookEvent::PermissionRequest
            | CodexHookEvent::PostToolUse
            | CodexHookEvent::PreCompact
            | CodexHookEvent::PostCompact => {
                let turn = turn_digest(context)?;
                let matches_current = state.current_turn.as_deref() == Some(&turn);
                if matches_current {
                    requested.decision()
                } else if state.current_turn.is_none() && !state.retired_turns.contains(&turn) {
                    // A mid-session install or a prior fail-open Hook loss can omit the
                    // prompt observation. Admit exactly one untracked current turn; once
                    // a current generation exists, different turn IDs are always stale.
                    state.generation = state.generation.saturating_add(1);
                    state.current_turn = Some(turn);
                    requested.decision()
                } else {
                    AdmissionDecision::RejectStale
                }
            }
            CodexHookEvent::Stop => {
                let turn = turn_digest(context)?;
                let matches_current = state.current_turn.as_deref() == Some(&turn);
                if matches_current {
                    // A Stop is terminal for its turn. Retiring it before
                    // returning the admitted generation makes later same-turn
                    // PreTool/PostTool/compact traffic stale instead of
                    // allowing it to repaint a completed response.
                    state.retire_current();
                    AdmissionDecision::Apply
                } else if state.current_turn.is_none() && !state.retired_turns.contains(&turn) {
                    // A lost prompt observation may leave Stop as the first
                    // event. Admit that one terminal state, then retire it so
                    // a replay cannot revive the generation.
                    state.generation = state.generation.saturating_add(1);
                    state.current_turn = Some(turn);
                    state.retire_current();
                    AdmissionDecision::Apply
                } else {
                    AdmissionDecision::RejectStale
                }
            }
            CodexHookEvent::SubagentStart | CodexHookEvent::SubagentStop => {
                unreachable!("subagent lifecycle is filtered before generation admission")
            }
        };
        let admission = match decision {
            AdmissionDecision::Apply => {
                state.event_sequence = state.event_sequence.saturating_add(1);
                GenerationAdmission::Apply(AdmittedGeneration {
                    session_sha256: session_digest,
                    turn_sha256: context.turn_id().map(identifier_digest),
                    generation: state.generation,
                    event_sequence: state.event_sequence,
                })
            }
            AdmissionDecision::Preserve => GenerationAdmission::Preserve,
            AdmissionDecision::RejectStale => GenerationAdmission::RejectStale,
        };
        if state != before {
            write_state(&path, &state)?;
        }
        Ok(admission)
    }
}

impl RequestedHandling {
    const fn decision(self) -> AdmissionDecision {
        match self {
            Self::Apply => AdmissionDecision::Apply,
            Self::Preserve => AdmissionDecision::Preserve,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct GenerationState {
    schema: String,
    session_digest: String,
    generation: u64,
    #[serde(default)]
    event_sequence: u64,
    current_turn: Option<String>,
    retired_turns: Vec<String>,
}

impl GenerationState {
    fn new(session_digest: &str) -> Self {
        Self {
            schema: STATE_SCHEMA.to_owned(),
            session_digest: session_digest.to_owned(),
            generation: 0,
            event_sequence: 0,
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

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf, time::SystemTime};

    use super::{
        CodexGenerationStore, GenerationAdmission, RequestedHandling, identifier_digest, load_state,
    };
    use crate::providers::codex::{CodexHookContext, CodexHookEvent};

    struct TestRoot(PathBuf);

    impl TestRoot {
        fn new(name: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("test clock is after epoch")
                .as_nanos();
            Self(std::env::temp_dir().join(format!(
                "tabbeacon-generation-{name}-{}-{nonce}",
                std::process::id()
            )))
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn context(event: CodexHookEvent, turn_id: Option<&str>) -> CodexHookContext {
        CodexHookContext {
            event,
            session_id: "session-a".to_owned(),
            turn_id: turn_id.map(str::to_owned),
            agent_id: None,
            agent_type: None,
            session_start_source: None,
            cwd: PathBuf::from("workspace"),
        }
    }

    #[test]
    fn legacy_generation_state_defaults_the_new_event_sequence() {
        let root = TestRoot::new("legacy");
        fs::create_dir_all(&root.0).expect("test root creates");
        let session_digest = identifier_digest("session-a");
        let path = root.0.join("legacy.json");
        fs::write(
            &path,
            format!(
                "{{\"schema\":\"tabbeacon-codex-turn-state-v1\",\"session_digest\":\"{session_digest}\",\"generation\":3,\"current_turn\":null,\"retired_turns\":[]}}"
            ),
        )
        .expect("legacy fixture writes");
        let state = load_state(&path, &session_digest).expect("legacy state migrates in memory");
        assert_eq!(state.generation, 3);
        assert_eq!(state.event_sequence, 0);
    }

    #[test]
    fn every_applied_event_receives_a_monotonic_content_free_sequence() {
        let root = TestRoot::new("sequence");
        let store = CodexGenerationStore::new(&root.0);
        let prompt = context(CodexHookEvent::UserPromptSubmit, Some("turn-a"));
        let stop = context(CodexHookEvent::Stop, Some("turn-a"));
        let GenerationAdmission::Apply(first) = store
            .admit(&prompt, RequestedHandling::Apply)
            .expect("prompt admits")
        else {
            panic!("prompt must apply");
        };
        let GenerationAdmission::Apply(second) = store
            .admit(&stop, RequestedHandling::Apply)
            .expect("stop admits")
        else {
            panic!("stop must apply");
        };
        assert_eq!(first.generation(), second.generation());
        assert_eq!(first.event_sequence() + 1, second.event_sequence());
        assert_eq!(first.session_sha256(), identifier_digest("session-a"));
        assert_eq!(
            first.turn_sha256(),
            Some(identifier_digest("turn-a").as_str())
        );
    }
}
