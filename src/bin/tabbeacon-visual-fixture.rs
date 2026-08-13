//! Child process used only inside an owned G03 Windows Terminal test tab.

use std::{
    io::{self, Write},
    thread,
    time::Duration,
};

use tabbeacon::{
    presentation::presentation_fixture,
    visual::{FixtureDriver, VisualError, VisualResult},
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
    if command != "emit" {
        return Err(VisualError::Platform(
            "expected `emit --fixture <name> --run-id <id> --hold-ms <milliseconds>`".to_owned(),
        ));
    }
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
    thread::sleep(Duration::from_millis(hold_millis));
    stdout.write_all(&reset.vt_bytes)?;
    stdout.flush()?;
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
