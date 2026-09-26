//! Fail-open Cursor Hook ingress with exact project-declaration and route checks.
//! Terminal color remains gated on a separate owned-output proof.

use std::{
    env,
    io::{self, Write},
    path::Path,
};

use sha2::{Digest, Sha256};

use super::{
    cursor::{CursorRouteStore, normalize_hook},
    cursor_color,
    cursor_integration::{CursorHookIntegration, CursorHookState},
};
use crate::{
    presentation_policy::{
        ApplicationStatus, CliTarget, PresentationCapabilities, ResolvedPresentation,
    },
    settings::PresentationSettingsStore,
};

/// A content-free outcome from the public Hook ingress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorDispatchOutcome {
    Ignored,
    RouteAdmitted,
    OutputFlushed,
    OutputFailed,
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
    dispatch_with(
        raw,
        workspace,
        executable,
        state_root,
        expected_terminal_sha256,
        terminal_identity,
        console_openable,
        |_, _| Ok(false),
    )
}

/// Executes the admitted route and the final color write in one cross-process
/// lock scope. `sink` must target the separately checked owned console.
///
/// # Errors
///
/// Returns a bounded ownership, config, checkpoint, or output error.
#[allow(clippy::too_many_arguments)]
pub fn dispatch_with_output_bounded(
    raw: &[u8],
    workspace: &Path,
    executable: &Path,
    state_root: &Path,
    expected_terminal_sha256: &str,
    terminal_identity: &str,
    console_openable: bool,
    resolved: &ResolvedPresentation,
    sink: &mut impl Write,
) -> io::Result<CursorDispatchOutcome> {
    dispatch_with(
        raw,
        workspace,
        executable,
        state_root,
        expected_terminal_sha256,
        terminal_identity,
        console_openable,
        |event, terminal| cursor_color::apply_admitted(state_root, terminal, event, resolved, sink),
    )
}

#[allow(clippy::too_many_arguments)]
fn dispatch_with(
    raw: &[u8],
    workspace: &Path,
    executable: &Path,
    state_root: &Path,
    expected_terminal_sha256: &str,
    terminal_identity: &str,
    console_openable: bool,
    apply: impl FnOnce(&super::cursor::CursorLifecycle, &str) -> io::Result<bool>,
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
    let output_failed = std::cell::Cell::new(false);
    let result = store.admit_with(
        &event,
        expected_terminal_sha256,
        &terminal_sha256,
        console_openable,
        || match apply(&event, &terminal_sha256) {
            Ok(written) => Ok(written),
            Err(error) => {
                output_failed.set(true);
                Err(error)
            }
        },
    );
    let (admitted, written) = match result {
        Ok(result) => result,
        Err(_) if output_failed.get() => return Ok(CursorDispatchOutcome::OutputFailed),
        Err(error) => return Err(error),
    };
    if !admitted {
        Ok(CursorDispatchOutcome::Ignored)
    } else if output_failed.get() {
        Ok(CursorDispatchOutcome::OutputFailed)
    } else if written {
        Ok(CursorDispatchOutcome::OutputFlushed)
    } else {
        Ok(CursorDispatchOutcome::RouteAdmitted)
    }
}

/// Runs the installed one-shot Hook path without retaining request content.
#[must_use]
pub fn dispatch_system(raw: &[u8]) -> CursorDispatchOutcome {
    let Ok(settings) = PresentationSettingsStore::from_environment() else {
        return CursorDispatchOutcome::Ignored;
    };
    let Some(state_root) = settings.path().parent() else {
        return CursorDispatchOutcome::Ignored;
    };
    let Ok(resolved) = settings.resolve_provider_read_only(
        CliTarget::Cursor,
        PresentationCapabilities::CURSOR_COLOR_ONLY,
        ApplicationStatus::Unproven,
    ) else {
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
    // The console process list and OS parent relation are independent of the
    // inherited WT_SESSION label. Fail open if this Hook is detached or its
    // parent is not attached to the same console.
    if !parent_attached_to_current_console() {
        return CursorDispatchOutcome::Ignored;
    }
    let expected_terminal_sha256 = format!("{:x}", Sha256::digest(terminal_identity.as_bytes()));
    let Ok(mut sink) = crate::console_output::open_owned_console() else {
        return CursorDispatchOutcome::Ignored;
    };
    dispatch_with_output_bounded(
        raw,
        &workspace,
        &executable,
        state_root,
        &expected_terminal_sha256,
        &terminal_identity,
        true,
        &resolved,
        &mut sink,
    )
    .unwrap_or(CursorDispatchOutcome::Ignored)
}

#[cfg(windows)]
#[allow(unsafe_code)] // Bounded Win32 console/parent snapshot; no foreign process is opened or changed.
fn parent_attached_to_current_console() -> bool {
    use windows::Win32::{
        Foundation::CloseHandle,
        System::{
            Console::GetConsoleProcessList,
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
                TH32CS_SNAPPROCESS,
            },
        },
    };

    let mut attached = [0_u32; 256];
    let count = unsafe { GetConsoleProcessList(&mut attached) } as usize;
    let self_pid = std::process::id();
    if count == 0 || count > attached.len() || !attached[..count].contains(&self_pid) {
        return false;
    }
    let Ok(snapshot) = (unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }) else {
        return false;
    };
    let mut entry = PROCESSENTRY32W {
        dwSize: u32::try_from(std::mem::size_of::<PROCESSENTRY32W>()).unwrap_or(0),
        ..Default::default()
    };
    let mut parent = None;
    if unsafe { Process32FirstW(snapshot, &raw mut entry) }.is_ok() {
        loop {
            if entry.th32ProcessID == self_pid {
                parent = Some(entry.th32ParentProcessID);
                break;
            }
            if unsafe { Process32NextW(snapshot, &raw mut entry) }.is_err() {
                break;
            }
        }
    }
    let _ = unsafe { CloseHandle(snapshot) };
    parent.is_some_and(|pid| pid != 0 && pid != self_pid && attached[..count].contains(&pid))
}

#[cfg(not(windows))]
fn parent_attached_to_current_console() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::{Arc, Mutex, mpsc};
    use std::thread;
    use std::time::Duration;

    use super::*;
    use crate::presentation_policy::{
        PresentationMode, PresentationOverride, resolve_presentation,
    };
    use crate::settings::PresentationSettings;

    #[test]
    #[allow(clippy::too_many_lines)] // One barrier covers the route and the final color write.
    fn delayed_color_write_cannot_overtake_newer_cross_process_route() {
        struct BlockingSink {
            entered: mpsc::Sender<()>,
            release: mpsc::Receiver<()>,
            order: Arc<Mutex<Vec<&'static str>>>,
        }
        impl Write for BlockingSink {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                self.entered.send(()).unwrap();
                self.release.recv().unwrap();
                self.order.lock().unwrap().push("g1");
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        struct TrackingSink(Arc<Mutex<Vec<&'static str>>>);
        impl Write for TrackingSink {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                self.0.lock().unwrap().push("g2");
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let workspace = tempfile::tempdir().unwrap();
        let state = tempfile::tempdir().unwrap();
        let executable = workspace.path().join("tabbeacon.exe");
        fs::write(&executable, b"synthetic binary").unwrap();
        CursorHookIntegration::new(workspace.path(), &executable)
            .unwrap()
            .install()
            .unwrap();
        let resolved = resolve_presentation(
            PresentationSettings::default(),
            PresentationOverride::default().with_mode(PresentationMode::ColorOnly),
            PresentationCapabilities::CURSOR_COLOR_ONLY,
            ApplicationStatus::Unproven,
        );
        let terminal = "test-terminal";
        let digest = format!("{:x}", Sha256::digest(terminal.as_bytes()));
        let start = br#"{"hook_event_name":"sessionStart","session_id":"one"}"#;
        let prompt_one =
            br#"{"hook_event_name":"beforeSubmitPrompt","session_id":"one","generation_id":"g1"}"#;
        let prompt_two =
            br#"{"hook_event_name":"beforeSubmitPrompt","session_id":"one","generation_id":"g2"}"#;
        assert_eq!(
            dispatch_with_output_bounded(
                start,
                workspace.path(),
                &executable,
                state.path(),
                &digest,
                terminal,
                true,
                &resolved,
                &mut Vec::new()
            )
            .unwrap(),
            CursorDispatchOutcome::RouteAdmitted
        );
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let order = Arc::new(Mutex::new(Vec::new()));
        let first_order = Arc::clone(&order);
        let a = thread::spawn({
            let workspace = workspace.path().to_path_buf();
            let state = state.path().to_path_buf();
            let executable = executable.clone();
            let digest = digest.clone();
            move || {
                dispatch_with_output_bounded(
                    prompt_one,
                    &workspace,
                    &executable,
                    &state,
                    &digest,
                    terminal,
                    true,
                    &resolved,
                    &mut BlockingSink {
                        entered: entered_tx,
                        release: release_rx,
                        order: first_order,
                    },
                )
            }
        });
        entered_rx.recv().unwrap();
        let (done_tx, done_rx) = mpsc::channel();
        let second_order = Arc::clone(&order);
        let b = thread::spawn({
            let workspace = workspace.path().to_path_buf();
            let state = state.path().to_path_buf();
            let executable = executable.clone();
            let digest = digest.clone();
            move || {
                done_tx
                    .send(dispatch_with_output_bounded(
                        prompt_two,
                        &workspace,
                        &executable,
                        &state,
                        &digest,
                        terminal,
                        true,
                        &resolved,
                        &mut TrackingSink(second_order),
                    ))
                    .unwrap();
            }
        });
        assert!(done_rx.recv_timeout(Duration::from_millis(20)).is_err());
        release_tx.send(()).unwrap();
        assert_eq!(
            a.join().unwrap().unwrap(),
            CursorDispatchOutcome::OutputFlushed
        );
        assert_eq!(
            done_rx.recv().unwrap().unwrap(),
            CursorDispatchOutcome::OutputFlushed
        );
        b.join().unwrap();
        assert_eq!(*order.lock().unwrap(), ["g1", "g2"]);
    }

    #[test]
    fn public_dispatch_distinguishes_admitted_route_from_failed_output() {
        struct FailingSink;
        impl Write for FailingSink {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::Error::other("injected terminal failure"))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let workspace = tempfile::tempdir().unwrap();
        let state = tempfile::tempdir().unwrap();
        let executable = workspace.path().join("tabbeacon.exe");
        fs::write(&executable, b"synthetic binary").unwrap();
        CursorHookIntegration::new(workspace.path(), &executable)
            .unwrap()
            .install()
            .unwrap();
        let resolved = resolve_presentation(
            PresentationSettings::default(),
            PresentationOverride::default().with_mode(PresentationMode::ColorOnly),
            PresentationCapabilities::CURSOR_COLOR_ONLY,
            ApplicationStatus::Unproven,
        );
        let terminal = "test-terminal";
        let digest = format!("{:x}", Sha256::digest(terminal.as_bytes()));
        let start = br#"{"hook_event_name":"sessionStart","session_id":"one"}"#;
        let prompt =
            br#"{"hook_event_name":"beforeSubmitPrompt","session_id":"one","generation_id":"g1"}"#;
        assert_eq!(
            dispatch_with_output_bounded(
                start,
                workspace.path(),
                &executable,
                state.path(),
                &digest,
                terminal,
                true,
                &resolved,
                &mut Vec::new()
            )
            .unwrap(),
            CursorDispatchOutcome::RouteAdmitted
        );
        assert_eq!(
            dispatch_with_output_bounded(
                prompt,
                workspace.path(),
                &executable,
                state.path(),
                &digest,
                terminal,
                true,
                &resolved,
                &mut FailingSink
            )
            .unwrap(),
            CursorDispatchOutcome::OutputFailed
        );
    }

    #[test]
    fn color_output_follows_newest_generation_and_exact_session_ownership() {
        let workspace = tempfile::tempdir().unwrap();
        let state = tempfile::tempdir().unwrap();
        let executable = workspace.path().join("tabbeacon.exe");
        fs::write(&executable, b"synthetic binary").unwrap();
        CursorHookIntegration::new(workspace.path(), &executable)
            .unwrap()
            .install()
            .unwrap();
        let color = resolve_presentation(
            PresentationSettings::default(),
            PresentationOverride::default().with_mode(PresentationMode::ColorOnly),
            PresentationCapabilities::CURSOR_COLOR_ONLY,
            ApplicationStatus::Unproven,
        );
        let native = resolve_presentation(
            PresentationSettings::default(),
            PresentationOverride::default().with_mode(PresentationMode::PreserveNative),
            PresentationCapabilities::CURSOR_COLOR_ONLY,
            ApplicationStatus::Unproven,
        );
        let terminal = "test-terminal";
        let digest = format!("{:x}", Sha256::digest(terminal.as_bytes()));
        let mut sink = Vec::new();
        let mut dispatch =
            |name: &str, session: &str, generation: Option<&str>, mode: &ResolvedPresentation| {
                let mut input = serde_json::json!({"hook_event_name":name,"session_id":session});
                if let Some(generation) = generation {
                    input["generation_id"] = generation.into();
                }
                if name == "stop" {
                    input["status"] = "completed".into();
                }
                let prior = sink.len();
                let outcome = dispatch_with_output_bounded(
                    input.to_string().as_bytes(),
                    workspace.path(),
                    &executable,
                    state.path(),
                    &digest,
                    terminal,
                    true,
                    mode,
                    &mut sink,
                )
                .unwrap();
                (outcome, sink[prior..].to_vec())
            };
        assert_eq!(
            dispatch("sessionStart", "a", None, &color).0,
            CursorDispatchOutcome::RouteAdmitted
        );
        let (first, bytes) = dispatch("beforeSubmitPrompt", "a", Some("g1"), &color);
        assert_eq!(first, CursorDispatchOutcome::OutputFlushed);
        assert!(bytes.starts_with(b"\x1b]4;264;rgb:"));
        assert!(!bytes.windows(4).any(|part| part == b"]0;"));
        assert!(!bytes.windows(4).any(|part| part == b"]9;4"));
        assert_eq!(
            dispatch("beforeSubmitPrompt", "a", Some("g2"), &color).0,
            CursorDispatchOutcome::OutputFlushed
        );
        assert_eq!(
            dispatch("stop", "a", Some("g1"), &color),
            (CursorDispatchOutcome::Ignored, Vec::new())
        );
        assert_eq!(
            dispatch("stop", "a", Some("g2"), &color).0,
            CursorDispatchOutcome::OutputFlushed
        );
        // A new same-tab session releases the prior owned color, then takes
        // over. The old session's late end cannot reset the new owner.
        let (start_b, release) = dispatch("sessionStart", "b", None, &color);
        assert_eq!(start_b, CursorDispatchOutcome::OutputFlushed);
        assert_eq!(release, b"\x1b]104;264\x1b\\");
        assert_eq!(
            dispatch("beforeSubmitPrompt", "b", Some("b1"), &color).0,
            CursorDispatchOutcome::OutputFlushed
        );
        assert_eq!(
            dispatch("sessionEnd", "a", None, &color),
            (CursorDispatchOutcome::RouteAdmitted, Vec::new())
        );
        let (native_outcome, native_release) = dispatch("stop", "b", Some("b1"), &native);
        assert_eq!(native_outcome, CursorDispatchOutcome::OutputFlushed);
        assert_eq!(native_release, b"\x1b]104;264\x1b\\");
        assert_eq!(
            dispatch("sessionEnd", "b", None, &native),
            (CursorDispatchOutcome::RouteAdmitted, Vec::new())
        );
    }

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
