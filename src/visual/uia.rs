//! Windows UI Automation target discovery for an owned Windows Terminal tab.

use uiautomation::{
    UIAutomation,
    types::{ControlType, Rect},
};

use super::{ScreenRect, UiaDump, VisualError, VisualResult};

/// Read-only UIA lookup contract for the dedicated visual-test target.
pub trait TargetLocator {
    /// Locates the uniquely titled, owned Windows Terminal tab.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError::Platform`] when UIA cannot resolve the dedicated
    /// target. It never operates or injects into a terminal window.
    fn locate(&self, run_id: &str, expected_title: &str) -> VisualResult<UiaDump>;
}

/// The Windows UIA implementation used by G03.
#[derive(Debug, Default, Clone, Copy)]
pub struct WindowsUiaLocator;

impl WindowsUiaLocator {
    /// Reports whether the UIA root can be created in this process context.
    #[must_use]
    pub fn is_available() -> bool {
        UIAutomation::new()
            .and_then(|automation| automation.get_root_element())
            .is_ok()
    }

    /// Returns the UIA desktop geometry when the interactive root is readable.
    #[must_use]
    pub fn desktop_geometry() -> Option<ScreenRect> {
        let automation = UIAutomation::new().ok()?;
        let root = automation.get_root_element().ok()?;
        screen_rect(root.get_bounding_rectangle().ok()?)
    }
}

impl TargetLocator for WindowsUiaLocator {
    fn locate(&self, run_id: &str, expected_title: &str) -> VisualResult<UiaDump> {
        let automation = UIAutomation::new().map_err(platform_error)?;
        let window = automation
            .create_matcher()
            .contains_name(run_id)
            .control_type(ControlType::Window)
            .timeout(0)
            .find_first()
            .map_err(platform_error)?;
        let tab = automation
            .create_matcher()
            .from_ref(&window)
            .depth(12)
            .name(expected_title)
            .control_type(ControlType::TabItem)
            .timeout(0)
            .find_first()
            .map_err(platform_error)?;

        Ok(UiaDump {
            window_name: window.get_name().map_err(platform_error)?,
            tab_name: tab.get_name().map_err(platform_error)?,
            window_bounds: screen_rect(window.get_bounding_rectangle().map_err(platform_error)?),
            tab_bounds: screen_rect(tab.get_bounding_rectangle().map_err(platform_error)?),
            native_window_handle: window
                .get_native_window_handle()
                .map(|handle| handle.to_string())
                .ok(),
            window_has_keyboard_focus: window.has_keyboard_focus().ok(),
            detail:
                "resolved only the run-token-matched Windows Terminal window and exact fixture tab"
                    .to_owned(),
        })
    }
}

fn screen_rect(rectangle: Rect) -> Option<ScreenRect> {
    let width = rectangle.get_right().checked_sub(rectangle.get_left())?;
    let height = rectangle.get_bottom().checked_sub(rectangle.get_top())?;
    Some(ScreenRect::new(
        rectangle.get_left(),
        rectangle.get_top(),
        u32::try_from(width).ok()?,
        u32::try_from(height).ok()?,
    ))
}

fn platform_error(error: impl std::fmt::Display) -> VisualError {
    VisualError::Platform(format!(
        "UI Automation unavailable or target unresolved: {error}"
    ))
}
