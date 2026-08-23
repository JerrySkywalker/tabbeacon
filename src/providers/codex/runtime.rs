use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
    time::{Instant, SystemTime},
};

use sha2::{Digest, Sha256};

use crate::{
    activity::{ActivityCoordinator, ActivityRender},
    core::SessionReconciler,
    presentation::{
        PresentationPolicy, SemanticPresentationInput, WindowsTerminalCapabilities,
        WindowsTerminalRenderer,
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
    renderer: WindowsTerminalRenderer,
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
            renderer: WindowsTerminalRenderer::with_settings(
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

    /// Handles a hook using the platform state root and owned console output.
    ///
    /// This function is deliberately infallible to its caller. The internal
    /// hook CLI exits successfully for every returned outcome.
    #[must_use]
    pub fn dispatch_system(raw: &[u8]) -> HookDispatchOutcome {
        let mut timing = HookTimingCapture::from_environment();
        let outcome = Self::dispatch_system_with_timing(raw, &mut timing);
        timing.emit(outcome);
        outcome
    }

    fn dispatch_system_with_timing(
        raw: &[u8],
        timing: &mut HookTimingCapture,
    ) -> HookDispatchOutcome {
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
        runtime.dispatch_to_with_timing(raw, SystemTime::now(), &mut console, timing)
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
        self.dispatch_to_with_timing(raw, observed_at, sink, &mut timing)
    }

    #[allow(clippy::too_many_lines)]
    fn dispatch_to_with_timing(
        &self,
        raw: &[u8],
        observed_at: SystemTime,
        sink: &mut impl Write,
        timing: &mut HookTimingCapture,
    ) -> HookDispatchOutcome {
        let observed_at_unix_seconds = observed_at
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs());
        let started = Instant::now();
        let Ok(normalized) = CodexHookNormalizer.normalize(raw, observed_at) else {
            timing.record("normalization", started);
            return HookDispatchOutcome::DegradedInput;
        };
        timing.record("normalization", started);

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
                return HookDispatchOutcome::DegradedWorkspaceIdentity;
            }
            Err(AnchorSelectionError::Anchor) => {
                return HookDispatchOutcome::DegradedRootWorkspaceAnchor;
            }
        };
        timing.record("workspace_anchor", started);

        let started = Instant::now();
        let mut reconciler = SessionReconciler::default();
        let snapshot = reconciler.apply(normalized.evidence());
        // Hook input contains no current Codex release/profile evidence. Do
        // not infer admission from an older setup, an owned declaration, or
        // the normalizer's source profile: a newer CLI can keep delivering
        // shaped input after it has become unadmitted. The hook path therefore
        // uses an explicitly unknown registry and withholds an `always` badge
        // until a future runtime can carry a current, bounded probe result.
        let runtime_registry = ProviderRegistry::default();
        let provider_badge =
            runtime_registry.title_badge_for("codex", self.renderer.settings().provider_badge());
        let action = PresentationPolicy::resolve(
            SemanticPresentationInput::from_snapshot_with_provider_badge(
                &snapshot,
                selection.effective_alias().as_str(),
                provider_badge.as_deref(),
            ),
        );
        let title_workspace_alias = match &action {
            crate::presentation::PresentationAction::Apply(state)
            | crate::presentation::PresentationAction::Reset(state) => {
                state.workspace_alias().as_str()
            }
        };
        let render = self.activity.reconcile_with_workspace_observability(
            admitted.session_sha256(),
            admitted.turn_sha256(),
            admitted.generation(),
            admitted.event_sequence(),
            "codex",
            title_workspace_alias,
            &action,
            self.renderer.settings(),
            selection.workspace_observability(),
        );
        let bytes = match render {
            ActivityRender::UncoordinatedFull | ActivityRender::Full => {
                self.renderer.render(&action)
            }
            ActivityRender::WithoutTitle => self.renderer.render_without_title(&action),
            ActivityRender::Suppress => Vec::new(),
        };
        timing.record("presentation_and_activity", started);

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

fn open_owned_console() -> io::Result<crate::console_output::OwnedConsole> {
    crate::console_output::open_owned_console()
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
    destination: Option<PathBuf>,
}

impl HookTimingCapture {
    fn from_environment() -> Self {
        let destination = std::env::var_os("TABBEACON_HOOK_TIMING_FILE")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        Self {
            enabled: destination.is_some()
                || std::env::var_os("TABBEACON_HOOK_TIMING_CAPTURE")
                    .is_some_and(|value| value == "1"),
            started: Instant::now(),
            phases: Vec::new(),
            destination,
        }
    }

    fn disabled() -> Self {
        Self {
            enabled: false,
            started: Instant::now(),
            phases: Vec::new(),
            destination: None,
        }
    }

    fn record(&mut self, phase: &'static str, started: Instant) {
        if self.enabled {
            self.phases.push((phase, started.elapsed().as_millis()));
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
            "TABBEACON_HOOK_TIMING_V1 total_ms={} outcome={} phases={phases}",
            self.started.elapsed().as_millis(),
            hook_dispatch_outcome_name(outcome)
        );
        if let Some(destination) = &self.destination
            && write_timing_line_once(destination, &line).is_ok()
        {
            return;
        }
        eprintln!("{line}");
    }
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

#[cfg(test)]
mod tests {
    use std::{
        env, fs,
        path::Path,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };

    use serde_json::json;

    use crate::repo::WorkspaceIdentityResolver;

    use super::{CodexHookRuntime, HookDispatchOutcome, write_timing_line_once};

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
}
