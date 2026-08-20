use std::{
    io::{self, Write},
    path::PathBuf,
    time::SystemTime,
};

use sha2::{Digest, Sha256};

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
            CodexNormalization::IgnoreSubagent(context) => {
                // Explicit lifecycle events contribute only a bounded count;
                // subagent-attributed ordinary events deliberately retain no
                // extra data and cannot affect root anchoring.
                if context.event().is_subagent_lifecycle() {
                    let _ = self.root_workspace_anchors.observe_subagent(&context);
                }
                return HookDispatchOutcome::IgnoredSubagent;
            }
            CodexNormalization::UnsupportedEvent => {
                return HookDispatchOutcome::IgnoredUnsupported;
            }
        };
        let selection = match self.root_workspace_selection(normalized.context(), &admitted) {
            Ok(selection) => selection,
            Err(AnchorSelectionError::Workspace) => {
                return HookDispatchOutcome::DegradedWorkspaceIdentity;
            }
            Err(AnchorSelectionError::Anchor) => {
                return HookDispatchOutcome::DegradedRootWorkspaceAnchor;
            }
        };
        let mut reconciler = SessionReconciler::default();
        let snapshot = reconciler.apply(normalized.evidence());
        let action = PresentationPolicy::resolve(SemanticPresentationInput::from_snapshot(
            &snapshot,
            selection.effective_alias().as_str(),
        ));
        let render = self.activity.reconcile_with_workspace_observability(
            admitted.session_sha256(),
            admitted.turn_sha256(),
            admitted.generation(),
            admitted.event_sequence(),
            selection.effective_alias().as_str(),
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

    fn root_workspace_selection(
        &self,
        context: &super::CodexHookContext,
        admitted: &super::generation::AdmittedGeneration,
    ) -> Result<RootWorkspaceSelection, AnchorSelectionError> {
        let session_sha256 = admitted.session_sha256();
        match context.event() {
            super::CodexHookEvent::SessionStart => {
                let source = context
                    .session_start_source()
                    .map(RootWorkspaceBindingSource::from_session_start)
                    .ok_or(AnchorSelectionError::Anchor)?;
                self.bind_root_workspace(context, admitted, source)
            }
            super::CodexHookEvent::UserPromptSubmit => {
                let has_anchor = self
                    .root_workspace_anchors
                    .has_anchor(session_sha256)
                    .map_err(|_| AnchorSelectionError::Anchor)?;
                if !has_anchor {
                    let resolved = self
                        .identity_resolver
                        .resolve(context.cwd())
                        .map_err(|_| AnchorSelectionError::Workspace)?;
                    return self.bind_resolved_root_workspace(
                        &resolved,
                        admitted,
                        RootWorkspaceBindingSource::UserPromptFallback,
                    );
                }
                let observed_identity = self
                    .identity_resolver
                    .workspace_identity_sha256(context.cwd())
                    .map_err(|_| AnchorSelectionError::Workspace)?;
                self.root_workspace_anchors
                    .select_existing_or_observe_mismatch(session_sha256, &observed_identity)
                    .map_err(|_| AnchorSelectionError::Anchor)?
                    .ok_or(AnchorSelectionError::Anchor)
            }
            super::CodexHookEvent::SessionEnd => self
                .root_workspace_anchors
                .take_for_session_end(session_sha256)
                .map_err(|_| AnchorSelectionError::Anchor)?
                .ok_or(AnchorSelectionError::Workspace),
            super::CodexHookEvent::PreToolUse
            | super::CodexHookEvent::PostToolUse
            | super::CodexHookEvent::PermissionRequest
            | super::CodexHookEvent::Stop => {
                let observed_identity = self
                    .identity_resolver
                    .workspace_identity_sha256(context.cwd())
                    .map_err(|_| AnchorSelectionError::Workspace)?;
                self.root_workspace_anchors
                    .select_existing_or_observe_mismatch(session_sha256, &observed_identity)
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
        source: RootWorkspaceBindingSource,
    ) -> Result<RootWorkspaceSelection, AnchorSelectionError> {
        let resolved = self
            .identity_resolver
            .resolve(context.cwd())
            .map_err(|_| AnchorSelectionError::Workspace)?;
        self.bind_resolved_root_workspace(&resolved, admitted, source)
    }

    fn bind_resolved_root_workspace(
        &self,
        resolved: &crate::repo::ResolvedWorkspaceIdentity,
        admitted: &super::generation::AdmittedGeneration,
        source: RootWorkspaceBindingSource,
    ) -> Result<RootWorkspaceSelection, AnchorSelectionError> {
        let identity_sha256 = format!(
            "{:x}",
            Sha256::digest(resolved.identity.as_str().as_bytes())
        );
        self.root_workspace_anchors
            .bind(
                admitted.session_sha256(),
                admitted.generation(),
                &identity_sha256,
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
