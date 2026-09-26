#![cfg(windows)]

use std::{fs, time::SystemTime};

use serde_json::json;
use sha2::{Digest, Sha256};
use tabbeacon::{
    presentation_policy::{
        ApplicationStatus, PresentationCapabilities, PresentationMode, PresentationOverride,
        resolve_presentation,
    },
    providers::{
        agy_backend::{AgyTitleDispatchOutcome, AgyTitleRuntime},
        codex::{CodexHookRuntime, HookDispatchOutcome},
        cursor_integration::CursorHookIntegration,
        cursor_runtime::{CursorDispatchOutcome, dispatch_with_output_bounded},
    },
    settings::PresentationSettings,
};

#[test]
#[allow(clippy::too_many_lines)] // One interleaving keeps all providers on the same persisted state root.
fn cursor_codex_and_agy_share_one_state_root_without_cross_provider_output() {
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("workspace");
    let state = root.path().join("shared-tabbeacon-state");
    fs::create_dir_all(&workspace).unwrap();
    let executable = workspace.join("tabbeacon.exe");
    fs::write(&executable, b"synthetic binary").unwrap();
    CursorHookIntegration::new(&workspace, &executable)
        .unwrap()
        .install()
        .unwrap();

    let cursor_mode = resolve_presentation(
        PresentationSettings::default(),
        PresentationOverride::default().with_mode(PresentationMode::ColorOnly),
        PresentationCapabilities::CURSOR_COLOR_ONLY,
        ApplicationStatus::Unproven,
    );
    let cursor_event = |event: &str, session: &str, generation: Option<&str>, terminal: &str| {
        let mut payload = json!({"hook_event_name": event, "session_id": session});
        if let Some(generation) = generation {
            payload["generation_id"] = generation.into();
        }
        let digest = format!("{:x}", Sha256::digest(terminal.as_bytes()));
        let mut output = Vec::new();
        let outcome = dispatch_with_output_bounded(
            payload.to_string().as_bytes(),
            &workspace,
            &executable,
            &state,
            &digest,
            terminal,
            true,
            &cursor_mode,
            &mut output,
        )
        .unwrap();
        (outcome, output)
    };
    let codex = CodexHookRuntime::with_settings(&state, true, PresentationSettings::default());
    let codex_event = |event: &str, turn: Option<&str>| {
        let mut payload = json!({
            "hook_event_name": event,
            "session_id": "codex-session",
            "cwd": workspace,
        });
        if let Some(turn) = turn {
            payload["turn_id"] = turn.into();
        }
        if event == "SessionStart" {
            payload["source"] = "startup".into();
        }
        let mut output = Vec::new();
        let outcome = codex.dispatch_to(
            payload.to_string().as_bytes(),
            SystemTime::now(),
            &mut output,
        );
        (outcome, output)
    };
    let agy = AgyTitleRuntime::new(&state, PresentationSettings::default());
    let agy_event = |phase: &str| {
        let payload = json!({
            "version": "1.1.19",
            "agent_state": phase,
            "conversation_id": "agy-session",
            "workspace": {"current_dir": workspace, "project_dir": workspace},
        });
        agy.dispatch_to(payload.to_string().as_bytes(), SystemTime::now())
    };

    assert_eq!(
        cursor_event("sessionStart", "cursor-a", None, "wt-a").0,
        CursorDispatchOutcome::RouteAdmitted
    );
    assert_eq!(
        codex_event("SessionStart", None).0,
        HookDispatchOutcome::Applied
    );
    assert_eq!(
        agy_event("working").outcome,
        AgyTitleDispatchOutcome::Applied
    );
    let (cursor_working, cursor_bytes) =
        cursor_event("beforeSubmitPrompt", "cursor-a", Some("g1"), "wt-a");
    assert_eq!(cursor_working, CursorDispatchOutcome::OutputFlushed);
    assert!(cursor_bytes.starts_with(b"\x1b]4;264;rgb:"));
    assert!(!cursor_bytes.windows(4).any(|part| part == b"]0;"));
    let (codex_working, codex_bytes) = codex_event("UserPromptSubmit", Some("turn-1"));
    assert_eq!(codex_working, HookDispatchOutcome::Applied);
    assert!(!codex_bytes.is_empty());
    assert_eq!(agy_event("idle").outcome, AgyTitleDispatchOutcome::Applied);
    assert_eq!(
        cursor_event("sessionEnd", "cursor-a", None, "wt-a"),
        (
            CursorDispatchOutcome::OutputFlushed,
            b"\x1b]104;264\x1b\\".to_vec()
        )
    );
    let (codex_ready, ready_bytes) = codex_event("Stop", Some("turn-1"));
    assert_eq!(codex_ready, HookDispatchOutcome::Applied);
    assert!(
        !ready_bytes.is_empty(),
        "Cursor exit cannot suppress Codex output"
    );
    assert_eq!(
        cursor_event("beforeSubmitPrompt", "cursor-a", Some("g2"), "wt-a"),
        (CursorDispatchOutcome::Ignored, Vec::new())
    );
    assert_eq!(
        agy_event("working").outcome,
        AgyTitleDispatchOutcome::Applied
    );
}
