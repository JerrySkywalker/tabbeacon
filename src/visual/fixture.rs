//! The G03 driver for the deterministic, provider-free G02 fixture.

use crate::presentation::{
    PresentationAction, PresentationFixtureCase, WindowsTerminalCapabilities,
    WindowsTerminalRenderer, presentation_fixture,
};
use crate::settings::{PresentationSettings, PresentationTheme};

use super::{ColorSemantic, VisualError, VisualResult};

/// One uniquely titled replay of a G02 fixture case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisualTestCase {
    /// Stable G02 fixture name.
    pub fixture_name: String,
    /// Unique, title-policy-sanitized title expected through UIA.
    pub expected_title: String,
    /// Semantic color expected for a non-default G02 state.
    pub expected_color: ColorSemantic,
    /// Palette used to resolve the semantic color for this replay.
    pub theme: PresentationTheme,
    /// Whether the fixture uses indeterminate progress and needs a bounded
    /// animation observation.
    pub expects_animation: bool,
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

    /// Creates a driver using the supplied v0.1 settings for a visual candidate.
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
        let title = format!("TB03-{run_id}-{}", fixture.name());
        let action = fixture.action_with_title(&title);
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
        let case = VisualTestCase {
            fixture_name: fixture.name().to_owned(),
            expected_title: expected_title.as_str().to_owned(),
            expected_color,
            theme: self.renderer.settings().theme(),
            expects_animation: matches!(
                state.progress(),
                crate::presentation::Progress::Indeterminate
            ) && self.renderer.uses_progress_animation(),
        };
        let vt_bytes = self.renderer.render(&action);
        Ok(FixtureReplay {
            case,
            action,
            vt_bytes,
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

fn is_safe_run_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}
