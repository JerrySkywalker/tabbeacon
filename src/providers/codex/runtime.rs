use std::{
    fs::OpenOptions,
    io::{self, Write},
    path::PathBuf,
    time::SystemTime,
};

use crate::{
    core::{Attention, Phase, SessionReconciler},
    presentation::{
        PresentationPolicy, SemanticPresentationInput, WindowsTerminalCapabilities,
        WindowsTerminalRenderer,
    },
    repo::{RepositoryIdentityResolver, StableAliasRegistry},
};

use super::{CodexHookNormalizer, CodexNormalization};

/// Fail-open result for one internal hook invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookDispatchOutcome {
    /// Evidence traversed normalization, reconciliation, identity, and rendering.
    Applied,
    /// A compact start deliberately left the current presentation untouched.
    PreservedCurrentState,
    /// An unrecognized hook event was ignored for forward compatibility.
    IgnoredUnsupported,
    /// Invalid or incomplete input was contained without exposing raw content.
    DegradedInput,
    /// Offline repository identity was unavailable; Codex remains unaffected.
    DegradedRepositoryIdentity,
    /// The terminal output path was unavailable; Codex remains unaffected.
    DegradedPresentationOutput,
    /// No safe per-user `TabBeacon` state root was available.
    DegradedStateRoot,
}

/// One-shot Codex hook execution through the existing product layers.
#[derive(Debug, Clone)]
pub struct CodexHookRuntime {
    identity_resolver: RepositoryIdentityResolver,
    renderer: WindowsTerminalRenderer,
}

impl CodexHookRuntime {
    /// Creates a runtime using an injected state root and explicit renderer
    /// capability. Tests use this to avoid the owner's application data.
    #[must_use]
    pub fn new(state_root: impl Into<PathBuf>, frame_color_supported: bool) -> Self {
        Self {
            identity_resolver: RepositoryIdentityResolver::new(state_root),
            renderer: WindowsTerminalRenderer::new(WindowsTerminalCapabilities::new(
                frame_color_supported,
            )),
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
        let runtime = Self::new(state_root, frame_color_supported);
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
        let normalized = match normalized {
            CodexNormalization::Evidence(normalized) => normalized,
            CodexNormalization::PreserveCurrentState => {
                return HookDispatchOutcome::PreservedCurrentState;
            }
            CodexNormalization::UnsupportedEvent => {
                return HookDispatchOutcome::IgnoredUnsupported;
            }
        };
        let Ok(resolved) = self.identity_resolver.resolve(normalized.cwd()) else {
            return HookDispatchOutcome::DegradedRepositoryIdentity;
        };
        let mut reconciler = SessionReconciler::default();
        let snapshot = reconciler.apply(normalized.evidence());
        let title = format!(
            "{} {}",
            resolved.alias.as_str(),
            semantic_title_suffix(snapshot.phase(), snapshot.attention())
        );
        let action = PresentationPolicy::resolve(SemanticPresentationInput::from_snapshot(
            &snapshot, &title,
        ));
        let bytes = self.renderer.render(&action);
        if sink.write_all(&bytes).and_then(|()| sink.flush()).is_err() {
            return HookDispatchOutcome::DegradedPresentationOutput;
        }
        HookDispatchOutcome::Applied
    }
}

fn semantic_title_suffix(phase: Phase, attention: Attention) -> &'static str {
    match attention {
        Attention::Approval => "approval",
        Attention::ResultReady => "result-ready",
        Attention::Question => "question",
        Attention::None => match phase {
            Phase::Ready => "ready",
            Phase::Working => "working",
            Phase::WaitingUser => "waiting",
            Phase::Ended => "reset",
        },
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
