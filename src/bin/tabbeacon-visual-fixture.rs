//! G03 fixture renderer and bounded visual-harness entrypoint.

use std::{
    env, fs,
    fs::OpenOptions,
    io::{self, BufWriter, Write},
    path::{Path, PathBuf},
    process::{self, Command},
    thread,
    time::{Duration, Instant},
};

use serde::Serialize;
use serde_json::json;
use tabbeacon::{
    activity::next_animation_frame_deadline,
    presentation::{
        PresentationAction, TitleStatus, WindowsTerminalCapabilities, WindowsTerminalRenderer,
        presentation_fixture,
    },
    providers::codex::{CodexHookRuntime, HookDispatchOutcome},
    repo::WorkspaceIdentityResolver,
    settings::PresentationSettings,
    visual::{
        CaptureBackend, FixtureDriver, LiveVisualRunRequest, PrintWindowCaptureBackend,
        ROOT_WORKSPACE_ANCHOR_FIXTURE_NAME, TemporaryWindowProductDisposition,
        TemporaryWindowsTerminalOwnership, TerminalTestSession, TerminalTestSessionLauncher,
        VisualDisposition, VisualError, VisualResult, WindowsUiaLocator,
        root_workspace_anchor_fixture_alias,
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
        "promo" => run_promo_showcase(arguments),
        "showcase" => emit_showcase(arguments),
        _ => Err(VisualError::Platform(
            "expected `emit`, `run`, `promo`, or `showcase` visual fixture subcommand".to_owned(),
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

const PROMO_FPS: u32 = 10;
const PROMO_FRAME_COUNT: u32 = 100;
const PROMO_DURATION_MS: u64 = 10_000;
const PROMO_FRAME_SETTLE: Duration = Duration::from_millis(150);
const PROMO_CAPTURE_BUDGET: Duration = Duration::from_secs(85);

/// Content-minimal record of a successful deterministic showcase capture.
/// It deliberately contains no terminal text, Owner identity, or filesystem
/// paths; exact-owned lifecycle facts are retained as counts only.
#[derive(Debug, Serialize)]
#[allow(clippy::struct_excessive_bools)]
struct PromoShowcaseReceipt {
    source_sha: String,
    windows_terminal_version: String,
    frame_count: u32,
    fps: u32,
    duration_ms: u64,
    frame_width: u32,
    frame_height: u32,
    anchor_tab_match_count: u32,
    target_window_match_count: u32,
    real_windows_terminal: bool,
    real_tabbeacon_renderer: bool,
    real_model_session: bool,
    desktop_capture: bool,
    controlled_fixture_only: bool,
    temporary_windows_created: u32,
    temporary_windows_closed: u32,
    owned_temporary_wt_remaining: u32,
    owner_windows_closed: u32,
    broad_windows_terminal_kill: bool,
}

#[derive(Debug)]
struct PromoCapture {
    frame_width: u32,
    frame_height: u32,
}

/// Generates raw frames through a real stock Windows Terminal window. The
/// static anchor is registered before any dynamic `API`/`WEB`/`DOCS` tabs are
/// launched, so neither a changing title nor a top-level `Window.Name` is
/// capture or cleanup authority.
#[allow(clippy::too_many_lines)]
fn run_promo_showcase(arguments: &[String]) -> VisualResult<()> {
    let expected_head = argument_value(arguments, "--expected-head")?;
    let run_id = argument_value(arguments, "--run-id")?;
    let frames_directory = PathBuf::from(argument_value(arguments, "--frames-dir")?);
    let receipt_path = PathBuf::from(argument_value(arguments, "--receipt")?);

    if !is_exact_sha(&expected_head) || checked_out_head()? != expected_head {
        return Err(VisualError::Platform(
            "promo showcase requires EXPECTED_HEAD == CHECKED_OUT_HEAD".to_owned(),
        ));
    }
    if !is_safe_run_id(&run_id) {
        return Err(VisualError::InvalidIdentifier(run_id));
    }
    if frames_directory.exists() || receipt_path.exists() {
        return Err(VisualError::Platform(
            "promo output paths must be fresh exact-owned evidence paths".to_owned(),
        ));
    }
    let evidence_root = validate_promo_evidence_paths(&run_id, &frames_directory, &receipt_path)?;
    fs::create_dir(&frames_directory)?;
    let control_path = evidence_root.join("showcase-frame.txt");
    write_new_text(&control_path, "WAITING\n")?;

    let fixture_executable = env::current_exe().map_err(VisualError::Io)?;
    let initial_arguments = showcase_arguments("api", &run_id, &control_path);
    let session = TerminalTestSessionLauncher::default().launch_promo_showcase(
        &fixture_executable,
        &run_id,
        &initial_arguments,
        &evidence_root,
    )?;

    let body_result = capture_promo_showcase(
        &fixture_executable,
        &session,
        &run_id,
        &control_path,
        &frames_directory,
    );
    let product_disposition = match &body_result {
        Ok(_) => TemporaryWindowProductDisposition::Pass,
        Err(error)
            if error
                .to_string()
                .contains("promo capture exceeded its bounded") =>
        {
            TemporaryWindowProductDisposition::Timeout
        }
        Err(_) => TemporaryWindowProductDisposition::Fail,
    };
    let cleanup = session.cleanup(product_disposition)?;
    if cleanup.temporary_wt_cleanup != "PASS"
        || cleanup.temporary_windows_created != 1
        || cleanup.temporary_windows_closed != 1
        || cleanup.owned_temporary_wt_remaining != 0
        || cleanup.owner_windows_closed != 0
        || cleanup.broad_window_kill_used
    {
        return Err(VisualError::Platform(
            "promo exact-owned temporary Windows Terminal cleanup did not satisfy the hard gate"
                .to_owned(),
        ));
    }
    let capture = body_result?;
    let receipt = PromoShowcaseReceipt {
        source_sha: expected_head,
        windows_terminal_version: command_value("wt.exe", &["--version"]),
        frame_count: PROMO_FRAME_COUNT,
        fps: PROMO_FPS,
        duration_ms: PROMO_DURATION_MS,
        frame_width: capture.frame_width,
        frame_height: capture.frame_height,
        anchor_tab_match_count: 1,
        target_window_match_count: 1,
        real_windows_terminal: true,
        real_tabbeacon_renderer: true,
        real_model_session: false,
        desktop_capture: false,
        controlled_fixture_only: true,
        temporary_windows_created: cleanup.temporary_windows_created,
        temporary_windows_closed: cleanup.temporary_windows_closed,
        owned_temporary_wt_remaining: cleanup.owned_temporary_wt_remaining,
        owner_windows_closed: cleanup.owner_windows_closed,
        broad_windows_terminal_kill: cleanup.broad_window_kill_used,
    };
    write_new_json(&receipt_path, &receipt)?;
    println!(
        "{}",
        serde_json::to_string(&receipt).map_err(VisualError::Json)?
    );
    Ok(())
}

fn capture_promo_showcase(
    fixture_executable: &Path,
    session: &TerminalTestSession,
    run_id: &str,
    control_path: &Path,
    frames_directory: &Path,
) -> VisualResult<PromoCapture> {
    for role in ["web", "docs"] {
        launch_showcase_sibling(
            fixture_executable,
            &session.window_name,
            role,
            run_id,
            control_path,
        )?;
    }
    // Let each child enter its controlled wait loop before the first typed
    // synthetic frame is requested. This does not enumerate desktop state.
    thread::sleep(Duration::from_millis(800));

    let ownership = read_promo_ownership(&session.ownership_path, session)?;
    WindowsUiaLocator.verify_exact_anchor_window_tab_count(
        &session.anchor_title,
        ownership.native_window_id,
        4,
    )?;
    let capture_target = WindowsUiaLocator.activate_capture_target_for_exact_anchor(
        &session.anchor_title,
        ownership.native_window_id,
    )?;
    let backend = PrintWindowCaptureBackend;
    let deadline = Instant::now() + PROMO_CAPTURE_BUDGET;
    let mut dimensions = None;
    for frame_index in 0..PROMO_FRAME_COUNT {
        if Instant::now() >= deadline {
            return Err(VisualError::Platform(
                "promo capture exceeded its bounded exact-owned window budget".to_owned(),
            ));
        }
        write_promo_control(control_path, &frame_index.to_string())?;
        thread::sleep(PROMO_FRAME_SETTLE);
        let frame = backend.capture(&capture_target)?;
        dimensions.get_or_insert((frame.width(), frame.height()));
        write_png_frame(
            &frames_directory.join(format!("frame-{:04}.png", frame_index + 1)),
            &frame,
        )?;
    }
    write_promo_control(control_path, "DONE")?;
    let (frame_width, frame_height) = dimensions
        .ok_or_else(|| VisualError::Platform("promo capture produced no frames".to_owned()))?;
    Ok(PromoCapture {
        frame_width,
        frame_height,
    })
}

fn read_promo_ownership(
    ownership_path: &Path,
    session: &TerminalTestSession,
) -> VisualResult<TemporaryWindowsTerminalOwnership> {
    let file = fs::File::open(ownership_path)?;
    let ownership = serde_json::from_reader::<_, TemporaryWindowsTerminalOwnership>(file)
        .map_err(VisualError::Json)?;
    if ownership.anchor_title != session.anchor_title
        || ownership.window_routing_id != session.window_name
        || ownership.native_window_id == 0
    {
        return Err(VisualError::Platform(
            "promo lifecycle ownership record did not bind the exact registered window".to_owned(),
        ));
    }
    Ok(ownership)
}

/// Child mode for one synthetic showcase tab. Every visible state comes from
/// the production presentation fixture and renderer; no provider session,
/// model request, private content, or shell prompt is involved.
fn emit_showcase(arguments: &[String]) -> VisualResult<()> {
    let role = argument_value(arguments, "--role")?;
    let run_id = argument_value(arguments, "--run-id")?;
    let control_path = PathBuf::from(argument_value(arguments, "--control-file")?);
    if !is_safe_run_id(&run_id) || !matches!(role.as_str(), "api" | "web" | "docs") {
        return Err(VisualError::Platform(
            "promo showcase role or run identifier is invalid".to_owned(),
        ));
    }
    let renderer = WindowsTerminalRenderer::new(WindowsTerminalCapabilities::new(true));
    let alias = match role.as_str() {
        "api" => "API",
        "web" => "WEB",
        "docs" => "DOCS",
        _ => unreachable!("validated showcase role"),
    };
    let deadline = Instant::now() + Duration::from_secs(100);
    let mut last_frame = None;
    while Instant::now() < deadline {
        let signal = fs::read_to_string(&control_path).unwrap_or_default();
        let signal = signal.trim();
        if signal == "DONE" {
            let reset = presentation_fixture()
                .iter()
                .find(|case| case.name() == "reset")
                .ok_or_else(|| VisualError::Platform("reset fixture is missing".to_owned()))?;
            let mut stdout = io::stdout().lock();
            stdout.write_all(&renderer.render(&reset.action_with_title(alias)))?;
            stdout.flush()?;
            return Ok(());
        }
        if let Ok(frame_index) = signal.parse::<u32>()
            && last_frame != Some(frame_index)
        {
            if frame_index >= PROMO_FRAME_COUNT {
                return Err(VisualError::Platform(
                    "promo showcase frame control exceeded the bounded timeline".to_owned(),
                ));
            }
            render_showcase_frame(
                renderer,
                showcase_state(&role, frame_index),
                alias,
                usize::try_from(frame_index).expect("u32 fits usize"),
            )?;
            last_frame = Some(frame_index);
        }
        thread::sleep(Duration::from_millis(20));
    }
    Err(VisualError::Platform(
        "promo showcase did not receive a bounded completion control".to_owned(),
    ))
}

fn showcase_state(role: &str, frame_index: u32) -> &'static str {
    if frame_index < 20 {
        "ready"
    } else if frame_index < 55 {
        "working"
    } else {
        match role {
            "api" => "result-ready",
            "web" => "working",
            "docs" => "question",
            _ => "ready",
        }
    }
}

fn render_showcase_frame(
    renderer: WindowsTerminalRenderer,
    fixture_name: &str,
    alias: &str,
    frame_index: usize,
) -> VisualResult<()> {
    let fixture = presentation_fixture()
        .iter()
        .find(|case| case.name() == fixture_name)
        .ok_or_else(|| {
            VisualError::Platform(format!("promo fixture is missing: {fixture_name}"))
        })?;
    let action = fixture.action_with_title(alias);
    let state = match &action {
        PresentationAction::Apply(state) | PresentationAction::Reset(state) => state,
    };
    let mut stdout = io::stdout().lock();
    stdout.write_all(&renderer.render(&action))?;
    if state.title_status() == TitleStatus::Working {
        stdout.write_all(&renderer.render_title_spinner_frame(state, frame_index))?;
    }
    stdout.flush()?;
    Ok(())
}

fn showcase_arguments(role: &str, run_id: &str, control_path: &Path) -> Vec<String> {
    vec![
        "showcase".to_owned(),
        "--role".to_owned(),
        role.to_owned(),
        "--run-id".to_owned(),
        run_id.to_owned(),
        "--control-file".to_owned(),
        control_path.display().to_string(),
    ]
}

fn launch_showcase_sibling(
    executable: &Path,
    window_name: &str,
    role: &str,
    run_id: &str,
    control_path: &Path,
) -> VisualResult<()> {
    // Observe the named-window dispatch result before allowing a capture to
    // proceed. A successful `spawn` only proves the launcher process exists;
    // it does not prove the controlled sibling tab joined the registered
    // exact-owned window.
    let launch_status = Command::new("wt.exe")
        .args(["-w", window_name])
        .arg("new-tab")
        .arg("--useApplicationTitle")
        .arg(executable)
        .args(showcase_arguments(role, run_id, control_path))
        .status()
        .map_err(|error| {
            VisualError::Platform(format!(
                "could not launch controlled promo Windows Terminal tab: {error}"
            ))
        })?;
    if !launch_status.success() {
        return Err(VisualError::Platform(
            "Windows Terminal rejected the controlled promo tab launch".to_owned(),
        ));
    }
    Ok(())
}

fn validate_promo_evidence_paths(
    run_id: &str,
    frames_directory: &Path,
    receipt_path: &Path,
) -> VisualResult<PathBuf> {
    let expected_evidence_root = PathBuf::from(r"V:\build\tabbeacon").join(run_id);
    let evidence_root = frames_directory.parent().ok_or_else(|| {
        VisualError::Platform(
            "promo frames directory must have an owned evidence parent".to_owned(),
        )
    })?;
    if !evidence_root.is_dir() {
        return Err(VisualError::Platform(
            "promo evidence parent must exist before capture".to_owned(),
        ));
    }
    let canonical_evidence_root = fs::canonicalize(evidence_root)?;
    let canonical_expected_root = fs::canonicalize(&expected_evidence_root).map_err(|_| {
        VisualError::Platform(
            "promo evidence root must be the exact owned V:\\build\\tabbeacon run directory"
                .to_owned(),
        )
    })?;
    if canonical_evidence_root != canonical_expected_root
        || frames_directory != evidence_root.join("frames")
        || receipt_path != evidence_root.join("promo-fixture-receipt.json")
    {
        return Err(VisualError::Platform(
            "promo paths must stay inside the exact owned evidence root".to_owned(),
        ));
    }
    Ok(canonical_evidence_root)
}

fn write_new_text(path: &Path, value: &str) -> VisualResult<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(value.as_bytes())?;
    file.flush()?;
    Ok(())
}

fn write_promo_control(path: &Path, value: &str) -> VisualResult<()> {
    let mut file = OpenOptions::new().write(true).truncate(true).open(path)?;
    file.write_all(value.as_bytes())?;
    file.write_all(b"\n")?;
    file.flush()?;
    Ok(())
}

fn write_new_json<T: Serialize>(path: &Path, receipt: &T) -> VisualResult<()> {
    let file = OpenOptions::new().write(true).create_new(true).open(path)?;
    let mut writer = BufWriter::new(file);
    serde_json::to_writer_pretty(&mut writer, receipt).map_err(VisualError::Json)?;
    writer.flush()?;
    Ok(())
}

fn write_png_frame(path: &Path, frame: &tabbeacon::visual::RgbaFrame) -> VisualResult<()> {
    let file = OpenOptions::new().write(true).create_new(true).open(path)?;
    let writer = BufWriter::new(file);
    let mut encoder = png::Encoder::new(writer, frame.width(), frame.height());
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut png_writer = encoder.write_header()?;
    png_writer.write_image_data(frame.pixels())?;
    png_writer.finish()?;
    Ok(())
}

fn checked_out_head() -> VisualResult<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(VisualError::Io)?;
    if !output.status.success() {
        return Err(VisualError::Platform(
            "git rev-parse HEAD failed in promo fixture".to_owned(),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn command_value(program: &str, arguments: &[&str]) -> String {
    Command::new(program)
        .args(arguments)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "UNAVAILABLE".to_owned())
}

fn is_safe_run_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

fn is_exact_sha(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
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

#[cfg(test)]
mod promo_showcase_tests {
    use super::{PROMO_FRAME_COUNT, is_exact_sha, is_safe_run_id, showcase_state};

    #[test]
    fn showcase_timeline_is_deterministic_and_uses_only_admitted_states() {
        assert_eq!(showcase_state("api", 0), "ready");
        assert_eq!(showcase_state("web", 20), "working");
        assert_eq!(showcase_state("docs", 54), "working");
        assert_eq!(showcase_state("api", 55), "result-ready");
        assert_eq!(showcase_state("web", PROMO_FRAME_COUNT - 1), "working");
        assert_eq!(showcase_state("docs", PROMO_FRAME_COUNT - 1), "question");
    }

    #[test]
    fn promo_identifiers_require_safe_run_and_exact_head_values() {
        assert!(is_safe_run_id("TB-V073-G100-20260901-001"));
        assert!(!is_safe_run_id("TB V073/unsafe"));
        assert!(is_exact_sha("e53bd01688a924c02937c83c7436bba6a19999e1"));
        assert!(!is_exact_sha("e53bd016"));
    }
}
