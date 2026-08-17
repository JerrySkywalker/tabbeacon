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
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

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
/// Normative v0.3 activity-frame target. The worker uses monotonic deadlines
/// and drops overdue frames rather than accumulating catch-up work.
pub const TARGET_FRAME_INTERVAL_MS: u64 = 100;
const FRAME_INTERVAL: Duration = Duration::from_millis(TARGET_FRAME_INTERVAL_MS);
const PREDECESSOR_WAIT_MS: u64 = 750;
const PREDECESSOR_POLL_MS: u64 = 25;
const CLEANUP_OBSERVER_POLL_MS: u64 = 1_000;
const CLEANUP_OBSERVER_QUERY_TIMEOUT_MS: u64 = 5_000;
const CLEANUP_OBSERVER_UNKNOWN_MAX_MS: u64 = 30_000;
const CLEANUP_OBSERVER_REAP_TIMEOUT_MS: u64 = 1_000;
const MAX_DIAGNOSTIC_LEASE_FILES: usize = 512;
const MAX_DIAGNOSTIC_LEASE_BYTES: u64 = 128 * 1_024;

/// Safe health classification for a read-only activity-lease inspection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityLeaseHealth {
    /// The lease directory was read safely and contains no stale or invalid lease.
    Healthy,
    /// At least one stale, invalid, or uninspectable lease was observed.
    Warning,
    /// The lease directory could not be inspected safely.
    Unavailable,
}

impl ActivityLeaseHealth {
    /// Stable machine-oriented spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Healthy => "healthy",
            Self::Warning => "warning",
            Self::Unavailable => "unavailable",
        }
    }
}

/// Content-minimal aggregate of the ephemeral worker lease directory.
///
/// A non-expired lease proves only that a worker was recently authorized. It
/// does not prove that an operating-system process is still alive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActivityLeaseDiagnostics {
    health: ActivityLeaseHealth,
    active_leases: usize,
    stale_leases: usize,
    invalid_leases: usize,
}

impl ActivityLeaseDiagnostics {
    /// Overall read-only inspection health.
    #[must_use]
    pub const fn health(self) -> ActivityLeaseHealth {
        self.health
    }

    /// Count of valid, non-expired active leases.
    #[must_use]
    pub const fn active_leases(self) -> usize {
        self.active_leases
    }

    /// Count of active leases that are past their expiry timestamp.
    #[must_use]
    pub const fn stale_leases(self) -> usize {
        self.stale_leases
    }

    /// Count of malformed, unsafe, or bounded-out lease entries.
    #[must_use]
    pub const fn invalid_leases(self) -> usize {
        self.invalid_leases
    }

    const fn healthy() -> Self {
        Self {
            health: ActivityLeaseHealth::Healthy,
            active_leases: 0,
            stale_leases: 0,
            invalid_leases: 0,
        }
    }

    const fn unavailable() -> Self {
        Self {
            health: ActivityLeaseHealth::Unavailable,
            active_leases: 0,
            stale_leases: 0,
            invalid_leases: 0,
        }
    }
}

/// Inspects the current user's activity leases without creating state or locks.
#[must_use]
pub fn inspect_system_activity_leases() -> ActivityLeaseDiagnostics {
    let Ok(state_root) = crate::repo::StableAliasRegistry::default_state_root() else {
        return ActivityLeaseDiagnostics::unavailable();
    };
    inspect_activity_leases_read_only(&state_root, unix_ms())
}

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
                    if let Ok(worker_pid) = spawn_worker(executable, &lease) {
                        if spawn_cleanup_observer(executable, &lease, worker_pid).is_ok() {
                            ActivityRender::WithoutTitle
                        } else {
                            // A console-attached worker can be terminated with its
                            // terminal. Without the detached observer, leaving the
                            // lease active would suppress future presentation, so
                            // fail open to the static one-shot rendering instead.
                            let _ = self.store.deactivate_if_owned(&lease, unix_ms());
                            ActivityRender::Full
                        }
                    } else {
                        let _ = self.store.deactivate_if_owned(&lease, unix_ms());
                        ActivityRender::Full
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

/// Clears only a terminal-ended worker's exact lease from a detached,
/// session-scoped observer.
pub fn run_activity_cleanup_observer_system(
    worker_pid: u32,
    key_digest: &str,
    generation: u64,
    revision: u64,
    owner_sha256: &str,
    expected_executable: &str,
) {
    if worker_pid == 0
        || !is_sha256(key_digest)
        || !is_sha256(owner_sha256)
        || !is_safe_normalized_path(expected_executable)
    {
        return;
    }
    let Ok(state_root) = crate::repo::StableAliasRegistry::default_state_root() else {
        return;
    };
    let store = ActivityLeaseStore::new(state_root);
    let ownership = WorkerOwnership {
        key_sha256: key_digest.to_owned(),
        generation,
        revision,
        owner_sha256: owner_sha256.to_owned(),
    };
    let mut unknown_observation: Option<(WorkerLease, u64)> = None;
    loop {
        let now = unix_ms();
        // A read failure is not proof that the worker exited. Preserve the
        // exact lease and let its bounded TTL protect the next transition.
        let Ok(lease) = store.with_lock(|| store.load(&ownership.key_sha256)) else {
            return;
        };
        let Some(observed) = lease else {
            return;
        };
        let liveness = worker_process_liveness(worker_pid, &ownership, expected_executable);
        let unknown_timeout_elapsed = matches!(liveness, WorkerProcessLiveness::Unknown)
            && unknown_observation
                .as_ref()
                .is_some_and(|(snapshot, started)| {
                    snapshot == &observed
                        && now.saturating_sub(*started) >= CLEANUP_OBSERVER_UNKNOWN_MAX_MS
                });
        match cleanup_observer_action(
            Some(&observed),
            &ownership,
            now,
            liveness,
            unknown_timeout_elapsed,
        ) {
            CleanupObserverAction::Deactivate(reason) => {
                if store
                    .deactivate_observed_worker(&observed, unix_ms())
                    .unwrap_or(false)
                {
                    let _ = store.write_exit(&ownership, reason);
                    return;
                }
                // A same-owner refresh may have won while the liveness query
                // was in flight. That newer snapshot was deliberately
                // preserved; continue observing it rather than leaving an
                // active lease without a cleanup observer.
                unknown_observation = None;
                thread::sleep(Duration::from_millis(CLEANUP_OBSERVER_POLL_MS));
            }
            CleanupObserverAction::Stop => return,
            CleanupObserverAction::Wait => {
                unknown_observation = if matches!(liveness, WorkerProcessLiveness::Unknown) {
                    match unknown_observation {
                        Some((snapshot, started)) if snapshot == observed => {
                            Some((snapshot, started))
                        }
                        _ => Some((observed, now)),
                    }
                } else {
                    None
                };
                thread::sleep(Duration::from_millis(CLEANUP_OBSERVER_POLL_MS));
            }
        }
    }
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorkerOwnership {
    key_sha256: String,
    generation: u64,
    revision: u64,
    owner_sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkerProcessLiveness {
    Alive,
    Exited,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CleanupObserverAction {
    Deactivate(&'static str),
    Stop,
    Wait,
}

fn cleanup_observer_action(
    lease: Option<&WorkerLease>,
    ownership: &WorkerOwnership,
    now: u64,
    liveness: WorkerProcessLiveness,
    unknown_timeout_elapsed: bool,
) -> CleanupObserverAction {
    let Some(lease) = lease else {
        return CleanupObserverAction::Stop;
    };
    if lease.ownership() != *ownership || !lease.active {
        return CleanupObserverAction::Stop;
    }
    if now > lease.expires_unix_ms {
        return CleanupObserverAction::Deactivate("lease_expired");
    }
    match liveness {
        WorkerProcessLiveness::Exited => CleanupObserverAction::Deactivate("worker_process_ended"),
        WorkerProcessLiveness::Unknown if unknown_timeout_elapsed => {
            // The exact observed lease is deliberately settled after the
            // bounded dual-probe outage. This is a decoration-only fail-open
            // transition; it never targets a process and it cannot clear a
            // lease refreshed after this observer's snapshot.
            CleanupObserverAction::Deactivate("liveness_unavailable")
        }
        WorkerProcessLiveness::Alive | WorkerProcessLiveness::Unknown => {
            CleanupObserverAction::Wait
        }
    }
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
        self.deactivate_owned_worker(&expected.ownership(), now)
            .map(|_| ())
    }

    fn deactivate_owned_worker(&self, expected: &WorkerOwnership, now: u64) -> io::Result<bool> {
        self.with_lock(|| {
            let Some(mut current) = self.load(&expected.key_sha256)? else {
                return Ok(false);
            };
            if current.ownership() == *expected && current.active {
                current.active = false;
                current.presentation = None;
                current.updated_unix_ms = now;
                current.expires_unix_ms = now;
                self.write(&current)?;
                return Ok(true);
            }
            Ok(false)
        })
    }

    /// Deactivates only if the active lease is exactly the snapshot observed
    /// before an external liveness query. A same-owner refresh is a newer
    /// state, and must never be cleared by an observer that saw the older one.
    fn deactivate_observed_worker(&self, observed: &WorkerLease, now: u64) -> io::Result<bool> {
        self.with_lock(|| {
            let Some(mut current) = self.load(&observed.key_sha256)? else {
                return Ok(false);
            };
            if current == *observed && current.active {
                current.active = false;
                current.presentation = None;
                current.updated_unix_ms = now;
                current.expires_unix_ms = now;
                self.write(&current)?;
                return Ok(true);
            }
            Ok(false)
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
            let _ = self.deactivate_if_owned(&initial, unix_ms());
            let _ = self.write_exit(&ownership, "inactive");
            return;
        };
        let Some(spinner) = presentation.spinner() else {
            let _ = self.deactivate_if_owned(&initial, unix_ms());
            let _ = self.write_exit(&ownership, "invalid_presentation");
            return;
        };
        let Ok(mut console) = open_owned_console() else {
            let _ = self.deactivate_if_owned(&initial, unix_ms());
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
        let mut next_frame_deadline = Instant::now();
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
            next_frame_deadline =
                next_animation_frame_deadline(next_frame_deadline, Instant::now());
            let remaining = next_frame_deadline.saturating_duration_since(Instant::now());
            if !remaining.is_zero() {
                thread::sleep(remaining);
            }
        }
        let _ = self.deactivate_if_owned(&initial, unix_ms());
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

fn inspect_activity_leases_read_only(state_root: &Path, now: u64) -> ActivityLeaseDiagnostics {
    let directory = state_root.join(STATE_DIRECTORY);
    let metadata = match fs::symlink_metadata(&directory) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return ActivityLeaseDiagnostics::unavailable();
        }
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return ActivityLeaseDiagnostics::healthy();
        }
        Err(_) => return ActivityLeaseDiagnostics::unavailable(),
    };
    if !metadata.is_dir() {
        return ActivityLeaseDiagnostics::unavailable();
    }
    let Ok(entries) = fs::read_dir(&directory) else {
        return ActivityLeaseDiagnostics::unavailable();
    };
    let mut diagnostics = ActivityLeaseDiagnostics::healthy();
    for (index, entry) in entries.enumerate() {
        if index >= MAX_DIAGNOSTIC_LEASE_FILES {
            diagnostics.invalid_leases = diagnostics.invalid_leases.saturating_add(1);
            break;
        }
        let Ok(entry) = entry else {
            diagnostics.invalid_leases = diagnostics.invalid_leases.saturating_add(1);
            continue;
        };
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(key_digest) = name
            .strip_prefix("lease-")
            .and_then(|value| value.strip_suffix(".json"))
        else {
            continue;
        };
        let path = entry.path();
        let valid_entry = fs::symlink_metadata(&path)
            .ok()
            .is_some_and(|metadata| metadata.is_file() && !metadata.file_type().is_symlink());
        if !is_sha256(key_digest)
            || !valid_entry
            || fs::metadata(&path)
                .map_or(true, |metadata| metadata.len() > MAX_DIAGNOSTIC_LEASE_BYTES)
        {
            diagnostics.invalid_leases = diagnostics.invalid_leases.saturating_add(1);
            continue;
        }
        let lease = fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<WorkerLease>(&bytes).ok());
        let Some(lease) =
            lease.filter(|lease| lease.key_sha256 == key_digest && validate_lease(lease).is_ok())
        else {
            diagnostics.invalid_leases = diagnostics.invalid_leases.saturating_add(1);
            continue;
        };
        if lease.active {
            if now > lease.expires_unix_ms {
                diagnostics.stale_leases = diagnostics.stale_leases.saturating_add(1);
            } else {
                diagnostics.active_leases = diagnostics.active_leases.saturating_add(1);
            }
        }
    }
    if diagnostics.stale_leases > 0 || diagnostics.invalid_leases > 0 {
        diagnostics.health = ActivityLeaseHealth::Warning;
    }
    diagnostics
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

fn spawn_worker(executable: &Path, lease: &WorkerLease) -> io::Result<u32> {
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
        .map(|child| child.id())
}

fn spawn_cleanup_observer(
    executable: &Path,
    lease: &WorkerLease,
    worker_pid: u32,
) -> io::Result<()> {
    let expected_executable = normalized_executable_path(executable)?;
    let mut command = Command::new(executable);
    command
        .args([
            "__activity-cleanup-observer-v1",
            &worker_pid.to_string(),
            &lease.key_sha256,
            &lease.generation.to_string(),
            &lease.revision.to_string(),
            &lease.owner_sha256,
            &expected_executable,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    command.creation_flags(0x0000_0008 | 0x0000_0200);
    command.spawn().map(|_| ())
}

fn worker_process_liveness(
    worker_pid: u32,
    ownership: &WorkerOwnership,
    expected_executable: &str,
) -> WorkerProcessLiveness {
    let Some(powershell) = system_powershell_path() else {
        return WorkerProcessLiveness::Unknown;
    };
    let expected_pattern = format!(
        "(^|\\s)__activity-worker-v1 {} {} {}(\\s|$)",
        ownership.key_sha256, ownership.generation, ownership.revision
    );
    // The observer accepts `Exited` only when the system-owned PowerShell
    // command completed and returned its narrow sentinel. Querying command-line
    // identity prevents PID reuse or another TabBeacon process from clearing a
    // live lease. Every interpolated field is a validated digest or integer;
    // the executable path is provided through a process-local environment
    // variable rather than interpolated into PowerShell source.
    let command = format!(
        "$process = Get-CimInstance -ClassName Win32_Process -Filter 'ProcessId = {worker_pid}' -ErrorAction Stop; \
         if ($null -eq $process) {{ 'EXITED' }} \
         elseif ([string]::IsNullOrWhiteSpace($process.ExecutablePath)) {{ 'UNKNOWN' }} \
         else {{ \
           $actual = [IO.Path]::GetFullPath($process.ExecutablePath).Replace('\\','/').ToLowerInvariant(); \
           if (($actual -eq $env:TABBEACON_EXPECTED_WORKER_PATH) -and ($process.CommandLine -match '{expected_pattern}')) {{ 'ALIVE' }} else {{ 'EXITED' }} \
         }}"
    );
    let mut query = Command::new(powershell);
    query
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &command,
        ])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .env("TABBEACON_EXPECTED_WORKER_PATH", expected_executable);
    #[cfg(windows)]
    query.creation_flags(0x0800_0000);
    let output = command_output_with_timeout(
        &mut query,
        Duration::from_millis(CLEANUP_OBSERVER_QUERY_TIMEOUT_MS),
    );
    let Ok(output) = output else {
        return tasklist_liveness_fallback(worker_pid);
    };
    if !output.status.success() {
        return tasklist_liveness_fallback(worker_pid);
    }
    match String::from_utf8_lossy(&output.stdout).trim() {
        "ALIVE" => WorkerProcessLiveness::Alive,
        "EXITED" => WorkerProcessLiveness::Exited,
        _ => WorkerProcessLiveness::Unknown,
    }
}

fn tasklist_liveness_fallback(worker_pid: u32) -> WorkerProcessLiveness {
    tasklist_process_absence(worker_pid).map_or(WorkerProcessLiveness::Unknown, |absent| {
        if absent {
            WorkerProcessLiveness::Exited
        } else {
            WorkerProcessLiveness::Unknown
        }
    })
}

fn system_powershell_path() -> Option<PathBuf> {
    system_directory_path("WindowsPowerShell\\v1.0\\powershell.exe")
}

fn normalized_executable_path(path: &Path) -> io::Result<String> {
    let canonical = fs::canonicalize(path)?;
    let normalized = canonical
        .to_string_lossy()
        .replace('\\', "/")
        .to_lowercase();
    if is_safe_normalized_path(&normalized) {
        Ok(normalized)
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "activity worker executable path is unsafe",
        ))
    }
}

fn is_safe_normalized_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 32_768
        && !path.chars().any(char::is_control)
        && path.contains(':')
        && !path.contains('\\')
}

fn system_tasklist_path() -> Option<PathBuf> {
    system_directory_path("tasklist.exe")
}

fn system_directory_path(suffix: &str) -> Option<PathBuf> {
    let path = PathBuf::from(env::var_os("SystemRoot")?)
        .join("System32")
        .join(suffix);
    path.is_file().then_some(path)
}

fn tasklist_process_absence(worker_pid: u32) -> Option<bool> {
    let tasklist = system_tasklist_path()?;
    let mut command = Command::new(tasklist);
    command
        .args(["/FI", &format!("PID eq {worker_pid}"), "/FO", "CSV", "/NH"])
        .stdin(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    command.creation_flags(0x0800_0000);
    let output = command_output_with_timeout(
        &mut command,
        Duration::from_millis(CLEANUP_OBSERVER_QUERY_TIMEOUT_MS),
    )
    .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let process_rows = stdout
        .lines()
        .filter(|line| line.trim_start().starts_with('"'))
        .count();
    (process_rows == 0).then_some(true)
}

fn command_output_with_timeout(
    command: &mut Command,
    timeout: Duration,
) -> io::Result<std::process::Output> {
    command.stdout(Stdio::piped());
    let mut child = command.spawn()?;
    let deadline = deadline_after(timeout);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return child.wait_with_output(),
            Ok(None) => {}
            Err(error) => {
                let _ = child.kill();
                let _ = reap_owned_child_until(
                    &mut child,
                    deadline_after(Duration::from_millis(CLEANUP_OBSERVER_REAP_TIMEOUT_MS)),
                );
                return Err(error);
            }
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = reap_owned_child_until(
                &mut child,
                deadline_after(Duration::from_millis(CLEANUP_OBSERVER_REAP_TIMEOUT_MS)),
            );
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "bounded activity liveness command exceeded its deadline",
            ));
        }
        thread::sleep(Duration::from_millis(25));
    }
}

/// Schedules one v0.3 animation deadline from the prior deadline. A render
/// overrun drops missed frames and schedules one future frame, preventing a
/// busy loop or unbounded catch-up.
#[must_use]
pub fn next_animation_frame_deadline(previous_deadline: Instant, now: Instant) -> Instant {
    let scheduled = previous_deadline.checked_add(FRAME_INTERVAL).unwrap_or(now);
    if scheduled > now {
        scheduled
    } else {
        now.checked_add(FRAME_INTERVAL).unwrap_or(now)
    }
}

fn deadline_after(timeout: Duration) -> Instant {
    Instant::now()
        .checked_add(timeout)
        .unwrap_or_else(Instant::now)
}

fn reap_owned_child_until(child: &mut std::process::Child, deadline: Instant) -> io::Result<bool> {
    loop {
        match child.try_wait()? {
            Some(_) => return Ok(true),
            None if Instant::now() >= deadline => return Ok(false),
            None => thread::sleep(Duration::from_millis(25)),
        }
    }
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
    use std::{
        fs,
        path::PathBuf,
        process::{Command, Stdio},
        time::{Duration, Instant, SystemTime},
    };

    use super::{
        ActivityCoordinator, ActivityExecution, ActivityLeaseHealth, ActivityLeaseStore,
        ActivityRender, CleanupObserverAction, LeaseTransition, TARGET_FRAME_INTERVAL_MS,
        WorkerKey, WorkerPresentation, WorkerProcessLiveness, cleanup_observer_action,
        command_output_with_timeout, inspect_activity_leases_read_only,
        next_animation_frame_deadline, system_powershell_path,
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
    fn v03_worker_uses_the_normative_hundred_millisecond_interval() {
        assert_eq!(TARGET_FRAME_INTERVAL_MS, 100);
    }

    #[test]
    fn animation_deadlines_remain_anchored_when_render_is_faster_than_the_interval() {
        let start = Instant::now();
        let after_render = start + Duration::from_millis(20);
        let next = next_animation_frame_deadline(start, after_render);

        assert_eq!(next.duration_since(start), Duration::from_millis(100));
    }

    #[test]
    fn animation_deadline_drops_overrun_frames_without_busy_looping() {
        let start = Instant::now();
        let after_overrun = start + Duration::from_millis(250);
        let next = next_animation_frame_deadline(start, after_overrun);

        assert_eq!(
            next.duration_since(after_overrun),
            Duration::from_millis(100)
        );
    }

    #[test]
    fn read_only_activity_inspection_preserves_an_absent_state_root() {
        let root = TestRoot::new("diagnostic-absent");

        let diagnostics = inspect_activity_leases_read_only(&root.0, 1_000);

        assert_eq!(diagnostics.health(), ActivityLeaseHealth::Healthy);
        assert_eq!(diagnostics.active_leases(), 0);
        assert_eq!(diagnostics.stale_leases(), 0);
        assert_eq!(diagnostics.invalid_leases(), 0);
        assert!(
            !root.0.exists(),
            "read-only diagnostics must not create a state root or lock"
        );
    }

    #[test]
    fn read_only_activity_inspection_counts_active_stale_and_invalid_leases() {
        let root = TestRoot::new("diagnostic-counts");
        let store = ActivityLeaseStore::new(&root.0);
        let active_key = key(1, 'a', 'c');
        let stale_key = key(1, 'e', 'c');
        let owner = digest('d');
        store
            .publish_active(&active_key, 1, &owner, &presentation(), 1_000)
            .expect("active fixture publishes");
        store
            .publish_active(&stale_key, 1, &owner, &presentation(), 1_000)
            .expect("stale fixture publishes");

        let mut active = store
            .load(active_key.digest())
            .expect("active fixture reads")
            .expect("active fixture exists");
        active.expires_unix_ms = 2_000;
        store.write(&active).expect("active fixture updates");
        let mut stale = store
            .load(stale_key.digest())
            .expect("stale fixture reads")
            .expect("stale fixture exists");
        stale.expires_unix_ms = 999;
        store.write(&stale).expect("stale fixture updates");
        fs::write(store.lease_path(&digest('f')), b"not a lease").expect("invalid fixture writes");

        let diagnostics = inspect_activity_leases_read_only(&root.0, 1_000);

        assert_eq!(diagnostics.health(), ActivityLeaseHealth::Warning);
        assert_eq!(diagnostics.active_leases(), 1);
        assert_eq!(diagnostics.stale_leases(), 1);
        assert_eq!(diagnostics.invalid_leases(), 1);
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
    fn performance_matrix_keeps_one_owned_worker_per_active_session_and_cleans_up() {
        // This is the bounded ownership half of the v0.3 1/4/8-tab
        // performance matrix. Timing is covered separately by the monotonic
        // deadline tests above; this proves that increasing active tabs never
        // multiplies workers for any one session/terminal binding.
        for active_tabs in [1_usize, 4, 8] {
            let root = TestRoot::new(&format!("performance-{active_tabs}"));
            let store = ActivityLeaseStore::new(&root.0);
            let owner = digest('d');
            let mut leases = Vec::with_capacity(active_tabs);
            let sessions = ['1', '2', '3', '4', 'a', 'b', 'e', 'f'];

            for (index, session) in sessions.into_iter().take(active_tabs).enumerate() {
                let worker = key(1, session, 'c');
                let LeaseTransition::Published { lease, .. } = store
                    .publish_active(&worker, 1, &owner, &presentation(), 1_000)
                    .expect("performance fixture publishes one worker")
                else {
                    panic!("new session/terminal binding must publish one worker");
                };
                assert_eq!(
                    store
                        .publish_active(&worker, 1, &owner, &presentation(), 1_001)
                        .expect("same worker refreshes"),
                    LeaseTransition::AlreadyActive,
                    "tab {index} must not create a duplicate worker"
                );
                leases.push(lease);
            }

            assert_eq!(
                inspect_activity_leases_read_only(&root.0, 1_001).active_leases(),
                active_tabs,
                "{active_tabs} active tabs must have exactly {active_tabs} workers"
            );

            for lease in &leases {
                store
                    .deactivate_if_owned(lease, 1_002)
                    .expect("owned worker cleanup succeeds");
            }
            assert_eq!(
                inspect_activity_leases_read_only(&root.0, 1_002).active_leases(),
                0,
                "owned cleanup must leave no active workers"
            );
        }
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
    fn worker_exit_deactivates_only_its_owned_active_lease() {
        let root = TestRoot::new("worker-exit-deactivation");
        let store = ActivityLeaseStore::new(&root.0);
        let worker = key(1, 'a', 'c');
        let owner = digest('d');
        let LeaseTransition::Published { lease, .. } = store
            .publish_active(&worker, 1, &owner, &presentation(), 1_000)
            .expect("active lease publishes")
        else {
            panic!("expected active lease publication");
        };

        store
            .deactivate_if_owned(&lease, 1_001)
            .expect("owned worker exit deactivates lease");

        let current = store
            .load(worker.digest())
            .expect("deactivated lease reads")
            .expect("deactivated lease exists");
        assert!(!current.active);
        assert!(current.presentation.is_none());
        assert_eq!(current.expires_unix_ms, 1_001);
    }

    #[test]
    fn cleanup_observer_liveness_error_is_bounded_then_fails_open() {
        let root = TestRoot::new("cleanup-liveness-error");
        let store = ActivityLeaseStore::new(&root.0);
        let worker = key(1, 'a', 'c');
        let owner = digest('d');
        let LeaseTransition::Published { lease, .. } = store
            .publish_active(&worker, 1, &owner, &presentation(), 1_000)
            .expect("active lease publishes")
        else {
            panic!("expected active lease publication");
        };
        let ownership = lease.ownership();

        assert_eq!(
            cleanup_observer_action(
                Some(&lease),
                &ownership,
                1_001,
                WorkerProcessLiveness::Unknown,
                false,
            ),
            CleanupObserverAction::Wait
        );
        assert!(
            store
                .load(worker.digest())
                .expect("lease reads")
                .is_some_and(|current| current.active),
            "a liveness-query error must not clear an active lease"
        );
        assert_eq!(
            cleanup_observer_action(
                Some(&lease),
                &ownership,
                31_001,
                WorkerProcessLiveness::Unknown,
                true,
            ),
            CleanupObserverAction::Deactivate("liveness_unavailable")
        );
        assert!(
            store
                .deactivate_observed_worker(&lease, 31_001)
                .expect("bounded observer settles its exact snapshot"),
            "the bounded fail-open path clears only its owned snapshot"
        );
        assert!(
            store
                .load(worker.digest())
                .expect("settled lease reads")
                .is_some_and(|current| !current.active),
            "bounded liveness loss cannot leave an active lease indefinitely"
        );
    }

    #[test]
    fn cleanup_observer_deactivates_only_for_exit_or_expiry() {
        let root = TestRoot::new("cleanup-action");
        let store = ActivityLeaseStore::new(&root.0);
        let worker = key(1, 'a', 'c');
        let owner = digest('d');
        let LeaseTransition::Published { lease, .. } = store
            .publish_active(&worker, 1, &owner, &presentation(), 1_000)
            .expect("active lease publishes")
        else {
            panic!("expected active lease publication");
        };
        let ownership = lease.ownership();

        assert_eq!(
            cleanup_observer_action(
                Some(&lease),
                &ownership,
                1_001,
                WorkerProcessLiveness::Exited,
                false,
            ),
            CleanupObserverAction::Deactivate("worker_process_ended")
        );
        assert_eq!(
            cleanup_observer_action(
                Some(&lease),
                &ownership,
                lease.expires_unix_ms.saturating_add(1),
                WorkerProcessLiveness::Alive,
                false,
            ),
            CleanupObserverAction::Deactivate("lease_expired")
        );
    }

    #[test]
    fn observer_retries_against_a_same_owner_lease_refreshed_after_its_snapshot() {
        let root = TestRoot::new("cleanup-snapshot");
        let store = ActivityLeaseStore::new(&root.0);
        let worker = key(1, 'a', 'c');
        let owner = digest('d');
        let LeaseTransition::Published {
            lease: observed, ..
        } = store
            .publish_active(&worker, 1, &owner, &presentation(), 1_000)
            .expect("active lease publishes")
        else {
            panic!("expected active lease publication");
        };

        assert_eq!(
            store
                .publish_active(&worker, 2, &owner, &presentation(), 1_001)
                .expect("same owner refreshes"),
            LeaseTransition::AlreadyActive
        );
        assert!(
            !store
                .deactivate_observed_worker(&observed, 1_002)
                .expect("stale observer snapshot is checked"),
            "an observer cannot clear a same-owner lease refreshed during its liveness query"
        );
        let refreshed = store
            .load(worker.digest())
            .expect("refreshed lease reads")
            .expect("refreshed lease exists");
        assert!(
            refreshed.active,
            "the newer active lease remains authoritative"
        );
        assert!(
            store
                .deactivate_observed_worker(&refreshed, 1_003)
                .expect("continued observer checks the refreshed snapshot"),
            "a continued observer settles the newer exact snapshot once liveness is known"
        );
    }

    #[cfg(windows)]
    #[test]
    fn bounded_liveness_command_terminates_its_owned_timeout_child() {
        let powershell = system_powershell_path().expect("Windows PowerShell is available");
        let mut command = Command::new(powershell);
        command
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Start-Sleep -Seconds 2",
            ])
            .stdin(Stdio::null())
            .stderr(Stdio::null());
        let error = command_output_with_timeout(&mut command, Duration::from_millis(25))
            .expect_err("owned liveness child must be bounded");
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
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
