//! The G03 driver for the deterministic, provider-free G02 fixture.

use crate::presentation::{
    PresentationAction, PresentationFixtureCase, WindowsTerminalCapabilities,
    WindowsTerminalRenderer, presentation_fixture,
};
use crate::settings::{PresentationSettings, PresentationTheme};
use sha2::{Digest, Sha256};

use super::{ColorSemantic, VisualError, VisualResult};

/// Dedicated real-provider fixture name for the G59 root-anchor acceptance.
pub const ROOT_WORKSPACE_ANCHOR_FIXTURE_NAME: &str = "root-workspace-anchor";

/// One uniquely identified replay of a presentation fixture case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisualTestCase {
    /// Stable G02 fixture name.
    pub fixture_name: String,
    /// Unique, title-policy-sanitized title expected through UIA.
    pub expected_title: String,
    /// Every admissible title frame for a worker-animated fixture.
    pub expected_title_frames: Vec<String>,
    /// Semantic color expected for a non-default G02 state.
    pub expected_color: ColorSemantic,
    /// Palette used to resolve the semantic color for this replay.
    pub theme: PresentationTheme,
    /// Whether the fixture uses indeterminate progress and needs a bounded
    /// animation observation.
    pub expects_animation: bool,
    /// Whether UIA must observe at least two distinct title frames.
    pub expects_title_animation: bool,
}

/// A fixture's typed action and exact bytes after the production renderer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixtureReplay {
    /// Case metadata used by the test session and oracles.
    pub case: VisualTestCase,
    /// Production typed presentation action.
    pub action: PresentationAction,
    /// Exact VT bytes sent to the fixture child process.
    pub vt_bytes: Vec<u8>,
    /// Title-only production renderer bytes advanced by the fixture child.
    pub title_frame_bytes: Vec<Vec<u8>>,
}

/// Drives G02 fixtures through the production policy and renderer.
#[derive(Debug, Clone)]
pub struct FixtureDriver {
    renderer: WindowsTerminalRenderer,
}

impl Default for FixtureDriver {
    fn default() -> Self {
        Self::with_settings(
            WindowsTerminalCapabilities::new(true),
            PresentationSettings::default(),
        )
    }
}

impl FixtureDriver {
    /// Creates a driver using explicit Windows Terminal capabilities.
    #[must_use]
    pub const fn new(capabilities: WindowsTerminalCapabilities) -> Self {
        Self {
            renderer: WindowsTerminalRenderer::new(capabilities),
        }
    }

    /// Creates a driver using the supplied presentation settings for a visual candidate.
    #[must_use]
    pub const fn with_settings(
        capabilities: WindowsTerminalCapabilities,
        settings: PresentationSettings,
    ) -> Self {
        Self {
            renderer: WindowsTerminalRenderer::with_settings(capabilities, settings),
        }
    }

    /// Replays every named G02 fixture with a unique title for one visual run.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError::InvalidIdentifier`] when `run_id` cannot be used
    /// safely in a title/session correlation token.
    pub fn all_cases(&self, run_id: &str) -> VisualResult<Vec<FixtureReplay>> {
        presentation_fixture()
            .iter()
            .map(|case| self.replay(case, run_id))
            .collect()
    }

    /// Replays one named fixture with a unique title for one visual run.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError::InvalidIdentifier`] when `run_id` cannot be used
    /// safely in a title/session correlation token.
    pub fn replay(
        &self,
        fixture: &PresentationFixtureCase,
        run_id: &str,
    ) -> VisualResult<FixtureReplay> {
        if !is_safe_run_id(run_id) {
            return Err(VisualError::InvalidIdentifier(run_id.to_owned()));
        }
        let repository_alias = format!("TB03-{run_id}-{}", fixture.name());
        let action = fixture.action_with_title(&repository_alias);
        let state = match &action {
            PresentationAction::Apply(state) | PresentationAction::Reset(state) => state,
        };
        let expected_color = match state.tab_color() {
            crate::presentation::TabColor::Default => ColorSemantic::Default,
            crate::presentation::TabColor::Working => ColorSemantic::Working,
            crate::presentation::TabColor::ResultReady => ColorSemantic::ResultReady,
            crate::presentation::TabColor::Approval => ColorSemantic::Approval,
            crate::presentation::TabColor::Question => ColorSemantic::Question,
            crate::presentation::TabColor::Warning => ColorSemantic::Warning,
            crate::presentation::TabColor::Interrupted => ColorSemantic::Interrupted,
            crate::presentation::TabColor::Failed => ColorSemantic::Failed,
        };
        let expected_title = self.renderer.title_for(state).ok_or_else(|| {
            VisualError::Platform(
                "visual fixture requires TabBeacon title ownership for UIA correlation".to_owned(),
            )
        })?;
        let expects_title_animation = state.title_status()
            == crate::presentation::TitleStatus::Working
            && self.renderer.settings().activity().uses_worker_animation();
        let expected_title_frames = if expects_title_animation {
            self.renderer
                .settings()
                .spinner()
                .frames()
                .iter()
                .enumerate()
                .filter_map(|(index, _)| self.renderer.title_for_spinner_frame(state, index))
                .map(|title| title.as_str().to_owned())
                .collect::<Vec<_>>()
        } else {
            vec![expected_title.as_str().to_owned()]
        };
        let title_frame_bytes = if expects_title_animation {
            self.renderer
                .settings()
                .spinner()
                .frames()
                .iter()
                .enumerate()
                .map(|(index, _)| self.renderer.render_title_spinner_frame(state, index))
                .collect()
        } else {
            Vec::new()
        };
        let case = VisualTestCase {
            fixture_name: fixture.name().to_owned(),
            expected_title: expected_title.as_str().to_owned(),
            expected_title_frames,
            expected_color,
            theme: self.renderer.settings().theme(),
            expects_animation: matches!(
                state.progress(),
                crate::presentation::Progress::Indeterminate
            ) && self.renderer.uses_progress_animation(),
            expects_title_animation,
        };
        let vt_bytes = self.renderer.render(&action);
        Ok(FixtureReplay {
            case,
            action,
            vt_bytes,
            title_frame_bytes,
        })
    }

    /// Replays the G59 root-workspace-anchor visual acceptance case.
    ///
    /// The fixture child executes the real Codex hook runtime with this exact
    /// root alias, then observes alternate tool and subagent CWDs. The replay
    /// provides only the expected production-rendered title and semantic color
    /// to the bounded Windows Terminal harness.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError::InvalidIdentifier`] when `run_id` cannot safely
    /// participate in the uniquely correlated title.
    pub fn root_workspace_anchor_replay(&self, run_id: &str) -> VisualResult<FixtureReplay> {
        let root_alias = root_workspace_anchor_fixture_alias(run_id)?;
        let working = presentation_fixture()
            .iter()
            .find(|fixture| fixture.name() == "working")
            .ok_or_else(|| VisualError::Platform("working fixture is missing".to_owned()))?;
        let action = working.action_with_title(&root_alias);
        let state = match &action {
            PresentationAction::Apply(state) | PresentationAction::Reset(state) => state,
        };
        let expected_title = self.renderer.title_for(state).ok_or_else(|| {
            VisualError::Platform(
                "root-workspace visual fixture requires TabBeacon title ownership".to_owned(),
            )
        })?;
        Ok(FixtureReplay {
            case: VisualTestCase {
                fixture_name: ROOT_WORKSPACE_ANCHOR_FIXTURE_NAME.to_owned(),
                expected_title: expected_title.as_str().to_owned(),
                expected_title_frames: vec![expected_title.as_str().to_owned()],
                expected_color: ColorSemantic::Working,
                theme: self.renderer.settings().theme(),
                expects_animation: false,
                expects_title_animation: false,
            },
            action: action.clone(),
            vt_bytes: self.renderer.render(&action),
            title_frame_bytes: Vec::new(),
        })
    }

    /// Produces the G02 reset action for a title owned by this visual run.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError::InvalidIdentifier`] when `run_id` is unsafe.
    pub fn reset(&self, run_id: &str) -> VisualResult<FixtureReplay> {
        let reset = presentation_fixture()
            .iter()
            .find(|case| case.name() == "reset")
            .ok_or_else(|| VisualError::Platform("G02 reset fixture is missing".to_owned()))?;
        self.replay(reset, run_id)
    }
}

/// Alias used only by the owned, uniquely correlated G59 visual fixture.
///
/// The title-policy alias bound is shorter than a full evidence run ID, so the
/// fixture uses a fixed-length hash-derived suffix. The surrounding owned
/// Windows Terminal window name retains the complete run ID.
///
/// # Errors
///
/// Returns [`VisualError::InvalidIdentifier`] when `run_id` is unsafe.
pub fn root_workspace_anchor_fixture_alias(run_id: &str) -> VisualResult<String> {
    if !is_safe_run_id(run_id) {
        return Err(VisualError::InvalidIdentifier(run_id.to_owned()));
    }
    let digest = format!("{:x}", Sha256::digest(run_id.as_bytes()));
    Ok(format!("TB59-{}", &digest[..15]))
}

fn is_safe_run_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

#[cfg(test)]
mod tests {
    use super::{FixtureDriver, ROOT_WORKSPACE_ANCHOR_FIXTURE_NAME};
    use crate::presentation::presentation_fixture;

    #[test]
    fn default_driver_uses_the_normal_single_channel_presentation() {
        let working = presentation_fixture()
            .iter()
            .find(|fixture| fixture.name() == "working")
            .expect("working fixture exists");
        let replay = FixtureDriver::default()
            .replay(working, "G58-normal")
            .expect("normal fixture replay is valid");
        let output = String::from_utf8(replay.vt_bytes).expect("fixture output is UTF-8");

        assert!(replay.case.expects_title_animation);
        assert!(
            !output.contains("]9;4;3;0"),
            "normal visual evidence must not model simultaneous title and ring activity"
        );
    }

    #[test]
    fn root_workspace_anchor_replay_is_static_and_uses_a_safe_root_alias() {
        let replay = FixtureDriver::default()
            .root_workspace_anchor_replay("TB59-anchor")
            .expect("root workspace fixture replay is valid");

        assert_eq!(replay.case.fixture_name, ROOT_WORKSPACE_ANCHOR_FIXTURE_NAME);
        assert_eq!(replay.case.expected_title_frames.len(), 1);
        assert!(replay.case.expected_title.contains("TB59-"));
        assert!(!replay.case.expected_title.contains("TB59-anchor"));
        assert!(!replay.case.expects_title_animation);
        assert!(!replay.case.expects_animation);
    }
}
