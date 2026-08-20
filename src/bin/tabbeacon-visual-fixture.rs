//! G03 fixture renderer and bounded visual-harness entrypoint.

use std::{
    env, fs,
    io::{self, Write},
    path::PathBuf,
    process, thread,
    time::{Duration, Instant},
};

use serde_json::json;
use tabbeacon::{
    activity::next_animation_frame_deadline,
    presentation::presentation_fixture,
    providers::codex::{CodexHookRuntime, HookDispatchOutcome},
    repo::WorkspaceIdentityResolver,
    settings::PresentationSettings,
    visual::{
        FixtureDriver, LiveVisualRunRequest, ROOT_WORKSPACE_ANCHOR_FIXTURE_NAME, VisualDisposition,
        VisualError, VisualResult, root_workspace_anchor_fixture_alias,
        runner::{authorize_live_worker, run_live, run_live_in_worker},
    },
};

fn main() {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    if let Err(error) = run(&arguments) {
        eprintln!("tabbeacon visual fixture: {error}");
        std::process::exit(1);
    }
}

fn run(arguments: &[String]) -> VisualResult<()> {
    let command = arguments.first().map(String::as_str).unwrap_or_default();
    match command {
        "emit" => emit(arguments),
        "run" => run_live_harness(arguments),
        "run-worker" => run_live_worker(arguments),
        _ => Err(VisualError::Platform(
            "expected `emit` or `run` visual fixture subcommand".to_owned(),
        )),
    }
}

fn emit(arguments: &[String]) -> VisualResult<()> {
    let fixture_name = argument_value(arguments, "--fixture")?;
    let run_id = argument_value(arguments, "--run-id")?;
    let hold_millis = argument_value(arguments, "--hold-ms")?
        .parse::<u64>()
        .map_err(|error| VisualError::Platform(format!("invalid --hold-ms: {error}")))?;
    if fixture_name == ROOT_WORKSPACE_ANCHOR_FIXTURE_NAME {
        return emit_root_workspace_anchor(&run_id, hold_millis);
    }
    let fixture = presentation_fixture()
        .iter()
        .find(|fixture| fixture.name() == fixture_name)
        .ok_or_else(|| VisualError::Platform(format!("unknown G02 fixture: {fixture_name}")))?;
    let driver = FixtureDriver::default();
    let replay = driver.replay(fixture, &run_id)?;
    let reset = driver.reset(&run_id)?;
    let mut stdout = io::stdout().lock();
    stdout.write_all(&replay.vt_bytes)?;
    stdout.flush()?;
    if replay.title_frame_bytes.is_empty() {
        thread::sleep(Duration::from_millis(hold_millis));
    } else {
        let deadline = Instant::now() + Duration::from_millis(hold_millis);
        let mut frame_index = 1_usize;
        let mut next_frame_deadline = Instant::now();
        while Instant::now() < deadline {
            stdout.write_all(
                &replay.title_frame_bytes[frame_index % replay.title_frame_bytes.len()],
            )?;
            stdout.flush()?;
            frame_index = frame_index.saturating_add(1);
            next_frame_deadline =
                next_animation_frame_deadline(next_frame_deadline, Instant::now());
            let remaining = next_frame_deadline.saturating_duration_since(Instant::now());
            if !remaining.is_zero() {
                thread::sleep(remaining);
            }
        }
    }
    stdout.write_all(&reset.vt_bytes)?;
    stdout.flush()?;
    Ok(())
}

/// Emits a real Codex hook sequence with an anchored root and alternate CWDs.
///
/// The temporary state belongs solely to this uniquely correlated fixture. It
/// is removed on normal completion and contains no alternate alias assignment:
/// the alternate CWD is observed only through the G59 anchor runtime path.
fn emit_root_workspace_anchor(run_id: &str, hold_millis: u64) -> VisualResult<()> {
    let state_root =
        env::temp_dir().join(format!("tabbeacon-g59-visual-{run_id}-{}", process::id()));
    if state_root.exists() {
        return Err(VisualError::Platform(
            "owned root-workspace visual state already exists".to_owned(),
        ));
    }
    fs::create_dir_all(state_root.join("alternate-workspace"))?;
    let result = emit_root_workspace_anchor_in_state(&state_root, run_id, hold_millis);
    let _ = fs::remove_dir_all(&state_root);
    result
}

fn emit_root_workspace_anchor_in_state(
    state_root: &std::path::Path,
    run_id: &str,
    hold_millis: u64,
) -> VisualResult<()> {
    let root_cwd = env::current_dir()?;
    let alternate_cwd = state_root.join("alternate-workspace");
    let root_alias = root_workspace_anchor_fixture_alias(run_id)?;
    WorkspaceIdentityResolver::new(state_root)
        .set_alias_override(&root_cwd, &root_alias)
        .map_err(|_| VisualError::Platform("could not prepare owned root alias".to_owned()))?;
    let runtime =
        CodexHookRuntime::with_settings(state_root, true, PresentationSettings::default());
    let session_id = format!("g59-visual-{run_id}");
    let mut stdout = io::stdout().lock();

    dispatch_anchor_event(
        &runtime,
        &json!({
            "hook_event_name": "SessionStart",
            "session_id": &session_id,
            "turn_id": "root-turn",
            "cwd": &root_cwd,
            "source": "startup",
        }),
        &mut stdout,
    )?;
    dispatch_anchor_event(
        &runtime,
        &json!({
            "hook_event_name": "PreToolUse",
            "session_id": &session_id,
            "turn_id": "root-turn",
            "cwd": &alternate_cwd,
        }),
        &mut stdout,
    )?;
    dispatch_anchor_event(
        &runtime,
        &json!({
            "hook_event_name": "SubagentStart",
            "session_id": &session_id,
            "turn_id": "subagent-turn",
            "agent_id": "owned-visual-subagent",
            "agent_type": "thread",
            "cwd": &alternate_cwd,
        }),
        &mut stdout,
    )?;
    dispatch_anchor_event(
        &runtime,
        &json!({
            "hook_event_name": "PostToolUse",
            "session_id": &session_id,
            "turn_id": "root-turn",
            "cwd": &alternate_cwd,
        }),
        &mut stdout,
    )?;
    thread::sleep(Duration::from_millis(hold_millis));
    let reset = FixtureDriver::default().reset(run_id)?;
    stdout.write_all(&reset.vt_bytes)?;
    stdout.flush()?;
    Ok(())
}

fn dispatch_anchor_event(
    runtime: &CodexHookRuntime,
    payload: &serde_json::Value,
    stdout: &mut impl Write,
) -> VisualResult<()> {
    let raw = serde_json::to_vec(&payload).map_err(VisualError::Json)?;
    if !matches!(
        runtime.dispatch_to(&raw, std::time::SystemTime::now(), stdout),
        HookDispatchOutcome::Applied | HookDispatchOutcome::IgnoredSubagent
    ) {
        return Err(VisualError::Platform(
            "owned root-workspace fixture hook sequence was not admitted".to_owned(),
        ));
    }
    Ok(())
}

fn run_live_harness(arguments: &[String]) -> VisualResult<()> {
    let summary = run_live(&live_request(arguments)?)?;
    print_live_summary(&summary)
}

fn run_live_worker(arguments: &[String]) -> VisualResult<()> {
    let request = live_request(arguments)?;
    let authorization_path = PathBuf::from(argument_value(arguments, "--worker-authorization")?);
    let nonce = env::var("TABBEACON_VISUAL_WORKER_NONCE").map_err(|_| {
        VisualError::Platform("visual worker requires supervisor authorization".to_owned())
    })?;
    authorize_live_worker(&request, &authorization_path, &nonce)?;
    run_live_in_worker(&request)?;
    Ok(())
}

fn live_request(arguments: &[String]) -> VisualResult<LiveVisualRunRequest> {
    let expected_head = argument_value(arguments, "--expected-head")?;
    let run_id = argument_value(arguments, "--run-id")?;
    let evidence_root = argument_value(arguments, "--evidence-root")?;
    let fixture_name = optional_argument_value(arguments, "--fixture");
    Ok(LiveVisualRunRequest {
        expected_head,
        run_id,
        evidence_root: evidence_root.into(),
        fixture_name,
    })
}

fn print_live_summary(summary: &tabbeacon::visual::LiveVisualRunSummary) -> VisualResult<()> {
    println!(
        "{}",
        serde_json::to_string(&summary).map_err(VisualError::Json)?
    );
    let exit_code = match summary.disposition {
        VisualDisposition::Pass => 0,
        VisualDisposition::Blocked => 78,
        VisualDisposition::Unproven => 3,
        VisualDisposition::Fail => 2,
    };
    if exit_code != 0 {
        std::process::exit(exit_code);
    }
    Ok(())
}

fn argument_value(arguments: &[String], flag: &str) -> VisualResult<String> {
    let position = arguments
        .iter()
        .position(|argument| argument == flag)
        .ok_or_else(|| VisualError::Platform(format!("missing {flag}")))?;
    arguments
        .get(position + 1)
        .cloned()
        .ok_or_else(|| VisualError::Platform(format!("missing value for {flag}")))
}

fn optional_argument_value(arguments: &[String], flag: &str) -> Option<String> {
    arguments
        .iter()
        .position(|argument| argument == flag)
        .and_then(|position| arguments.get(position + 1))
        .cloned()
}
