use std::{
    fs::OpenOptions,
    io::{self, Write},
    path::PathBuf,
    time::SystemTime,
};

use crate::{
    activity::{ActivityCoordinator, ActivityRender},
    core::SessionReconciler,
    presentation::{
        PresentationPolicy, SemanticPresentationInput, WindowsTerminalCapabilities,
        WindowsTerminalRenderer,
    },
    repo::{StableAliasRegistry, WorkspaceIdentityResolver},
    settings::{PresentationSettings, PresentationSettingsStore},
};

use super::{
    CodexHookNormalizer, CodexNormalization,
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
}

/// One-shot Codex hook execution through the existing product layers.
#[derive(Debug, Clone)]
pub struct CodexHookRuntime {
    identity_resolver: WorkspaceIdentityResolver,
    generation_store: CodexGenerationStore,
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
            renderer: WindowsTerminalRenderer::with_settings(
                WindowsTerminalCapabilities::new(frame_color_supported),
                settings,
            ),
            activity: ActivityCoordinator::disabled(&state_root),
        }
    }

    /// Handles a hook using the platform state root and owned console output.
    ///
    /// This function is deliberately infallible to its caller. The internal
    /// hook CLI exits successfully for every returned outcome.
    #[must_use]
    pub fn dispatch_system(raw: &[u8]) -> HookDispatchOutcome {
        let Ok(state_root) = StableAliasRegistry::default_state_root() else {
            return HookDispatchOutcome::DegradedStateRoot;
        };
        let frame_color_supported = std::env::var_os("WT_SESSION").is_some();
        let settings = PresentationSettingsStore::from_environment().map_or_else(
            |_| PresentationSettings::default(),
            |store| store.load_or_default(),
        );
        let mut runtime = Self::with_settings(&state_root, frame_color_supported, settings);
        runtime.activity = ActivityCoordinator::system(&state_root)
            .unwrap_or_else(|_| ActivityCoordinator::disabled(&state_root));
        let Ok(mut console) = open_owned_console() else {
            return HookDispatchOutcome::DegradedPresentationOutput;
        };
        runtime.dispatch_to(raw, SystemTime::now(), &mut console)
    }

    /// Handles one hook with deterministic time and an injected byte sink.
    ///
    /// This is the functional integration seam used by isolated tests. It
    /// returns a degraded disposition instead of propagating provider,
    /// repository, or output failures into Codex.
    #[must_use]
    pub fn dispatch_to(
        &self,
        raw: &[u8],
        observed_at: SystemTime,
        sink: &mut impl Write,
    ) -> HookDispatchOutcome {
        let Ok(normalized) = CodexHookNormalizer.normalize(raw, observed_at) else {
            return HookDispatchOutcome::DegradedInput;
        };
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
            CodexNormalization::IgnoreSubagent(_) => {
                return HookDispatchOutcome::IgnoredSubagent;
            }
            CodexNormalization::UnsupportedEvent => {
                return HookDispatchOutcome::IgnoredUnsupported;
            }
        };
        let Ok(resolved) = self.identity_resolver.resolve(normalized.cwd()) else {
            return HookDispatchOutcome::DegradedWorkspaceIdentity;
        };
        let mut reconciler = SessionReconciler::default();
        let snapshot = reconciler.apply(normalized.evidence());
        let action = PresentationPolicy::resolve(SemanticPresentationInput::from_snapshot(
            &snapshot,
            resolved.alias.as_str(),
        ));
        let render = self.activity.reconcile(
            admitted.session_sha256(),
            admitted.turn_sha256(),
            admitted.generation(),
            admitted.event_sequence(),
            resolved.alias.as_str(),
            &action,
            self.renderer.settings(),
        );
        let bytes = match render {
            ActivityRender::UncoordinatedFull | ActivityRender::Full => {
                self.renderer.render(&action)
            }
            ActivityRender::WithoutTitle => self.renderer.render_without_title(&action),
            ActivityRender::Suppress => Vec::new(),
        };
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
            return HookDispatchOutcome::DegradedPresentationOutput;
        }
        HookDispatchOutcome::Applied
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
