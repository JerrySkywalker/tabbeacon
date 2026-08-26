//! Provider-neutral, session-scoped activity worker ownership.
//!
//! Hook adapters publish only hashed identity, semantic presentation state,
//! and a bounded lease. The worker never receives or persists raw Hook bodies.

use std::{
    collections::BTreeSet,
    env,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[cfg(windows)]
use std::os::windows::{ffi::OsStrExt, process::CommandExt};

use atomic_write_file::AtomicWriteFile;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    core::{Attention, Health, Phase},
    presentation::{
        PresentationAction, PresentationPolicy, SemanticPresentationInput, TitleStatus,
        WindowsTerminalCapabilities, WindowsTerminalRenderer,
    },
    providers::visual_identity::ProviderVisualIdentity,
    repo::RepositoryAlias,
    settings::{
        ActivityMode, PresentationSettings, PresentationTheme, SpinnerPreset, TabColorMode,
        TitleMode,
    },
    worker_runtime::{WorkerRuntimeImage, WorkerRuntimeStore},
};

const LEASE_SCHEMA: &str = "tabbeacon-activity-worker-lease-v1";
const STATE_DIRECTORY: &str = "activity-worker-v1";
const LOCK_FILE: &str = "activity-worker.lock";
const PROVIDER_SESSION_SCHEMA: &str = "tabbeacon-provider-session-observation-v1";
const PROVIDER_SESSION_DIRECTORY: &str = "provider-session-v1";
const PROVIDER_SESSION_LOCK_FILE: &str = "provider-session.lock";
const LEASE_TTL_MS: u64 = 24 * 60 * 60 * 1_000;
/// Normative v0.3 activity-frame target. The worker uses monotonic deadlines
/// and drops overdue frames rather than accumulating catch-up work.
pub const TARGET_FRAME_INTERVAL_MS: u64 = 100;
const FRAME_INTERVAL: Duration = Duration::from_millis(TARGET_FRAME_INTERVAL_MS);
/// Private opt-in receipt used solely by the isolated MCP activity probe.
/// It records timing/count facts after real worker-rendered bytes reach the
/// isolated probe sink, never title text, Hook input, or any other user
/// content.
pub const ACTIVITY_WORKER_PROBE_RECEIPT_ENV: &str = "TABBEACON_ACTIVITY_WORKER_PROBE_RECEIPT";
/// Required basename for the private activity probe receipt.
pub const ACTIVITY_WORKER_PROBE_RECEIPT_FILE: &str = "activity-worker-probe.json";
/// Required basename for the non-content worker-start marker used when a
/// probe must distinguish a spawn failure from a missing rendered frame.
pub const ACTIVITY_WORKER_PROBE_STARTED_FILE: &str = "activity-worker-probe-started.json";
/// Required basename for the non-content process-entry marker used by the
/// isolated MCP activity probe.
pub const ACTIVITY_WORKER_PROBE_PROCESS_FILE: &str = "activity-worker-probe-process.json";
/// Required basename for the content-minimal cleanup-observer process marker.
pub const ACTIVITY_OBSERVER_PROBE_PROCESS_FILE: &str = "activity-observer-probe-process.json";
// The worker renders at 100 ms, but cleanup is a bounded recovery path. A
// five-second native presence poll avoids turning every active Codex tab into
// a recurring PowerShell/CIM process under multi-session load.
const CLEANUP_OBSERVER_POLL_MIN_MS: u64 = 4_000;
const CLEANUP_OBSERVER_POLL_SPREAD_MS: u64 = 4_000;
const CLEANUP_OBSERVER_QUERY_TIMEOUT_MS: u64 = 5_000;
const CLEANUP_OBSERVER_IDENTITY_RECHECK_MS: u64 = 30_000;
const CLEANUP_OBSERVER_UNKNOWN_MAX_MS: u64 = 30_000;
const CLEANUP_OBSERVER_REAP_TIMEOUT_MS: u64 = 1_000;
// Result-ready and approval titles are static. Their worker retains exact
// ownership but must not wake ten times per second like an animated spinner.
const STATIC_ATTENTION_WORKER_POLL_MS: u64 = 5_000;
// A static attention title must outlive the one-shot Hook that set it, but it
// cannot remain authorized indefinitely if that Hook's host disappears.
const STATIC_ATTENTION_LEASE_TTL_MS: u64 = CLEANUP_OBSERVER_UNKNOWN_MAX_MS;
const MAX_DIAGNOSTIC_LEASE_FILES: usize = 512;
const MAX_DIAGNOSTIC_LEASE_BYTES: u64 = 128 * 1_024;

/// One-process capture proving that the spawned worker rendered two distinct
/// title frames. This is deliberately unavailable unless a bounded, isolated
/// probe supplies an exact receipt path below its own `LOCALAPPDATA` root.
struct ActivityWorkerProbeCapture {
    path: PathBuf,
    first_frame_sha256: Option<String>,
    first_frame_at: Option<Instant>,
    published: bool,
}

impl ActivityWorkerProbeCapture {
    fn from_environment() -> Option<Self> {
        let path = PathBuf::from(env::var_os(ACTIVITY_WORKER_PROBE_RECEIPT_ENV)?);
        let local_app_data = env::var_os("LOCALAPPDATA")?;
        if !(path.is_absolute()
            && path
                .file_name()
                .is_some_and(|name| name == ACTIVITY_WORKER_PROBE_RECEIPT_FILE)
            && path.parent() == Some(Path::new(&local_app_data)))
        {
            return None;
        }
        let started_path = path.with_file_name(ACTIVITY_WORKER_PROBE_STARTED_FILE);
        if let Ok(file) = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(started_path)
        {
            let _ = serde_json::to_writer(
                file,
                &serde_json::json!({
                    "schema": "tabbeacon-activity-worker-probe-v1",
                    "worker_entered": true,
                }),
            );
        }
        Some(Self {
            path,
            first_frame_sha256: None,
            first_frame_at: None,
            published: false,
        })
    }

    fn record_frame(&mut self, bytes: &[u8]) {
        if self.published {
            return;
        }
        let now = Instant::now();
        let frame_sha256 = format!("{:x}", Sha256::digest(bytes));
        let Some(first_frame_sha256) = self.first_frame_sha256.as_ref() else {
            self.first_frame_sha256 = Some(frame_sha256);
            self.first_frame_at = Some(now);
            return;
        };
        if first_frame_sha256 == &frame_sha256 {
            return;
        }
        let frame_interval_ms = self.first_frame_at.map_or(0, |first| {
            u64::try_from(now.saturating_duration_since(first).as_millis()).unwrap_or(u64::MAX)
        });
        let Ok(file) = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.path)
        else {
            return;
        };
        if serde_json::to_writer(
            file,
            &serde_json::json!({
                "schema": "tabbeacon-activity-worker-probe-v1",
                "worker_started": true,
                "distinct_spinner_frames": 2,
                "frame_interval_ms": frame_interval_ms,
            }),
        )
        .is_ok()
        {
            self.published = true;
        }
    }
}

/// Safe health classification for a read-only activity-lease inspection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
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

/// Stable JSON schema version for the read-only sessions view.
pub const SESSIONS_SCHEMA_VERSION: u32 = 2;

/// Bounded recency classification derived from a lease update timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionRecency {
    /// Updated no more than ten seconds ago.
    JustNow,
    /// Updated no more than five minutes ago.
    Recent,
    /// Updated more than five minutes ago.
    Aging,
}

impl SessionRecency {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::JustNow => "just_now",
            Self::Recent => "recent",
            Self::Aging => "aging",
        }
    }
}

/// Truthful health of one lease-backed session observation.
///
/// A current lease is only recently authorized. This view deliberately does
/// not probe an operating-system process and therefore never reports one as
/// alive merely because its lease has not expired.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionWorkerHealth {
    /// The lease is valid, active, and has not expired.
    RecentlyAuthorized,
    /// The lease is valid and active but its bounded authorization expired.
    StaleLease,
}

impl SessionWorkerHealth {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RecentlyAuthorized => "recently_authorized",
            Self::StaleLease => "stale_lease",
        }
    }
}

/// Bounded workspace facts that provider adapters may project into activity
/// leases without retaining raw paths or agent/session identifiers.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionWorkspaceObservability {
    /// Whether the session has an admitted root workspace anchor.
    pub root_binding_stable: bool,
    /// Whether an accepted event observed a different workspace identity.
    pub workspace_mismatch_observed: bool,
    /// Explicit lifecycle-derived count, capped by the provider adapter.
    pub active_subagents: u16,
    /// Absent unless a provider proves a background-task count.
    pub background_tasks: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ProviderSessionObservation {
    schema: String,
    session_sha256: String,
    provider: String,
    workspace_alias: String,
    semantic_state: String,
    updated_unix_ms: u64,
    expires_unix_ms: u64,
    workspace_observability: SessionWorkspaceObservability,
}

/// Persists one content-minimal provider observation without launching a
/// worker or claiming process liveness.
pub(crate) fn record_provider_session_observation(
    state_root: &Path,
    session_sha256: &str,
    provider: &str,
    workspace_alias: &str,
    semantic_state: &str,
    observed_unix_ms: u64,
    workspace_observability: SessionWorkspaceObservability,
) -> io::Result<()> {
    let observation = ProviderSessionObservation {
        schema: PROVIDER_SESSION_SCHEMA.to_owned(),
        session_sha256: session_sha256.to_owned(),
        provider: provider.to_owned(),
        workspace_alias: workspace_alias.to_owned(),
        semantic_state: semantic_state.to_owned(),
        updated_unix_ms: observed_unix_ms,
        expires_unix_ms: observed_unix_ms.saturating_add(LEASE_TTL_MS),
        workspace_observability,
    };
    validate_provider_session_observation(&observation)?;
    let directory = state_root.join(PROVIDER_SESSION_DIRECTORY);
    reject_symbolic_link(&directory)?;
    fs::create_dir_all(&directory)?;
    let lock_path = directory.join(PROVIDER_SESSION_LOCK_FILE);
    reject_symbolic_link(&lock_path)?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(lock_path)?;
    lock.lock()?;
    let path = directory.join(format!("session-{session_sha256}.json"));
    reject_symbolic_link(&path)?;
    let newer_exists = fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<ProviderSessionObservation>(&bytes).ok())
        .is_some_and(|current| current.updated_unix_ms > observed_unix_ms);
    let result = if newer_exists {
        Ok(())
    } else {
        let bytes = serde_json::to_vec_pretty(&observation)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        atomic_write(&path, &bytes)
    };
    File::unlock(&lock)?;
    result
}

/// One privacy-preserving row in the read-only sessions view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SessionOverview {
    /// Safe repository alias; never the canonical workspace path or identity.
    pub workspace_alias: String,
    /// Safe checked provider ID; never a native provider session ID.
    pub provider: String,
    /// Provider-neutral presentation state already admitted to the worker.
    pub semantic_state: String,
    /// Whole seconds since the lease was last updated.
    pub age_seconds: u64,
    /// Bounded human-oriented recency classification.
    pub recency: SessionRecency,
    /// Lease-backed worker health without a process-liveness claim.
    pub worker_health: SessionWorkerHealth,
    /// Content-minimal root-workspace and background facts.
    pub workspace_observability: SessionWorkspaceObservability,
}

/// Explicitly absent data and capabilities in the sessions interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SessionsBoundaries {
    pub raw_native_session_ids: bool,
    pub prompt_content: bool,
    pub remote_control: bool,
}

/// Content-minimal read-only projection of all inspectable activity leases.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SessionsOverview {
    pub schema_version: u32,
    pub observation: &'static str,
    pub health: ActivityLeaseHealth,
    pub active_sessions: usize,
    pub stale_sessions: usize,
    pub invalid_leases: usize,
    pub sessions: Vec<SessionOverview>,
    pub read_only: bool,
    pub boundaries: SessionsBoundaries,
}

impl SessionsOverview {
    const fn empty(health: ActivityLeaseHealth) -> Self {
        Self {
            schema_version: SESSIONS_SCHEMA_VERSION,
            observation: "ephemeral_lease_snapshot",
            health,
            active_sessions: 0,
            stale_sessions: 0,
            invalid_leases: 0,
            sessions: Vec::new(),
            read_only: true,
            boundaries: SessionsBoundaries {
                raw_native_session_ids: false,
                prompt_content: false,
                remote_control: false,
            },
        }
    }

    fn diagnostics(&self) -> ActivityLeaseDiagnostics {
        ActivityLeaseDiagnostics {
            health: self.health,
            active_leases: self.active_sessions,
            stale_leases: self.stale_sessions,
            invalid_leases: self.invalid_leases,
        }
    }
}

impl Default for SessionsOverview {
    fn default() -> Self {
        Self::empty(ActivityLeaseHealth::Unavailable)
    }
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

/// Inspects current-user activity leases as privacy-preserving session rows.
///
/// The inspection opens no lock and creates or changes no state.
#[must_use]
pub fn inspect_system_sessions() -> SessionsOverview {
    let Ok(state_root) = crate::repo::StableAliasRegistry::default_state_root() else {
        return SessionsOverview::empty(ActivityLeaseHealth::Unavailable);
    };
    inspect_sessions_read_only(&state_root, unix_ms())
}

/// One opaque activity-worker identity used only to prove a drain target.
///
/// The values remain internal to the preflight correlation. They are never
/// emitted through a Human or machine diagnostic surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActiveWorkerLeaseIdentity {
    pub(crate) key_sha256: String,
    pub(crate) generation: u64,
    pub(crate) revision: u64,
}

/// Read-only availability of the opaque worker identities needed for an
/// ownership-scoped upgrade drain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActiveWorkerLeaseInspection {
    pub(crate) health: ActivityLeaseHealth,
    pub(crate) identities: Vec<ActiveWorkerLeaseIdentity>,
    /// Content-addressed runtime images retained by any active lease,
    /// including an expired lease whose worker could still be winding down.
    pub(crate) runtime_image_hashes: BTreeSet<String>,
    /// A pre-G63 active lease has no image binding; that uncertainty blocks
    /// all runtime-image cleanup but does not invalidate the lease itself.
    pub(crate) active_legacy_lease_count: usize,
}

/// Reads active worker identities without creating state, taking a lock, or
/// exposing the hashes outside the ownership correlation path.
#[must_use]
pub(crate) fn inspect_system_active_worker_identities() -> ActiveWorkerLeaseInspection {
    let Ok(state_root) = crate::repo::StableAliasRegistry::default_state_root() else {
        return ActiveWorkerLeaseInspection {
            health: ActivityLeaseHealth::Unavailable,
            identities: Vec::new(),
            runtime_image_hashes: BTreeSet::new(),
            active_legacy_lease_count: 0,
        };
    };
    inspect_active_worker_identities_read_only(&state_root, unix_ms())
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
    /// Stable provider ID, defaulted for leases produced before G62.
    #[serde(default = "default_worker_provider")]
    provider: String,
    /// Bounded visibility fact selected by the originating presentation action.
    ///
    /// Leases written before provider visual identity keep the conservative
    /// false default: a worker must never infer visibility from a provider ID.
    #[serde(default)]
    provider_identity_visible: bool,
    semantic_state: String,
    spinner_preset: String,
    #[serde(default)]
    workspace_observability: SessionWorkspaceObservability,
}

impl WorkerPresentation {
    /// Creates an animated working-title presentation.
    #[must_use]
    pub fn working(workspace_alias: &str, spinner: SpinnerPreset) -> Self {
        Self::working_with_workspace_observability(
            "codex",
            workspace_alias,
            spinner,
            false,
            SessionWorkspaceObservability::default(),
        )
    }

    fn working_with_workspace_observability(
        provider: &str,
        workspace_alias: &str,
        spinner: SpinnerPreset,
        provider_identity_visible: bool,
        workspace_observability: SessionWorkspaceObservability,
    ) -> Self {
        Self {
            workspace_alias: workspace_alias.to_owned(),
            provider: provider.to_owned(),
            provider_identity_visible,
            semantic_state: "working".to_owned(),
            spinner_preset: spinner.as_str().to_owned(),
            workspace_observability,
        }
    }

    /// Creates a static result title that remains owned after a one-shot Hook
    /// returns control to its shell host.
    #[must_use]
    #[allow(dead_code)]
    fn result_ready(workspace_alias: &str, spinner: SpinnerPreset) -> Self {
        Self::result_ready_with_workspace_observability(
            "codex",
            workspace_alias,
            spinner,
            false,
            SessionWorkspaceObservability::default(),
        )
    }

    fn result_ready_with_workspace_observability(
        provider: &str,
        workspace_alias: &str,
        spinner: SpinnerPreset,
        provider_identity_visible: bool,
        workspace_observability: SessionWorkspaceObservability,
    ) -> Self {
        Self {
            workspace_alias: workspace_alias.to_owned(),
            provider: provider.to_owned(),
            provider_identity_visible,
            semantic_state: "result-ready".to_owned(),
            spinner_preset: spinner.as_str().to_owned(),
            workspace_observability,
        }
    }

    /// Creates a static approval title that remains owned after a one-shot Hook
    /// returns control to its shell host.
    #[must_use]
    #[allow(dead_code)]
    fn approval(workspace_alias: &str, spinner: SpinnerPreset) -> Self {
        Self::approval_with_workspace_observability(
            "codex",
            workspace_alias,
            spinner,
            false,
            SessionWorkspaceObservability::default(),
        )
    }

    fn approval_with_workspace_observability(
        provider: &str,
        workspace_alias: &str,
        spinner: SpinnerPreset,
        provider_identity_visible: bool,
        workspace_observability: SessionWorkspaceObservability,
    ) -> Self {
        Self {
            workspace_alias: workspace_alias.to_owned(),
            provider: provider.to_owned(),
            provider_identity_visible,
            semantic_state: "approval".to_owned(),
            spinner_preset: spinner.as_str().to_owned(),
            workspace_observability,
        }
    }

    fn spinner(&self) -> Option<SpinnerPreset> {
        SpinnerPreset::parse(&self.spinner_preset)
    }

    fn semantic_input(&self) -> Option<(Phase, Attention)> {
        match self.semantic_state.as_str() {
            "working" => Some((Phase::Working, Attention::None)),
            "result-ready" => Some((Phase::WaitingUser, Attention::ResultReady)),
            "approval" => Some((Phase::WaitingUser, Attention::Approval)),
            _ => None,
        }
    }

    fn from_action(
        provider: &str,
        workspace_alias: &str,
        action: &PresentationAction,
        settings: PresentationSettings,
        workspace_observability: SessionWorkspaceObservability,
    ) -> Option<Self> {
        if settings.title() != TitleMode::TabBeacon {
            return None;
        }
        let state = match action {
            PresentationAction::Apply(state) | PresentationAction::Reset(state) => state,
        };
        let provider_identity_visible = state.provider_visual_identity().is_some();
        match state.title_status() {
            TitleStatus::Working if settings.activity().uses_worker_animation() => {
                Some(Self::working_with_workspace_observability(
                    provider,
                    workspace_alias,
                    settings.spinner(),
                    provider_identity_visible,
                    workspace_observability,
                ))
            }
            TitleStatus::ResultReady => Some(Self::result_ready_with_workspace_observability(
                provider,
                workspace_alias,
                settings.spinner(),
                provider_identity_visible,
                workspace_observability,
            )),
            TitleStatus::Approval => Some(Self::approval_with_workspace_observability(
                provider,
                workspace_alias,
                settings.spinner(),
                provider_identity_visible,
                workspace_observability,
            )),
            _ => None,
        }
    }

    fn presentation_action(&self, phase: Phase, attention: Attention) -> PresentationAction {
        let provider_visual_identity = self
            .provider_identity_visible
            .then(|| ProviderVisualIdentity::for_provider_id(&self.provider));
        PresentationPolicy::resolve(
            SemanticPresentationInput::new_with_provider_visual_identity(
                phase,
                attention,
                Health::Normal,
                &self.workspace_alias,
                provider_visual_identity,
            ),
        )
    }

    fn lease_ttl_ms(&self) -> u64 {
        match self.semantic_state.as_str() {
            "result-ready" | "approval" => STATIC_ATTENTION_LEASE_TTL_MS,
            _ => LEASE_TTL_MS,
        }
    }
}

fn default_worker_provider() -> String {
    "codex".to_owned()
}

fn is_safe_worker_provider(provider: &str) -> bool {
    (1..=48).contains(&provider.len())
        && provider
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase())
        && provider
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && !provider.ends_with('-')
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

/// Bounded outcome of the asynchronous worker handoff after a lease is
/// published. None of these outcomes may suppress the originating Hook title:
/// spawn success is not worker-render readiness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PublishedWorkerStartup {
    Started,
    WorkerSpawnFailed,
}

impl PublishedWorkerStartup {
    const fn hook_render(self) -> ActivityRender {
        let _ = self;
        ActivityRender::Full
    }
}

fn start_published_worker(spawn: impl FnOnce() -> io::Result<u32>) -> PublishedWorkerStartup {
    match spawn() {
        Ok(_) => PublishedWorkerStartup::Started,
        Err(_) => PublishedWorkerStartup::WorkerSpawnFailed,
    }
}

const fn already_active_worker_render() -> ActivityRender {
    ActivityRender::WithoutTitle
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

    /// Whether this runtime has the inherited terminal binding needed to own
    /// a session-scoped activity worker.
    #[must_use]
    pub(crate) const fn system_enabled(&self) -> bool {
        matches!(&self.execution, ActivityExecution::System { .. })
    }

    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
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
        self.reconcile_with_workspace_observability(
            session_sha256,
            turn_sha256,
            generation,
            event_sequence,
            "codex",
            workspace_alias,
            action,
            settings,
            SessionWorkspaceObservability::default(),
        )
    }

    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub(crate) fn reconcile_with_workspace_observability(
        &self,
        session_sha256: &str,
        turn_sha256: Option<&str>,
        generation: u64,
        event_sequence: u64,
        provider: &str,
        workspace_alias: &str,
        action: &PresentationAction,
        settings: PresentationSettings,
        workspace_observability: SessionWorkspaceObservability,
    ) -> ActivityRender {
        let ActivityExecution::System {
            executable,
            owner_sha256,
            terminal_binding_sha256,
        } = &self.execution
        else {
            return ActivityRender::UncoordinatedFull;
        };
        if !is_safe_worker_provider(provider) {
            return ActivityRender::UncoordinatedFull;
        }
        let key = WorkerKey::new(
            session_sha256,
            turn_sha256,
            generation,
            terminal_binding_sha256,
        );
        let worker_presentation = WorkerPresentation::from_action(
            provider,
            workspace_alias,
            action,
            settings,
            workspace_observability,
        );
        let now = unix_ms();
        if let Some(presentation) = worker_presentation {
            match self.store.refresh_runtime_backed_active_if_current(
                &key,
                event_sequence,
                owner_sha256,
                &presentation,
                now,
            ) {
                Ok(Some(LeaseTransition::Stale)) => return ActivityRender::Suppress,
                Ok(Some(LeaseTransition::AlreadyActive)) => {
                    return already_active_worker_render();
                }
                Ok(None) => {}
                Ok(Some(LeaseTransition::Published { .. } | LeaseTransition::Stopped { .. })) => {
                    unreachable!("existing runtime refresh only returns stale or active")
                }
                Err(_) => return ActivityRender::UncoordinatedFull,
            }
            // A long-lived worker must never map the package-installed CLI.
            // Publishing is deliberately completed before its lease becomes
            // active, so an interrupted copy cannot authorize an ambiguous
            // process. A publication failure is decoration-only fail-open.
            let runtime_store = WorkerRuntimeStore::new(self.store.state_root());
            let Ok((runtime_image, transition)) = self.store.publish_runtime_backed_active(
                &runtime_store,
                executable,
                &key,
                event_sequence,
                owner_sha256,
                &presentation,
                now,
            ) else {
                return ActivityRender::UncoordinatedFull;
            };
            match transition {
                LeaseTransition::Stale => ActivityRender::Suppress,
                LeaseTransition::AlreadyActive => already_active_worker_render(),
                LeaseTransition::Published { lease, .. } => {
                    // Publishing this lease atomically revokes the predecessor.
                    // It is safe to start the successor immediately because
                    // every worker validates the current lease before writing;
                    // waiting for the old worker's exit would consume most of
                    // the synchronous one-second Hook budget.
                    match start_published_worker(|| spawn_worker(&runtime_image.executable, &lease))
                    {
                        PublishedWorkerStartup::Started => {
                            // Runtime-image collection is retention-only.  It
                            // re-enumerates state and hashes image files, so it
                            // must not delay the first one-second Hook frame.
                            // Setup prewarms the current immutable image;
                            // orphaned images remain safely retained until an
                            // explicit maintenance path collects them.
                            // Process creation establishes only that the worker
                            // was requested. It does not establish that it has
                            // opened the console or rendered its first frame.
                            // Keep this Hook's complete frame authoritative
                            // until a later event observes the active lease.
                            PublishedWorkerStartup::Started.hook_render()
                        }
                        PublishedWorkerStartup::WorkerSpawnFailed => {
                            let _ = self.store.deactivate_if_owned(&lease, unix_ms());
                            PublishedWorkerStartup::WorkerSpawnFailed.hook_render()
                        }
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
                // A stopped predecessor has already been atomically revoked.
                // Do not spend the synchronous Hook budget observing its exit.
                LeaseTransition::Stopped { .. } => ActivityRender::Full,
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
    record_activity_worker_probe_process_entry();
    let Ok(state_root) = crate::repo::StableAliasRegistry::default_state_root() else {
        return;
    };
    let Ok(coordinator) = ActivityCoordinator::system(state_root) else {
        return;
    };
    let ActivityExecution::System {
        executable,
        owner_sha256,
        terminal_binding_sha256,
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
        &executable,
    );
}

fn record_activity_worker_probe_process_entry() {
    record_activity_probe_process_entry(ACTIVITY_WORKER_PROBE_PROCESS_FILE, "worker");
}

fn record_activity_observer_probe_process_entry() {
    record_activity_probe_process_entry(ACTIVITY_OBSERVER_PROBE_PROCESS_FILE, "observer");
}

fn record_activity_probe_process_entry(file_name: &str, role: &str) {
    let Some(path) = activity_worker_probe_path(file_name) else {
        return;
    };
    let Ok(file) = OpenOptions::new().write(true).create_new(true).open(path) else {
        return;
    };
    let _ = serde_json::to_writer(
        file,
        &serde_json::json!({
            "schema": "tabbeacon-activity-worker-probe-v1",
            "role": role,
            "worker_process_entered": role == "worker",
            "stdin_class": standard_handle_class(StandardHandle::Input),
            "stdout_class": standard_handle_class(StandardHandle::Output),
            "stderr_class": standard_handle_class(StandardHandle::Error),
        }),
    );
}

#[derive(Clone, Copy)]
enum StandardHandle {
    Input,
    Output,
    Error,
}

#[cfg(windows)]
#[allow(unsafe_code)]
fn standard_handle_class(handle: StandardHandle) -> &'static str {
    use windows::Win32::{
        Storage::FileSystem::{FILE_TYPE_CHAR, FILE_TYPE_DISK, FILE_TYPE_PIPE, GetFileType},
        System::Console::{GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE},
    };

    let identifier = match handle {
        StandardHandle::Input => STD_INPUT_HANDLE,
        StandardHandle::Output => STD_OUTPUT_HANDLE,
        StandardHandle::Error => STD_ERROR_HANDLE,
    };
    let Ok(handle) = (unsafe { GetStdHandle(identifier) }) else {
        return "UNKNOWN";
    };
    match unsafe { GetFileType(handle) } {
        FILE_TYPE_PIPE => "PIPE",
        FILE_TYPE_CHAR => "CHAR",
        FILE_TYPE_DISK => "DISK",
        _ => "UNKNOWN",
    }
}

#[cfg(not(windows))]
fn standard_handle_class(_handle: StandardHandle) -> &'static str {
    "UNKNOWN"
}

fn activity_worker_probe_path(file_name: &str) -> Option<PathBuf> {
    let path = PathBuf::from(env::var_os(ACTIVITY_WORKER_PROBE_RECEIPT_ENV)?);
    let local_app_data = env::var_os("LOCALAPPDATA")?;
    (path.is_absolute()
        && path
            .file_name()
            .is_some_and(|name| name == ACTIVITY_WORKER_PROBE_RECEIPT_FILE)
        && path.parent() == Some(Path::new(&local_app_data)))
    .then(|| path.with_file_name(file_name))
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
    record_activity_observer_probe_process_entry();
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
    let mut last_identity_check_unix_ms: Option<u64> = None;
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
        let liveness = bounded_worker_process_liveness(
            worker_pid,
            &ownership,
            expected_executable,
            now,
            &mut last_identity_check_unix_ms,
        );
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
            CleanupObserverAction::Deactivate(_) => {
                if store
                    .deactivate_observed_worker(&observed, unix_ms())
                    .unwrap_or(false)
                {
                    return;
                }
                // A same-owner refresh may have won while the liveness query
                // was in flight. That newer snapshot was deliberately
                // preserved; continue observing it rather than leaving an
                // active lease without a cleanup observer.
                unknown_observation = None;
                thread::sleep(Duration::from_millis(cleanup_observer_poll_ms(
                    &ownership.key_sha256,
                )));
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
                thread::sleep(Duration::from_millis(cleanup_observer_poll_ms(
                    &ownership.key_sha256,
                )));
            }
        }
    }
}

/// Keeps routine native liveness checks bounded while spreading simultaneous
/// session starts across a four-second window. The digest is opaque and stable
/// for one session/terminal binding, so this cannot expose workspace or Hook
/// input data.
fn cleanup_observer_poll_ms(key_sha256: &str) -> u64 {
    let prefix = key_sha256
        .get(..2)
        .and_then(|value| u8::from_str_radix(value, 16).ok())
        .map_or(0, u64::from);
    CLEANUP_OBSERVER_POLL_MIN_MS
        .saturating_add(prefix.saturating_mul(CLEANUP_OBSERVER_POLL_SPREAD_MS) / 255)
}

/// Preserves the exact process-identity proof at observer start and on a
/// bounded recheck cadence. Between those checks, a native absence probe is
/// sufficient to observe a crashed worker without repeatedly cold-starting
/// PowerShell for every active terminal session.
fn bounded_worker_process_liveness(
    worker_pid: u32,
    ownership: &WorkerOwnership,
    expected_executable: &str,
    now_unix_ms: u64,
    last_identity_check_unix_ms: &mut Option<u64>,
) -> WorkerProcessLiveness {
    match tasklist_process_absence(worker_pid) {
        Some(true) => return WorkerProcessLiveness::Exited,
        Some(false) => {}
        None => return WorkerProcessLiveness::Unknown,
    }
    let identity_recheck_due =
        cleanup_identity_recheck_due(*last_identity_check_unix_ms, now_unix_ms);
    if identity_recheck_due {
        *last_identity_check_unix_ms = Some(now_unix_ms);
        worker_process_liveness(worker_pid, ownership, expected_executable)
    } else {
        WorkerProcessLiveness::Alive
    }
}

fn cleanup_identity_recheck_due(
    last_identity_check_unix_ms: Option<u64>,
    now_unix_ms: u64,
) -> bool {
    last_identity_check_unix_ms
        .is_none_or(|last| now_unix_ms.saturating_sub(last) >= CLEANUP_OBSERVER_IDENTITY_RECHECK_MS)
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
    /// Absent only for a pre-G63 lease. Such a lease stays valid for its
    /// original worker, but it blocks runtime-image garbage collection and is
    /// deliberately superseded by a new runtime-backed publication.
    #[serde(default)]
    runtime_image_sha256: Option<String>,
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

    #[cfg(test)]
    fn publish_active(
        &self,
        key: &WorkerKey,
        event_sequence: u64,
        owner_sha256: &str,
        presentation: &WorkerPresentation,
        now: u64,
    ) -> io::Result<LeaseTransition> {
        self.publish_active_with_runtime_image(
            key,
            event_sequence,
            owner_sha256,
            None,
            presentation,
            now,
        )
    }

    #[cfg(test)]
    fn publish_active_with_runtime_image(
        &self,
        key: &WorkerKey,
        event_sequence: u64,
        owner_sha256: &str,
        runtime_image_sha256: Option<&str>,
        presentation: &WorkerPresentation,
        now: u64,
    ) -> io::Result<LeaseTransition> {
        let expires_unix_ms = now.saturating_add(presentation.lease_ttl_ms());
        self.with_lock(|| {
            self.publish_active_locked(
                key,
                event_sequence,
                owner_sha256,
                runtime_image_sha256,
                presentation,
                now,
                expires_unix_ms,
            )
        })
    }

    #[allow(clippy::too_many_arguments)] // Keeps the image, lease identity, and presentation atomic at one visible boundary.
    fn publish_runtime_backed_active(
        &self,
        runtime_store: &WorkerRuntimeStore,
        executable: &Path,
        key: &WorkerKey,
        event_sequence: u64,
        owner_sha256: &str,
        presentation: &WorkerPresentation,
        now: u64,
    ) -> io::Result<(WorkerRuntimeImage, LeaseTransition)> {
        let expires_unix_ms = now.saturating_add(presentation.lease_ttl_ms());
        // This lock is the atomic ownership boundary for a runtime image:
        // another Hook cannot publish an active lease between collection's
        // proven lease scan and a stale image deletion.
        self.with_lock(|| {
            let runtime_image = runtime_store.publish(executable)?;
            let transition = self.publish_active_locked(
                key,
                event_sequence,
                owner_sha256,
                Some(&runtime_image.content_sha256),
                presentation,
                now,
                expires_unix_ms,
            )?;
            Ok((runtime_image, transition))
        })
    }

    /// Refreshes a proven active runtime lease without reopening or hashing the
    /// executable image. Ordinary Hook events can take this path only after a
    /// prior publication has established the exact content-bound image; a
    /// missing, stale, changed, or legacy lease still takes the full safe
    /// publication path.
    #[allow(clippy::too_many_arguments)]
    fn refresh_runtime_backed_active_if_current(
        &self,
        key: &WorkerKey,
        event_sequence: u64,
        owner_sha256: &str,
        presentation: &WorkerPresentation,
        now: u64,
    ) -> io::Result<Option<LeaseTransition>> {
        let expires_unix_ms = now.saturating_add(presentation.lease_ttl_ms());
        self.with_lock(|| {
            let Some(mut current) = self.load(key.digest())? else {
                return Ok(None);
            };
            if is_stale(key.generation, event_sequence, &current) {
                return Ok(Some(LeaseTransition::Stale));
            }
            if !current.active
                || current.generation != key.generation
                || current.owner_sha256 != owner_sha256
                || current.runtime_image_sha256.is_none()
                || current.presentation.as_ref() != Some(presentation)
            {
                return Ok(None);
            }
            current.event_sequence = current.event_sequence.max(event_sequence);
            current.updated_unix_ms = now;
            current.expires_unix_ms = expires_unix_ms;
            self.write(&current)?;
            Ok(Some(LeaseTransition::AlreadyActive))
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn publish_active_locked(
        &self,
        key: &WorkerKey,
        event_sequence: u64,
        owner_sha256: &str,
        runtime_image_sha256: Option<&str>,
        presentation: &WorkerPresentation,
        now: u64,
        expires_unix_ms: u64,
    ) -> io::Result<LeaseTransition> {
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
            && current.runtime_image_sha256.as_deref() == runtime_image_sha256
            && current.presentation.as_ref() == Some(presentation)
        {
            current.event_sequence = current.event_sequence.max(event_sequence);
            current.updated_unix_ms = now;
            current.expires_unix_ms = expires_unix_ms;
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
            runtime_image_sha256: runtime_image_sha256.map(str::to_owned),
            active: true,
            presentation: Some(presentation.clone()),
            updated_unix_ms: now,
            expires_unix_ms,
        };
        validate_lease(&lease)?;
        self.write(&lease)?;
        Ok(LeaseTransition::Published {
            lease: Box::new(lease),
            predecessor,
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
                runtime_image_sha256: None,
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

    fn run_worker(
        &self,
        key_digest: &str,
        generation: u64,
        revision: u64,
        owner_sha256: &str,
        terminal_binding_sha256: &str,
        executable: &Path,
    ) {
        let ownership = WorkerOwnership {
            key_sha256: key_digest.to_owned(),
            generation,
            revision,
            owner_sha256: owner_sha256.to_owned(),
        };
        let Some(initial) = self.load_worker_lease(&ownership, terminal_binding_sha256, unix_ms())
        else {
            return;
        };
        let Some(presentation) = initial.presentation.clone() else {
            let _ = self.deactivate_if_owned(&initial, unix_ms());
            return;
        };
        let Some(spinner) = presentation.spinner() else {
            let _ = self.deactivate_if_owned(&initial, unix_ms());
            return;
        };
        let Some((phase, attention)) = presentation.semantic_input() else {
            let _ = self.deactivate_if_owned(&initial, unix_ms());
            return;
        };
        let mut probe = ActivityWorkerProbeCapture::from_environment();
        let Ok(mut console) = open_owned_console(probe.is_some()) else {
            let _ = self.deactivate_if_owned(&initial, unix_ms());
            return;
        };
        // The command Hook has already left a complete persistent frame. Do
        // not make that one-second path synchronously create a second process
        // merely to observe this worker. Once this verified runtime image has
        // opened the inherited console, it can establish its own exact cleanup
        // observer before it takes title ownership. A failure leaves the
        // Hook's full frame intact and deactivates this lease without writing a
        // worker frame.
        if spawn_cleanup_observer(executable, &initial, std::process::id()).is_err() {
            let _ = self.deactivate_if_owned(&initial, unix_ms());
            return;
        }
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
        let action = presentation.presentation_action(phase, attention);
        let state = match &action {
            PresentationAction::Apply(state) | PresentationAction::Reset(state) => state,
        };
        let mut frame_index = 0_usize;
        let mut next_frame_deadline = Instant::now();
        let animated = presentation.semantic_state == "working";
        loop {
            let bytes = renderer.render_title_spinner_frame(state, frame_index);
            // Keep lease validation and the terminal write in one critical section:
            // a newer event cannot supersede this worker between its authorization
            // check and the title write.
            match self.with_lock(|| {
                let Some(_) =
                    self.load_worker_lease_locked(&ownership, terminal_binding_sha256, unix_ms())?
                else {
                    return Ok(None);
                };
                console.write_all(&bytes)?;
                console.flush()?;
                Ok(Some(()))
            }) {
                Ok(Some(())) => {}
                Ok(None) | Err(_) => break,
            }
            if let Some(probe) = probe.as_mut() {
                // The recorder runs only after the real owned-console write
                // and flush succeeded. It stores no rendered title content.
                probe.record_frame(&bytes);
            }
            frame_index = frame_index.saturating_add(1);
            if animated {
                next_frame_deadline =
                    next_animation_frame_deadline(next_frame_deadline, Instant::now());
                let remaining = next_frame_deadline.saturating_duration_since(Instant::now());
                if !remaining.is_zero() {
                    thread::sleep(remaining);
                }
            } else {
                thread::sleep(Duration::from_millis(STATIC_ATTENTION_WORKER_POLL_MS));
            }
        }
        let _ = self.deactivate_if_owned(&initial, unix_ms());
    }

    fn load_worker_lease(
        &self,
        ownership: &WorkerOwnership,
        terminal_binding_sha256: &str,
        now: u64,
    ) -> Option<WorkerLease> {
        self.with_lock(|| self.load_worker_lease_locked(ownership, terminal_binding_sha256, now))
            .ok()
            .flatten()
    }

    /// Validates a worker lease while the caller holds this store's lock.
    fn load_worker_lease_locked(
        &self,
        ownership: &WorkerOwnership,
        terminal_binding_sha256: &str,
        now: u64,
    ) -> io::Result<Option<WorkerLease>> {
        let Some(lease) = self.load(&ownership.key_sha256)? else {
            return Ok(None);
        };
        Ok((lease.active
            && lease.generation == ownership.generation
            && lease.revision == ownership.revision
            && lease.owner_sha256 == ownership.owner_sha256
            && lease.terminal_binding_sha256 == terminal_binding_sha256
            && lease.presentation.is_some()
            && now <= lease.expires_unix_ms)
            .then_some(lease))
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

    fn state_root(&self) -> PathBuf {
        self.directory
            .parent()
            .map_or_else(PathBuf::new, Path::to_path_buf)
    }

    /// Returns the active runtime images only when every relevant lease can
    /// be validated while holding the lease lock. Unknown or legacy active
    /// leases deliberately retain every image.
    #[cfg(test)]
    fn active_runtime_images_for_gc(&self) -> (BTreeSet<String>, bool) {
        self.with_lock(|| self.active_runtime_images_for_gc_locked())
            .map_or_else(|_| (BTreeSet::new(), false), |images| (images, true))
    }

    #[cfg(test)]
    fn active_runtime_images_for_gc_locked(&self) -> io::Result<BTreeSet<String>> {
        let mut active_images = BTreeSet::new();
        let mut lease_files = 0_usize;
        for entry in fs::read_dir(&self.directory)? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(key_digest) = name
                .to_str()
                .and_then(|name| name.strip_prefix("lease-"))
                .and_then(|value| value.strip_suffix(".json"))
            else {
                continue;
            };
            lease_files = lease_files.saturating_add(1);
            if lease_files > MAX_DIAGNOSTIC_LEASE_FILES {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "too many activity leases to prove runtime-image ownership",
                ));
            }
            let Some(lease) = self.load(key_digest)? else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "activity lease disappeared during runtime-image inspection",
                ));
            };
            if lease.active {
                let Some(image_hash) = lease.runtime_image_sha256 else {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "legacy active activity lease has no runtime-image ownership",
                    ));
                };
                active_images.insert(image_hash);
            }
        }
        Ok(active_images)
    }

    #[cfg(test)]
    fn collect_unused_runtime_images(&self, runtime_store: &WorkerRuntimeStore) {
        let _ = self.with_lock(|| {
            let (active_images, ownership_proven) = self
                .active_runtime_images_for_gc_locked()
                .map_or_else(|_| (BTreeSet::new(), false), |images| (images, true));
            let _ = runtime_store.collect_unused(&active_images, ownership_proven);
            Ok(())
        });
    }

    fn lease_path(&self, key_digest: &str) -> PathBuf {
        self.directory.join(format!("lease-{key_digest}.json"))
    }
}

fn inspect_activity_leases_read_only(state_root: &Path, now: u64) -> ActivityLeaseDiagnostics {
    inspect_sessions_read_only(state_root, now).diagnostics()
}

#[allow(clippy::too_many_lines)] // Lease validation and opaque drain identity stay in one fail-closed read path.
fn inspect_active_worker_identities_read_only(
    state_root: &Path,
    now: u64,
) -> ActiveWorkerLeaseInspection {
    let directory = state_root.join(STATE_DIRECTORY);
    let metadata = match fs::symlink_metadata(&directory) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return ActiveWorkerLeaseInspection {
                health: ActivityLeaseHealth::Unavailable,
                identities: Vec::new(),
                runtime_image_hashes: BTreeSet::new(),
                active_legacy_lease_count: 0,
            };
        }
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return ActiveWorkerLeaseInspection {
                health: ActivityLeaseHealth::Healthy,
                identities: Vec::new(),
                runtime_image_hashes: BTreeSet::new(),
                active_legacy_lease_count: 0,
            };
        }
        Err(_) => {
            return ActiveWorkerLeaseInspection {
                health: ActivityLeaseHealth::Unavailable,
                identities: Vec::new(),
                runtime_image_hashes: BTreeSet::new(),
                active_legacy_lease_count: 0,
            };
        }
    };
    if !metadata.is_dir() {
        return ActiveWorkerLeaseInspection {
            health: ActivityLeaseHealth::Unavailable,
            identities: Vec::new(),
            runtime_image_hashes: BTreeSet::new(),
            active_legacy_lease_count: 0,
        };
    }
    let Ok(entries) = fs::read_dir(&directory) else {
        return ActiveWorkerLeaseInspection {
            health: ActivityLeaseHealth::Unavailable,
            identities: Vec::new(),
            runtime_image_hashes: BTreeSet::new(),
            active_legacy_lease_count: 0,
        };
    };

    let mut inspection = ActiveWorkerLeaseInspection {
        health: ActivityLeaseHealth::Healthy,
        identities: Vec::new(),
        runtime_image_hashes: BTreeSet::new(),
        active_legacy_lease_count: 0,
    };
    for (index, entry) in entries.enumerate() {
        if index >= MAX_DIAGNOSTIC_LEASE_FILES {
            inspection.health = ActivityLeaseHealth::Warning;
            break;
        }
        let Ok(entry) = entry else {
            inspection.health = ActivityLeaseHealth::Warning;
            continue;
        };
        let name = entry.file_name();
        let Some(key_digest) = name
            .to_str()
            .and_then(|name| name.strip_prefix("lease-"))
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
            inspection.health = ActivityLeaseHealth::Warning;
            continue;
        }
        let Some(lease) = fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<WorkerLease>(&bytes).ok())
            .filter(|lease| lease.key_sha256 == key_digest && validate_lease(lease).is_ok())
        else {
            inspection.health = ActivityLeaseHealth::Warning;
            continue;
        };
        if lease.updated_unix_ms > now {
            inspection.health = ActivityLeaseHealth::Warning;
            continue;
        }
        if lease.active {
            if let Some(runtime_image_sha256) = lease.runtime_image_sha256 {
                inspection.runtime_image_hashes.insert(runtime_image_sha256);
            } else {
                inspection.active_legacy_lease_count =
                    inspection.active_legacy_lease_count.saturating_add(1);
            }
        }
        if lease.active && lease.presentation.is_some() && now <= lease.expires_unix_ms {
            inspection.identities.push(ActiveWorkerLeaseIdentity {
                key_sha256: lease.key_sha256,
                generation: lease.generation,
                revision: lease.revision,
            });
        }
    }
    inspection.identities.sort_by(|left, right| {
        left.key_sha256
            .cmp(&right.key_sha256)
            .then(left.generation.cmp(&right.generation))
            .then(left.revision.cmp(&right.revision))
    });
    inspection
}

fn inspect_sessions_read_only(state_root: &Path, now: u64) -> SessionsOverview {
    let directory = state_root.join(STATE_DIRECTORY);
    let entries = match fs::symlink_metadata(&directory) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return SessionsOverview::empty(ActivityLeaseHealth::Unavailable);
        }
        Ok(_) => match fs::read_dir(&directory) {
            Ok(entries) => Some(entries),
            Err(_) => return SessionsOverview::empty(ActivityLeaseHealth::Unavailable),
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(_) => return SessionsOverview::empty(ActivityLeaseHealth::Unavailable),
    };
    let mut overview = SessionsOverview::empty(ActivityLeaseHealth::Healthy);
    if let Some(entries) = entries {
        inspect_worker_sessions(entries, now, &mut overview);
    }
    inspect_provider_sessions(state_root, now, &mut overview);
    overview.sessions.sort_by(|left, right| {
        left.workspace_alias
            .cmp(&right.workspace_alias)
            .then(left.provider.cmp(&right.provider))
            .then(left.semantic_state.cmp(&right.semantic_state))
            .then(left.age_seconds.cmp(&right.age_seconds))
    });
    if overview.stale_sessions > 0 || overview.invalid_leases > 0 {
        overview.health = ActivityLeaseHealth::Warning;
    }
    overview
}

fn inspect_worker_sessions(entries: fs::ReadDir, now: u64, overview: &mut SessionsOverview) {
    for (index, entry) in entries.enumerate() {
        if index >= MAX_DIAGNOSTIC_LEASE_FILES {
            overview.invalid_leases = overview.invalid_leases.saturating_add(1);
            break;
        }
        let Ok(entry) = entry else {
            overview.invalid_leases = overview.invalid_leases.saturating_add(1);
            continue;
        };
        let name = entry.file_name();
        let Some(key_digest) = name
            .to_str()
            .and_then(|name| name.strip_prefix("lease-"))
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
            overview.invalid_leases = overview.invalid_leases.saturating_add(1);
            continue;
        }
        let lease = fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<WorkerLease>(&bytes).ok());
        let Some(lease) =
            lease.filter(|lease| lease.key_sha256 == key_digest && validate_lease(lease).is_ok())
        else {
            overview.invalid_leases = overview.invalid_leases.saturating_add(1);
            continue;
        };
        let Some(presentation) = lease.presentation.filter(|_| lease.active) else {
            continue;
        };
        push_session(
            overview,
            now,
            lease.updated_unix_ms,
            lease.expires_unix_ms,
            presentation.workspace_alias,
            presentation.provider,
            presentation.semantic_state,
            presentation.workspace_observability,
        );
    }
}

fn inspect_provider_sessions(state_root: &Path, now: u64, overview: &mut SessionsOverview) {
    let directory = state_root.join(PROVIDER_SESSION_DIRECTORY);
    let Ok(metadata) = fs::symlink_metadata(&directory) else {
        return;
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        overview.invalid_leases = overview.invalid_leases.saturating_add(1);
        return;
    }
    let Ok(entries) = fs::read_dir(&directory) else {
        overview.invalid_leases = overview.invalid_leases.saturating_add(1);
        return;
    };
    for (index, entry) in entries.enumerate() {
        if index >= MAX_DIAGNOSTIC_LEASE_FILES {
            overview.invalid_leases = overview.invalid_leases.saturating_add(1);
            break;
        }
        let Ok(entry) = entry else {
            overview.invalid_leases = overview.invalid_leases.saturating_add(1);
            continue;
        };
        let name = entry.file_name();
        let Some(session_sha256) = name
            .to_str()
            .and_then(|name| name.strip_prefix("session-"))
            .and_then(|value| value.strip_suffix(".json"))
        else {
            continue;
        };
        let path = entry.path();
        let observation = fs::symlink_metadata(&path)
            .ok()
            .filter(|metadata| {
                metadata.is_file()
                    && !metadata.file_type().is_symlink()
                    && metadata.len() <= MAX_DIAGNOSTIC_LEASE_BYTES
            })
            .and_then(|_| fs::read(&path).ok())
            .and_then(|bytes| serde_json::from_slice::<ProviderSessionObservation>(&bytes).ok());
        let Some(observation) = observation.filter(|observation| {
            observation.session_sha256 == session_sha256
                && validate_provider_session_observation(observation).is_ok()
        }) else {
            overview.invalid_leases = overview.invalid_leases.saturating_add(1);
            continue;
        };
        push_session(
            overview,
            now,
            observation.updated_unix_ms,
            observation.expires_unix_ms,
            observation.workspace_alias,
            observation.provider,
            observation.semantic_state,
            observation.workspace_observability,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn push_session(
    overview: &mut SessionsOverview,
    now: u64,
    updated_unix_ms: u64,
    expires_unix_ms: u64,
    workspace_alias: String,
    provider: String,
    semantic_state: String,
    workspace_observability: SessionWorkspaceObservability,
) {
    if updated_unix_ms > now {
        overview.invalid_leases = overview.invalid_leases.saturating_add(1);
        return;
    }
    let age_seconds = now.saturating_sub(updated_unix_ms) / 1_000;
    let worker_health = if now > expires_unix_ms {
        overview.stale_sessions = overview.stale_sessions.saturating_add(1);
        SessionWorkerHealth::StaleLease
    } else {
        overview.active_sessions = overview.active_sessions.saturating_add(1);
        SessionWorkerHealth::RecentlyAuthorized
    };
    overview.sessions.push(SessionOverview {
        workspace_alias,
        provider,
        semantic_state,
        age_seconds,
        recency: session_recency(age_seconds),
        worker_health,
        workspace_observability,
    });
}

fn validate_provider_session_observation(
    observation: &ProviderSessionObservation,
) -> io::Result<()> {
    if observation.schema != PROVIDER_SESSION_SCHEMA
        || !is_sha256(&observation.session_sha256)
        || observation.provider != "agy"
        || !matches!(observation.semantic_state.as_str(), "ready" | "working")
        || RepositoryAlias::new(observation.workspace_alias.clone()).is_err()
        || observation.updated_unix_ms > observation.expires_unix_ms
        || observation
            .expires_unix_ms
            .saturating_sub(observation.updated_unix_ms)
            > LEASE_TTL_MS
        || observation.workspace_observability.active_subagents != 0
        || observation
            .workspace_observability
            .background_tasks
            .is_some()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "provider session observation is incompatible or unsafe",
        ));
    }
    Ok(())
}

fn session_recency(age_seconds: u64) -> SessionRecency {
    if age_seconds <= 10 {
        SessionRecency::JustNow
    } else if age_seconds <= 5 * 60 {
        SessionRecency::Recent
    } else {
        SessionRecency::Aging
    }
}

fn is_stale(generation: u64, event_sequence: u64, current: &WorkerLease) -> bool {
    generation < current.generation
        || (generation == current.generation && event_sequence < current.event_sequence)
}

fn validate_lease(lease: &WorkerLease) -> io::Result<()> {
    let presentation_valid = lease.presentation.as_ref().is_none_or(|presentation| {
        matches!(
            presentation.semantic_state.as_str(),
            "working" | "result-ready" | "approval"
        ) && presentation.spinner().is_some()
            && matches!(presentation.provider.as_str(), "codex" | "agy")
            && RepositoryAlias::new(presentation.workspace_alias.clone()).is_ok()
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
        || lease
            .runtime_image_sha256
            .as_deref()
            .is_some_and(|value| !is_sha256(value))
        || lease.active != lease.presentation.is_some()
        || lease.updated_unix_ms > lease.expires_unix_ms
        || lease.expires_unix_ms.saturating_sub(lease.updated_unix_ms) > LEASE_TTL_MS
        || !presentation_valid
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "activity worker lease is incompatible or unsafe",
        ));
    }
    Ok(())
}

#[cfg(not(windows))]
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

#[cfg(windows)]
struct OwnedWorkerHandle(windows::Win32::Foundation::HANDLE);

#[cfg(windows)]
impl Drop for OwnedWorkerHandle {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        let _ = unsafe { windows::Win32::Foundation::CloseHandle(self.0) };
    }
}

#[cfg(windows)]
struct WorkerAttributeList {
    pointer: windows::Win32::System::Threading::LPPROC_THREAD_ATTRIBUTE_LIST,
    _storage: Vec<usize>,
}

#[cfg(windows)]
impl Drop for WorkerAttributeList {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        unsafe {
            windows::Win32::System::Threading::DeleteProcThreadAttributeList(self.pointer);
        }
    }
}

#[cfg(windows)]
fn windows_io_error(error: windows::core::Error) -> io::Error {
    io::Error::other(error)
}

#[cfg(windows)]
#[allow(unsafe_code)]
fn open_inheritable_nul(access: u32) -> io::Result<OwnedWorkerHandle> {
    use std::{mem, ptr};

    use windows::Win32::{
        Security::SECURITY_ATTRIBUTES,
        Storage::FileSystem::{
            CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
        },
    };

    let security = SECURITY_ATTRIBUTES {
        nLength: u32::try_from(mem::size_of::<SECURITY_ATTRIBUTES>()).unwrap_or(u32::MAX),
        lpSecurityDescriptor: ptr::null_mut(),
        bInheritHandle: true.into(),
    };
    unsafe {
        CreateFileW(
            windows::core::w!("NUL"),
            access,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            Some(&raw const security),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None,
        )
    }
    .map(OwnedWorkerHandle)
    .map_err(windows_io_error)
}

#[cfg(windows)]
#[allow(unsafe_code)]
fn worker_attribute_list(
    handles: &[windows::Win32::Foundation::HANDLE],
) -> io::Result<WorkerAttributeList> {
    use std::{ffi::c_void, mem};

    use windows::Win32::System::Threading::{
        DeleteProcThreadAttributeList, InitializeProcThreadAttributeList,
        LPPROC_THREAD_ATTRIBUTE_LIST, PROC_THREAD_ATTRIBUTE_HANDLE_LIST, UpdateProcThreadAttribute,
    };

    let mut bytes = 0_usize;
    let _ = unsafe { InitializeProcThreadAttributeList(None, 1, None, &raw mut bytes) };
    if bytes == 0 {
        return Err(io::Error::last_os_error());
    }
    let words = bytes.div_ceil(mem::size_of::<usize>());
    let mut storage = vec![0_usize; words];
    let pointer = LPPROC_THREAD_ATTRIBUTE_LIST(storage.as_mut_ptr().cast::<c_void>());
    unsafe { InitializeProcThreadAttributeList(Some(pointer), 1, None, &raw mut bytes) }
        .map_err(windows_io_error)?;
    if let Err(error) = unsafe {
        UpdateProcThreadAttribute(
            pointer,
            0,
            PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
            Some(handles.as_ptr().cast::<c_void>()),
            mem::size_of_val(handles),
            None,
            None,
        )
    } {
        unsafe { DeleteProcThreadAttributeList(pointer) };
        return Err(windows_io_error(error));
    }
    Ok(WorkerAttributeList {
        pointer,
        _storage: storage,
    })
}

/// Starts a long-lived worker with an explicit standard-handle allowlist.
///
/// Rust's Windows `Command` implementation currently calls `CreateProcessW`
/// with `bInheritHandles=TRUE`. `Stdio::null()` replaces the three standard
/// handles, but it does not exclude other inheritable handles already present
/// in the Hook process (notably the command runner's redirected pipe ends).
/// A `PROC_THREAD_ATTRIBUTE_HANDLE_LIST` preserves explicit NUL stdio without
/// allowing those ambient handles to keep Codex's output pipes open.
#[cfg(windows)]
#[allow(unsafe_code)]
fn spawn_worker(executable: &Path, lease: &WorkerLease) -> io::Result<u32> {
    use std::mem;

    use windows::{
        Win32::{
            Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE},
            System::Threading::{
                CREATE_UNICODE_ENVIRONMENT, CreateProcessW, EXTENDED_STARTUPINFO_PRESENT,
                PROCESS_INFORMATION, STARTF_USESTDHANDLES, STARTUPINFOEXW,
            },
        },
        core::{PCWSTR, PWSTR},
    };

    let stdin = open_inheritable_nul(GENERIC_READ.0)?;
    let output = open_inheritable_nul(GENERIC_WRITE.0)?;
    let inherited_handles = [stdin.0, output.0];
    let attributes = worker_attribute_list(&inherited_handles)?;

    let mut application = executable.as_os_str().encode_wide().collect::<Vec<_>>();
    if application.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "worker executable contains an embedded NUL",
        ));
    }
    application.push(0);
    let mut command_line = Vec::new();
    command_line.push(u16::from(b'"'));
    command_line.extend(executable.as_os_str().encode_wide());
    command_line.extend(
        format!(
            "\" __activity-worker-v1 {} {} {}",
            lease.key_sha256, lease.generation, lease.revision
        )
        .encode_utf16(),
    );
    command_line.push(0);

    let mut startup = STARTUPINFOEXW::default();
    startup.StartupInfo.cb = u32::try_from(mem::size_of::<STARTUPINFOEXW>()).unwrap_or(u32::MAX);
    startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    startup.StartupInfo.hStdInput = stdin.0;
    startup.StartupInfo.hStdOutput = output.0;
    startup.StartupInfo.hStdError = output.0;
    startup.lpAttributeList = attributes.pointer;
    let mut process_information = PROCESS_INFORMATION::default();
    unsafe {
        CreateProcessW(
            PCWSTR(application.as_ptr()),
            Some(PWSTR(command_line.as_mut_ptr())),
            None,
            None,
            true,
            CREATE_UNICODE_ENVIRONMENT | EXTENDED_STARTUPINFO_PRESENT,
            None,
            PCWSTR::null(),
            &raw const startup.StartupInfo,
            &raw mut process_information,
        )
    }
    .map_err(windows_io_error)?;
    let process_id = process_information.dwProcessId;
    let _ = unsafe { CloseHandle(process_information.hThread) };
    let _ = unsafe { CloseHandle(process_information.hProcess) };
    Ok(process_id)
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
           if ($actual.StartsWith('//?/')) {{ $actual = $actual.Substring(4) }} \
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
    let normalized = normalized_windows_path(&canonical.to_string_lossy());
    if is_safe_normalized_path(&normalized) {
        Ok(normalized)
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "activity worker executable path is unsafe",
        ))
    }
}

fn normalized_windows_path(value: &str) -> String {
    let normalized = value.replace('\\', "/").to_lowercase();
    normalized
        .strip_prefix("//?/")
        .unwrap_or(&normalized)
        .to_owned()
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
    Some(tasklist_output_reports_absence(&String::from_utf8_lossy(
        &output.stdout,
    )))
}

fn tasklist_output_reports_absence(stdout: &str) -> bool {
    !stdout
        .lines()
        .any(|line| line.trim_start().starts_with('"'))
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

/// Stable executable-content identity shared by the one-shot package CLI and
/// the immutable runtime image copied from it. The runtime publisher rejects
/// redirected paths and verifies that exact content before it can be leased.
fn executable_owner_sha256(path: &Path) -> io::Result<String> {
    let canonical = fs::canonicalize(path)?;
    let mut digest = Sha256::new();
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

fn open_owned_console(probe_enabled: bool) -> io::Result<Box<dyn Write>> {
    if probe_enabled {
        // A test process can inherit a syntactically open CONOUT$ handle that
        // still rejects a direct console-title API because its host is noninteractive.
        // Route the isolated probe's already-rendered bytes to NUL instead:
        // this proves the worker's real write/flush/cadence path without
        // persisting a title or conflating test-host UI ownership with the
        // MCP environment contract.
        #[cfg(windows)]
        {
            return OpenOptions::new()
                .write(true)
                .open("NUL")
                .map(|sink| Box::new(sink) as Box<dyn Write>);
        }
        #[cfg(not(windows))]
        {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "owned Windows console output is unavailable",
            ));
        }
    }
    match crate::console_output::open_owned_console() {
        Ok(console) => Ok(Box::new(console)),
        Err(error) => Err(error),
    }
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
        ActivityRender, CleanupObserverAction, LeaseTransition, PublishedWorkerStartup,
        SESSIONS_SCHEMA_VERSION, STATIC_ATTENTION_LEASE_TTL_MS, SessionWorkspaceObservability,
        TARGET_FRAME_INTERVAL_MS, WorkerKey, WorkerPresentation, WorkerProcessLiveness,
        already_active_worker_render, cleanup_identity_recheck_due, cleanup_observer_action,
        cleanup_observer_poll_ms, command_output_with_timeout, inspect_activity_leases_read_only,
        inspect_sessions_read_only, next_animation_frame_deadline, normalized_windows_path,
        record_provider_session_observation, start_published_worker, system_powershell_path,
        tasklist_output_reports_absence,
    };
    use crate::{
        core::{Attention, Health, Phase},
        presentation::{
            PresentationAction, PresentationPolicy, SemanticPresentationInput, TitleStatus,
            WindowsTerminalCapabilities, WindowsTerminalRenderer,
        },
        providers::visual_identity::ProviderVisualIdentity,
        settings::{
            ActivityMode, PresentationSettings, PresentationTheme, SpinnerPreset, TabColorMode,
            TitleMode,
        },
        worker_runtime::WorkerRuntimeStore,
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

    fn rendered_worker_title(presentation: &WorkerPresentation) -> String {
        let (phase, attention) = presentation
            .semantic_input()
            .expect("fixture presentation has a supported semantic state");
        let action = presentation.presentation_action(phase, attention);
        let state = match action {
            PresentationAction::Apply(state) | PresentationAction::Reset(state) => state,
        };
        let renderer = WindowsTerminalRenderer::with_settings(
            WindowsTerminalCapabilities::new(false),
            PresentationSettings::new(
                TitleMode::TabBeacon,
                TabColorMode::Off,
                ActivityMode::TitleSpinner,
                SpinnerPreset::Braille,
                PresentationTheme::MutedDark,
            ),
        );
        String::from_utf8(renderer.render_title_spinner_frame(&state, 0))
            .expect("worker title frame is UTF-8")
    }

    fn reconstructed_worker_state(
        presentation: &WorkerPresentation,
    ) -> crate::presentation::VisualState {
        let (phase, attention) = presentation
            .semantic_input()
            .expect("fixture presentation has a supported semantic state");
        match presentation.presentation_action(phase, attention) {
            PresentationAction::Apply(state) | PresentationAction::Reset(state) => state,
        }
    }

    fn source_action(
        phase: Phase,
        attention: Attention,
        provider_visual_identity: Option<ProviderVisualIdentity>,
    ) -> PresentationAction {
        PresentationPolicy::resolve(
            SemanticPresentationInput::new_with_provider_visual_identity(
                phase,
                attention,
                Health::Normal,
                "OWH",
                provider_visual_identity,
            ),
        )
    }

    fn worker_settings() -> PresentationSettings {
        PresentationSettings::new(
            TitleMode::TabBeacon,
            TabColorMode::Off,
            ActivityMode::TitleSpinner,
            SpinnerPreset::Braille,
            PresentationTheme::MutedDark,
        )
    }

    fn worker_from_source_action(
        provider: &str,
        action: &PresentationAction,
    ) -> WorkerPresentation {
        WorkerPresentation::from_action(
            provider,
            "OWH",
            action,
            worker_settings(),
            SessionWorkspaceObservability::default(),
        )
        .expect("source action needs a worker lease")
    }

    #[test]
    fn worker_reconstruction_retains_codex_provider_visibility() {
        let source = source_action(
            Phase::Working,
            Attention::None,
            Some(ProviderVisualIdentity::codex()),
        );
        let presentation = worker_from_source_action("codex", &source);

        assert!(presentation.provider_identity_visible);
        assert!(
            rendered_worker_title(&presentation).contains("Codex"),
            "the worker must reconstruct the originating provider identity"
        );
    }

    #[test]
    fn static_worker_reconstruction_retains_codex_provider_visibility() {
        for (attention, expected_status) in [
            (Attention::ResultReady, TitleStatus::ResultReady),
            (Attention::Approval, TitleStatus::Approval),
        ] {
            let source = source_action(
                Phase::WaitingUser,
                attention,
                Some(ProviderVisualIdentity::codex()),
            );
            let presentation = worker_from_source_action("codex", &source);
            let state = reconstructed_worker_state(&presentation);

            assert!(presentation.provider_identity_visible);
            assert_eq!(state.title_status(), expected_status);
            assert!(rendered_worker_title(&presentation).contains("Codex"));
        }
    }

    #[test]
    fn worker_reconstruction_does_not_infer_identity_when_provider_badge_is_off() {
        let source = source_action(Phase::Working, Attention::None, None);
        let presentation = worker_from_source_action("codex", &source);

        assert!(!presentation.provider_identity_visible);
        assert!(!rendered_worker_title(&presentation).contains("Codex"));
    }

    #[test]
    fn legacy_worker_lease_defaults_provider_identity_visibility_conservatively() {
        let source = source_action(
            Phase::Working,
            Attention::None,
            Some(ProviderVisualIdentity::codex()),
        );
        let presentation = worker_from_source_action("codex", &source);
        let mut legacy_value = serde_json::to_value(presentation).expect("fixture serializes");
        legacy_value
            .as_object_mut()
            .expect("serialized worker presentation is an object")
            .remove("provider_identity_visible");
        let legacy: WorkerPresentation =
            serde_json::from_value(legacy_value).expect("legacy worker presentation deserializes");

        assert!(!legacy.provider_identity_visible);
        assert!(!rendered_worker_title(&legacy).contains("Codex"));
    }

    #[test]
    fn worker_provider_transition_keeps_runtime_and_workspace_independent() {
        let codex = worker_from_source_action(
            "codex",
            &source_action(
                Phase::WaitingUser,
                Attention::ResultReady,
                Some(ProviderVisualIdentity::codex()),
            ),
        );
        let agy = worker_from_source_action(
            "agy",
            &source_action(
                Phase::WaitingUser,
                Attention::ResultReady,
                Some(ProviderVisualIdentity::agy()),
            ),
        );
        let codex_state = reconstructed_worker_state(&codex);
        let agy_state = reconstructed_worker_state(&agy);

        assert_eq!(codex_state.title_status(), agy_state.title_status());
        assert_eq!(codex_state.tab_color(), agy_state.tab_color());
        assert_eq!(codex_state.progress(), agy_state.progress());
        assert_eq!(codex_state.workspace_alias().as_str(), "OWH");
        assert_eq!(agy_state.workspace_alias().as_str(), "OWH");
        assert!(rendered_worker_title(&codex).contains("Codex"));
        assert!(rendered_worker_title(&agy).contains("Agy"));
    }

    #[test]
    fn v03_worker_uses_the_normative_hundred_millisecond_interval() {
        assert_eq!(TARGET_FRAME_INTERVAL_MS, 100);
    }

    #[test]
    fn cleanup_observer_poll_is_bounded_and_session_staggered() {
        let low = cleanup_observer_poll_ms(&digest('0'));
        let high = cleanup_observer_poll_ms(&digest('f'));
        assert!(low >= 4_000);
        assert!(high <= 8_000);
        assert!(high > low, "distinct session digests must not herd polls");
    }

    #[test]
    fn cleanup_observer_native_presence_and_identity_cadence_are_explicit() {
        assert!(tasklist_output_reports_absence(
            "INFO: No tasks are running which match the specified criteria."
        ));
        assert!(!tasklist_output_reports_absence(
            "\"tabbeacon.exe\",\"52080\",\"Console\",\"1\",\"16,384 K\""
        ));
        assert!(cleanup_identity_recheck_due(None, 1_000));
        assert!(!cleanup_identity_recheck_due(Some(1_000), 30_999));
        assert!(cleanup_identity_recheck_due(Some(1_000), 31_000));
    }

    #[test]
    fn executable_path_normalization_removes_the_windows_extended_prefix() {
        assert_eq!(
            normalized_windows_path(r"\\?\C:\Build\TabBeacon\tabbeacon.exe"),
            "c:/build/tabbeacon/tabbeacon.exe"
        );
        assert_eq!(
            normalized_windows_path(r"C:\Build\TabBeacon\tabbeacon.exe"),
            "c:/build/tabbeacon/tabbeacon.exe"
        );
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
        active.expires_unix_ms = 4_000;
        store.write(&active).expect("active fixture updates");
        let mut stale = store
            .load(stale_key.digest())
            .expect("stale fixture reads")
            .expect("stale fixture exists");
        stale.expires_unix_ms = 2_000;
        store.write(&stale).expect("stale fixture updates");
        fs::write(store.lease_path(&digest('f')), b"not a lease").expect("invalid fixture writes");

        let diagnostics = inspect_activity_leases_read_only(&root.0, 3_000);

        assert_eq!(diagnostics.health(), ActivityLeaseHealth::Warning);
        assert_eq!(diagnostics.active_leases(), 1);
        assert_eq!(diagnostics.stale_leases(), 1);
        assert_eq!(diagnostics.invalid_leases(), 1);
    }

    #[test]
    fn sessions_view_preserves_concurrent_rows_and_exposes_only_safe_projection() {
        let root = TestRoot::new("sessions-safe-projection");
        let store = ActivityLeaseStore::new(&root.0);
        let first_key = key(1, 'a', 'c');
        let second_key = key(1, 'e', 'f');
        let owner = digest('d');
        store
            .publish_active(
                &first_key,
                1,
                &owner,
                &WorkerPresentation::working("OWH", SpinnerPreset::Braille),
                95_000,
            )
            .expect("first concurrent lease publishes");
        store
            .publish_active(
                &second_key,
                1,
                &owner,
                &WorkerPresentation::approval("OWH", SpinnerPreset::Braille),
                40_000,
            )
            .expect("second concurrent lease publishes");
        let mut stale = store
            .load(second_key.digest())
            .expect("second lease reads")
            .expect("second lease exists");
        stale.expires_unix_ms = 99_999;
        store.write(&stale).expect("stale fixture updates");
        let invalid_path = store.lease_path(&digest('9'));
        fs::write(
            &invalid_path,
            br#"{"session_id":"session-secret","prompt":"prompt-secret"}"#,
        )
        .expect("private invalid fixture writes");

        let first_before = fs::read(store.lease_path(first_key.digest())).expect("first snapshot");
        let second_before =
            fs::read(store.lease_path(second_key.digest())).expect("second snapshot");
        let invalid_before = fs::read(&invalid_path).expect("invalid snapshot");
        let overview = inspect_sessions_read_only(&root.0, 100_000);

        assert_eq!(overview.active_sessions, 1);
        assert_eq!(overview.stale_sessions, 1);
        assert_eq!(overview.invalid_leases, 1);
        assert_eq!(overview.sessions.len(), 2, "concurrent rows stay isolated");
        assert_eq!(SESSIONS_SCHEMA_VERSION, 2);
        assert_eq!(overview.sessions[0].workspace_alias, "OWH");
        assert_eq!(overview.sessions[0].provider, "codex");
        assert_ne!(
            overview.sessions[0].semantic_state,
            overview.sessions[1].semantic_state
        );
        assert!(overview.read_only);
        assert!(!overview.boundaries.raw_native_session_ids);
        assert!(!overview.boundaries.prompt_content);
        assert!(!overview.boundaries.remote_control);

        let serialized = serde_json::to_string(&overview).expect("sessions view serializes");
        for forbidden in [
            "session-secret",
            "prompt-secret",
            &digest('a'),
            &digest('e'),
            &digest('c'),
            &digest('f'),
            &owner,
        ] {
            assert!(
                !serialized.contains(forbidden),
                "sessions view leaked {forbidden}"
            );
        }
        assert_eq!(
            fs::read(store.lease_path(first_key.digest())).expect("first rereads"),
            first_before
        );
        assert_eq!(
            fs::read(store.lease_path(second_key.digest())).expect("second rereads"),
            second_before
        );
        assert_eq!(
            fs::read(&invalid_path).expect("invalid rereads"),
            invalid_before
        );
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
    fn admitted_agy_provider_is_visible_without_native_identity_content() {
        let root = TestRoot::new("agy-provider");
        record_provider_session_observation(
            &root.0,
            &digest('a'),
            "agy",
            "AGY",
            "working",
            1_000,
            SessionWorkspaceObservability {
                root_binding_stable: true,
                workspace_mismatch_observed: false,
                active_subagents: 0,
                background_tasks: None,
            },
        )
        .expect("Agy observation publishes");
        let overview = inspect_sessions_read_only(&root.0, 1_001);
        assert_eq!(overview.sessions.len(), 1);
        assert_eq!(overview.sessions[0].provider, "agy");
        assert_eq!(overview.sessions[0].workspace_alias, "AGY");
        let serialized = serde_json::to_string(&overview).expect("sessions serialize");
        assert!(!serialized.contains(&digest('a')));
    }

    #[test]
    fn agy_ready_updates_its_own_row_and_foreign_provider_cannot_cross_namespace() {
        let root = TestRoot::new("agy-provider-namespace");
        let session = digest('b');
        let observability = SessionWorkspaceObservability {
            root_binding_stable: true,
            workspace_mismatch_observed: false,
            active_subagents: 0,
            background_tasks: None,
        };
        record_provider_session_observation(
            &root.0,
            &session,
            "agy",
            "AGY",
            "working",
            1_000,
            observability.clone(),
        )
        .expect("working observation publishes");
        assert!(
            record_provider_session_observation(
                &root.0,
                &session,
                "codex",
                "CODEX",
                "working",
                1_001,
                observability.clone(),
            )
            .is_err(),
            "Codex cannot write the Agy-only provider-session namespace"
        );
        record_provider_session_observation(
            &root.0,
            &session,
            "agy",
            "AGY",
            "ready",
            1_002,
            observability,
        )
        .expect("ready observation updates the same Agy row");

        let overview = inspect_sessions_read_only(&root.0, 1_003);
        assert_eq!(overview.sessions.len(), 1);
        assert_eq!(overview.sessions[0].provider, "agy");
        assert_eq!(overview.sessions[0].semantic_state, "ready");
        assert_eq!(overview.sessions[0].workspace_alias, "AGY");
    }

    #[test]
    fn attention_titles_replace_the_animated_worker_without_overlap() {
        let root = TestRoot::new("attention-title-owner");
        let store = ActivityLeaseStore::new(&root.0);
        let key = key(7, 'a', 'c');
        let owner = digest('d');
        let working = presentation();
        let result = WorkerPresentation::result_ready("OWH", SpinnerPreset::Braille);
        let approval = WorkerPresentation::approval("OWH", SpinnerPreset::Braille);

        let LeaseTransition::Published { lease: initial, .. } = store
            .publish_active(&key, 10, &owner, &working, 1_000)
            .expect("animated worker publishes")
        else {
            panic!("first title owner must publish");
        };
        let LeaseTransition::Published {
            lease: result_lease,
            predecessor: Some(result_predecessor),
        } = store
            .publish_active(&key, 11, &owner, &result, 1_100)
            .expect("result title replaces animation")
        else {
            panic!("result title must replace the animated owner");
        };
        assert_eq!(result_predecessor, initial.ownership());
        assert_eq!(result_lease.presentation.as_ref(), Some(&result));
        assert_eq!(
            result_lease
                .expires_unix_ms
                .saturating_sub(result_lease.updated_unix_ms),
            STATIC_ATTENTION_LEASE_TTL_MS
        );
        assert!(
            store
                .load_worker_lease(
                    &result_lease.ownership(),
                    &digest('c'),
                    result_lease.expires_unix_ms.saturating_add(1),
                )
                .is_none()
        );
        assert_eq!(
            result.semantic_input(),
            Some((Phase::WaitingUser, Attention::ResultReady))
        );

        let LeaseTransition::Published {
            lease: approval_lease,
            predecessor: Some(approval_predecessor),
        } = store
            .publish_active(&key, 12, &owner, &approval, 1_200)
            .expect("approval title replaces result")
        else {
            panic!("approval title must replace the result owner");
        };
        assert_eq!(approval_predecessor, result_lease.ownership());
        assert_eq!(approval_lease.presentation.as_ref(), Some(&approval));
        assert_eq!(
            approval_lease
                .expires_unix_ms
                .saturating_sub(approval_lease.updated_unix_ms),
            STATIC_ATTENTION_LEASE_TTL_MS
        );
        assert_eq!(
            approval.semantic_input(),
            Some((Phase::WaitingUser, Attention::Approval))
        );
        assert!(matches!(
            store.publish_stopped(&key, 13, &owner, 1_300),
            Ok(LeaseTransition::Stopped {
                predecessor: Some(predecessor)
            }) if predecessor == approval_lease.ownership()
        ));
        let stopped = store
            .load(key.digest())
            .expect("stopped lease reads")
            .expect("stopped lease exists");
        assert!(!stopped.active);
    }

    #[test]
    fn animated_worker_stops_for_static_native_and_off_settings() {
        let root = TestRoot::new("activity-mode-transition");
        let store = ActivityLeaseStore::new(&root.0);
        let owner = digest('d');
        let terminal_binding = digest('c');
        let coordinator = ActivityCoordinator {
            store: store.clone(),
            execution: ActivityExecution::System {
                executable: root.0.join("unused.exe"),
                owner_sha256: owner.clone(),
                terminal_binding_sha256: terminal_binding,
            },
        };
        let action = PresentationPolicy::resolve(SemanticPresentationInput::new(
            Phase::Working,
            Attention::None,
            Health::Normal,
            "OWH",
        ));
        let cases = [
            (
                'a',
                PresentationSettings::new(
                    TitleMode::TabBeacon,
                    TabColorMode::Off,
                    ActivityMode::TitleIndicator,
                    SpinnerPreset::Braille,
                    PresentationTheme::MutedDark,
                ),
            ),
            (
                'e',
                PresentationSettings::new(
                    TitleMode::Native,
                    TabColorMode::Native,
                    ActivityMode::Native,
                    SpinnerPreset::Braille,
                    PresentationTheme::MutedDark,
                ),
            ),
            (
                'f',
                PresentationSettings::new(
                    TitleMode::Off,
                    TabColorMode::Off,
                    ActivityMode::Off,
                    SpinnerPreset::Braille,
                    PresentationTheme::MutedDark,
                ),
            ),
        ];

        for (session, settings) in cases {
            let worker = key(1, session, 'c');
            let LeaseTransition::Published { .. } = store
                .publish_active(&worker, 10, &owner, &presentation(), 1_000)
                .expect("animated worker publishes")
            else {
                panic!("animated worker must be active before a mode transition");
            };
            assert_eq!(
                coordinator.reconcile(
                    &digest(session),
                    Some(&digest('b')),
                    1,
                    11,
                    "OWH",
                    &action,
                    settings,
                ),
                ActivityRender::Full,
                "mode transition for session {session} retires the animation without waiting"
            );
            let stopped = store
                .load(worker.digest())
                .expect("stopped lease reads")
                .expect("stopped lease exists");
            assert!(
                !stopped.active,
                "mode transition for {session} leaves no worker"
            );
        }
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
    fn active_runtime_image_refresh_skips_republication_but_preserves_ordering() {
        let root = TestRoot::new("runtime-refresh");
        let store = ActivityLeaseStore::new(&root.0);
        let key = key(2, 'a', 'c');
        let owner = digest('d');
        let image = digest('e');
        assert!(matches!(
            store.publish_active_with_runtime_image(
                &key,
                10,
                &owner,
                Some(&image),
                &presentation(),
                1_000,
            ),
            Ok(LeaseTransition::Published { .. })
        ));
        assert_eq!(
            store
                .refresh_runtime_backed_active_if_current(&key, 11, &owner, &presentation(), 1_100,)
                .expect("runtime-backed active lease refreshes"),
            Some(LeaseTransition::AlreadyActive)
        );
        let lease = store
            .load(key.digest())
            .expect("refreshed lease reads")
            .expect("refreshed lease exists");
        assert_eq!(lease.event_sequence, 11);
        assert_eq!(lease.runtime_image_sha256.as_deref(), Some(image.as_str()));
        assert_eq!(
            store
                .refresh_runtime_backed_active_if_current(&key, 10, &owner, &presentation(), 1_200,)
                .expect("delayed refresh classifies"),
            Some(LeaseTransition::Stale)
        );
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
    fn newer_turn_identifies_its_predecessor_without_waiting_for_exit() {
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
        assert_eq!(predecessor.owner_sha256, owner);
        let active = store
            .load(second.digest())
            .expect("successor lease reads")
            .expect("successor lease exists");
        assert!(active.active);
        assert_eq!(active.ownership(), lease.ownership());
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
    fn runtime_backed_lease_binds_the_image_and_supersedes_a_legacy_worker() {
        let root = TestRoot::new("runtime-image-binding");
        let store = ActivityLeaseStore::new(&root.0);
        let key = key(4, 'a', 'c');
        let owner = digest('d');
        let image = digest('e');
        store
            .publish_active(&key, 20, &owner, &presentation(), 1_000)
            .expect("legacy lease publishes for migration coverage");
        let LeaseTransition::Published {
            lease,
            predecessor: Some(_),
        } = store
            .publish_active_with_runtime_image(
                &key,
                21,
                &owner,
                Some(&image),
                &presentation(),
                1_100,
            )
            .expect("runtime-backed worker supersedes the legacy worker")
        else {
            panic!("runtime image binding must replace a legacy active lease");
        };
        assert_eq!(lease.runtime_image_sha256.as_deref(), Some(image.as_str()));
        let (active_images, ownership_proven) = store.active_runtime_images_for_gc();
        assert!(ownership_proven);
        assert_eq!(active_images, std::collections::BTreeSet::from([image]));
    }

    #[test]
    fn runtime_backed_successor_stays_active_while_predecessor_winds_down() {
        let root = TestRoot::new("runtime-successor-no-wait");
        fs::create_dir_all(&root.0).expect("isolated runtime root creates");
        let source = root.0.join("tabbeacon.exe");
        fs::write(&source, b"runtime-successor-no-wait").expect("runtime source writes");
        let store = ActivityLeaseStore::new(&root.0);
        let runtime_store = WorkerRuntimeStore::new(&root.0);
        let owner = digest('d');
        let first_key = key(1, 'a', 'c');
        let second_key = key(2, 'a', 'c');
        let (first_image, first_transition) = store
            .publish_runtime_backed_active(
                &runtime_store,
                &source,
                &first_key,
                1,
                &owner,
                &presentation(),
                1_000,
            )
            .expect("first runtime-backed lease publishes");
        let LeaseTransition::Published {
            lease: first_lease,
            predecessor: None,
        } = first_transition
        else {
            panic!("first runtime-backed lease has no predecessor");
        };

        let result = WorkerPresentation::result_ready("OWH", SpinnerPreset::Braille);
        let (successor_image, successor_transition) = store
            .publish_runtime_backed_active(
                &runtime_store,
                &source,
                &second_key,
                2,
                &owner,
                &result,
                1_100,
            )
            .expect("successor runtime-backed lease publishes without an exit receipt");
        let LeaseTransition::Published {
            lease: successor_lease,
            predecessor: Some(predecessor),
        } = successor_transition
        else {
            panic!("successor must identify the winding-down predecessor");
        };

        assert_eq!(predecessor, first_lease.ownership());
        let current = store
            .load(second_key.digest())
            .expect("successor lease reads")
            .expect("successor lease exists");
        assert!(
            current.active,
            "successor remains authoritative without waiting"
        );
        assert_eq!(
            current.runtime_image_sha256.as_deref(),
            Some(successor_image.content_sha256.as_str())
        );
        assert!(
            store
                .load_worker_lease(
                    &successor_lease.ownership(),
                    &digest('c'),
                    successor_lease.updated_unix_ms,
                )
                .is_some(),
            "successor lease continues authorizing its runtime-backed worker"
        );
        assert_eq!(first_image.content_sha256, successor_image.content_sha256);
    }

    #[test]
    fn repeated_supersession_does_not_accumulate_exit_receipts() {
        let root = TestRoot::new("supersession-without-exit-receipts");
        let store = ActivityLeaseStore::new(&root.0);
        let owner = digest('d');
        for generation in 1..=16 {
            let worker = key(generation, 'a', 'c');
            assert!(matches!(
                store.publish_active(
                    &worker,
                    generation,
                    &owner,
                    &presentation(),
                    1_000 + generation,
                ),
                Ok(LeaseTransition::Published { .. })
            ));
        }
        let exit_receipt_count = fs::read_dir(&store.directory)
            .expect("activity state reads")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("exit-"))
            .count();
        assert_eq!(exit_receipt_count, 0);
    }

    #[test]
    fn legacy_active_lease_blocks_runtime_image_collection_proof() {
        let root = TestRoot::new("legacy-runtime-image-retention");
        let store = ActivityLeaseStore::new(&root.0);
        let key = key(1, 'a', 'c');
        store
            .publish_active(&key, 1, &digest('d'), &presentation(), 1_000)
            .expect("legacy active lease publishes");
        let (active_images, ownership_proven) = store.active_runtime_images_for_gc();
        assert!(active_images.is_empty());
        assert!(!ownership_proven);
    }

    #[test]
    fn runtime_publication_and_collection_share_the_activity_lease_lock() {
        let root = TestRoot::new("runtime-image-lock");
        fs::create_dir_all(&root.0).expect("isolated runtime root creates");
        let source = root.0.join("installed.exe");
        fs::write(&source, b"runtime-image-lock-test").expect("runtime source writes");
        let store = ActivityLeaseStore::new(&root.0);
        let runtime_store = WorkerRuntimeStore::new(&root.0);
        let key = key(1, 'a', 'c');
        let owner = digest('d');
        let (image, transition) = store
            .publish_runtime_backed_active(
                &runtime_store,
                &source,
                &key,
                1,
                &owner,
                &presentation(),
                1_000,
            )
            .expect("runtime image and active lease publish atomically");
        assert!(matches!(transition, LeaseTransition::Published { .. }));
        assert!(image.executable.is_file());

        store.collect_unused_runtime_images(&runtime_store);
        assert!(
            image.executable.is_file(),
            "an image named by an active lease cannot be collected"
        );

        store
            .publish_stopped(&key, 2, &owner, 1_001)
            .expect("owned lease retires before collection");
        store.collect_unused_runtime_images(&runtime_store);
        assert!(
            !image.executable.exists(),
            "the old image becomes collectible only after lease retirement"
        );
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
    fn missing_worker_executable_falls_open_without_publishing_a_lease() {
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
            ActivityRender::UncoordinatedFull
        );
        let lease_key = key(1, 'a', 'c');
        assert!(
            store
                .load(lease_key.digest())
                .expect("fallback lease lookup succeeds")
                .is_none(),
            "runtime publication must finish before an active lease is written"
        );
    }

    #[test]
    fn published_worker_leaves_the_hook_owned_full_provider_title_before_animation() {
        let startup = start_published_worker(|| Ok(17));
        assert_eq!(startup, PublishedWorkerStartup::Started);
        assert_eq!(
            startup.hook_render(),
            ActivityRender::Full,
            "a spawned worker is not yet proof that it rendered a title frame"
        );

        let action = source_action(
            Phase::Working,
            Attention::None,
            Some(ProviderVisualIdentity::codex()),
        );
        let settings = worker_settings();
        let renderer = WindowsTerminalRenderer::with_settings(
            WindowsTerminalCapabilities::new(false),
            settings,
        );
        let title = String::from_utf8(renderer.render(&action)).expect("renderer output is UTF-8");
        assert!(title.contains("Codex"));
        assert!(title.contains("OWH"));
    }

    #[test]
    fn already_active_worker_keeps_its_animation_without_resetting_the_title() {
        assert_eq!(
            already_active_worker_render(),
            ActivityRender::WithoutTitle,
            "an already active worker continues its animation without resetting the first frame"
        );
    }

    #[test]
    fn worker_startup_failures_leave_the_hook_with_a_full_render() {
        let worker_spawn_failure = start_published_worker(|| {
            Err(std::io::Error::other(
                "owned worker fixture refuses to spawn",
            ))
        });
        assert_eq!(
            worker_spawn_failure,
            PublishedWorkerStartup::WorkerSpawnFailed
        );
        assert_eq!(worker_spawn_failure.hook_render(), ActivityRender::Full);

        assert_eq!(
            PublishedWorkerStartup::Started.hook_render(),
            ActivityRender::Full,
            "a later worker-side observer failure cannot retract the originating Hook frame"
        );
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
