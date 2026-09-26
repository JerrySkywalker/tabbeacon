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
    expected_terminal_sha256: &str,
    terminal_identity: &str,
    console_openable: bool,
) -> io::Result<CursorDispatchOutcome> {
    if terminal_identity.is_empty()
        || terminal_identity.len() > 256
        || terminal_identity.chars().any(char::is_control)
        || !state_root.is_absolute()
        || expected_terminal_sha256.len() != 64
        || !expected_terminal_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
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
    hasher.update(terminal_identity.as_bytes());
    let terminal_sha256 = format!("{:x}", hasher.finalize());
    let store = CursorRouteStore::new(state_root);
    if store.admit(
        &event,
        expected_terminal_sha256,
        &terminal_sha256,
        console_openable,
    )? {
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
    // This independently captured parent-tab digest is currently provided
    // only by the Owner's isolated qualification entry. Ambient WT_SESSION
    // alone cannot prove which terminal the Hook inherited.
    let Ok(expected_terminal_sha256) = env::var("TABBEACON_CURSOR_EXPECTED_WT_SHA256") else {
        return CursorDispatchOutcome::Ignored;
    };
    let console_openable = crate::console_output::open_owned_console().is_ok();
    dispatch_bounded(
        raw,
        &workspace,
        &executable,
        &state_root,
        &expected_terminal_sha256,
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
        let expected = {
            let mut hasher = Sha256::new();
            hasher.update(b"wt-1");
            format!("{:x}", hasher.finalize())
        };
        assert_eq!(
            dispatch_bounded(
                start,
                workspace.path(),
                &executable,
                state.path(),
                &expected,
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
                &expected,
                terminal,
                true,
            )
            .unwrap()
        };
        assert_eq!(dispatch(prior_end, "wt-1"), CursorDispatchOutcome::Ignored);
        assert_eq!(dispatch(start, "wt-2"), CursorDispatchOutcome::Ignored);
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

    #[test]
    fn two_cursor_sessions_keep_independent_terminal_and_generation_routes() {
        let workspace = tempfile::tempdir().unwrap();
        let state = tempfile::tempdir().unwrap();
        let executable = workspace.path().join("tabbeacon.exe");
        fs::write(&executable, b"synthetic binary").unwrap();
        CursorHookIntegration::new(workspace.path(), &executable)
            .unwrap()
            .install()
            .unwrap();
        let terminal_hash = |terminal: &str| format!("{:x}", Sha256::digest(terminal.as_bytes()));
        let dispatch = |event: &str,
                        session: &str,
                        generation: Option<&str>,
                        expected: &str,
                        observed: &str| {
            let mut payload = serde_json::json!({"hook_event_name":event,"session_id":session});
            if let Some(generation) = generation {
                payload["generation_id"] = generation.into();
            }
            if event == "stop" {
                payload["status"] = "completed".into();
            }
            dispatch_bounded(
                payload.to_string().as_bytes(),
                workspace.path(),
                &executable,
                state.path(),
                expected,
                observed,
                true,
            )
            .unwrap()
        };
        let a = terminal_hash("wt-a");
        let b = terminal_hash("wt-b");
        assert_eq!(
            dispatch("sessionStart", "a", None, &a, "wt-a"),
            CursorDispatchOutcome::RouteAdmitted
        );
        assert_eq!(
            dispatch("sessionStart", "b", None, &b, "wt-b"),
            CursorDispatchOutcome::RouteAdmitted
        );
        assert_eq!(
            dispatch("beforeSubmitPrompt", "a", Some("a-1"), &a, "wt-a"),
            CursorDispatchOutcome::RouteAdmitted
        );
        assert_eq!(
            dispatch("beforeSubmitPrompt", "b", Some("b-1"), &b, "wt-b"),
            CursorDispatchOutcome::RouteAdmitted
        );
        assert_eq!(
            dispatch("stop", "a", Some("a-1"), &a, "wt-b"),
            CursorDispatchOutcome::Ignored
        );
        assert_eq!(
            dispatch("sessionEnd", "a", None, &a, "wt-a"),
            CursorDispatchOutcome::RouteAdmitted
        );
        assert_eq!(
            dispatch("stop", "b", Some("b-1"), &b, "wt-b"),
            CursorDispatchOutcome::RouteAdmitted
        );
        assert_eq!(
            dispatch("beforeSubmitPrompt", "a", Some("a-2"), &a, "wt-a"),
            CursorDispatchOutcome::Ignored
        );
        assert_eq!(
            dispatch("beforeSubmitPrompt", "b", Some("b-2"), &b, "wt-b"),
            CursorDispatchOutcome::RouteAdmitted
        );
    }
}
