//! Windows UI Automation target discovery for an owned Windows Terminal tab.

use std::{
    collections::BTreeSet,
    thread,
    time::{Duration, Instant},
};

use uiautomation::{
    UIAutomation, UIElement,
    controls::WindowControl,
    types::{ControlType, Rect},
};
use windows::Win32::Foundation::HWND;

use super::{ScreenRect, UiaDump, VisualError, VisualResult, WindowActivation};
use crate::title_authority::TitleProbeSample;

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

/// A live UIA tab element that was already correlated to one owned test window.
pub struct OwnedTabTitleReader {
    tab: UIElement,
}

/// A live owned Windows Terminal window already correlated through a unique
/// static anchor tab. It can select a subsequently opened sibling tab without
/// using the sibling's mutable title as identity.
pub struct OwnedWindowTabReader {
    window: UIElement,
}

/// Outcome of trying to activate one already-correlated owned tab.
pub enum OwnedTabActivation {
    /// The owned window accepted foreground and focus activation.
    Activated {
        /// UIA evidence for the admitted top-level owned window and tab.
        dump: UiaDump,
        /// Live title reader pinned to that exact owned tab.
        title_reader: OwnedTabTitleReader,
        /// Window reader retained for owned sibling-tab correlation.
        window_reader: OwnedWindowTabReader,
    },
    /// The owned target resolved, but Windows refused a visibility precondition.
    Refused {
        /// UIA evidence retained so the caller can classify capture as blocked.
        dump: UiaDump,
        /// Sanitized platform diagnostic for the refused activation.
        detail: String,
    },
}

impl OwnedTabTitleReader {
    /// Samples the exact already-correlated tab on a bounded monotonic
    /// timeline, reducing each raw title to a non-sensitive classification.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError::Platform`] when UIA cannot read the live title
    /// property for the already-correlated tab.
    pub fn observe_title_samples(
        &self,
        desired_title: &str,
        offsets: &[Duration],
    ) -> VisualResult<Vec<TitleProbeSample>> {
        let start = Instant::now();
        let mut samples = Vec::with_capacity(offsets.len());
        for offset in offsets {
            let deadline = start + *offset;
            let remaining = deadline.saturating_duration_since(Instant::now());
            if !remaining.is_zero() {
                thread::sleep(remaining);
            }
            let title = self.tab.get_name().map_err(platform_error)?;
            samples.push(if title == desired_title {
                TitleProbeSample::Desired
            } else {
                TitleProbeSample::Other
            });
        }
        Ok(samples)
    }

    /// Samples admitted title frames from the exact tab selected during owned
    /// window activation.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError::Platform`] when UIA cannot read the live title
    /// property for the already-correlated tab.
    pub fn observe_frames(
        &self,
        expected_titles: &[String],
        budget: Duration,
    ) -> VisualResult<Vec<String>> {
        let deadline = Instant::now() + budget;
        let mut observed = BTreeSet::new();
        while Instant::now() < deadline {
            let title = self.tab.get_name().map_err(platform_error)?;
            if expected_titles.contains(&title) {
                observed.insert(title);
            }
            if observed.len() >= 2 {
                break;
            }
            // Deliberately incommensurate with the 180 ms fixture cadence.
            thread::sleep(Duration::from_millis(137));
        }
        Ok(observed.into_iter().collect())
    }
}

impl OwnedWindowTabReader {
    /// Returns the one tab in this already-correlated window that is not the
    /// exact static anchor. Its title may be native, desired, or later
    /// overwritten; only the anchor establishes ownership.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError::Platform`] when UIA cannot inspect the retained
    /// owned window or a tab name.
    pub fn non_anchor_tab(&self, anchor_title: &str) -> VisualResult<Option<OwnedTabTitleReader>> {
        let automation = UIAutomation::new().map_err(platform_error)?;
        let tabs = automation
            .create_matcher()
            .from_ref(&self.window)
            .depth(12)
            .control_type(ControlType::TabItem)
            .timeout(0)
            .find_all()
            .map_err(platform_error)?;
        let mut candidate = None;
        for tab in tabs {
            if tab.get_name().map_err(platform_error)? != anchor_title {
                if candidate.is_some() {
                    return Err(VisualError::Platform(
                        "owned title probe window has ambiguous non-anchor tabs".to_owned(),
                    ));
                }
                candidate = Some(tab);
            }
        }
        Ok(candidate.map(|tab| OwnedTabTitleReader { tab }))
    }
}

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

    /// Activates a worker-animated target at whichever admitted frame is live.
    ///
    /// # Errors
    ///
    /// Returns an error when no admitted frame identifies the owned target or
    /// Windows refuses foreground/focus activation.
    pub fn activate_owned_window_any(
        &self,
        run_id: &str,
        expected_titles: &[String],
    ) -> VisualResult<WindowActivation> {
        let (window, _) = owned_window_and_tab_any(run_id, expected_titles)?;
        let control = WindowControl::try_from(window.clone()).map_err(platform_error)?;
        let set_foreground = control.set_foregrand().map_err(platform_error)?;
        window.set_focus().map_err(platform_error)?;
        Ok(WindowActivation {
            set_foreground,
            set_focus: true,
        })
    }

    /// Locates a worker-animated target at whichever admitted frame is live.
    ///
    /// # Errors
    ///
    /// Returns an error when no admitted frame resolves to one owned Windows
    /// Terminal window and tab, or UIA properties cannot be read.
    pub fn locate_any(&self, run_id: &str, expected_titles: &[String]) -> VisualResult<UiaDump> {
        let (window, tab) = owned_window_and_tab_any(run_id, expected_titles)?;
        uia_dump(&window, &tab, None)
    }

    /// Locates and activates a worker-animated target without resolving its
    /// changing title a second time.
    ///
    /// # Errors
    ///
    /// Returns an error when no admitted frame identifies the owned target,
    /// UIA properties cannot be read, or Windows rejects activation.
    pub fn locate_and_activate_any(
        &self,
        run_id: &str,
        expected_titles: &[String],
    ) -> VisualResult<UiaDump> {
        match self.locate_and_activate_any_with_title_reader(run_id, expected_titles)? {
            OwnedTabActivation::Activated { dump, .. } => Ok(dump),
            OwnedTabActivation::Refused { detail, .. } => Err(VisualError::Platform(detail)),
        }
    }

    /// Activates an owned window and retains its exact animated tab for later
    /// title sampling without a second dynamic-title lookup.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError::Platform`] when the owned target cannot be
    /// resolved or Windows refuses foreground/focus activation.
    pub fn locate_and_activate_any_with_title_reader(
        &self,
        run_id: &str,
        expected_titles: &[String],
    ) -> VisualResult<OwnedTabActivation> {
        let (window, tab) = owned_window_and_tab_any(run_id, expected_titles)?;
        let mut dump = uia_dump(&window, &tab, None)?;
        let control = match WindowControl::try_from(window.clone()) {
            Ok(control) => control,
            Err(error) => {
                return Ok(OwnedTabActivation::Refused {
                    dump,
                    detail: platform_error(error).to_string(),
                });
            }
        };
        let set_foreground = match control.set_foregrand() {
            Ok(value) => value,
            Err(error) => {
                return Ok(OwnedTabActivation::Refused {
                    dump,
                    detail: platform_error(error).to_string(),
                });
            }
        };
        if let Err(error) = window.set_focus() {
            return Ok(OwnedTabActivation::Refused {
                dump,
                detail: platform_error(error).to_string(),
            });
        }
        let activation = WindowActivation {
            set_foreground,
            set_focus: true,
        };
        dump.activation = Some(activation);
        Ok(OwnedTabActivation::Activated {
            dump,
            title_reader: OwnedTabTitleReader { tab },
            window_reader: OwnedWindowTabReader { window },
        })
    }
}

impl TargetLocator for WindowsUiaLocator {
    fn locate(&self, run_id: &str, expected_title: &str) -> VisualResult<UiaDump> {
        let (window, tab) = owned_window_and_tab(run_id, expected_title)?;
        uia_dump(&window, &tab, None)
    }
}

fn uia_dump(
    window: &UIElement,
    tab: &UIElement,
    activation: Option<WindowActivation>,
) -> VisualResult<UiaDump> {
    let native_window_handle = window.get_native_window_handle().ok();
    let native_window_id = native_window_handle.map(|handle| {
        let hwnd: HWND = handle.into();
        hwnd.0 as isize
    });
    Ok(UiaDump {
        window_name: window.get_name().map_err(platform_error)?,
        tab_name: tab.get_name().map_err(platform_error)?,
        window_bounds: screen_rect(window.get_bounding_rectangle().map_err(platform_error)?),
        tab_bounds: screen_rect(tab.get_bounding_rectangle().map_err(platform_error)?),
        native_window_handle: native_window_handle.map(|handle| handle.to_string()),
        native_window_id,
        window_has_keyboard_focus: window.has_keyboard_focus().ok(),
        activation,
        detail: "resolved an owned Windows Terminal window and tab at an admitted title frame through UIA"
            .to_owned(),
    })
}

fn owned_window_and_tab(
    run_id: &str,
    expected_title: &str,
) -> VisualResult<(UIElement, UIElement)> {
    let automation = UIAutomation::new().map_err(platform_error)?;
    // `expected_title` is generated by FixtureDriver and embeds the safe
    // per-run token. Match it exactly at the window level as well as the tab
    // level: several short-lived fixture windows can coexist while their
    // children finish their bounded hold, and a run-token-only match could
    // otherwise select an older window first.
    if !expected_title.contains(run_id) {
        return Err(VisualError::Platform(
            "fixture title is not correlated to the requested visual run".to_owned(),
        ));
    }
    let window = automation
        .create_matcher()
        .name(expected_title)
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

fn owned_window_and_tab_any(
    run_id: &str,
    expected_titles: &[String],
) -> VisualResult<(UIElement, UIElement)> {
    if expected_titles.is_empty() || expected_titles.iter().any(|title| !title.contains(run_id)) {
        return Err(VisualError::Platform(
            "animated fixture titles are not correlated to the requested visual run".to_owned(),
        ));
    }
    let mut last_error = None;
    for expected_title in expected_titles {
        match owned_window_and_tab(run_id, expected_title) {
            Ok(target) => return Ok(target),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or_else(|| {
        VisualError::Platform("no admitted animated title frame was visible".to_owned())
    }))
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
