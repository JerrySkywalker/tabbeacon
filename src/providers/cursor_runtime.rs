//! Fail-open Cursor Hook ingress with exact project-declaration and route checks.
//! Terminal color remains gated on a separate owned-output proof.

use std::{env, io, path::Path};

use sha2::{Digest, Sha256};

use super::{
    cursor::{CursorRouteStore, normalize_hook},
    cursor_integration::{CursorHookIntegration, CursorHookState},
};

/// A content-free outcome from the public Hook ingress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorDispatchOutcome {
    Ignored,
    RouteAdmitted,
}

/// Routes one structured event after exact local installation and terminal checks.
///
/// # Errors
///
/// Returns a content-free local ownership or checkpoint error; callers fail open.
pub fn dispatch_bounded(
    raw: &[u8],
    workspace: &Path,
    executable: &Path,
    state_root: &Path,
    terminal_identity: &str,
    console_openable: bool,
) -> io::Result<CursorDispatchOutcome> {
    if terminal_identity.is_empty()
        || terminal_identity.len() > 256
        || terminal_identity.chars().any(char::is_control)
        || !state_root.is_absolute()
        || !console_openable
    {
        return Ok(CursorDispatchOutcome::Ignored);
    }
    let integration = CursorHookIntegration::new(workspace, executable)?;
    if integration.check()? != CursorHookState::Installed {
        return Ok(CursorDispatchOutcome::Ignored);
    }
    let Some(event) = normalize_hook(raw)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid Cursor Hook event"))?
    else {
        return Ok(CursorDispatchOutcome::Ignored);
    };
    let mut hasher = Sha256::new();
    hasher.update(b"cursor-terminal-v1:");
    hasher.update(terminal_identity.as_bytes());
    let terminal_sha256 = format!("{:x}", hasher.finalize());
    let store = CursorRouteStore::new(state_root);
    if store.admit(&event, &terminal_sha256, &terminal_sha256, console_openable)? {
        Ok(CursorDispatchOutcome::RouteAdmitted)
    } else {
        Ok(CursorDispatchOutcome::Ignored)
    }
}

/// Runs the installed one-shot Hook path without retaining request content.
#[must_use]
pub fn dispatch_system(raw: &[u8]) -> CursorDispatchOutcome {
    let Some(state_root) = env::var_os("CURSOR_DATA_DIR")
        .filter(|value| !value.is_empty())
        .map(|value| Path::new(&value).join("tabbeacon"))
    else {
        return CursorDispatchOutcome::Ignored;
    };
    let Ok(workspace) = env::current_dir() else {
        return CursorDispatchOutcome::Ignored;
    };
    let Ok(executable) = env::current_exe() else {
        return CursorDispatchOutcome::Ignored;
    };
    let Ok(terminal_identity) = env::var("WT_SESSION") else {
        return CursorDispatchOutcome::Ignored;
    };
    let console_openable = crate::console_output::open_owned_console().is_ok();
    dispatch_bounded(
        raw,
        &workspace,
        &executable,
        &state_root,
        &terminal_identity,
        console_openable,
    )
    .unwrap_or(CursorDispatchOutcome::Ignored)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn real_two_turn_shape_rejects_superseded_stop_and_foreign_terminal() {
        let workspace = tempfile::tempdir().unwrap();
        let state = tempfile::tempdir().unwrap();
        let executable = workspace.path().join("tabbeacon.exe");
        fs::write(&executable, b"synthetic binary").unwrap();
        let integration = CursorHookIntegration::new(workspace.path(), &executable).unwrap();
        let start = br#"{"hook_event_name":"sessionStart","conversation_id":"session-1","session_id":"session-1"}"#;
        let prompt_one = br#"{"hook_event_name":"beforeSubmitPrompt","conversation_id":"session-1","session_id":"session-1","generation_id":"g1"}"#;
        let prompt_two = br#"{"hook_event_name":"beforeSubmitPrompt","conversation_id":"session-1","session_id":"session-1","generation_id":"g2"}"#;
        let stop_one = br#"{"hook_event_name":"stop","conversation_id":"session-1","session_id":"session-1","generation_id":"g1","status":"completed"}"#;
        let stop_two = br#"{"hook_event_name":"stop","conversation_id":"session-1","session_id":"session-1","generation_id":"g2","status":"completed"}"#;
        let end = br#"{"hook_event_name":"sessionEnd","conversation_id":"session-1","session_id":"session-1"}"#;
        let prior_end = br#"{"hook_event_name":"sessionEnd","conversation_id":"prior-session","session_id":"prior-session"}"#;
        assert_eq!(
            dispatch_bounded(
                start,
                workspace.path(),
                &executable,
                state.path(),
                "wt-1",
                true
            )
            .unwrap(),
            CursorDispatchOutcome::Ignored
        );
        integration.install().unwrap();
        let dispatch = |input: &[u8], terminal: &str| {
            dispatch_bounded(
                input,
                workspace.path(),
                &executable,
                state.path(),
                terminal,
                true,
            )
            .unwrap()
        };
        assert_eq!(dispatch(prior_end, "wt-1"), CursorDispatchOutcome::Ignored);
        assert_eq!(
            dispatch(start, "wt-1"),
            CursorDispatchOutcome::RouteAdmitted
        );
        assert_eq!(
            dispatch(prompt_one, "wt-1"),
            CursorDispatchOutcome::RouteAdmitted
        );
        assert_eq!(
            dispatch(prompt_two, "wt-1"),
            CursorDispatchOutcome::RouteAdmitted
        );
        assert_eq!(dispatch(stop_one, "wt-1"), CursorDispatchOutcome::Ignored);
        assert_eq!(dispatch(stop_two, "wt-2"), CursorDispatchOutcome::Ignored);
        assert_eq!(
            dispatch(stop_two, "wt-1"),
            CursorDispatchOutcome::RouteAdmitted
        );
        assert_eq!(dispatch(end, "wt-1"), CursorDispatchOutcome::RouteAdmitted);
        assert_eq!(dispatch(prompt_two, "wt-1"), CursorDispatchOutcome::Ignored);
    }
}
