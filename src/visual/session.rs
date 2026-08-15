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
    hold_millis: u64,
}

impl Default for TerminalTestSessionLauncher {
    fn default() -> Self {
        Self {
            requested_size: (100, 30),
            requested_position: (80, 80),
            // A capture needs initial UIA discovery, owned-window activation,
            // focus re-observation, and three bounded frames. Keep the child
            // alive long enough for that sequence while retaining deterministic
            // reset-and-exit cleanup.
            hold_millis: 10_000,
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
        if !fixture_executable.is_file() {
            return Err(VisualError::Platform(format!(
                "fixture executable does not exist: {}",
                fixture_executable.display()
            )));
        }
        // A fixture needs its own owned window. Reusing one named window turns
        // later fixtures into additional tabs; then Windows Terminal may retain
        // valid UIA elements for a hidden or expiring earlier tab. That makes a
        // pixel capture target ambiguous even when the title lookup succeeds.
        let window_name = fixture_window_name(run_id, &replay.case.fixture_name);
        let position = format!(
            "{},{}",
            self.requested_position.0, self.requested_position.1
        );
        let size = format!("{},{}", self.requested_size.0, self.requested_size.1);
        Command::new("wt.exe")
            .args(["-w", &window_name, "--pos", &position, "--size", &size])
            .arg("new-tab")
            .arg(fixture_executable)
            .args([
                "emit",
                "--fixture",
                &replay.case.fixture_name,
                "--run-id",
                run_id,
                "--hold-ms",
                &self.hold_millis.to_string(),
            ])
            .spawn()
            .map_err(|error| {
                VisualError::Platform(format!(
                    "could not launch owned Windows Terminal session: {error}"
                ))
            })?;
        Ok(TerminalTestSession {
            run_id: run_id.to_owned(),
            window_name,
            expected_title: replay.case.expected_title.clone(),
            requested_size: self.requested_size,
        })
    }
}

fn fixture_window_name(run_id: &str, fixture_name: &str) -> String {
    format!("tabbeacon-g03-{run_id}-{fixture_name}")
}

#[cfg(test)]
mod tests {
    use super::fixture_window_name;

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
