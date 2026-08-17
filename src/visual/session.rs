//! Owned Windows Terminal fixture-session launch contract.

use std::{path::Path, process::Command};

use super::{FixtureReplay, VisualError, VisualResult};

/// A dedicated Windows Terminal test session positively correlated by run ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTestSession {
    /// Unique test run ID.
    pub run_id: String,
    /// Dedicated Windows Terminal window name.
    pub window_name: String,
    /// Expected UIA tab title.
    pub expected_title: String,
    /// Fixed requested terminal grid size, recorded even if Terminal rounds it.
    pub requested_size: (u16, u16),
}

/// Launches a fixture child in an owned Windows Terminal window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalTestSessionLauncher {
    requested_size: (u16, u16),
    requested_position: (u16, u16),
}

const STATIC_FIXTURE_HOLD_MILLIS: u64 = 10_000;
// UIA queries are synchronous and can take materially longer than the runner's
// nominal polling cadence. A title-animated fixture therefore reserves enough
// time for activation, actual UIA observation, and owned-window capture.
const TITLE_ANIMATION_FIXTURE_HOLD_MILLIS: u64 = 60_000;

impl Default for TerminalTestSessionLauncher {
    fn default() -> Self {
        Self {
            requested_size: (100, 30),
            requested_position: (80, 80),
        }
    }
}

impl TerminalTestSessionLauncher {
    /// Launches the supplied fixture executable in a new, uniquely named
    /// Windows Terminal window. The child emits G02 reset bytes before its
    /// bounded hold completes; this launcher never closes unrelated windows.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError::Platform`] when `wt.exe` cannot launch the owned
    /// fixture session.
    pub fn launch(
        &self,
        fixture_executable: &Path,
        replay: &FixtureReplay,
        run_id: &str,
    ) -> VisualResult<TerminalTestSession> {
        let hold_millis = fixture_hold_millis(replay.case.expects_title_animation);
        let arguments = [
            "emit".to_owned(),
            "--fixture".to_owned(),
            replay.case.fixture_name.clone(),
            "--run-id".to_owned(),
            run_id.to_owned(),
            "--hold-ms".to_owned(),
            hold_millis.to_string(),
        ];
        self.launch_program(
            fixture_executable,
            run_id,
            &replay.case.fixture_name,
            &replay.case.expected_title,
            &[],
            &arguments,
        )
    }

    /// Launches an owned static-title anchor window for a title-authority
    /// probe. The anchor uses the same production-rendered fixture path as
    /// the visual runner, so UIA correlation does not depend on a separate
    /// Windows Terminal launch-title mechanism. A second probe tab then tests
    /// the profile's ordinary application-title policy.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError::Platform`] when Windows Terminal cannot launch
    /// the owned fixture session.
    pub fn launch_title_authority_anchor(
        &self,
        executable: &Path,
        run_id: &str,
        anchor_run_id: &str,
        anchor_title: &str,
        hold_millis: u64,
    ) -> VisualResult<TerminalTestSession> {
        let arguments = [
            "__title-probe-fixture-v1".to_owned(),
            anchor_run_id.to_owned(),
            hold_millis.to_string(),
        ];
        self.launch_program(
            executable,
            run_id,
            "title-authority",
            anchor_title,
            &[],
            &arguments,
        )
    }

    /// Launches the short-lived probe child in the exact named anchor window.
    /// The tab deliberately receives no title-policy override, so the active
    /// probe observes the profile's ordinary application-title behavior.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError::Platform`] when the executable is unavailable or
    /// Windows Terminal cannot create the owned probe tab.
    pub fn launch_title_authority_probe(
        &self,
        executable: &Path,
        window_name: &str,
        run_id: &str,
        hold_millis: u64,
    ) -> VisualResult<()> {
        if !executable.is_file() {
            return Err(VisualError::Platform(format!(
                "fixture executable does not exist: {}",
                executable.display()
            )));
        }
        Command::new("wt.exe")
            .args(["-w", window_name])
            .arg("new-tab")
            .arg(executable)
            .args(["__title-probe-fixture-v1", run_id, &hold_millis.to_string()])
            .spawn()
            .map(|_| ())
            .map_err(|error| {
                VisualError::Platform(format!(
                    "could not launch owned Windows Terminal probe tab: {error}"
                ))
            })
    }

    fn launch_program(
        self,
        executable: &Path,
        run_id: &str,
        fixture_name: &str,
        expected_title: &str,
        tab_options: &[String],
        arguments: &[String],
    ) -> VisualResult<TerminalTestSession> {
        if !executable.is_file() {
            return Err(VisualError::Platform(format!(
                "fixture executable does not exist: {}",
                executable.display()
            )));
        }
        // A fixture needs its own owned window. Reusing one named window turns
        // later fixtures into additional tabs; then Windows Terminal may retain
        // valid UIA elements for a hidden or expiring earlier tab. That makes a
        // pixel capture target ambiguous even when the title lookup succeeds.
        let window_name = fixture_window_name(run_id, fixture_name);
        let position = format!(
            "{},{}",
            self.requested_position.0, self.requested_position.1
        );
        let size = format!("{},{}", self.requested_size.0, self.requested_size.1);
        Command::new("wt.exe")
            .args(["-w", &window_name, "--pos", &position, "--size", &size])
            .arg("new-tab")
            .args(tab_options)
            .arg(executable)
            .args(arguments)
            .spawn()
            .map_err(|error| {
                VisualError::Platform(format!(
                    "could not launch owned Windows Terminal session: {error}"
                ))
            })?;
        Ok(TerminalTestSession {
            run_id: run_id.to_owned(),
            window_name,
            expected_title: expected_title.to_owned(),
            requested_size: self.requested_size,
        })
    }
}

fn fixture_hold_millis(expects_title_animation: bool) -> u64 {
    if expects_title_animation {
        TITLE_ANIMATION_FIXTURE_HOLD_MILLIS
    } else {
        STATIC_FIXTURE_HOLD_MILLIS
    }
}

fn fixture_window_name(run_id: &str, fixture_name: &str) -> String {
    format!("tabbeacon-g03-{run_id}-{fixture_name}")
}

#[cfg(test)]
mod tests {
    use super::{
        STATIC_FIXTURE_HOLD_MILLIS, TITLE_ANIMATION_FIXTURE_HOLD_MILLIS, fixture_hold_millis,
        fixture_window_name,
    };

    #[test]
    fn title_animated_fixture_reserves_the_uia_observation_budget() {
        assert_eq!(fixture_hold_millis(false), STATIC_FIXTURE_HOLD_MILLIS);
        assert_eq!(
            fixture_hold_millis(true),
            TITLE_ANIMATION_FIXTURE_HOLD_MILLIS
        );
    }

    #[test]
    fn fixture_windows_are_isolated_within_one_visual_run() {
        assert_ne!(
            fixture_window_name("GHA-123-1", "working"),
            fixture_window_name("GHA-123-1", "result-ready")
        );
        assert_eq!(
            fixture_window_name("GHA-123-1", "working"),
            "tabbeacon-g03-GHA-123-1-working"
        );
    }
}
