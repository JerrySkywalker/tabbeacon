//! Windows UI Automation target discovery for an owned Windows Terminal tab.

use uiautomation::{
    UIAutomation, UIElement,
    controls::WindowControl,
    types::{ControlType, Rect},
};

use super::{ScreenRect, UiaDump, VisualError, VisualResult, WindowActivation};

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

    /// Activates only the exact run-token/title-correlated fixture window.
    ///
    /// This is a harness-only visibility precondition, never product behavior.
    /// It invokes the maintained UIA binding's `SetForegroundWindow` wrapper
    /// and then requests UIA keyboard focus. It does not send input, mutate
    /// Terminal settings, or act on an unrelated window.
    ///
    /// # Errors
    ///
    /// Returns an error when the owned target cannot be resolved or Windows/UIA
    /// rejects a required focus operation. Callers classify this as a capture
    /// precondition, not a title or color assertion failure.
    pub fn activate_owned_window(
        &self,
        run_id: &str,
        expected_title: &str,
    ) -> VisualResult<WindowActivation> {
        let (window, _) = owned_window_and_tab(run_id, expected_title)?;
        let control = WindowControl::try_from(window.clone()).map_err(platform_error)?;
        let set_foreground = control.set_foregrand().map_err(platform_error)?;
        window.set_focus().map_err(platform_error)?;
        Ok(WindowActivation {
            set_foreground,
            set_focus: true,
        })
    }
}

impl TargetLocator for WindowsUiaLocator {
    fn locate(&self, run_id: &str, expected_title: &str) -> VisualResult<UiaDump> {
        let (window, tab) = owned_window_and_tab(run_id, expected_title)?;
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
            activation: None,
            detail:
                "resolved only the run-token-matched Windows Terminal window and exact fixture tab"
                    .to_owned(),
        })
    }
}

fn owned_window_and_tab(
    run_id: &str,
    expected_title: &str,
) -> VisualResult<(UIElement, UIElement)> {
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
    Ok((window, tab))
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
