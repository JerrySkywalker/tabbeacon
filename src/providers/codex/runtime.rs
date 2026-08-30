use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use sha2::{Digest, Sha256};

use crate::{
    activity::{ActivityCoordinator, ActivityReconciliationTiming, ActivityRender},
    core::SessionReconciler,
    presentation::{
        PresentationPolicy, SemanticPresentationInput, TitleMarkBackend,
        WindowsTerminalCapabilities,
    },
    providers::registry::ProviderRegistry,
    repo::{StableAliasRegistry, WorkspaceIdentityResolver},
    settings::{PresentationSettings, PresentationSettingsStore},
};

use super::{
    CodexHookNormalizer, CodexNormalization,
    anchor::{RootWorkspaceAnchorStore, RootWorkspaceBindingSource, RootWorkspaceSelection},
    generation::{CodexGenerationStore, GenerationAdmission, RequestedHandling},
};

/// Private receipt path used only by the isolated hybrid `SessionEnd` probe.
/// The receipt contains fixed cleanup facts rather than terminal/title bytes or
/// Hook content.
pub(crate) const SESSION_END_PROBE_RECEIPT_ENV: &str = "TABBEACON_SESSION_END_PROBE_RECEIPT";
/// Required basename for the isolated hybrid `SessionEnd` receipt.
pub(crate) const SESSION_END_PROBE_RECEIPT_FILE: &str = "session-end-probe.json";

/// Fail-open result for one internal hook invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookDispatchOutcome {
    /// Evidence traversed normalization, reconciliation, identity, and rendering.
    Applied,
    /// A compact start deliberately left the current presentation untouched.
    PreservedCurrentState,
    /// A thread-spawned subagent event was isolated from root presentation state.
    IgnoredSubagent,
    /// An event from a superseded turn was rejected before terminal output.
    RejectedStaleGeneration,
    /// An unrecognized hook event was ignored for forward compatibility.
    IgnoredUnsupported,
    /// Invalid or incomplete input was contained without exposing raw content.
    DegradedInput,
    /// Offline repository identity was unavailable; Codex remains unaffected.
    ///
    /// Retained for API compatibility. The generalized runtime reports
    /// [`Self::DegradedWorkspaceIdentity`] instead.
    DegradedRepositoryIdentity,
    /// Offline workspace identity was unavailable; Codex remains unaffected.
    DegradedWorkspaceIdentity,
    /// The terminal output path was unavailable; Codex remains unaffected.
    DegradedPresentationOutput,
    /// No safe per-user `TabBeacon` state root was available.
    DegradedStateRoot,
    /// Durable turn generation state was unavailable or incompatible.
    DegradedGenerationState,
    /// Durable root-workspace anchor state was unavailable or incompatible.
    DegradedRootWorkspaceAnchor,
}

/// One-shot Codex hook execution through the existing product layers.
#[derive(Debug, Clone)]
pub struct CodexHookRuntime {
    identity_resolver: WorkspaceIdentityResolver,
    generation_store: CodexGenerationStore,
    root_workspace_anchors: RootWorkspaceAnchorStore,
    renderer: TitleMarkBackend,
    activity: ActivityCoordinator,
}

impl CodexHookRuntime {
    /// Creates a runtime using an injected state root and explicit renderer
    /// capability. Tests use this to avoid the owner's application data.
    #[must_use]
    pub fn new(state_root: impl Into<PathBuf>, frame_color_supported: bool) -> Self {
        Self::with_settings(
            state_root,
            frame_color_supported,
            PresentationSettings::new(
                crate::settings::TitleMode::TabBeacon,
                crate::settings::TabColorMode::TabBeacon,
                crate::settings::ActivityMode::WindowsTerminalRing,
                crate::settings::SpinnerPreset::Codex,
                crate::settings::PresentationTheme::Classic,
            ),
        )
    }

    /// Creates a runtime with explicit persistent presentation preferences.
    #[must_use]
    pub fn with_settings(
        state_root: impl Into<PathBuf>,
        frame_color_supported: bool,
        settings: PresentationSettings,
    ) -> Self {
        let state_root = state_root.into();
        Self {
            identity_resolver: WorkspaceIdentityResolver::new(&state_root),
            generation_store: CodexGenerationStore::new(&state_root),
            root_workspace_anchors: RootWorkspaceAnchorStore::new(&state_root),
            renderer: TitleMarkBackend::with_settings(
                WindowsTerminalCapabilities::new(frame_color_supported),
                settings,
            ),
            activity: ActivityCoordinator::disabled(&state_root),
        }
    }

    /// Creates the production runtime once for a long-lived transport.
    ///
    /// Unlike [`Self::with_settings`], this enables the same bounded activity
    /// coordination used by the command Hook runtime. The MCP server keeps
    /// this runtime for its Codex-owned stdio lifetime, avoiding repeat
    /// settings and state-root discovery on every Hook event.
    ///
    /// # Errors
    ///
    /// Returns a fail-open dispatch outcome when the platform state root cannot
    /// be established.
    pub fn from_system_environment() -> Result<Self, HookDispatchOutcome> {
        let state_root = StableAliasRegistry::default_state_root()
            .map_err(|_| HookDispatchOutcome::DegradedStateRoot)?;
        let frame_color_supported = std::env::var_os("WT_SESSION").is_some();
        let settings = PresentationSettingsStore::from_environment().map_or_else(
            |_| PresentationSettings::default(),
            |store| store.load_or_default(),
        );
        let mut runtime = Self::with_settings(&state_root, frame_color_supported, settings);
        runtime.activity = ActivityCoordinator::system(&state_root)
            .unwrap_or_else(|_| ActivityCoordinator::disabled(&state_root));
        Ok(runtime)
    }

    /// Whether the production runtime acquired a terminal-bound activity
    /// coordinator. The MCP probe uses this only for its isolated receipt.
    #[must_use]
    pub(crate) const fn activity_system_enabled(&self) -> bool {
        self.activity.system_enabled()
    }

    /// Handles a hook using the platform state root and owned console output.
    ///
    /// This function is deliberately infallible to its caller. The internal
    /// hook CLI exits successfully for every returned outcome.
    #[must_use]
    pub fn dispatch_system(raw: &[u8]) -> HookDispatchOutcome {
        let mut timing = HookTimingCapture::from_environment();
        let mut session_end_probe = SessionEndProbeCapture::from_environment();
        let outcome = Self::dispatch_system_with_timing(raw, &mut timing, &mut session_end_probe);
        timing.emit(outcome);
        outcome
    }

    fn dispatch_system_with_timing(
        raw: &[u8],
        timing: &mut HookTimingCapture,
        session_end_probe: &mut Option<SessionEndProbeCapture>,
    ) -> HookDispatchOutcome {
        let observed_at = SystemTime::now();
        let normalized = match Self::normalize_with_timing(raw, observed_at, timing) {
            Ok(normalized) => normalized,
            Err(outcome) => return outcome,
        };
        if let Some(outcome) = system_fast_path_outcome(&normalized) {
            return outcome;
        }

        let started = Instant::now();
        let Ok(state_root) = StableAliasRegistry::default_state_root() else {
            return HookDispatchOutcome::DegradedStateRoot;
        };
        timing.record("state_root", started);

        let started = Instant::now();
        let frame_color_supported = std::env::var_os("WT_SESSION").is_some();
        let settings = PresentationSettingsStore::from_environment().map_or_else(
            |_| PresentationSettings::default(),
            |store| store.load_or_default(),
        );
        let mut runtime = Self::with_settings(&state_root, frame_color_supported, settings);
        runtime.activity = ActivityCoordinator::system(&state_root)
            .unwrap_or_else(|_| ActivityCoordinator::disabled(&state_root));
        timing.record("runtime_initialization", started);

        let started = Instant::now();
        let Ok(mut console) = open_owned_console() else {
            return HookDispatchOutcome::DegradedPresentationOutput;
        };
        timing.record("console_open", started);
        runtime.dispatch_normalized_with_timing(
            normalized,
            observed_at,
            &mut console,
            timing,
            session_end_probe,
        )
    }

    /// Handles one hook with deterministic time and an injected byte sink.
    ///
    /// This is the functional integration seam used by isolated tests. It
    /// returns a degraded disposition instead of propagating provider,
    /// repository, or output failures into Codex.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn dispatch_to(
        &self,
        raw: &[u8],
        observed_at: SystemTime,
        sink: &mut impl Write,
    ) -> HookDispatchOutcome {
        let mut timing = HookTimingCapture::disabled();
        let mut session_end_probe = None;
        self.dispatch_to_with_timing(raw, observed_at, sink, &mut timing, &mut session_end_probe)
    }

    #[allow(clippy::too_many_lines)]
    fn dispatch_to_with_timing(
        &self,
        raw: &[u8],
        observed_at: SystemTime,
        sink: &mut impl Write,
        timing: &mut HookTimingCapture,
        session_end_probe: &mut Option<SessionEndProbeCapture>,
    ) -> HookDispatchOutcome {
        let normalized = match Self::normalize_with_timing(raw, observed_at, timing) {
            Ok(normalized) => normalized,
            Err(outcome) => return outcome,
        };
        self.dispatch_normalized_with_timing(
            normalized,
            observed_at,
            sink,
            timing,
            session_end_probe,
        )
    }

    fn normalize_with_timing(
        raw: &[u8],
        observed_at: SystemTime,
        timing: &mut HookTimingCapture,
    ) -> Result<CodexNormalization, HookDispatchOutcome> {
        let started = Instant::now();
        let Ok(normalized) = CodexHookNormalizer.normalize(raw, observed_at) else {
            timing.record("normalization", started);
            return Err(HookDispatchOutcome::DegradedInput);
        };
        timing.record("normalization", started);
        match &normalized {
            CodexNormalization::Evidence(normalized) => {
                timing.record_event(normalized.context().event());
            }
            CodexNormalization::PreserveCurrentState(context)
            | CodexNormalization::IgnoreSubagent(context) => {
                timing.record_event(context.event());
            }
            CodexNormalization::UnsupportedEvent => {}
        }
        Ok(normalized)
    }

    #[allow(clippy::too_many_lines)]
    fn dispatch_normalized_with_timing(
        &self,
        normalized: CodexNormalization,
        observed_at: SystemTime,
        sink: &mut impl Write,
        timing: &mut HookTimingCapture,
        session_end_probe: &mut Option<SessionEndProbeCapture>,
    ) -> HookDispatchOutcome {
        let observed_at_unix_seconds = observed_at
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs());
        let started = Instant::now();
        let (normalized, admitted) = match normalized {
            CodexNormalization::Evidence(normalized) => {
                match self
                    .generation_store
                    .admit(normalized.context(), RequestedHandling::Apply)
                {
                    Ok(GenerationAdmission::Apply(admitted)) => (normalized, admitted),
                    Ok(GenerationAdmission::RejectStale) => {
                        return HookDispatchOutcome::RejectedStaleGeneration;
                    }
                    Ok(GenerationAdmission::Preserve) => {
                        unreachable!("apply handling cannot produce preserve admission")
                    }
                    Err(_) => return HookDispatchOutcome::DegradedGenerationState,
                }
            }
            CodexNormalization::PreserveCurrentState(context) => {
                return match self
                    .generation_store
                    .admit(&context, RequestedHandling::Preserve)
                {
                    Ok(GenerationAdmission::Preserve) => HookDispatchOutcome::PreservedCurrentState,
                    Ok(GenerationAdmission::RejectStale) => {
                        HookDispatchOutcome::RejectedStaleGeneration
                    }
                    Ok(GenerationAdmission::Apply(_)) => {
                        unreachable!("preserve handling cannot produce apply admission")
                    }
                    Err(_) => HookDispatchOutcome::DegradedGenerationState,
                };
            }
            CodexNormalization::IgnoreSubagent(context) => {
                // Explicit lifecycle events contribute only a bounded count;
                // subagent-attributed ordinary events deliberately retain no
                // extra data and cannot affect root anchoring.
                if context.event().is_subagent_lifecycle() {
                    let _ = self
                        .root_workspace_anchors
                        .observe_subagent(&context, observed_at_unix_seconds);
                }
                return HookDispatchOutcome::IgnoredSubagent;
            }
            CodexNormalization::UnsupportedEvent => {
                return HookDispatchOutcome::IgnoredUnsupported;
            }
        };
        timing.record("generation_admission", started);

        let started = Instant::now();
        let selection = match self.root_workspace_selection(
            normalized.context(),
            &admitted,
            observed_at_unix_seconds,
        ) {
            Ok(selection) => selection,
            Err(AnchorSelectionError::Workspace) => {
                timing.record("workspace_anchor", started);
                return HookDispatchOutcome::DegradedWorkspaceIdentity;
            }
            Err(AnchorSelectionError::Anchor) => {
                timing.record("workspace_anchor", started);
                return HookDispatchOutcome::DegradedRootWorkspaceAnchor;
            }
        };
        timing.record("workspace_anchor", started);

        let presentation_started = Instant::now();
        let mut reconciler = SessionReconciler::default();
        let snapshot = reconciler.apply(normalized.evidence());
        // Hook input contains no current local capability evidence. Provider
        // identity is decorative rather than compatibility authority, so the
        // runtime can obtain fixed Codex text from the registry without an
        // expensive probe or any inference about setup/trust authority.
        let runtime_registry = ProviderRegistry::default();
        let provider_visual_identity = runtime_registry
            .visual_identity_for("codex", self.renderer.settings().provider_badge());
        let action = PresentationPolicy::resolve(
            SemanticPresentationInput::from_snapshot_with_provider_visual_identity(
                &snapshot,
                selection.effective_alias().as_str(),
                provider_visual_identity,
            ),
        );
        let title_workspace_alias = match &action {
            crate::presentation::PresentationAction::Apply(state)
            | crate::presentation::PresentationAction::Reset(state) => {
                state.workspace_alias().as_str()
            }
        };
        timing.record("presentation", presentation_started);

        let activity_started = Instant::now();
        let (render, activity_timing) = self.activity.reconcile_with_workspace_observability(
            admitted.session_sha256(),
            admitted.turn_sha256(),
            admitted.generation(),
            admitted.event_sequence(),
            "codex",
            title_workspace_alias,
            &action,
            self.renderer.settings(),
            selection.workspace_observability(),
            allows_persistent_activity_worker(normalized.context().event()),
        );
        timing.record("activity_reconciliation", activity_started);
        record_activity_reconciliation_timing(timing, activity_timing);

        let presentation_render_started = Instant::now();
        let bytes = match render {
            ActivityRender::UncoordinatedFull | ActivityRender::Full => {
                self.renderer.render(&action)
            }
            ActivityRender::WithoutTitle => self.renderer.render_without_title(&action),
            ActivityRender::Suppress => Vec::new(),
        };
        timing.record("presentation_render", presentation_render_started);

        let started = Instant::now();
        if self
            .activity
            .write_rendered(
                admitted.session_sha256(),
                admitted.turn_sha256(),
                admitted.generation(),
                admitted.event_sequence(),
                render,
                &bytes,
                sink,
            )
            .is_err()
        {
            timing.record("terminal_write", started);
            return HookDispatchOutcome::DegradedPresentationOutput;
        }
        timing.record("terminal_write", started);
        if normalized.context().event() == super::CodexHookEvent::SessionEnd
            && let Some(probe) = session_end_probe.as_mut()
        {
            probe.record(&bytes, matches!(render, ActivityRender::Full));
        }
        HookDispatchOutcome::Applied
    }

    fn root_workspace_selection(
        &self,
        context: &super::CodexHookContext,
        admitted: &super::generation::AdmittedGeneration,
        observed_at_unix_seconds: u64,
    ) -> Result<RootWorkspaceSelection, AnchorSelectionError> {
        let session_sha256 = admitted.session_sha256();
        match context.event() {
            super::CodexHookEvent::SessionStart => {
                let source = context
                    .session_start_source()
                    .map(RootWorkspaceBindingSource::from_session_start)
                    .ok_or(AnchorSelectionError::Anchor)?;
                self.bind_root_workspace(context, admitted, observed_at_unix_seconds, source)
            }
            super::CodexHookEvent::UserPromptSubmit => {
                let has_anchor = self
                    .root_workspace_anchors
                    .has_anchor(
                        session_sha256,
                        admitted.generation(),
                        observed_at_unix_seconds,
                    )
                    .map_err(|_| AnchorSelectionError::Anchor)?;
                if !has_anchor {
                    let resolved = self
                        .identity_resolver
                        .resolve(context.cwd())
                        .map_err(|_| AnchorSelectionError::Workspace)?;
                    return self.bind_resolved_root_workspace(
                        &resolved,
                        admitted,
                        observed_at_unix_seconds,
                        RootWorkspaceBindingSource::UserPromptFallback,
                    );
                }
                let observed_location =
                    WorkspaceIdentityResolver::fast_workspace_location_sha256(context.cwd()).ok();
                self.root_workspace_anchors
                    .select_existing_or_observe_fast_mismatch(
                        session_sha256,
                        admitted.generation(),
                        observed_at_unix_seconds,
                        observed_location.as_deref(),
                    )
                    .map_err(|_| AnchorSelectionError::Anchor)?
                    .ok_or(AnchorSelectionError::Anchor)
            }
            super::CodexHookEvent::SessionEnd => self
                .root_workspace_anchors
                .take_for_session_end(
                    session_sha256,
                    admitted.generation(),
                    observed_at_unix_seconds,
                )
                .map_err(|_| AnchorSelectionError::Anchor)?
                .ok_or(AnchorSelectionError::Workspace),
            super::CodexHookEvent::PreToolUse
            | super::CodexHookEvent::PostToolUse
            | super::CodexHookEvent::PermissionRequest
            | super::CodexHookEvent::Stop => {
                let observed_location =
                    WorkspaceIdentityResolver::fast_workspace_location_sha256(context.cwd()).ok();
                self.root_workspace_anchors
                    .select_existing_or_observe_fast_mismatch(
                        session_sha256,
                        admitted.generation(),
                        observed_at_unix_seconds,
                        observed_location.as_deref(),
                    )
                    .map_err(|_| AnchorSelectionError::Anchor)?
                    .ok_or(AnchorSelectionError::Workspace)
            }
            super::CodexHookEvent::PreCompact
            | super::CodexHookEvent::PostCompact
            | super::CodexHookEvent::SubagentStart
            | super::CodexHookEvent::SubagentStop => Err(AnchorSelectionError::Anchor),
        }
    }

    fn bind_root_workspace(
        &self,
        context: &super::CodexHookContext,
        admitted: &super::generation::AdmittedGeneration,
        observed_at_unix_seconds: u64,
        source: RootWorkspaceBindingSource,
    ) -> Result<RootWorkspaceSelection, AnchorSelectionError> {
        let resolved = self
            .identity_resolver
            .resolve(context.cwd())
            .map_err(|_| AnchorSelectionError::Workspace)?;
        self.bind_resolved_root_workspace(&resolved, admitted, observed_at_unix_seconds, source)
    }

    fn bind_resolved_root_workspace(
        &self,
        resolved: &crate::repo::ResolvedWorkspaceIdentity,
        admitted: &super::generation::AdmittedGeneration,
        observed_at_unix_seconds: u64,
        source: RootWorkspaceBindingSource,
    ) -> Result<RootWorkspaceSelection, AnchorSelectionError> {
        let identity_sha256 = format!(
            "{:x}",
            Sha256::digest(resolved.identity.as_str().as_bytes())
        );
        let workspace_location_sha256 =
            WorkspaceIdentityResolver::fast_workspace_location_sha256(&resolved.workspace_root)
                .map_err(|_| AnchorSelectionError::Workspace)?;
        self.root_workspace_anchors
            .bind(
                admitted.session_sha256(),
                admitted.generation(),
                observed_at_unix_seconds,
                &identity_sha256,
                &workspace_location_sha256,
                &resolved.effective_alias,
                source,
            )
            .map_err(|_| AnchorSelectionError::Anchor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnchorSelectionError {
    Workspace,
    Anchor,
}

/// Avoids platform initialization for Hook events that cannot mutate root state.
///
/// Subagent lifecycle events deliberately remain on the normal path so their
/// bounded anchor accounting continues to run.
fn system_fast_path_outcome(normalized: &CodexNormalization) -> Option<HookDispatchOutcome> {
    match normalized {
        CodexNormalization::IgnoreSubagent(context) if !context.event().is_subagent_lifecycle() => {
            Some(HookDispatchOutcome::IgnoredSubagent)
        }
        CodexNormalization::UnsupportedEvent => Some(HookDispatchOutcome::IgnoredUnsupported),
        CodexNormalization::Evidence(_)
        | CodexNormalization::PreserveCurrentState(_)
        | CodexNormalization::IgnoreSubagent(_) => None,
    }
}

/// `Stop` renders the final result frame synchronously, but must not spend the
/// same one-second Hook budget launching an optional persistent decoration
/// worker. The activity reconciler retires any predecessor lease on this path.
const fn allows_persistent_activity_worker(event: super::CodexHookEvent) -> bool {
    !matches!(event, super::CodexHookEvent::Stop)
}

fn open_owned_console() -> io::Result<Box<dyn Write>> {
    #[cfg(windows)]
    if activity_worker_probe_enabled() {
        return fs::OpenOptions::new()
            .write(true)
            .open("NUL")
            .map(|sink| Box::new(sink) as Box<dyn Write>);
    }
    crate::console_output::open_owned_console().map(|sink| Box::new(sink) as Box<dyn Write>)
}

#[cfg(windows)]
fn activity_worker_probe_enabled() -> bool {
    let Some(path) =
        std::env::var_os(crate::activity::ACTIVITY_WORKER_PROBE_RECEIPT_ENV).map(PathBuf::from)
    else {
        return false;
    };
    let Some(local_app_data) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) else {
        return false;
    };
    path.is_absolute()
        && path
            .file_name()
            .is_some_and(|name| name == crate::activity::ACTIVITY_WORKER_PROBE_RECEIPT_FILE)
        && path.parent() == Some(local_app_data.as_path())
}

/// Opt-in, content-free timing capture for the production Hook command.
///
/// The capture is disabled unless an explicitly named diagnostic environment
/// variable is set. It never records Hook input, session IDs, paths, aliases,
/// or any settings; the diagnostic line contains fixed phase names and elapsed
/// milliseconds only. Production Hooks therefore retain their silent contract.
struct HookTimingCapture {
    enabled: bool,
    started: Instant,
    phases: Vec<(&'static str, u128)>,
    destination: Option<HookTimingDestination>,
    event: Option<&'static str>,
}

/// The isolated timing harness can request one immutable receipt per Hook.
/// Ordinary diagnostic use remains a single exact file, which preserves the
/// existing no-overwrite contract.
enum HookTimingDestination {
    File(PathBuf),
    Directory(PathBuf),
}

/// Opt-in content-free receipt for the independent command `SessionEnd` path.
/// It is deliberately unavailable to ordinary Hook execution and records no
/// title text, OSC payload, path, session identity, or Hook input.
struct SessionEndProbeCapture {
    destination: PathBuf,
}

impl SessionEndProbeCapture {
    fn from_environment() -> Option<Self> {
        let destination = PathBuf::from(std::env::var_os(SESSION_END_PROBE_RECEIPT_ENV)?);
        let local_app_data = std::env::var_os("LOCALAPPDATA")?;
        (destination.is_absolute()
            && destination
                .file_name()
                .is_some_and(|name| name == SESSION_END_PROBE_RECEIPT_FILE)
            && destination.parent() == Some(std::path::Path::new(&local_app_data)))
        .then_some(Self { destination })
    }

    fn record(&self, bytes: &[u8], activity_lease_revoked: bool) {
        const PROGRESS_CLEAR: &[u8] = b"\x1b]9;4;0;0\x1b\\";
        const FRAME_COLOR_RESET: &[u8] = b"\x1b]104;264\x1b\\";
        let Ok(file) = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.destination)
        else {
            return;
        };
        let _ = serde_json::to_writer(
            file,
            &serde_json::json!({
                "schema": "tabbeacon-session-end-probe-v1",
                "generation_retired": true,
                "root_anchor_retired": true,
                "activity_lease_revoked": activity_lease_revoked,
                "progress_reset": bytes.windows(PROGRESS_CLEAR.len()).any(|window| window == PROGRESS_CLEAR),
                "frame_color_reset": bytes.windows(FRAME_COLOR_RESET.len()).any(|window| window == FRAME_COLOR_RESET),
                "windows_terminal_indexed_reset": bytes.windows(FRAME_COLOR_RESET.len()).any(|window| window == FRAME_COLOR_RESET),
            }),
        );
    }
}

impl HookTimingCapture {
    fn from_environment() -> Self {
        let destination = std::env::var_os("TABBEACON_HOOK_TIMING_FILE")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .map(HookTimingDestination::File)
            .or_else(|| {
                std::env::var_os("TABBEACON_HOOK_TIMING_DIRECTORY")
                    .filter(|value| !value.is_empty())
                    .map(PathBuf::from)
                    .filter(|path| path.is_absolute() && path.is_dir())
                    .map(HookTimingDestination::Directory)
            });
        Self {
            enabled: destination.is_some()
                || std::env::var_os("TABBEACON_HOOK_TIMING_CAPTURE")
                    .is_some_and(|value| value == "1"),
            started: Instant::now(),
            phases: Vec::new(),
            destination,
            event: None,
        }
    }

    fn disabled() -> Self {
        Self {
            enabled: false,
            started: Instant::now(),
            phases: Vec::new(),
            destination: None,
            event: None,
        }
    }

    fn record(&mut self, phase: &'static str, started: Instant) {
        if self.enabled {
            self.phases.push((phase, started.elapsed().as_millis()));
        }
    }

    fn record_elapsed_ms(&mut self, phase: &'static str, elapsed_ms: Option<u128>) {
        if self.enabled
            && let Some(elapsed_ms) = elapsed_ms
        {
            self.phases.push((phase, elapsed_ms));
        }
    }

    fn record_event(&mut self, event: super::CodexHookEvent) {
        if self.enabled {
            self.event = Some(hook_event_name(event));
        }
    }

    fn emit(&self, outcome: HookDispatchOutcome) {
        if !self.enabled {
            return;
        }
        let phases = self
            .phases
            .iter()
            .map(|(name, milliseconds)| format!("{name}={milliseconds}"))
            .collect::<Vec<_>>()
            .join(",");
        let line = format!(
            "TABBEACON_HOOK_TIMING_V2 event={} total_ms={} outcome={} phases={phases}",
            self.event.unwrap_or("unrecognized"),
            self.started.elapsed().as_millis(),
            hook_dispatch_outcome_name(outcome)
        );
        if let Some(destination) = &self.destination {
            let written = match destination {
                HookTimingDestination::File(destination) => {
                    write_timing_line_once(destination, &line)
                }
                HookTimingDestination::Directory(directory) => {
                    write_timing_line_in_directory(directory, &line)
                }
            };
            if written.is_ok() {
                return;
            }
        }
        eprintln!("{line}");
    }
}

fn record_activity_reconciliation_timing(
    timing: &mut HookTimingCapture,
    activity_timing: ActivityReconciliationTiming,
) {
    timing.record_elapsed_ms("activity_lease_refresh", activity_timing.lease_refresh);
    timing.record_elapsed_ms(
        "runtime_image_preparation",
        activity_timing.runtime_image_preparation,
    );
    timing.record_elapsed_ms("worker_launch", activity_timing.worker_launch);
    timing.record_elapsed_ms("stop_cleanup", activity_timing.stop_cleanup);
}

/// Writes opt-in timing evidence only to a previously absent final path.
/// Collisions, including an existing sentinel or final-path symlink, are
/// reported through the capture's stderr fallback rather than overwritten.
fn write_timing_line_once(destination: &std::path::Path, line: &str) -> io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)?;
    file.write_all(line.as_bytes())
}

/// Writes one per-process, collision-safe receipt into an explicitly supplied
/// existing directory.  The file name carries no Hook input or state: only a
/// process id, an instant-derived nonce, and a bounded collision attempt.
fn write_timing_line_in_directory(directory: &std::path::Path, line: &str) -> io::Result<()> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    for attempt in 0..8 {
        let destination = directory.join(format!(
            "tabbeacon-hook-timing-{}-{nonce}-{attempt}.txt",
            std::process::id()
        ));
        match write_timing_line_once(&destination, line) {
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            result => return result,
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "timing receipt collision budget exhausted",
    ))
}

const fn hook_dispatch_outcome_name(outcome: HookDispatchOutcome) -> &'static str {
    match outcome {
        HookDispatchOutcome::Applied => "applied",
        HookDispatchOutcome::PreservedCurrentState => "preserved_current_state",
        HookDispatchOutcome::IgnoredSubagent => "ignored_subagent",
        HookDispatchOutcome::RejectedStaleGeneration => "rejected_stale_generation",
        HookDispatchOutcome::IgnoredUnsupported => "ignored_unsupported",
        HookDispatchOutcome::DegradedInput => "degraded_input",
        HookDispatchOutcome::DegradedRepositoryIdentity => "degraded_repository_identity",
        HookDispatchOutcome::DegradedWorkspaceIdentity => "degraded_workspace_identity",
        HookDispatchOutcome::DegradedPresentationOutput => "degraded_presentation_output",
        HookDispatchOutcome::DegradedStateRoot => "degraded_state_root",
        HookDispatchOutcome::DegradedGenerationState => "degraded_generation_state",
        HookDispatchOutcome::DegradedRootWorkspaceAnchor => "degraded_root_workspace_anchor",
    }
}

const fn hook_event_name(event: super::CodexHookEvent) -> &'static str {
    match event {
        super::CodexHookEvent::SessionStart => "SessionStart",
        super::CodexHookEvent::SessionEnd => "SessionEnd",
        super::CodexHookEvent::UserPromptSubmit => "UserPromptSubmit",
        super::CodexHookEvent::PreToolUse => "PreToolUse",
        super::CodexHookEvent::PostToolUse => "PostToolUse",
        super::CodexHookEvent::PermissionRequest => "PermissionRequest",
        super::CodexHookEvent::Stop => "Stop",
        super::CodexHookEvent::PreCompact => "PreCompact",
        super::CodexHookEvent::PostCompact => "PostCompact",
        super::CodexHookEvent::SubagentStart => "SubagentStart",
        super::CodexHookEvent::SubagentStop => "SubagentStop",
    }
}

#[cfg(test)]
mod tests {
    use std::{
        env, fs,
        path::Path,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };

    use serde_json::json;

    use crate::{activity::ActivityReconciliationTiming, repo::WorkspaceIdentityResolver};

    use super::{
        CodexHookRuntime, HookDispatchOutcome, HookTimingCapture, HookTimingDestination,
        allows_persistent_activity_worker, record_activity_reconciliation_timing,
        system_fast_path_outcome, write_timing_line_in_directory, write_timing_line_once,
    };
    use crate::providers::codex::CodexHookNormalizer;

    fn test_root(label: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test clock follows Unix epoch")
            .as_nanos();
        let root = env::temp_dir().join(format!(
            "tabbeacon-fast-anchor-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("owned test root creates");
        root
    }

    fn initialize_repository(path: &Path) {
        for arguments in [
            vec!["init"],
            vec!["config", "user.email", "test@example.invalid"],
            vec!["config", "user.name", "TabBeacon Test"],
        ] {
            assert!(
                Command::new("git")
                    .args(arguments)
                    .current_dir(path)
                    .status()
                    .expect("Git command starts")
                    .success(),
                "Git setup command succeeds"
            );
        }
        fs::write(path.join("anchor.txt"), "anchor").expect("fixture file writes");
        for arguments in [vec!["add", "."], vec!["commit", "-m", "anchor"]] {
            assert!(
                Command::new("git")
                    .args(arguments)
                    .current_dir(path)
                    .status()
                    .expect("Git command starts")
                    .success(),
                "Git fixture commit succeeds"
            );
        }
    }

    #[test]
    fn anchored_normal_hook_remains_applied_when_git_is_unavailable() {
        let root = test_root("no-git-after-anchor");
        let repository = root.join("repository");
        let state = root.join("state");
        fs::create_dir_all(&repository).expect("repository directory creates");
        initialize_repository(&repository);
        let mut runtime = CodexHookRuntime::new(&state, true);
        let start = json!({
            "hook_event_name": "SessionStart",
            "session_id": "session-fast-anchor",
            "cwd": repository,
            "source": "startup"
        });
        assert_eq!(
            runtime.dispatch_to(
                &serde_json::to_vec(&start).expect("start serializes"),
                UNIX_EPOCH,
                &mut Vec::new()
            ),
            HookDispatchOutcome::Applied
        );

        runtime.identity_resolver = WorkspaceIdentityResolver::with_git_executable(
            &state,
            "tabbeacon-test-git-must-not-run.exe",
        );
        let normal = json!({
            "hook_event_name": "PostToolUse",
            "session_id": "session-fast-anchor",
            "turn_id": "turn-fast-anchor",
            "cwd": repository.join("nested"),
        });
        fs::create_dir_all(repository.join("nested")).expect("nested directory creates");
        assert_eq!(
            runtime.dispatch_to(
                &serde_json::to_vec(&normal).expect("normal event serializes"),
                UNIX_EPOCH,
                &mut Vec::new()
            ),
            HookDispatchOutcome::Applied,
            "an anchored ordinary Hook must not run Git discovery"
        );

        fs::remove_dir_all(root).expect("owned test root removes");
    }

    #[test]
    fn ordinary_subagent_tool_events_avoid_system_runtime_initialization() {
        for event in ["PreToolUse", "PostToolUse"] {
            let raw = json!({
                "hook_event_name": event,
                "session_id": "subagent-session",
                "turn_id": "subagent-turn",
                "agent_id": "thread-1",
                "agent_type": "thread",
                "cwd": "V:\\fixture"
            });
            let normalized = CodexHookNormalizer
                .normalize(raw.to_string().as_bytes(), SystemTime::UNIX_EPOCH)
                .expect("thread-attributed tool event normalizes");

            assert_eq!(
                system_fast_path_outcome(&normalized),
                Some(HookDispatchOutcome::IgnoredSubagent),
                "event={event}"
            );
        }
    }

    #[test]
    fn stop_defers_the_optional_persistent_activity_worker() {
        assert!(!allows_persistent_activity_worker(
            crate::providers::codex::CodexHookEvent::Stop
        ));
        assert!(allows_persistent_activity_worker(
            crate::providers::codex::CodexHookEvent::PostToolUse
        ));
    }

    #[test]
    fn timing_capture_refuses_to_overwrite_existing_evidence() {
        let root = test_root("timing-collision");
        let destination = root.join("timing.txt");
        fs::write(&destination, "sentinel").expect("sentinel writes");
        let error = write_timing_line_once(&destination, "new timing")
            .expect_err("existing timing evidence is never overwritten");
        assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(
            fs::read_to_string(&destination).expect("sentinel rereads"),
            "sentinel"
        );
        fs::remove_dir_all(root).expect("owned test root removes");
    }

    #[test]
    fn timing_directory_writes_distinct_content_free_receipts() {
        let root = test_root("timing-directory");
        write_timing_line_in_directory(&root, "first timing").expect("first timing receipt writes");
        write_timing_line_in_directory(&root, "second timing")
            .expect("second timing receipt writes");
        let mut receipts = fs::read_dir(&root)
            .expect("timing directory reads")
            .map(|entry| entry.expect("timing entry reads").path())
            .collect::<Vec<_>>();
        receipts.sort();
        assert_eq!(receipts.len(), 2);
        assert!(receipts.iter().all(|receipt| {
            receipt
                .file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("tabbeacon-hook-timing-"))
        }));
        assert_eq!(
            receipts
                .iter()
                .map(|receipt| fs::read_to_string(receipt).expect("timing receipt rereads"))
                .collect::<Vec<_>>(),
            vec!["first timing", "second timing"]
        );
        fs::remove_dir_all(root).expect("owned test root removes");
    }

    #[test]
    fn timing_capture_records_only_the_fixed_hook_event_name() {
        let root = test_root("timing-event");
        let capture = HookTimingCapture {
            enabled: true,
            started: std::time::Instant::now(),
            phases: Vec::new(),
            destination: Some(HookTimingDestination::Directory(root.clone())),
            event: Some("PostToolUse"),
        };
        capture.emit(HookDispatchOutcome::IgnoredSubagent);
        let receipt = fs::read_dir(&root)
            .expect("timing directory reads")
            .next()
            .expect("one timing receipt exists")
            .expect("timing receipt entry reads")
            .path();
        let line = fs::read_to_string(receipt).expect("timing receipt rereads");
        assert!(line.starts_with("TABBEACON_HOOK_TIMING_V2 event=PostToolUse total_ms="));
        assert!(line.contains("outcome=ignored_subagent"));
        fs::remove_dir_all(root).expect("owned test root removes");
    }

    #[test]
    fn activity_timing_records_only_fixed_content_free_phase_names() {
        let mut capture = HookTimingCapture {
            enabled: true,
            started: std::time::Instant::now(),
            phases: Vec::new(),
            destination: None,
            event: None,
        };
        record_activity_reconciliation_timing(
            &mut capture,
            ActivityReconciliationTiming {
                lease_refresh: Some(1),
                runtime_image_preparation: Some(2),
                worker_launch: Some(3),
                stop_cleanup: Some(4),
            },
        );
        assert_eq!(
            capture.phases,
            vec![
                ("activity_lease_refresh", 1),
                ("runtime_image_preparation", 2),
                ("worker_launch", 3),
                ("stop_cleanup", 4),
            ]
        );
    }
}
