//! G03 fixture renderer and bounded visual-harness entrypoint.

use std::{
    env, fs,
    fs::OpenOptions,
    io::{self, BufWriter, Write},
    path::{Path, PathBuf},
    process::{self, Command},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
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
        CaptureBackend, FixtureDriver, LiveVisualRunRequest, OwnedWindowCaptureTarget,
        PrintWindowCaptureBackend, ROOT_WORKSPACE_ANCHOR_FIXTURE_NAME, VisualDisposition,
        VisualError, VisualResult, WindowsUiaLocator, root_workspace_anchor_fixture_alias,
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
        "promo-worker" => run_promo_worker(arguments),
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
const PROMO_DURATION_MS: u64 = 10_000;
const PROMO_CORRELATION_ALIAS_PREFIX: &str = "TB100-";

#[derive(Debug, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)]
struct PromoShowcaseReceipt {
    source_sha: String,
    windows_terminal_version: String,
    frame_count: u32,
    fps: u32,
    duration_ms: u64,
    frame_width: u32,
    frame_height: u32,
    target_window_match_count: u32,
    real_windows_terminal: bool,
    real_tabbeacon_renderer: bool,
    real_model_session: bool,
    desktop_capture: bool,
    controlled_fixture_only: bool,
}

/// Launches one controlled Windows Terminal window, proves a unique initial
/// fixture target with UIA, and captures only that admitted HWND. This remains
/// inside the `visual-fixture` feature boundary; normal `TabBeacon` never exposes
/// this promotional helper.
fn run_promo_showcase(arguments: &[String]) -> VisualResult<()> {
    let executable = env::current_exe().map_err(VisualError::Io)?;
    let mut worker = Command::new(executable)
        .arg("promo-worker")
        .args(arguments)
        .spawn()
        .map_err(|error| {
            VisualError::Platform(format!("could not launch bounded promo worker: {error}"))
        })?;
    let deadline = Instant::now() + Duration::from_secs(45);
    while Instant::now() < deadline {
        if let Some(status) = worker.try_wait().map_err(VisualError::Io)? {
            return status.success().then_some(()).ok_or_else(|| {
                VisualError::Platform(format!("promo worker exited with {status}"))
            });
        }
        thread::sleep(Duration::from_millis(100));
    }
    let worker_id = worker.id().to_string();
    let _ = Command::new("taskkill")
        .args(["/PID", &worker_id, "/T", "/F"])
        .status();
    let _ = worker.wait();
    Err(VisualError::Platform(
        "promo worker exceeded its 45-second owned UIA/capture supervision limit".to_owned(),
    ))
}

#[allow(clippy::too_many_lines)] // Linear admission-to-receipt flow keeps the capture boundary auditable.
fn run_promo_worker(arguments: &[String]) -> VisualResult<()> {
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

    let fixture_executable = env::current_exe().map_err(VisualError::Io)?;
    let window_name = format!("tabbeacon-g100-{run_id}");
    let start_path = evidence_root.join("showcase-start-unix-ms.txt");
    let correlation_alias = format!("{PROMO_CORRELATION_ALIAS_PREFIX}{run_id}");
    let expected_title = showcase_title("ready", &correlation_alias)?;

    launch_showcase_tab(
        &fixture_executable,
        &window_name,
        "anchor",
        &run_id,
        &start_path,
        true,
    )?;
    let (target, target_window_match_count) = locate_unique_owned_target(&run_id, &expected_title)?;

    for role in ["web", "docs"] {
        launch_showcase_tab(
            &fixture_executable,
            &window_name,
            role,
            &run_id,
            &start_path,
            false,
        )?;
    }
    // Allow the two exact-owned sibling tabs to initialize before the first
    // public timeline frame. This is not a desktop or process discovery step.
    thread::sleep(Duration::from_millis(800));
    let start_unix_ms = unix_millis().saturating_add(1_000);
    write_new_text(&start_path, &start_unix_ms.to_string())?;

    let capture_target = OwnedWindowCaptureTarget::new(
        target.native_window_id.ok_or_else(|| {
            VisualError::Platform("owned promo target did not expose a native HWND".to_owned())
        })?,
        target.window_bounds.ok_or_else(|| {
            VisualError::Platform("owned promo target did not expose window bounds".to_owned())
        })?,
    )?;
    wait_until_unix_millis(start_unix_ms);
    let backend = PrintWindowCaptureBackend;
    let frame_interval_ms = 1_000 / u64::from(PROMO_FPS);
    let captured_frame_count = PROMO_DURATION_MS / frame_interval_ms;
    let frame_count = u32::try_from(captured_frame_count).map_err(|_| {
        VisualError::Platform("promo frame count does not fit the receipt contract".to_owned())
    })?;
    let mut dimensions = None;
    for index in 0..captured_frame_count {
        let deadline = start_unix_ms.saturating_add(index * frame_interval_ms);
        wait_until_unix_millis(deadline);
        let frame = backend.capture(&capture_target)?;
        dimensions.get_or_insert((frame.width(), frame.height()));
        let frame_path = frames_directory.join(format!("frame-{:04}.png", index + 1));
        write_png_frame(&frame_path, &frame)?;
    }
    let (frame_width, frame_height) = dimensions
        .ok_or_else(|| VisualError::Platform("promo capture produced no frames".to_owned()))?;
    let receipt = PromoShowcaseReceipt {
        source_sha: expected_head,
        windows_terminal_version: command_value("wt.exe", &["--version"]),
        frame_count,
        fps: PROMO_FPS,
        duration_ms: PROMO_DURATION_MS,
        frame_width,
        frame_height,
        target_window_match_count,
        real_windows_terminal: true,
        real_tabbeacon_renderer: true,
        real_model_session: false,
        desktop_capture: false,
        controlled_fixture_only: true,
    };
    write_new_json(&receipt_path, &receipt)?;
    println!(
        "{}",
        serde_json::to_string(&receipt).map_err(VisualError::Json)?
    );
    Ok(())
}

/// Child mode for the controlled promo tabs. It renders only fixture-derived
/// production presentation actions; it does not create a provider session or a
/// model request.
fn emit_showcase(arguments: &[String]) -> VisualResult<()> {
    let role = argument_value(arguments, "--role")?;
    let run_id = argument_value(arguments, "--run-id")?;
    let start_path = PathBuf::from(argument_value(arguments, "--start-file")?);
    if !is_safe_run_id(&run_id) || !matches!(role.as_str(), "anchor" | "web" | "docs") {
        return Err(VisualError::Platform(
            "promo showcase role or run identifier is invalid".to_owned(),
        ));
    }
    let renderer = WindowsTerminalRenderer::new(WindowsTerminalCapabilities::new(true));
    if role == "anchor" {
        render_showcase_frame(
            renderer,
            "ready",
            &format!("{PROMO_CORRELATION_ALIAS_PREFIX}{run_id}"),
            0,
        )?;
    }
    let start_unix_ms = wait_for_showcase_start(&start_path)?;
    let alias = match role.as_str() {
        "anchor" => "API",
        "web" => "WEB",
        "docs" => "DOCS",
        _ => unreachable!("validated showcase role"),
    };
    let end_unix_ms = start_unix_ms.saturating_add(PROMO_DURATION_MS);
    let mut frame_index = 0_usize;
    while unix_millis() < end_unix_ms {
        let elapsed = unix_millis().saturating_sub(start_unix_ms);
        render_showcase_frame(renderer, showcase_state(&role, elapsed), alias, frame_index)?;
        frame_index = frame_index.saturating_add(1);
        thread::sleep(Duration::from_millis(1_000 / u64::from(PROMO_FPS)));
    }
    let reset = presentation_fixture()
        .iter()
        .find(|case| case.name() == "reset")
        .ok_or_else(|| VisualError::Platform("reset fixture is missing".to_owned()))?;
    let mut stdout = io::stdout().lock();
    stdout.write_all(&renderer.render(&reset.action_with_title(alias)))?;
    stdout.flush()?;
    Ok(())
}

fn showcase_state(role: &str, elapsed_ms: u64) -> &'static str {
    if elapsed_ms < 2_000 {
        "ready"
    } else if elapsed_ms < 5_500 {
        "working"
    } else {
        match role {
            "anchor" => "result-ready",
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

fn showcase_title(fixture_name: &str, alias: &str) -> VisualResult<String> {
    let fixture = presentation_fixture()
        .iter()
        .find(|case| case.name() == fixture_name)
        .ok_or_else(|| {
            VisualError::Platform(format!("promo fixture is missing: {fixture_name}"))
        })?;
    let renderer = WindowsTerminalRenderer::new(WindowsTerminalCapabilities::new(true));
    let action = fixture.action_with_title(alias);
    let state = match &action {
        PresentationAction::Apply(state) | PresentationAction::Reset(state) => state,
    };
    renderer
        .title_for(state)
        .map(|title| title.as_str().to_owned())
        .ok_or_else(|| VisualError::Platform("promo fixture has no owned title".to_owned()))
}

fn launch_showcase_tab(
    executable: &Path,
    window_name: &str,
    role: &str,
    run_id: &str,
    start_path: &Path,
    new_window: bool,
) -> VisualResult<()> {
    let escaped_executable = executable.display().to_string().replace('\'', "''");
    let escaped_start_path = start_path.display().to_string().replace('\'', "''");
    let command = format!(
        "& '{escaped_executable}' showcase --role {role} --run-id '{run_id}' --start-file '{escaped_start_path}'; exit $LASTEXITCODE"
    );
    let mut launch = Command::new("wt.exe");
    launch.args(["-w", window_name]);
    if new_window {
        launch.args(["--pos", "80,80", "--size", "100,30"]);
    }
    launch
        .arg("new-tab")
        // This applies only to the newly created controlled fixture tab. It
        // does not edit a Windows Terminal profile, and lets the production
        // renderer's title bytes remain observable when a default profile
        // otherwise suppresses application title changes.
        .arg("--useApplicationTitle")
        .args(["pwsh.exe", "-NoLogo", "-NoProfile", "-Command", &command])
        .spawn()
        .map(|_| ())
        .map_err(|error| {
            VisualError::Platform(format!(
                "could not launch controlled promo Windows Terminal tab: {error}"
            ))
        })
}

fn locate_unique_owned_target(
    run_id: &str,
    expected_title: &str,
) -> VisualResult<(tabbeacon::visual::UiaDump, u32)> {
    let deadline = Instant::now() + Duration::from_secs(6);
    let mut last_error = None;
    while Instant::now() < deadline {
        match WindowsUiaLocator.locate_and_activate_exactly_one(run_id, expected_title) {
            Ok((target, target_window_match_count))
                if target.window_name == expected_title && target.tab_name == expected_title =>
            {
                return Ok((target, target_window_match_count));
            }
            Ok(_) => {
                last_error = Some(VisualError::Platform(
                    "owned promo locator returned a title that did not exactly match the fresh run anchor"
                        .to_owned(),
                ));
            }
            Err(error) => last_error = Some(error),
        }
        thread::sleep(Duration::from_millis(200));
    }
    Err(last_error.unwrap_or_else(|| {
        VisualError::Platform(
            "owned promo target did not appear before the bounded deadline".to_owned(),
        )
    }))
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

fn wait_for_showcase_start(path: &Path) -> VisualResult<u64> {
    let deadline = Instant::now() + Duration::from_secs(12);
    while Instant::now() < deadline {
        if let Ok(value) = fs::read_to_string(path)
            && let Ok(start) = value.trim().parse::<u64>()
        {
            return Ok(start);
        }
        thread::sleep(Duration::from_millis(100));
    }
    Err(VisualError::Platform(
        "promo showcase did not receive its bounded owned start signal".to_owned(),
    ))
}

fn write_new_text(path: &Path, value: &str) -> VisualResult<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(value.as_bytes())?;
    file.flush()?;
    Ok(())
}

fn write_new_json(path: &Path, receipt: &PromoShowcaseReceipt) -> VisualResult<()> {
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

fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}

fn wait_until_unix_millis(target: u64) {
    while unix_millis() < target {
        thread::sleep(Duration::from_millis(1));
    }
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
mod showcase_tests {
    use super::{PROMO_DURATION_MS, is_exact_sha, is_safe_run_id, showcase_state};

    #[test]
    fn promo_timeline_uses_only_admitted_production_fixture_states() {
        assert_eq!(showcase_state("anchor", 0), "ready");
        assert_eq!(showcase_state("web", 2_000), "working");
        assert_eq!(showcase_state("anchor", 5_500), "result-ready");
        assert_eq!(showcase_state("web", 5_500), "working");
        assert_eq!(showcase_state("docs", PROMO_DURATION_MS - 1), "question");
    }

    #[test]
    fn promo_identifiers_and_exact_heads_are_bounded() {
        assert!(is_safe_run_id("TB-V072-PROMO-A004"));
        assert!(!is_safe_run_id("TB V072"));
        assert!(is_exact_sha("0123456789abcdef0123456789abcdef01234567"));
        assert!(!is_exact_sha("not-a-sha"));
    }
}
