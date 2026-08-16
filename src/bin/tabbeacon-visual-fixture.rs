//! Child process used only inside an owned G03 Windows Terminal test tab.

use std::{
    io::{self, Write},
    thread,
    time::{Duration, Instant},
};

use tabbeacon::{
    presentation::presentation_fixture,
    visual::{
        FixtureDriver, LiveVisualRunRequest, VisualDisposition, VisualError, VisualResult,
        runner::run_live,
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
        while Instant::now() < deadline {
            stdout.write_all(
                &replay.title_frame_bytes[frame_index % replay.title_frame_bytes.len()],
            )?;
            stdout.flush()?;
            frame_index = frame_index.saturating_add(1);
            thread::sleep(Duration::from_millis(180));
        }
    }
    stdout.write_all(&reset.vt_bytes)?;
    stdout.flush()?;
    Ok(())
}

fn run_live_harness(arguments: &[String]) -> VisualResult<()> {
    let expected_head = argument_value(arguments, "--expected-head")?;
    let run_id = argument_value(arguments, "--run-id")?;
    let evidence_root = argument_value(arguments, "--evidence-root")?;
    let fixture_name = optional_argument_value(arguments, "--fixture");
    let summary = run_live(&LiveVisualRunRequest {
        expected_head,
        run_id,
        evidence_root: evidence_root.into(),
        fixture_name,
    })?;
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
