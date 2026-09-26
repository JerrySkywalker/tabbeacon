//! Content-minimal Cursor Hook normalization. This module does not install a
//! Hook or claim a terminal route; runtime admission must bind both first.

use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use atomic_write_file::AtomicWriteFile;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::core::{Attention, FieldUpdate, Health, Phase, StatePatch};

/// Upper bound for one structured Hook request before JSON parsing.
pub const MAX_CURSOR_HOOK_BYTES: usize = 64 * 1024;
const CURSOR_GENERATION_BITS_BYTES: usize = 16 * 1024;
const CURSOR_GENERATION_HASHES: usize = 7;
const MAX_CURSOR_ROUTE_CHECKPOINT_BYTES: usize = 80 * 1024;
const CURSOR_ROUTE_SCHEMA: &str = "tabbeacon-cursor-route-v1";
const CURSOR_ROUTE_DIRECTORY: &str = "cursor-route-v1";
const CURSOR_ROUTE_LOCK: &str = "route.lock";
const CURSOR_ENDED_SESSIONS: &str = "ended-sessions.json";
const MAX_CURSOR_SESSION_FILES: usize = 1_024;
const CURSOR_ROUTE_LOCK_BUDGET: Duration = Duration::from_millis(250);

/// The small lifecycle subset needed for presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorEvent {
    SessionStart,
    BeforeSubmitPrompt,
    Stop,
    SessionEnd,
}

/// A normalized event contains only opaque session/generation identities and
/// typed state. It never retains prompts, tool data, transcripts, or email.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CursorLifecycle {
    pub event: CursorEvent,
    pub session_sha256: String,
    pub generation_sha256: Option<String>,
    pub patch: StatePatch,
}

/// A bounded, in-memory route for one positively supplied terminal/session
/// binding. The caller must independently prove the console binding before
/// constructing or using this guard; this type does not grant Hook trust.
#[derive(Debug, Clone)]
pub struct CursorSessionRoute {
    terminal_binding_sha256: String,
    session_sha256: String,
    active_generation_sha256: Option<String>,
    // An append-only Bloom filter bounds per-session state while preserving
    // rejection of every previously admitted generation. False positives
    // fail closed; no old generation is ever forgotten to make room.
    seen_generation_bits: Vec<u8>,
    ending: bool,
    ended: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CursorRouteCheckpoint {
    schema: String,
    terminal_binding_sha256: String,
    session_sha256: String,
    active_generation_sha256: Option<String>,
    seen_generation_bits: Vec<u8>,
    #[serde(default)]
    ending: bool,
    ended: bool,
}

/// An append-only bounded tombstone for completed session identities. Bloom
/// false positives can refuse a new session, but cannot revive a late one.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EndedCursorSessions {
    schema: String,
    bits: Vec<u8>,
}

impl EndedCursorSessions {
    fn empty() -> Self {
        Self {
            schema: "tabbeacon-cursor-ended-v1".to_owned(),
            bits: vec![0; CURSOR_GENERATION_BITS_BYTES],
        }
    }

    fn valid(&self) -> bool {
        self.schema == "tabbeacon-cursor-ended-v1"
            && self.bits.len() == CURSOR_GENERATION_BITS_BYTES
    }

    fn contains(&self, session: &str) -> bool {
        CursorSessionRoute::generation_positions(session).is_some_and(|positions| {
            positions
                .iter()
                .all(|position| self.bits[position / 8] & (1 << (position % 8)) != 0)
        })
    }

    fn insert(&mut self, session: &str) -> bool {
        let Some(positions) = CursorSessionRoute::generation_positions(session) else {
            return false;
        };
        for position in positions {
            self.bits[position / 8] |= 1 << (position % 8);
        }
        true
    }
}

impl CursorSessionRoute {
    /// Begins one route only from a sessionStart event on the expected console.
    #[must_use]
    pub fn from_session_start(
        event: &CursorLifecycle,
        expected_terminal_binding_sha256: &str,
        observed_terminal_binding_sha256: &str,
        console_openable: bool,
    ) -> Option<Self> {
        (event.event == CursorEvent::SessionStart
            && console_openable
            && is_sha256(expected_terminal_binding_sha256)
            && expected_terminal_binding_sha256 == observed_terminal_binding_sha256)
            .then(|| Self {
                terminal_binding_sha256: expected_terminal_binding_sha256.to_owned(),
                session_sha256: event.session_sha256.clone(),
                active_generation_sha256: None,
                seen_generation_bits: vec![0; CURSOR_GENERATION_BITS_BYTES],
                ending: false,
                ended: false,
            })
    }

    /// Serializes only bounded hashed route state for the next Hook process.
    ///
    /// # Errors
    ///
    /// Returns an error if the checkpoint cannot be serialized.
    pub fn checkpoint(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(&CursorRouteCheckpoint {
            schema: CURSOR_ROUTE_SCHEMA.to_owned(),
            terminal_binding_sha256: self.terminal_binding_sha256.clone(),
            session_sha256: self.session_sha256.clone(),
            active_generation_sha256: self.active_generation_sha256.clone(),
            seen_generation_bits: self.seen_generation_bits.clone(),
            ending: self.ending,
            ended: self.ended,
        })
    }

    /// Restores a checkpoint only when all hashes, size and invariants match.
    #[must_use]
    pub fn from_checkpoint(raw: &[u8]) -> Option<Self> {
        if raw.len() > MAX_CURSOR_ROUTE_CHECKPOINT_BYTES {
            return None;
        }
        let checkpoint: CursorRouteCheckpoint = serde_json::from_slice(raw).ok()?;
        if checkpoint.schema != CURSOR_ROUTE_SCHEMA
            || !is_sha256(&checkpoint.terminal_binding_sha256)
            || !is_sha256(&checkpoint.session_sha256)
            || (!checkpoint.ended
                && checkpoint.seen_generation_bits.len() != CURSOR_GENERATION_BITS_BYTES)
            || (checkpoint.ended && !checkpoint.seen_generation_bits.is_empty())
            || (checkpoint.ended && checkpoint.ending)
            || (checkpoint.ending && checkpoint.active_generation_sha256.is_some())
            || checkpoint
                .active_generation_sha256
                .as_deref()
                .is_some_and(|generation| !is_sha256(generation))
            || (checkpoint.ended && checkpoint.active_generation_sha256.is_some())
        {
            return None;
        }
        let route = Self {
            terminal_binding_sha256: checkpoint.terminal_binding_sha256,
            session_sha256: checkpoint.session_sha256,
            active_generation_sha256: checkpoint.active_generation_sha256,
            seen_generation_bits: checkpoint.seen_generation_bits,
            ending: checkpoint.ending,
            ended: checkpoint.ended,
        };
        if route
            .active_generation_sha256
            .as_deref()
            .is_some_and(|generation| !route.seen_contains(generation))
        {
            return None;
        }
        Some(route)
    }

    fn generation_positions(generation: &str) -> Option<[usize; CURSOR_GENERATION_HASHES]> {
        if !is_sha256(generation) {
            return None;
        }
        let mut positions = [0; CURSOR_GENERATION_HASHES];
        for (index, position) in positions.iter_mut().enumerate() {
            let start = index * 8;
            let chunk = u32::from_str_radix(&generation[start..start + 8], 16).ok()?;
            *position = (chunk as usize) % (CURSOR_GENERATION_BITS_BYTES * 8);
        }
        Some(positions)
    }

    fn seen_contains(&self, generation: &str) -> bool {
        Self::generation_positions(generation).is_some_and(|positions| {
            positions.iter().all(|position| {
                self.seen_generation_bits[position / 8] & (1 << (position % 8)) != 0
            })
        })
    }

    fn seen_insert(&mut self, generation: &str) -> bool {
        let Some(positions) = Self::generation_positions(generation) else {
            return false;
        };
        for position in positions {
            self.seen_generation_bits[position / 8] |= 1 << (position % 8);
        }
        true
    }

    /// Admits only events from the exact bound terminal and current generation.
    /// A seen superseded generation cannot reopen a newer one.
    pub fn admit(
        &mut self,
        event: &CursorLifecycle,
        observed_terminal_binding_sha256: &str,
        console_openable: bool,
    ) -> bool {
        if self.ended
            || (self.ending && event.event != CursorEvent::SessionEnd)
            || !console_openable
            || observed_terminal_binding_sha256 != self.terminal_binding_sha256
            || event.session_sha256 != self.session_sha256
        {
            return false;
        }
        match event.event {
            CursorEvent::SessionStart => false,
            CursorEvent::BeforeSubmitPrompt => {
                let Some(generation) = event.generation_sha256.as_ref() else {
                    return false;
                };
                if self.active_generation_sha256.as_ref() == Some(generation) {
                    return true;
                }
                if self.seen_contains(generation) {
                    return false;
                }
                if !self.seen_insert(generation) {
                    return false;
                }
                self.active_generation_sha256 = Some(generation.clone());
                true
            }
            CursorEvent::Stop => {
                if self.active_generation_sha256.as_ref() != event.generation_sha256.as_ref() {
                    return false;
                }
                self.active_generation_sha256 = None;
                true
            }
            CursorEvent::SessionEnd => {
                self.active_generation_sha256 = None;
                self.ending = true;
                true
            }
        }
    }

    fn finish_end(&mut self) {
        self.ending = false;
        self.ended = true;
        self.seen_generation_bits.clear();
    }
}

/// Process-safe, bounded route checkpoints for separate one-shot Hook runs.
/// This store supplies no terminal proof: its caller must establish the exact
/// owned terminal before presenting an event for admission.
#[derive(Debug, Clone)]
pub struct CursorRouteStore {
    directory: PathBuf,
}

impl CursorRouteStore {
    /// Places route state under a caller-owned local state root.
    #[must_use]
    pub fn new(state_root: impl Into<PathBuf>) -> Self {
        Self {
            directory: state_root.into().join(CURSOR_ROUTE_DIRECTORY),
        }
    }

    fn ended_path(&self) -> PathBuf {
        self.directory.join(CURSOR_ENDED_SESSIONS)
    }

    fn load_ended(&self) -> io::Result<EndedCursorSessions> {
        let path = self.ended_path();
        reject_route_symlink(&path)?;
        let file = match File::open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(EndedCursorSessions::empty());
            }
            Err(error) => return Err(error),
        };
        let mut bytes = Vec::new();
        file.take((MAX_CURSOR_ROUTE_CHECKPOINT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        let ended: EndedCursorSessions = serde_json::from_slice(&bytes)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid ended sessions"))?;
        if !ended.valid() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid ended sessions",
            ));
        }
        Ok(ended)
    }

    fn save_ended(&self, ended: &EndedCursorSessions) -> io::Result<()> {
        let path = self.ended_path();
        reject_route_symlink(&path)?;
        let bytes = serde_json::to_vec(ended).map_err(io::Error::other)?;
        let mut file = AtomicWriteFile::options().open(path)?;
        file.write_all(&bytes)?;
        file.flush()?;
        file.commit()
    }

    fn remove_exact_ended_checkpoint(path: &Path, expected: &[u8]) -> io::Result<()> {
        reject_route_symlink(path)?;
        if fs::read(path)? != expected {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Cursor route checkpoint changed before cleanup",
            ));
        }
        fs::remove_file(path)
    }

    fn prune_ended(&self, ended: &mut EndedCursorSessions) -> io::Result<usize> {
        let mut active = 0;
        let mut scanned = 0;
        for entry in fs::read_dir(&self.directory)? {
            scanned += 1;
            if scanned > MAX_CURSOR_SESSION_FILES + 16 {
                return Err(io::Error::other("Cursor route directory capacity reached"));
            }
            let entry = entry?;
            let path = entry.path();
            let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
                continue;
            };
            if path.extension().is_none_or(|extension| extension != "json") || !is_sha256(stem) {
                continue;
            }
            reject_route_symlink(&path)?;
            let mut bytes = Vec::new();
            File::open(&path)?
                .take((MAX_CURSOR_ROUTE_CHECKPOINT_BYTES + 1) as u64)
                .read_to_end(&mut bytes)?;
            let Some(route) = CursorSessionRoute::from_checkpoint(&bytes) else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "invalid Cursor route checkpoint",
                ));
            };
            if route.session_sha256 != stem {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Cursor route filename mismatch",
                ));
            }
            if route.ended {
                if !ended.contains(stem) {
                    ended.insert(stem);
                    self.save_ended(ended)?;
                }
                Self::remove_exact_ended_checkpoint(&path, &bytes)?;
            } else {
                active += 1;
            }
        }
        Ok(active)
    }

    /// Admits and persists an event under a cross-process lock.
    ///
    /// # Errors
    ///
    /// Fails closed on unsafe paths, malformed state, storage errors or full
    /// session capacity. It never reconstructs a missing start from a later event.
    pub fn admit(
        &self,
        event: &CursorLifecycle,
        expected_terminal_binding_sha256: &str,
        observed_terminal_binding_sha256: &str,
        console_openable: bool,
    ) -> io::Result<bool> {
        self.admit_with(
            event,
            expected_terminal_binding_sha256,
            observed_terminal_binding_sha256,
            console_openable,
            || Ok(false),
        )
        .map(|(admitted, _)| admitted)
    }

    /// Admits an event and applies its final presentation action before the
    /// cross-process route lock is released. A delayed Hook cannot write after
    /// a newer Hook has advanced the same route.
    ///
    /// # Errors
    ///
    /// Fails closed on route, lock, checkpoint, or presentation I/O errors.
    pub fn admit_with(
        &self,
        event: &CursorLifecycle,
        expected_terminal_binding_sha256: &str,
        observed_terminal_binding_sha256: &str,
        console_openable: bool,
        apply: impl FnOnce() -> io::Result<bool>,
    ) -> io::Result<(bool, bool)> {
        if !is_sha256(&event.session_sha256)
            || !is_sha256(expected_terminal_binding_sha256)
            || !is_sha256(observed_terminal_binding_sha256)
        {
            return Ok((false, false));
        }
        self.with_route_lock(|| {
            self.admit_locked(
                event,
                expected_terminal_binding_sha256,
                observed_terminal_binding_sha256,
                console_openable,
                apply,
            )
        })
    }

    /// Serializes a preference change with admitted Hook output. The caller
    /// must keep the critical section bounded and must not touch a terminal
    /// it cannot independently bind.
    ///
    /// # Errors
    ///
    /// Refuses unsafe paths and a route lock that stays busy beyond the Hook
    /// budget; no preference write is attempted in that case.
    pub fn with_route_lock<T>(&self, action: impl FnOnce() -> io::Result<T>) -> io::Result<T> {
        for ancestor in self.directory.ancestors() {
            reject_route_symlink(ancestor)?;
        }
        fs::create_dir_all(&self.directory)?;
        let lock_path = self.directory.join(CURSOR_ROUTE_LOCK);
        reject_route_symlink(&lock_path)?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(lock_path)?;
        let deadline = Instant::now() + CURSOR_ROUTE_LOCK_BUDGET;
        loop {
            match lock.try_lock() {
                Ok(()) => break,
                Err(fs::TryLockError::WouldBlock) => {
                    if Instant::now() >= deadline {
                        return Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "Cursor route lock busy",
                        ));
                    }
                    thread::sleep(Duration::from_millis(5));
                }
                Err(error) => return Err(error.into()),
            }
        }
        let result = action();
        // The handle drop releases the lock even if an explicit unlock reports
        // an error. Preserve the actual output disposition from the callback.
        let _ = File::unlock(&lock);
        result
    }

    fn admit_locked(
        &self,
        event: &CursorLifecycle,
        expected_terminal_binding_sha256: &str,
        observed_terminal_binding_sha256: &str,
        console_openable: bool,
        apply: impl FnOnce() -> io::Result<bool>,
    ) -> io::Result<(bool, bool)> {
        let mut ended = self.load_ended()?;
        let active_count = if event.event == CursorEvent::SessionStart {
            self.prune_ended(&mut ended)?
        } else {
            0
        };
        if event.event == CursorEvent::SessionStart && ended.contains(&event.session_sha256) {
            return Ok((false, false));
        }
        let path = self
            .directory
            .join(format!("{}.json", event.session_sha256));
        reject_route_symlink(&path)?;
        let current = match File::open(&path) {
            Ok(file) => {
                let mut bytes = Vec::new();
                file.take((MAX_CURSOR_ROUTE_CHECKPOINT_BYTES + 1) as u64)
                    .read_to_end(&mut bytes)?;
                Some(CursorSessionRoute::from_checkpoint(&bytes).ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "invalid Cursor route checkpoint",
                    )
                })?)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(error),
        };
        let mut route = if event.event == CursorEvent::SessionStart {
            if current.is_some() {
                return Ok((false, false));
            }
            if active_count >= MAX_CURSOR_SESSION_FILES {
                return Err(io::Error::other("Cursor route session capacity reached"));
            }
            let Some(route) = CursorSessionRoute::from_session_start(
                event,
                expected_terminal_binding_sha256,
                observed_terminal_binding_sha256,
                console_openable,
            ) else {
                return Ok((false, false));
            };
            route
        } else {
            let Some(route) = current else {
                return Ok((false, false));
            };
            route
        };
        if route.session_sha256 != event.session_sha256
            || route.terminal_binding_sha256 != expected_terminal_binding_sha256
        {
            return Ok((false, false));
        }
        if event.event != CursorEvent::SessionStart
            && !route.admit(event, observed_terminal_binding_sha256, console_openable)
        {
            return Ok((false, false));
        }
        // Persist the ending barrier before touching color. A crash after
        // release cannot revive a prompt; a failed release can retry only the
        // exact end while the route remains in this state.
        let bytes = route
            .checkpoint()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let mut file = AtomicWriteFile::options().open(&path)?;
        file.write_all(&bytes)?;
        file.flush()?;
        file.commit()?;
        let written = apply()?;
        if route.ending {
            route.finish_end();
            let finished = route
                .checkpoint()
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            let mut file = AtomicWriteFile::options().open(&path)?;
            file.write_all(&finished)?;
            file.flush()?;
            file.commit()?;
            if !ended.insert(&route.session_sha256) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "invalid ended Cursor session",
                ));
            }
            self.save_ended(&ended)?;
            Self::remove_exact_ended_checkpoint(&path, &finished)?;
        }
        Ok((true, written))
    }
}

fn reject_route_symlink(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Cursor route state cannot use a symbolic link",
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Content-free parsing disposition. Unsupported events remain fail-open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorParseError {
    Oversize,
    Malformed,
    MissingIdentity,
    UnsupportedOutcome,
}

/// Parses an allowlisted lifecycle event without retaining arbitrary payload.
///
/// A caller must still prove that the event belongs to the exact terminal and
/// current generation before applying its patch or writing color.
///
/// # Errors
///
/// Returns a content-free reason for malformed or insufficient evidence.
pub fn normalize_hook(raw: &[u8]) -> Result<Option<CursorLifecycle>, CursorParseError> {
    if raw.len() > MAX_CURSOR_HOOK_BYTES {
        return Err(CursorParseError::Oversize);
    }
    // Windows Cursor Hook processes may receive a UTF-8 BOM on stdin. Accept
    // that transport prefix only; the content still has to be one JSON value.
    let raw = raw.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(raw);
    let value: Value = serde_json::from_slice(raw).map_err(|_| CursorParseError::Malformed)?;
    let object = value.as_object().ok_or(CursorParseError::Malformed)?;
    let Some(event) = object.get("hook_event_name").and_then(Value::as_str) else {
        return Err(CursorParseError::Malformed);
    };
    let event = match event {
        "sessionStart" => CursorEvent::SessionStart,
        "beforeSubmitPrompt" => CursorEvent::BeforeSubmitPrompt,
        "stop" => CursorEvent::Stop,
        "sessionEnd" => CursorEvent::SessionEnd,
        _ => return Ok(None),
    };
    let conversation_id = bounded_identity(object, "conversation_id", b"cursor-session-v1:");
    let hook_session_id = bounded_identity(object, "session_id", b"cursor-session-v1:");
    if conversation_id.is_some() && hook_session_id.is_some() && conversation_id != hook_session_id
    {
        return Err(CursorParseError::MissingIdentity);
    }
    let session_sha256 = conversation_id
        .or(hook_session_id)
        .ok_or(CursorParseError::MissingIdentity)?;
    let generation_sha256 = match event {
        CursorEvent::BeforeSubmitPrompt | CursorEvent::Stop => Some(
            bounded_identity(object, "generation_id", b"cursor-generation-v1:")
                .ok_or(CursorParseError::MissingIdentity)?,
        ),
        CursorEvent::SessionStart | CursorEvent::SessionEnd => {
            bounded_identity(object, "generation_id", b"cursor-generation-v1:")
        }
    };
    let patch = match event {
        CursorEvent::SessionStart => StatePatch {
            phase: FieldUpdate::set(Phase::Ready),
            attention: FieldUpdate::clear(),
            health: FieldUpdate::clear(),
        },
        CursorEvent::BeforeSubmitPrompt => StatePatch {
            phase: FieldUpdate::set(Phase::Working),
            attention: FieldUpdate::clear(),
            health: FieldUpdate::clear(),
        },
        CursorEvent::Stop => {
            let status = object
                .get("status")
                .and_then(Value::as_str)
                .ok_or(CursorParseError::UnsupportedOutcome)?;
            let (attention, health) = match status {
                "completed" => (
                    FieldUpdate::set(Attention::ResultReady),
                    FieldUpdate::clear(),
                ),
                "aborted" => (FieldUpdate::clear(), FieldUpdate::set(Health::Interrupted)),
                "error" => (FieldUpdate::clear(), FieldUpdate::set(Health::Failed)),
                _ => return Err(CursorParseError::UnsupportedOutcome),
            };
            StatePatch {
                phase: FieldUpdate::set(Phase::WaitingUser),
                attention,
                health,
            }
        }
        CursorEvent::SessionEnd => StatePatch {
            phase: FieldUpdate::set(Phase::Ended),
            attention: FieldUpdate::clear(),
            health: FieldUpdate::unchanged(),
        },
    };
    Ok(Some(CursorLifecycle {
        event,
        session_sha256,
        generation_sha256,
        patch,
    }))
}

fn bounded_identity(object: &Map<String, Value>, name: &str, domain: &[u8]) -> Option<String> {
    let value = object.get(name)?.as_str()?;
    if value.is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
        return None;
    }
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(value.as_bytes());
    Some(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex, mpsc};

    #[test]
    fn route_lock_orders_a_delayed_output_before_newer_generation_output() {
        let root = tempfile::tempdir().unwrap();
        let store = CursorRouteStore::new(root.path());
        let terminal = "a".repeat(64);
        let event = |name: &str, generation: Option<&str>| {
            let mut input =
                serde_json::json!({"hook_event_name":name,"session_id":"barrier-session"});
            if let Some(generation) = generation {
                input["generation_id"] = generation.into();
            }
            normalize_hook(input.to_string().as_bytes())
                .unwrap()
                .unwrap()
        };
        assert!(
            store
                .admit(&event("sessionStart", None), &terminal, &terminal, true)
                .unwrap()
        );
        let first = event("beforeSubmitPrompt", Some("g1"));
        let second = event("beforeSubmitPrompt", Some("g2"));
        let writes = Arc::new(Mutex::new(Vec::new()));
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let store_a = store.clone();
        let terminal_a = terminal.clone();
        let writes_a = Arc::clone(&writes);
        let a = thread::spawn(move || {
            store_a
                .admit_with(&first, &terminal_a, &terminal_a, true, || {
                    entered_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                    writes_a.lock().unwrap().push("g1");
                    Ok(true)
                })
                .unwrap()
        });
        entered_rx.recv().unwrap();
        let store_b = store;
        let terminal_b = terminal;
        let writes_b = Arc::clone(&writes);
        let (attempt_tx, attempt_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        let b = thread::spawn(move || {
            attempt_tx.send(()).unwrap();
            let result = store_b.admit_with(&second, &terminal_b, &terminal_b, true, || {
                writes_b.lock().unwrap().push("g2");
                Ok(true)
            });
            done_tx.send(result).unwrap();
        });
        attempt_rx.recv().unwrap();
        assert!(done_rx.recv_timeout(Duration::from_millis(20)).is_err());
        release_tx.send(()).unwrap();
        assert_eq!(a.join().unwrap(), (true, true));
        assert_eq!(done_rx.recv().unwrap().unwrap(), (true, true));
        b.join().unwrap();
        assert_eq!(*writes.lock().unwrap(), ["g1", "g2"]);
    }

    #[test]
    fn failed_end_release_keeps_exact_route_retryable() {
        let root = tempfile::tempdir().unwrap();
        let store = CursorRouteStore::new(root.path());
        let terminal = "a".repeat(64);
        let event = |name: &str| {
            normalize_hook(
                serde_json::json!({"hook_event_name":name,"session_id":"retry-end"})
                    .to_string()
                    .as_bytes(),
            )
            .unwrap()
            .unwrap()
        };
        let start = event("sessionStart");
        let end = event("sessionEnd");
        assert!(store.admit(&start, &terminal, &terminal, true).unwrap());
        assert!(
            store
                .admit_with(&end, &terminal, &terminal, true, || Err(io::Error::other(
                    "injected pre-release lease failure"
                )))
                .is_err()
        );
        assert_eq!(
            store
                .admit_with(&end, &terminal, &terminal, true, || Ok(true))
                .unwrap(),
            (true, true)
        );
        assert!(!store.admit(&end, &terminal, &terminal, true).unwrap());
    }

    #[test]
    fn persisted_ending_barrier_survives_post_release_interruption() {
        let root = tempfile::tempdir().unwrap();
        let store = CursorRouteStore::new(root.path());
        let terminal = "a".repeat(64);
        let event = |name: &str, generation: Option<&str>| {
            let mut input =
                serde_json::json!({"hook_event_name":name,"session_id":"interrupted-end"});
            if let Some(generation) = generation {
                input["generation_id"] = generation.into();
            }
            normalize_hook(input.to_string().as_bytes())
                .unwrap()
                .unwrap()
        };
        let start = event("sessionStart", None);
        let end = event("sessionEnd", None);
        let late_prompt = event("beforeSubmitPrompt", Some("late"));
        assert!(store.admit(&start, &terminal, &terminal, true).unwrap());
        let path = store
            .directory
            .join(format!("{}.json", start.session_sha256));
        let mut route = CursorSessionRoute::from_checkpoint(&fs::read(&path).unwrap()).unwrap();
        assert!(route.admit(&end, &terminal, true));
        // This is the exact durable boundary after the release has flushed but
        // before final tombstone commit. A restarted Hook sees `ending`.
        fs::write(&path, route.checkpoint().unwrap()).unwrap();
        assert!(
            !store
                .admit(&late_prompt, &terminal, &terminal, true)
                .unwrap()
        );
        assert_eq!(
            store
                .admit_with(&end, &terminal, &terminal, true, || Ok(false))
                .unwrap(),
            (true, false)
        );
        assert!(
            !store
                .admit(&late_prompt, &terminal, &terminal, true)
                .unwrap()
        );
    }

    #[test]
    fn stop_status_is_typed_and_private_content_is_discarded() {
        for (status, health) in [
            ("completed", FieldUpdate::clear()),
            ("aborted", FieldUpdate::set(Health::Interrupted)),
            ("error", FieldUpdate::set(Health::Failed)),
        ] {
            let raw = format!(
                "{{\"hook_event_name\":\"stop\",\"conversation_id\":\"session-a\",\"generation_id\":\"turn-a\",\"status\":\"{status}\",\"user_email\":\"secret@example.org\",\"transcript_path\":\"private-transcript\",\"last_message\":\"private-prompt\"}}"
            );
            let normalized = normalize_hook(raw.as_bytes()).unwrap().unwrap();
            assert_eq!(normalized.patch.health, health);
            let debug = format!("{normalized:?}");
            assert!(!debug.contains("secret@example.org"));
            assert!(!debug.contains("private-transcript"));
            assert!(!debug.contains("private-prompt"));
            assert!(!debug.contains("session-a"));
            assert!(!debug.contains("turn-a"));
        }
    }

    #[test]
    fn unknown_or_unauthenticated_event_cannot_claim_state() {
        assert_eq!(
            normalize_hook(br#"{"hook_event_name":"afterAgentThought","text":"private"}"#),
            Ok(None)
        );
        assert_eq!(
            normalize_hook(br#"{"hook_event_name":"stop","status":"error"}"#),
            Err(CursorParseError::MissingIdentity)
        );
        assert_eq!(
            normalize_hook(br#"{"hook_event_name":"stop","conversation_id":"s","generation_id":"g","status":"unknown"}"#),
            Err(CursorParseError::UnsupportedOutcome)
        );
        assert_eq!(
            normalize_hook(br#"{"hook_event_name":"stop","conversation_id":"s1","session_id":"s2","generation_id":"g","status":"completed"}"#),
            Err(CursorParseError::MissingIdentity)
        );
    }

    #[test]
    fn new_structured_prompt_clears_prior_health_without_retaining_prompt() {
        let event = normalize_hook(br#"{"hook_event_name":"beforeSubmitPrompt","conversation_id":"s","generation_id":"g2","prompt":"secret text"}"#)
            .unwrap()
            .unwrap();
        assert_eq!(event.event, CursorEvent::BeforeSubmitPrompt);
        assert_eq!(event.session_sha256.len(), 64);
        assert_eq!(event.generation_sha256.as_ref().map(String::len), Some(64));
        assert!(!format!("{event:?}").contains("g2"));
        assert_eq!(event.patch.phase, FieldUpdate::set(Phase::Working));
        assert_eq!(event.patch.health, FieldUpdate::clear());
        assert!(!format!("{event:?}").contains("secret text"));
    }

    #[test]
    fn oversize_payload_is_rejected_before_json_parse() {
        let oversized = vec![b'x'; MAX_CURSOR_HOOK_BYTES + 1];
        assert_eq!(normalize_hook(&oversized), Err(CursorParseError::Oversize));
    }

    #[test]
    fn windows_stdin_bom_does_not_hide_structured_lifecycle() {
        let mut raw = vec![0xef, 0xbb, 0xbf];
        raw.extend_from_slice(br#"{"hook_event_name":"sessionStart","session_id":"session-a"}"#);
        let event = normalize_hook(&raw).unwrap().unwrap();
        assert_eq!(event.event, CursorEvent::SessionStart);

        let mut malformed = vec![0xef, 0xbb, 0xbf, 0xef, 0xbb, 0xbf];
        malformed
            .extend_from_slice(br#"{"hook_event_name":"sessionStart","session_id":"session-a"}"#);
        assert_eq!(normalize_hook(&malformed), Err(CursorParseError::Malformed));
    }

    #[test]
    fn route_rejects_foreign_terminal_session_and_superseded_generation() {
        fn event(name: &str, session: &str, generation: Option<&str>) -> CursorLifecycle {
            let mut input = serde_json::json!({
                "hook_event_name": name,
                "conversation_id": session,
            });
            if let Some(generation) = generation {
                input["generation_id"] = generation.into();
            }
            if name == "stop" {
                input["status"] = "completed".into();
            }
            normalize_hook(input.to_string().as_bytes())
                .unwrap()
                .unwrap()
        }
        let terminal = "a".repeat(64);
        let foreign_terminal = "b".repeat(64);
        let start = event("sessionStart", "session-a", None);
        assert!(
            CursorSessionRoute::from_session_start(&start, &terminal, &foreign_terminal, true)
                .is_none()
        );
        assert!(
            CursorSessionRoute::from_session_start(&start, &terminal, &terminal, false).is_none()
        );
        let mut route =
            CursorSessionRoute::from_session_start(&start, &terminal, &terminal, true).unwrap();
        let first = event("beforeSubmitPrompt", "session-a", Some("g1"));
        let second = event("beforeSubmitPrompt", "session-a", Some("g2"));
        assert!(route.admit(&first, &terminal, true));
        assert!(!route.admit(&second, &foreign_terminal, true));
        assert!(!route.admit(&event("stop", "session-b", Some("g1")), &terminal, true));
        assert!(route.admit(&second, &terminal, true));
        assert!(!route.admit(&event("stop", "session-a", Some("g1")), &terminal, true));
        assert!(!route.admit(&first, &terminal, true));
        assert!(route.admit(&event("stop", "session-a", Some("g2")), &terminal, true));
        assert!(!route.admit(&second, &terminal, true));
        assert!(route.admit(&event("sessionEnd", "session-a", None), &terminal, true));
        assert!(!route.admit(
            &event("beforeSubmitPrompt", "session-a", Some("g3")),
            &terminal,
            true
        ));
    }

    #[test]
    fn bounded_route_accepts_long_session_across_hook_process_rebuilds() {
        let terminal = "a".repeat(64);
        let start = normalize_hook(
            br#"{"hook_event_name":"sessionStart","conversation_id":"long-session"}"#,
        )
        .unwrap()
        .unwrap();
        let mut route = CursorSessionRoute::from_session_start(&start, &terminal, &terminal, true)
            .expect("bound start");
        let mut first_prompt = None;
        let mut first_stop = None;
        for round in 1..=1_000 {
            let prompt = normalize_hook(
                format!("{{\"hook_event_name\":\"beforeSubmitPrompt\",\"conversation_id\":\"long-session\",\"generation_id\":\"round-{round}\"}}").as_bytes(),
            )
            .unwrap()
            .unwrap();
            let stop = normalize_hook(
                format!("{{\"hook_event_name\":\"stop\",\"conversation_id\":\"long-session\",\"generation_id\":\"round-{round}\",\"status\":\"completed\"}}").as_bytes(),
            )
            .unwrap()
            .unwrap();
            assert!(route.admit(&prompt, &terminal, true), "prompt {round}");
            if round == 1 {
                first_prompt = Some(prompt.clone());
                first_stop = Some(stop.clone());
            }
            assert!(route.admit(&stop, &terminal, true), "stop {round}");
            // Every Hook invocation may be a new process. Persist only the
            // bounded opaque checkpoint, then rebuild before the next event.
            route = CursorSessionRoute::from_checkpoint(&route.checkpoint().unwrap())
                .expect("checkpoint accepted");
            if round >= 33 {
                assert!(!route.admit(first_prompt.as_ref().unwrap(), &terminal, true));
                assert!(!route.admit(first_stop.as_ref().unwrap(), &terminal, true));
            }
        }
        assert!(!route.admit(first_prompt.as_ref().unwrap(), &terminal, true));
        let end =
            normalize_hook(br#"{"hook_event_name":"sessionEnd","conversation_id":"long-session"}"#)
                .unwrap()
                .unwrap();
        assert!(route.admit(&end, &terminal, true));
        let mut ended = CursorSessionRoute::from_checkpoint(&route.checkpoint().unwrap()).unwrap();
        assert!(!ended.admit(first_prompt.as_ref().unwrap(), &terminal, true));
        assert!(
            CursorSessionRoute::from_checkpoint(&vec![b'x'; MAX_CURSOR_ROUTE_CHECKPOINT_BYTES + 1])
                .is_none()
        );
    }

    #[test]
    fn separate_hook_processes_reuse_only_the_exact_persisted_session_route() {
        let root = std::env::temp_dir().join(format!(
            "tabbeacon-cursor-route-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let terminal = "a".repeat(64);
        let foreign = "b".repeat(64);
        let event = |name: &str, generation: Option<&str>| {
            let mut input = serde_json::json!({
                "hook_event_name": name,
                "conversation_id": "stored-session",
            });
            if let Some(generation) = generation {
                input["generation_id"] = generation.into();
            }
            if name == "stop" {
                input["status"] = "completed".into();
            }
            normalize_hook(input.to_string().as_bytes())
                .unwrap()
                .unwrap()
        };
        let start = event("sessionStart", None);
        let first = event("beforeSubmitPrompt", Some("round-1"));
        assert!(
            !CursorRouteStore::new(&root)
                .admit(&first, &terminal, &terminal, true)
                .unwrap()
        );
        assert!(
            !CursorRouteStore::new(&root)
                .admit(&start, &terminal, &foreign, true)
                .unwrap()
        );
        assert!(
            CursorRouteStore::new(&root)
                .admit(&start, &terminal, &terminal, true)
                .unwrap()
        );
        assert!(
            !CursorRouteStore::new(&root)
                .admit(&start, &terminal, &terminal, true)
                .unwrap()
        );
        for round in 1..=34 {
            let generation = format!("round-{round}");
            let prompt = event("beforeSubmitPrompt", Some(&generation));
            let stop = event("stop", Some(&generation));
            assert!(
                CursorRouteStore::new(&root)
                    .admit(&prompt, &terminal, &terminal, true)
                    .unwrap()
            );
            assert!(
                !CursorRouteStore::new(&root)
                    .admit(&stop, &terminal, &foreign, true)
                    .unwrap()
            );
            assert!(
                CursorRouteStore::new(&root)
                    .admit(&stop, &terminal, &terminal, true)
                    .unwrap()
            );
        }
        assert!(
            !CursorRouteStore::new(&root)
                .admit(&first, &terminal, &terminal, true)
                .unwrap()
        );
        let end = event("sessionEnd", None);
        assert!(
            CursorRouteStore::new(&root)
                .admit(&end, &terminal, &terminal, true)
                .unwrap()
        );
        assert!(
            !CursorRouteStore::new(&root)
                .admit(&first, &terminal, &terminal, true)
                .unwrap()
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn ended_sessions_are_compacted_without_forgetting_late_or_duplicate_events() {
        let root = tempfile::tempdir().unwrap();
        let store = CursorRouteStore::new(root.path());
        let terminal = "a".repeat(64);
        let foreign = store.directory.join("foreign.json");
        for number in 0..=MAX_CURSOR_SESSION_FILES {
            let session = format!("session-{number}");
            let start = normalize_hook(
                serde_json::json!({"hook_event_name":"sessionStart","session_id":session})
                    .to_string()
                    .as_bytes(),
            )
            .unwrap()
            .unwrap();
            let end = normalize_hook(
                serde_json::json!({"hook_event_name":"sessionEnd","session_id":session})
                    .to_string()
                    .as_bytes(),
            )
            .unwrap()
            .unwrap();
            assert!(store.admit(&start, &terminal, &terminal, true).unwrap());
            if number == 0 {
                fs::write(&foreign, b"foreign content").unwrap();
            }
            assert!(store.admit(&end, &terminal, &terminal, true).unwrap());
            assert!(!store.admit(&start, &terminal, &terminal, true).unwrap());
            assert!(!store.admit(&end, &terminal, &terminal, true).unwrap());
            assert!(
                !store
                    .admit(&start, &terminal, &"b".repeat(64), true)
                    .unwrap()
            );
        }
        assert_eq!(fs::read(&foreign).unwrap(), b"foreign content");
        let session_files = fs::read_dir(&store.directory)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .path()
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .is_some_and(is_sha256)
            })
            .count();
        assert_eq!(session_files, 0);
    }

    #[test]
    fn start_only_routes_reach_a_bounded_fail_closed_capacity_without_deletion() {
        let root = tempfile::tempdir().unwrap();
        let store = CursorRouteStore::new(root.path());
        fs::create_dir_all(&store.directory).unwrap();
        let terminal = "a".repeat(64);
        let mut first_path = None;
        for number in 0..MAX_CURSOR_SESSION_FILES {
            let input = serde_json::json!({
                "hook_event_name":"sessionStart",
                "session_id":format!("start-only-{number}")
            });
            let start = normalize_hook(input.to_string().as_bytes())
                .unwrap()
                .unwrap();
            let route =
                CursorSessionRoute::from_session_start(&start, &terminal, &terminal, true).unwrap();
            let path = store
                .directory
                .join(format!("{}.json", start.session_sha256));
            fs::write(&path, route.checkpoint().unwrap()).unwrap();
            first_path.get_or_insert(path);
        }
        let next =
            normalize_hook(br#"{"hook_event_name":"sessionStart","session_id":"one-too-many"}"#)
                .unwrap()
                .unwrap();
        let error = store.admit(&next, &terminal, &terminal, true).unwrap_err();
        assert!(error.to_string().contains("capacity reached"));
        assert!(
            first_path.unwrap().exists(),
            "unknown live routes are retained"
        );
        assert!(
            !store
                .directory
                .join(format!("{}.json", next.session_sha256))
                .exists()
        );
    }

    #[test]
    fn restart_finishes_only_an_exact_ended_checkpoint() {
        let root = tempfile::tempdir().unwrap();
        let store = CursorRouteStore::new(root.path());
        let terminal = "a".repeat(64);
        let event = |name: &str, session: &str| {
            normalize_hook(
                serde_json::json!({"hook_event_name":name,"session_id":session})
                    .to_string()
                    .as_bytes(),
            )
            .unwrap()
            .unwrap()
        };
        let start = event("sessionStart", "interrupted-cleanup");
        let end = event("sessionEnd", "interrupted-cleanup");
        assert!(store.admit(&start, &terminal, &terminal, true).unwrap());
        let path = store
            .directory
            .join(format!("{}.json", start.session_sha256));
        let mut route = CursorSessionRoute::from_checkpoint(&fs::read(&path).unwrap()).unwrap();
        assert!(route.admit(&end, &terminal, true));
        route.finish_end();
        fs::write(&path, route.checkpoint().unwrap()).unwrap();
        // The previous Hook process stopped after persisting the ended route,
        // before updating the tombstone and deleting its exact checkpoint.
        let other = event("sessionStart", "new-session");
        assert!(store.admit(&other, &terminal, &terminal, true).unwrap());
        assert!(!path.exists());
        assert!(!store.admit(&start, &terminal, &terminal, true).unwrap());
        assert!(!store.admit(&end, &terminal, &terminal, true).unwrap());
    }
}
