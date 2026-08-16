//! Provider-neutral, session-scoped activity worker ownership.
//!
//! Hook adapters publish only hashed identity, semantic presentation state,
//! and a bounded lease. The worker never receives or persists raw Hook bodies.

use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use atomic_write_file::AtomicWriteFile;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    core::{Attention, Health, Phase},
    presentation::{
        PresentationAction, PresentationPolicy, SemanticPresentationInput, TitleStatus,
        WindowsTerminalCapabilities, WindowsTerminalRenderer,
    },
    settings::{
        ActivityMode, PresentationSettings, PresentationTheme, SpinnerPreset, TabColorMode,
        TitleMode,
    },
};

const LEASE_SCHEMA: &str = "tabbeacon-activity-worker-lease-v1";
const EXIT_SCHEMA: &str = "tabbeacon-activity-worker-exit-v1";
const STATE_DIRECTORY: &str = "activity-worker-v1";
const LOCK_FILE: &str = "activity-worker.lock";
const LEASE_TTL_MS: u64 = 24 * 60 * 60 * 1_000;
const FRAME_INTERVAL_MS: u64 = 180;
const PREDECESSOR_WAIT_MS: u64 = 750;
const PREDECESSOR_POLL_MS: u64 = 25;

/// Minimal provider-neutral identity for one ephemeral worker generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerKey {
    session_sha256: String,
    turn_sha256: Option<String>,
    generation: u64,
    terminal_binding_sha256: String,
    digest: String,
}

impl WorkerKey {
    /// Creates a key from content-minimal digests and a turn generation.
    #[must_use]
    pub fn new(
        session_sha256: &str,
        turn_sha256: Option<&str>,
        generation: u64,
        terminal_binding_sha256: &str,
    ) -> Self {
        // The storage locator is stable for one session/terminal binding so a
        // newer turn can atomically discover and supersede its predecessor.
        // Turn and generation remain explicit fields in the lease itself.
        let digest = framed_digest(&[session_sha256, terminal_binding_sha256]);
        Self {
            session_sha256: session_sha256.to_owned(),
            turn_sha256: turn_sha256.map(str::to_owned),
            generation,
            terminal_binding_sha256: terminal_binding_sha256.to_owned(),
            digest,
        }
    }

    /// Opaque key used for lease filenames and worker launch arguments.
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

/// Content-minimal presentation snapshot consumed by an active worker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerPresentation {
    workspace_alias: String,
    semantic_state: String,
    spinner_preset: String,
}

impl WorkerPresentation {
    /// Creates the only currently admitted worker state: active work.
    #[must_use]
    pub fn working(workspace_alias: &str, spinner: SpinnerPreset) -> Self {
        Self {
            workspace_alias: workspace_alias.to_owned(),
            semantic_state: "working".to_owned(),
            spinner_preset: spinner.as_str().to_owned(),
        }
    }

    fn spinner(&self) -> Option<SpinnerPreset> {
        SpinnerPreset::parse(&self.spinner_preset)
    }
}

/// Rendering decision returned to the one-shot Hook runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActivityRender {
    /// Coordination is unavailable, so the Hook uses its static fail-open path.
    UncoordinatedFull,
    /// The Hook owns the complete static action.
    Full,
    /// A live worker owns title frames; the Hook owns other channels.
    WithoutTitle,
    /// Ownership could not be proved, so decoration is suppressed fail-open.
    Suppress,
}

#[derive(Debug, Clone)]
enum ActivityExecution {
    Disabled,
    System {
        executable: PathBuf,
        owner_sha256: String,
        terminal_binding_sha256: String,
    },
}

/// Atomic lease coordinator used by a one-shot provider Hook.
#[derive(Debug, Clone)]
pub(crate) struct ActivityCoordinator {
    store: ActivityLeaseStore,
    execution: ActivityExecution,
}

impl ActivityCoordinator {
    /// Creates a deterministic no-worker coordinator for injected tests.
    #[must_use]
    pub(crate) fn disabled(state_root: impl Into<PathBuf>) -> Self {
        Self {
            store: ActivityLeaseStore::new(state_root),
            execution: ActivityExecution::Disabled,
        }
    }

    /// Creates the production coordinator for the inherited terminal binding.
    pub(crate) fn system(state_root: impl Into<PathBuf>) -> io::Result<Self> {
        let executable = env::current_exe()?;
        let owner_sha256 = executable_owner_sha256(&executable)?;
        let terminal_binding_sha256 = terminal_binding_from_environment()?;
        Ok(Self {
            store: ActivityLeaseStore::new(state_root),
            execution: ActivityExecution::System {
                executable,
                owner_sha256,
                terminal_binding_sha256,
            },
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn reconcile(
        &self,
        session_sha256: &str,
        turn_sha256: Option<&str>,
        generation: u64,
        event_sequence: u64,
        workspace_alias: &str,
        action: &PresentationAction,
        settings: PresentationSettings,
    ) -> ActivityRender {
        let ActivityExecution::System {
            executable,
            owner_sha256,
            terminal_binding_sha256,
        } = &self.execution
        else {
            return ActivityRender::UncoordinatedFull;
        };
        let key = WorkerKey::new(
            session_sha256,
            turn_sha256,
            generation,
            terminal_binding_sha256,
        );
        let working = match action {
            PresentationAction::Apply(state) | PresentationAction::Reset(state) => {
                state.title_status() == TitleStatus::Working
            }
        };
        let worker_managed = working
            && settings.title() == TitleMode::TabBeacon
            && settings.activity().uses_worker_animation();
        let now = unix_ms();
        if worker_managed {
            let presentation = WorkerPresentation::working(workspace_alias, settings.spinner());
            let Ok(transition) =
                self.store
                    .publish_active(&key, event_sequence, owner_sha256, &presentation, now)
            else {
                return ActivityRender::UncoordinatedFull;
            };
            match transition {
                LeaseTransition::Stale => ActivityRender::Suppress,
                LeaseTransition::AlreadyActive => ActivityRender::WithoutTitle,
                LeaseTransition::Published { lease, predecessor } => {
                    if predecessor.as_ref().is_some_and(|predecessor| {
                        !self.store.wait_for_exit(predecessor, PREDECESSOR_WAIT_MS)
                    }) {
                        let _ = self.store.deactivate_if_owned(&lease, unix_ms());
                        return ActivityRender::Suppress;
                    }
                    if spawn_worker(executable, &lease).is_err() {
                        let _ = self.store.deactivate_if_owned(&lease, unix_ms());
                        ActivityRender::Full
                    } else {
                        ActivityRender::WithoutTitle
                    }
                }
                LeaseTransition::Stopped { .. } => {
                    unreachable!("active publication cannot return a stopped transition")
                }
            }
        } else {
            let Ok(transition) =
                self.store
                    .publish_stopped(&key, event_sequence, owner_sha256, now)
            else {
                return ActivityRender::UncoordinatedFull;
            };
            match transition {
                LeaseTransition::Stale => ActivityRender::Suppress,
                LeaseTransition::Stopped { predecessor } => {
                    if predecessor.as_ref().is_some_and(|predecessor| {
                        !self.store.wait_for_exit(predecessor, PREDECESSOR_WAIT_MS)
                    }) {
                        ActivityRender::Suppress
                    } else {
                        ActivityRender::Full
                    }
                }
                LeaseTransition::AlreadyActive | LeaseTransition::Published { .. } => {
                    unreachable!("stop publication cannot return an active transition")
                }
            }
        }
    }

    /// Writes only if this event still owns the atomic lease ordering point.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn write_rendered(
        &self,
        session_sha256: &str,
        turn_sha256: Option<&str>,
        generation: u64,
        event_sequence: u64,
        render: ActivityRender,
        bytes: &[u8],
        sink: &mut impl Write,
    ) -> io::Result<()> {
        match render {
            ActivityRender::Suppress => Ok(()),
            ActivityRender::UncoordinatedFull => {
                sink.write_all(bytes)?;
                sink.flush()
            }
            ActivityRender::Full | ActivityRender::WithoutTitle => {
                let ActivityExecution::System {
                    terminal_binding_sha256,
                    ..
                } = &self.execution
                else {
                    return Ok(());
                };
                let key = WorkerKey::new(
                    session_sha256,
                    turn_sha256,
                    generation,
                    terminal_binding_sha256,
                );
                // The lease lock orders the final one-shot write against every
                // competing Hook transition. A delayed event that admitted
                // before a newer event cannot write after the newer lease.
                self.store.with_lock(|| {
                    let current = self.store.load(key.digest())?;
                    if current.as_ref().is_some_and(|lease| {
                        lease.generation == generation && lease.event_sequence == event_sequence
                    }) {
                        sink.write_all(bytes)?;
                        sink.flush()?;
                    }
                    Ok(())
                })
            }
        }
    }
}

/// Runs the hidden production worker command. Every failure is decoration-only.
pub fn run_activity_worker_system(key_digest: &str, generation: u64, revision: u64) {
    let Ok(state_root) = crate::repo::StableAliasRegistry::default_state_root() else {
        return;
    };
    let Ok(coordinator) = ActivityCoordinator::system(state_root) else {
        return;
    };
    let ActivityExecution::System {
        owner_sha256,
        terminal_binding_sha256,
        ..
    } = coordinator.execution
    else {
        return;
    };
    if !is_sha256(key_digest) {
        return;
    }
    coordinator.store.run_worker(
        key_digest,
        generation,
        revision,
        &owner_sha256,
        &terminal_binding_sha256,
    );
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct WorkerLease {
    schema: String,
    key_sha256: String,
    session_sha256: String,
    turn_sha256: Option<String>,
    terminal_binding_sha256: String,
    generation: u64,
    event_sequence: u64,
    revision: u64,
    owner_sha256: String,
    active: bool,
    presentation: Option<WorkerPresentation>,
    updated_unix_ms: u64,
    expires_unix_ms: u64,
}

impl WorkerLease {
    fn ownership(&self) -> WorkerOwnership {
        WorkerOwnership {
            key_sha256: self.key_sha256.clone(),
            generation: self.generation,
            revision: self.revision,
            owner_sha256: self.owner_sha256.clone(),
        }
    }

    fn same_worker(&self, other: &Self) -> bool {
        self.key_sha256 == other.key_sha256
            && self.generation == other.generation
            && self.revision == other.revision
            && self.owner_sha256 == other.owner_sha256
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorkerOwnership {
    key_sha256: String,
    generation: u64,
    revision: u64,
    owner_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct WorkerExitReceipt {
    schema: String,
    key_sha256: String,
    generation: u64,
    revision: u64,
    owner_sha256: String,
    exited_unix_ms: u64,
    exit_reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LeaseTransition {
    Stale,
    AlreadyActive,
    Published {
        lease: Box<WorkerLease>,
        predecessor: Option<WorkerOwnership>,
    },
    Stopped {
        predecessor: Option<WorkerOwnership>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ActivityLeaseStore {
    directory: PathBuf,
}

impl ActivityLeaseStore {
    fn new(state_root: impl Into<PathBuf>) -> Self {
        Self {
            directory: state_root.into().join(STATE_DIRECTORY),
        }
    }

    fn publish_active(
        &self,
        key: &WorkerKey,
        event_sequence: u64,
        owner_sha256: &str,
        presentation: &WorkerPresentation,
        now: u64,
    ) -> io::Result<LeaseTransition> {
        self.with_lock(|| {
            let current = self.load(key.digest())?;
            if current
                .as_ref()
                .is_some_and(|lease| is_stale(key.generation, event_sequence, lease))
            {
                return Ok(LeaseTransition::Stale);
            }
            if let Some(mut current) = current.clone()
                && current.active
                && current.generation == key.generation
                && current.owner_sha256 == owner_sha256
                && current.presentation.as_ref() == Some(presentation)
            {
                current.event_sequence = current.event_sequence.max(event_sequence);
                current.updated_unix_ms = now;
                current.expires_unix_ms = now.saturating_add(LEASE_TTL_MS);
                self.write(&current)?;
                return Ok(LeaseTransition::AlreadyActive);
            }
            let predecessor = current
                .as_ref()
                .filter(|lease| lease.active)
                .map(WorkerLease::ownership);
            let revision = current
                .as_ref()
                .map_or(1, |lease| lease.revision.saturating_add(1));
            let lease = WorkerLease {
                schema: LEASE_SCHEMA.to_owned(),
                key_sha256: key.digest.clone(),
                session_sha256: key.session_sha256.clone(),
                turn_sha256: key.turn_sha256.clone(),
                terminal_binding_sha256: key.terminal_binding_sha256.clone(),
                generation: key.generation,
                event_sequence,
                revision,
                owner_sha256: owner_sha256.to_owned(),
                active: true,
                presentation: Some(presentation.clone()),
                updated_unix_ms: now,
                expires_unix_ms: now.saturating_add(LEASE_TTL_MS),
            };
            validate_lease(&lease)?;
            self.write(&lease)?;
            Ok(LeaseTransition::Published {
                lease: Box::new(lease),
                predecessor,
            })
        })
    }

    fn publish_stopped(
        &self,
        key: &WorkerKey,
        event_sequence: u64,
        owner_sha256: &str,
        now: u64,
    ) -> io::Result<LeaseTransition> {
        self.with_lock(|| {
            let current = self.load(key.digest())?;
            if current
                .as_ref()
                .is_some_and(|lease| is_stale(key.generation, event_sequence, lease))
            {
                return Ok(LeaseTransition::Stale);
            }
            let predecessor = current
                .as_ref()
                .filter(|lease| lease.active)
                .map(WorkerLease::ownership);
            let revision = current.as_ref().map_or(1, |lease| {
                if lease.active {
                    lease.revision.saturating_add(1)
                } else {
                    lease.revision
                }
            });
            let lease = WorkerLease {
                schema: LEASE_SCHEMA.to_owned(),
                key_sha256: key.digest.clone(),
                session_sha256: key.session_sha256.clone(),
                turn_sha256: key.turn_sha256.clone(),
                terminal_binding_sha256: key.terminal_binding_sha256.clone(),
                generation: key.generation,
                event_sequence,
                revision,
                owner_sha256: owner_sha256.to_owned(),
                active: false,
                presentation: None,
                updated_unix_ms: now,
                expires_unix_ms: now,
            };
            validate_lease(&lease)?;
            self.write(&lease)?;
            Ok(LeaseTransition::Stopped { predecessor })
        })
    }

    fn deactivate_if_owned(&self, expected: &WorkerLease, now: u64) -> io::Result<()> {
        self.with_lock(|| {
            let Some(mut current) = self.load(&expected.key_sha256)? else {
                return Ok(());
            };
            if current.same_worker(expected) && current.active {
                current.active = false;
                current.presentation = None;
                current.updated_unix_ms = now;
                current.expires_unix_ms = now;
                self.write(&current)?;
            }
            Ok(())
        })
    }

    fn wait_for_exit(&self, predecessor: &WorkerOwnership, timeout_ms: u64) -> bool {
        let deadline = unix_ms().saturating_add(timeout_ms);
        loop {
            let path = self.exit_path(predecessor);
            if path.is_file() {
                let valid = fs::read(&path)
                    .ok()
                    .and_then(|bytes| serde_json::from_slice::<WorkerExitReceipt>(&bytes).ok())
                    .is_some_and(|receipt| {
                        receipt.schema == EXIT_SCHEMA
                            && receipt.key_sha256 == predecessor.key_sha256
                            && receipt.generation == predecessor.generation
                            && receipt.revision == predecessor.revision
                            && receipt.owner_sha256 == predecessor.owner_sha256
                    });
                if valid {
                    let _ = fs::remove_file(path);
                    return true;
                }
            }
            if unix_ms() >= deadline {
                return false;
            }
            thread::sleep(Duration::from_millis(PREDECESSOR_POLL_MS));
        }
    }

    fn run_worker(
        &self,
        key_digest: &str,
        generation: u64,
        revision: u64,
        owner_sha256: &str,
        terminal_binding_sha256: &str,
    ) {
        let ownership = WorkerOwnership {
            key_sha256: key_digest.to_owned(),
            generation,
            revision,
            owner_sha256: owner_sha256.to_owned(),
        };
        let mut reason = "lease_unavailable";
        let Some(initial) = self.load_worker_lease(&ownership, terminal_binding_sha256, unix_ms())
        else {
            let _ = self.write_exit(&ownership, reason);
            return;
        };
        let Some(presentation) = initial.presentation.clone() else {
            let _ = self.write_exit(&ownership, "inactive");
            return;
        };
        let Some(spinner) = presentation.spinner() else {
            let _ = self.write_exit(&ownership, "invalid_presentation");
            return;
        };
        let Ok(mut console) = open_owned_console() else {
            let _ = self.write_exit(&ownership, "terminal_unavailable");
            return;
        };
        let settings = PresentationSettings::new(
            TitleMode::TabBeacon,
            TabColorMode::Off,
            ActivityMode::TitleSpinner,
            spinner,
            PresentationTheme::MutedDark,
        );
        let renderer = WindowsTerminalRenderer::with_settings(
            WindowsTerminalCapabilities::new(false),
            settings,
        );
        let action = PresentationPolicy::resolve(SemanticPresentationInput::new(
            Phase::Working,
            Attention::None,
            Health::Normal,
            &presentation.workspace_alias,
        ));
        let state = match &action {
            PresentationAction::Apply(state) | PresentationAction::Reset(state) => state,
        };
        let mut frame_index = 0_usize;
        loop {
            let Some(_) = self.load_worker_lease(&ownership, terminal_binding_sha256, unix_ms())
            else {
                reason = "superseded_or_expired";
                break;
            };
            let bytes = renderer.render_title_spinner_frame(state, frame_index);
            if console
                .write_all(&bytes)
                .and_then(|()| console.flush())
                .is_err()
            {
                reason = "terminal_unavailable";
                break;
            }
            frame_index = frame_index.saturating_add(1);
            thread::sleep(Duration::from_millis(FRAME_INTERVAL_MS));
        }
        let _ = self.write_exit(&ownership, reason);
    }

    fn load_worker_lease(
        &self,
        ownership: &WorkerOwnership,
        terminal_binding_sha256: &str,
        now: u64,
    ) -> Option<WorkerLease> {
        let lease = self.with_lock(|| self.load(&ownership.key_sha256)).ok()??;
        (lease.active
            && lease.generation == ownership.generation
            && lease.revision == ownership.revision
            && lease.owner_sha256 == ownership.owner_sha256
            && lease.terminal_binding_sha256 == terminal_binding_sha256
            && lease.presentation.is_some()
            && now <= lease.expires_unix_ms)
            .then_some(lease)
    }

    fn write_exit(&self, ownership: &WorkerOwnership, reason: &str) -> io::Result<()> {
        let receipt = WorkerExitReceipt {
            schema: EXIT_SCHEMA.to_owned(),
            key_sha256: ownership.key_sha256.clone(),
            generation: ownership.generation,
            revision: ownership.revision,
            owner_sha256: ownership.owner_sha256.clone(),
            exited_unix_ms: unix_ms(),
            exit_reason: reason.to_owned(),
        };
        let bytes = serde_json::to_vec_pretty(&receipt)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        self.with_lock(|| atomic_write(&self.exit_path(ownership), &bytes))
    }

    fn with_lock<T>(&self, operation: impl FnOnce() -> io::Result<T>) -> io::Result<T> {
        reject_symbolic_link(&self.directory)?;
        fs::create_dir_all(&self.directory)?;
        let lock_path = self.directory.join(LOCK_FILE);
        reject_symbolic_link(&lock_path)?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(lock_path)?;
        lock.lock()?;
        let result = operation();
        File::unlock(&lock)?;
        result
    }

    fn load(&self, key_digest: &str) -> io::Result<Option<WorkerLease>> {
        if !is_sha256(key_digest) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "activity worker key is not an opaque SHA-256 digest",
            ));
        }
        let path = self.lease_path(key_digest);
        reject_symbolic_link(&path)?;
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let lease: WorkerLease = serde_json::from_slice(&bytes)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        validate_lease(&lease)?;
        if lease.key_sha256 != key_digest {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "activity lease filename does not match its key",
            ));
        }
        Ok(Some(lease))
    }

    fn write(&self, lease: &WorkerLease) -> io::Result<()> {
        validate_lease(lease)?;
        let bytes = serde_json::to_vec_pretty(lease)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        atomic_write(&self.lease_path(&lease.key_sha256), &bytes)
    }

    fn lease_path(&self, key_digest: &str) -> PathBuf {
        self.directory.join(format!("lease-{key_digest}.json"))
    }

    fn exit_path(&self, ownership: &WorkerOwnership) -> PathBuf {
        self.directory.join(format!(
            "exit-{}-{}-{}-{}.json",
            ownership.key_sha256,
            ownership.generation,
            ownership.revision,
            &ownership.owner_sha256[..16]
        ))
    }
}

fn is_stale(generation: u64, event_sequence: u64, current: &WorkerLease) -> bool {
    generation < current.generation
        || (generation == current.generation && event_sequence < current.event_sequence)
}

fn validate_lease(lease: &WorkerLease) -> io::Result<()> {
    let presentation_valid = lease.presentation.as_ref().is_none_or(|presentation| {
        presentation.semantic_state == "working"
            && presentation.spinner().is_some()
            && presentation.workspace_alias.chars().count() <= 80
            && !presentation.workspace_alias.chars().any(char::is_control)
    });
    if lease.schema != LEASE_SCHEMA
        || !is_sha256(&lease.key_sha256)
        || !is_sha256(&lease.session_sha256)
        || lease
            .turn_sha256
            .as_deref()
            .is_some_and(|value| !is_sha256(value))
        || !is_sha256(&lease.terminal_binding_sha256)
        || !is_sha256(&lease.owner_sha256)
        || lease.active != lease.presentation.is_some()
        || !presentation_valid
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "activity worker lease is incompatible or unsafe",
        ));
    }
    Ok(())
}

fn spawn_worker(executable: &Path, lease: &WorkerLease) -> io::Result<()> {
    Command::new(executable)
        .args([
            "__activity-worker-v1",
            &lease.key_sha256,
            &lease.generation.to_string(),
            &lease.revision.to_string(),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
}

fn executable_owner_sha256(path: &Path) -> io::Result<String> {
    let canonical = fs::canonicalize(path)?;
    let normalized = canonical
        .to_string_lossy()
        .replace('\\', "/")
        .to_lowercase();
    let mut digest = Sha256::new();
    digest.update(normalized.len().to_le_bytes());
    digest.update(normalized.as_bytes());
    let mut file = File::open(canonical)?;
    let mut buffer = vec![0_u8; 64 * 1_024].into_boxed_slice();
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn terminal_binding_from_environment() -> io::Result<String> {
    env::var("WT_SESSION")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|value| framed_digest(&[&value]))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "originating Windows Terminal binding is unavailable",
            )
        })
}

fn framed_digest(values: &[&str]) -> String {
    let mut digest = Sha256::new();
    for value in values {
        digest.update(value.len().to_le_bytes());
        digest.update(value.as_bytes());
    }
    format!("{:x}", digest.finalize())
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}

fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let Some(parent) = path.parent() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "activity state target has no parent",
        ));
    };
    fs::create_dir_all(parent)?;
    reject_symbolic_link(path)?;
    let mut file = AtomicWriteFile::options().open(path)?;
    file.write_all(bytes)?;
    file.flush()?;
    file.commit()
}

fn reject_symbolic_link(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "activity worker state cannot use a symbolic link",
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(windows)]
fn open_owned_console() -> io::Result<std::fs::File> {
    OpenOptions::new().write(true).open("CONOUT$")
}

#[cfg(not(windows))]
fn open_owned_console() -> io::Result<std::fs::File> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "owned Windows console output is unavailable",
    ))
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf, time::SystemTime};

    use super::{
        ActivityCoordinator, ActivityExecution, ActivityLeaseStore, ActivityRender,
        LeaseTransition, WorkerKey, WorkerPresentation,
    };
    use crate::{
        core::{Attention, Health, Phase},
        presentation::{PresentationPolicy, SemanticPresentationInput},
        settings::{
            ActivityMode, PresentationSettings, PresentationTheme, SpinnerPreset, TabColorMode,
            TitleMode,
        },
    };

    struct TestRoot(PathBuf);

    impl TestRoot {
        fn new(name: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("test clock is after epoch")
                .as_nanos();
            Self(std::env::temp_dir().join(format!(
                "tabbeacon-g11-{name}-{}-{nonce}",
                std::process::id()
            )))
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn digest(character: char) -> String {
        std::iter::repeat_n(character, 64).collect()
    }

    fn key(generation: u64, session: char, terminal: char) -> WorkerKey {
        WorkerKey::new(
            &digest(session),
            Some(&digest('b')),
            generation,
            &digest(terminal),
        )
    }

    fn presentation() -> WorkerPresentation {
        WorkerPresentation::working("OWH", SpinnerPreset::Braille)
    }

    #[test]
    fn lease_persists_only_hashed_identity_and_safe_presentation() {
        let root = TestRoot::new("content-minimal");
        let store = ActivityLeaseStore::new(&root.0);
        let key = key(7, 'a', 'c');
        let transition = store
            .publish_active(&key, 11, &digest('d'), &presentation(), 1_000)
            .expect("active lease publishes");
        assert!(matches!(transition, LeaseTransition::Published { .. }));
        let text = fs::read_to_string(store.lease_path(key.digest()))
            .expect("published lease is readable");
        for forbidden in [
            "prompt",
            "assistant",
            "tool_input",
            "tool_output",
            "credential",
            "C:\\\\Users",
            "raw-session",
            "raw-turn",
        ] {
            assert!(!text.contains(forbidden), "lease leaked {forbidden}");
        }
        assert!(text.contains("\"semantic_state\": \"working\""));
        assert!(text.contains("\"spinner_preset\": \"braille\""));
        assert!(text.contains("\"workspace_alias\": \"OWH\""));
    }

    #[test]
    fn same_generation_updates_in_place_and_delayed_events_cannot_revive() {
        let root = TestRoot::new("event-order");
        let store = ActivityLeaseStore::new(&root.0);
        let key = key(2, 'a', 'c');
        let owner = digest('d');
        assert!(matches!(
            store.publish_active(&key, 10, &owner, &presentation(), 1_000),
            Ok(LeaseTransition::Published { .. })
        ));
        assert_eq!(
            store
                .publish_active(&key, 11, &owner, &presentation(), 1_100)
                .expect("same worker refreshes"),
            LeaseTransition::AlreadyActive
        );
        assert!(matches!(
            store.publish_stopped(&key, 12, &owner, 1_200),
            Ok(LeaseTransition::Stopped {
                predecessor: Some(_)
            })
        ));
        assert_eq!(
            store
                .publish_active(&key, 11, &owner, &presentation(), 1_300)
                .expect("delayed activity is classified"),
            LeaseTransition::Stale
        );
        let stopped = store
            .load(key.digest())
            .expect("lease reads")
            .expect("lease exists");
        assert!(!stopped.active);
        assert_eq!(stopped.event_sequence, 12);
    }

    #[test]
    fn lease_lock_orders_one_shot_writes_after_competing_hook_transitions() {
        let root = TestRoot::new("write-order");
        let store = ActivityLeaseStore::new(&root.0);
        let coordinator = ActivityCoordinator {
            store: store.clone(),
            execution: ActivityExecution::System {
                executable: root.0.join("unused.exe"),
                owner_sha256: digest('d'),
                terminal_binding_sha256: digest('c'),
            },
        };
        let key = key(3, 'a', 'c');
        store
            .publish_active(&key, 22, &digest('d'), &presentation(), 1_000)
            .expect("newer transition publishes");
        let mut stale_output = Vec::new();
        coordinator
            .write_rendered(
                &digest('a'),
                Some(&digest('b')),
                3,
                21,
                ActivityRender::WithoutTitle,
                b"stale",
                &mut stale_output,
            )
            .expect("stale write is suppressed without error");
        assert!(stale_output.is_empty());

        let mut current_output = Vec::new();
        coordinator
            .write_rendered(
                &digest('a'),
                Some(&digest('b')),
                3,
                22,
                ActivityRender::WithoutTitle,
                b"current",
                &mut current_output,
            )
            .expect("current write succeeds");
        assert_eq!(current_output, b"current");
    }

    #[test]
    fn newer_turn_waits_for_the_exact_predecessor_exit_receipt() {
        let root = TestRoot::new("supersession");
        let store = ActivityLeaseStore::new(&root.0);
        let first = key(1, 'a', 'c');
        let second = key(2, 'a', 'c');
        let owner = digest('d');
        store
            .publish_active(&first, 1, &owner, &presentation(), 1_000)
            .expect("first generation publishes");
        let transition = store
            .publish_active(&second, 2, &owner, &presentation(), 1_100)
            .expect("successor publishes");
        let LeaseTransition::Published {
            lease,
            predecessor: Some(predecessor),
        } = transition
        else {
            panic!("new generation must identify its predecessor");
        };
        assert_eq!(predecessor.generation, 1);
        assert_eq!(lease.generation, 2);
        store
            .write_exit(&predecessor, "superseded")
            .expect("owned exit receipt publishes");
        assert!(store.wait_for_exit(&predecessor, 50));
        assert!(!store.exit_path(&predecessor).exists());
    }

    #[test]
    fn session_and_terminal_bindings_isolate_parallel_workers() {
        let root = TestRoot::new("isolation");
        let store = ActivityLeaseStore::new(&root.0);
        let owner = digest('d');
        let first = key(1, 'a', 'c');
        let other_session = key(1, 'e', 'c');
        let other_terminal = key(1, 'a', 'f');
        assert_ne!(first.digest(), other_session.digest());
        assert_ne!(first.digest(), other_terminal.digest());
        for worker in [&first, &other_session, &other_terminal] {
            assert!(matches!(
                store.publish_active(worker, 1, &owner, &presentation(), 1_000),
                Ok(LeaseTransition::Published {
                    predecessor: None,
                    ..
                })
            ));
        }
        let lease_count = fs::read_dir(&store.directory)
            .expect("worker directory reads")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("lease-"))
            .count();
        assert_eq!(lease_count, 3);
    }

    #[test]
    fn changed_executable_owner_supersedes_an_obsolete_worker() {
        let root = TestRoot::new("upgrade-owner");
        let store = ActivityLeaseStore::new(&root.0);
        let key = key(4, 'a', 'c');
        let old_owner = digest('d');
        let new_owner = digest('e');
        store
            .publish_active(&key, 20, &old_owner, &presentation(), 1_000)
            .expect("old owner publishes");
        let transition = store
            .publish_active(&key, 21, &new_owner, &presentation(), 1_100)
            .expect("new owner publishes");
        let LeaseTransition::Published {
            lease,
            predecessor: Some(predecessor),
        } = transition
        else {
            panic!("new executable owner must supersede the old worker");
        };
        assert_eq!(predecessor.owner_sha256, old_owner);
        assert_eq!(lease.owner_sha256, new_owner);
        assert_eq!(lease.revision, 2);
    }

    #[test]
    fn expired_and_stopped_leases_cannot_authorize_worker_frames() {
        let root = TestRoot::new("expiry");
        let store = ActivityLeaseStore::new(&root.0);
        let key = key(1, 'a', 'c');
        let owner = digest('d');
        let transition = store
            .publish_active(&key, 1, &owner, &presentation(), 1_000)
            .expect("active lease publishes");
        let LeaseTransition::Published { lease, .. } = transition else {
            panic!("expected new lease");
        };
        let ownership = lease.ownership();
        assert!(
            store
                .load_worker_lease(&ownership, &digest('c'), 1_001)
                .is_some()
        );
        assert!(
            store
                .load_worker_lease(&ownership, &digest('c'), lease.expires_unix_ms + 1)
                .is_none()
        );
        store
            .publish_stopped(&key, 2, &owner, 1_200)
            .expect("stop publishes");
        assert!(
            store
                .load_worker_lease(&ownership, &digest('c'), 1_201)
                .is_none()
        );
    }

    #[test]
    fn disabled_coordinator_preserves_static_fail_open_rendering() {
        let root = TestRoot::new("disabled");
        let coordinator = ActivityCoordinator::disabled(&root.0);
        let action = PresentationPolicy::resolve(SemanticPresentationInput::new(
            Phase::Working,
            Attention::None,
            Health::Normal,
            "OWH",
        ));
        let settings = PresentationSettings::new(
            TitleMode::TabBeacon,
            TabColorMode::TabBeacon,
            ActivityMode::TitleSpinner,
            SpinnerPreset::Braille,
            PresentationTheme::MutedDark,
        );
        assert_eq!(
            coordinator.reconcile(
                &digest('a'),
                Some(&digest('b')),
                1,
                1,
                "OWH",
                &action,
                settings,
            ),
            ActivityRender::UncoordinatedFull
        );
        assert!(!root.0.join("activity-worker-v1").exists());
    }

    #[test]
    fn missing_worker_executable_falls_back_and_deactivates_its_lease() {
        let root = TestRoot::new("missing-worker");
        let store = ActivityLeaseStore::new(&root.0);
        let coordinator = ActivityCoordinator {
            store: store.clone(),
            execution: ActivityExecution::System {
                executable: root.0.join("missing-tabbeacon.exe"),
                owner_sha256: digest('d'),
                terminal_binding_sha256: digest('c'),
            },
        };
        let action = PresentationPolicy::resolve(SemanticPresentationInput::new(
            Phase::Working,
            Attention::None,
            Health::Normal,
            "OWH",
        ));
        let settings = PresentationSettings::default()
            .with_activity(ActivityMode::TitleSpinner)
            .with_spinner(SpinnerPreset::Braille);
        assert_eq!(
            coordinator.reconcile(
                &digest('a'),
                Some(&digest('b')),
                1,
                1,
                "OWH",
                &action,
                settings,
            ),
            ActivityRender::Full
        );
        let lease_key = key(1, 'a', 'c');
        let lease = store
            .load(lease_key.digest())
            .expect("fallback lease reads")
            .expect("fallback lease exists");
        assert!(!lease.active);
        assert!(lease.presentation.is_none());
    }

    #[test]
    fn unwritable_worker_state_is_decoration_only() {
        let root = TestRoot::new("state-failure");
        fs::create_dir_all(&root.0).expect("test root creates");
        let blocked_state_root = root.0.join("not-a-directory");
        fs::write(&blocked_state_root, b"owned test blocker").expect("blocker file writes");
        let coordinator = ActivityCoordinator {
            store: ActivityLeaseStore::new(&blocked_state_root),
            execution: ActivityExecution::System {
                executable: root.0.join("unused.exe"),
                owner_sha256: digest('d'),
                terminal_binding_sha256: digest('c'),
            },
        };
        let action = PresentationPolicy::resolve(SemanticPresentationInput::new(
            Phase::Working,
            Attention::None,
            Health::Normal,
            "OWH",
        ));
        assert_eq!(
            coordinator.reconcile(
                &digest('a'),
                Some(&digest('b')),
                1,
                1,
                "OWH",
                &action,
                PresentationSettings::default().with_activity(ActivityMode::TitleSpinner),
            ),
            ActivityRender::UncoordinatedFull
        );
    }
}
