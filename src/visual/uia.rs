//! Windows UI Automation target discovery for an owned Windows Terminal tab.

use std::{
    collections::BTreeSet,
    thread,
    time::{Duration, Instant},
};

use uiautomation::{
    UIAutomation, UIElement,
    actions::Window,
    controls::WindowControl,
    types::{ControlType, Rect},
};
use windows::Win32::UI::WindowsAndMessaging::IsWindow;
use windows::Win32::{
    Foundation::{CloseHandle, ERROR_INVALID_PARAMETER, FILETIME, HANDLE, HWND},
    System::Threading::{GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
};

// Windows Terminal can take longer than five seconds to retire a window that
// owns multiple ConPTY tabs, even after WindowPattern.Close has been accepted.
// Keep the wait bounded while allowing the exact admitted HWND to disappear
// before classifying cleanup as failed.
const EXACT_WINDOW_CLOSE_BUDGET: Duration = Duration::from_secs(30);

use super::{
    ExactOwnedWindowBackend, ExactOwnedWindowRecoveryBackend, ExactWindowObservation,
    OwnedWindowCaptureTarget, ScreenRect, UiaDump, VisualError, VisualResult, WindowActivation,
    root_workspace_anchor_fixture_alias,
};
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
        minimum_distinct_frames: usize,
    ) -> VisualResult<Vec<String>> {
        let deadline = Instant::now() + budget;
        let mut observed = BTreeSet::new();
        while Instant::now() < deadline {
            let title = self.tab.get_name().map_err(platform_error)?;
            if expected_titles.contains(&title) {
                observed.insert(title);
            }
            if observed.len() >= minimum_distinct_frames {
                break;
            }
            // Deliberately incommensurate with the 100 ms fixture cadence.
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

    /// Activates and returns a capture target for one already-registered
    /// temporary Windows Terminal window.
    ///
    /// The fixed anchor is the sole identity authority: it must resolve to one
    /// `TabItem`, one ancestor window, and the exact HWND stored by the
    /// lifecycle record. Dynamic demo-tab titles are never used to find or
    /// close a window.
    ///
    /// # Errors
    ///
    /// Returns a platform error when exact correlation, foreground activation,
    /// or usable window geometry cannot be established.
    pub fn activate_capture_target_for_exact_anchor(
        &self,
        anchor_title: &str,
        expected_hwnd: isize,
    ) -> VisualResult<OwnedWindowCaptureTarget> {
        let (observation, window) = exact_anchor_window(anchor_title)?;
        if observation.anchor_tab_match_count != 1
            || observation.target_window_match_count != 1
            || observation.native_window_id != Some(expected_hwnd)
        {
            return Err(VisualError::Platform(format!(
                "exact Windows Terminal capture refused: anchor_matches={} window_matches={} hwnd_match={}",
                observation.anchor_tab_match_count,
                observation.target_window_match_count,
                observation.native_window_id == Some(expected_hwnd)
            )));
        }
        let window = window.ok_or_else(|| {
            VisualError::Platform(
                "exact Windows Terminal capture refused because the ancestor window vanished"
                    .to_owned(),
            )
        })?;
        let control = WindowControl::try_from(window.clone()).map_err(platform_error)?;
        let set_foreground = control.set_foregrand().map_err(platform_error)?;
        window.set_focus().map_err(platform_error)?;
        if !set_foreground {
            return Err(VisualError::Platform(
                "Windows did not accept foreground activation for the exact owned fixture window"
                    .to_owned(),
            ));
        }
        let bounds = screen_rect(window.get_bounding_rectangle().map_err(platform_error)?)
            .filter(|bounds| bounds.width > 0 && bounds.height > 0)
            .ok_or_else(|| {
                VisualError::Platform(
                    "exact owned fixture window did not expose capturable bounds".to_owned(),
                )
            })?;
        OwnedWindowCaptureTarget::new(expected_hwnd, bounds)
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
        if !set_foreground {
            return Ok(OwnedTabActivation::Refused {
                dump,
                detail: "Windows did not accept foreground activation for the owned fixture window"
                    .to_owned(),
            });
        }
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

impl ExactOwnedWindowBackend for WindowsUiaLocator {
    fn observe_exact_anchor(&self, anchor_title: &str) -> VisualResult<ExactWindowObservation> {
        exact_anchor_window(anchor_title).map(|(observation, _)| observation)
    }

    fn close_exact_anchor(&self, anchor_title: &str, expected_hwnd: isize) -> VisualResult<()> {
        let (observation, window) = exact_anchor_window(anchor_title)?;
        if observation.anchor_tab_match_count != 1
            || observation.target_window_match_count != 1
            || observation.native_window_id != Some(expected_hwnd)
        {
            return Err(VisualError::Platform(format!(
                "exact Windows Terminal close refused: anchor_matches={} window_matches={} hwnd_match={}",
                observation.anchor_tab_match_count,
                observation.target_window_match_count,
                observation.native_window_id == Some(expected_hwnd)
            )));
        }
        let window = window.ok_or_else(|| {
            VisualError::Platform(
                "exact Windows Terminal close refused because the ancestor window vanished"
                    .to_owned(),
            )
        })?;
        let control = WindowControl::try_from(window).map_err(platform_error)?;
        let close_result = control.close();

        let deadline = Instant::now() + EXACT_WINDOW_CLOSE_BUDGET;
        while native_window_exists(expected_hwnd) {
            if Instant::now() >= deadline {
                let detail = close_result.as_ref().err().map_or_else(
                    || "exact close returned but its admitted HWND remained".to_owned(),
                    |error| {
                        format!("exact close was refused and its admitted HWND remained: {error}")
                    },
                );
                return Err(VisualError::Platform(detail));
            }
            thread::sleep(Duration::from_millis(50));
        }
        Ok(())
    }
}

impl ExactOwnedWindowRecoveryBackend for WindowsUiaLocator {
    fn creator_process_started_unix_ms(
        &self,
        creator_process_id: u32,
    ) -> VisualResult<Option<u64>> {
        creator_process_started_unix_ms(creator_process_id)
    }
}

struct ProcessHandle(HANDLE);

impl Drop for ProcessHandle {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        // SAFETY: this guard owns the successful `OpenProcess` result exactly once.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

#[allow(unsafe_code)]
fn creator_process_started_unix_ms(creator_process_id: u32) -> VisualResult<Option<u64>> {
    // SAFETY: the access mask is query-only, handle inheritance is disabled,
    // and the successful handle is closed by `ProcessHandle`.
    let process = match unsafe {
        OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, creator_process_id)
    } {
        Ok(process) => ProcessHandle(process),
        Err(error) if error.code() == ERROR_INVALID_PARAMETER.to_hresult() => {
            return Ok(None);
        }
        Err(error) => {
            return Err(VisualError::Platform(format!(
                "temporary Windows Terminal creator process state is unproven: {error}"
            )));
        }
    };
    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    // SAFETY: all pointers refer to initialized writable `FILETIME` values and
    // the process handle remains alive for the call.
    unsafe {
        GetProcessTimes(
            process.0,
            &raw mut creation,
            &raw mut exit,
            &raw mut kernel,
            &raw mut user,
        )
    }
    .map_err(|error| {
        VisualError::Platform(format!(
            "temporary Windows Terminal creator process time is unproven: {error}"
        ))
    })?;
    let process_created_unix_ms = filetime_unix_ms(creation).ok_or_else(|| {
        VisualError::Platform(
            "temporary Windows Terminal creator process time predates the Unix epoch".to_owned(),
        )
    })?;
    Ok(Some(process_created_unix_ms))
}

fn filetime_unix_ms(value: FILETIME) -> Option<u64> {
    const WINDOWS_TO_UNIX_EPOCH_MILLIS: u64 = 11_644_473_600_000;
    let ticks = (u64::from(value.dwHighDateTime) << 32) | u64::from(value.dwLowDateTime);
    (ticks / 10_000).checked_sub(WINDOWS_TO_UNIX_EPOCH_MILLIS)
}

#[allow(unsafe_code)]
fn native_window_exists(native_window_id: isize) -> bool {
    // UIA may retain a stale element immediately after WindowPattern.Close.
    // The native handle was admitted through the exact anchor correlation, so
    // checking only that handle is a narrower and fresher disappearance proof.
    unsafe { IsWindow(Some(HWND(native_window_id as *mut core::ffi::c_void))).as_bool() }
}

fn exact_anchor_window(
    anchor_title: &str,
) -> VisualResult<(ExactWindowObservation, Option<UIElement>)> {
    if anchor_title.is_empty() || anchor_title.chars().any(char::is_control) {
        return Err(VisualError::InvalidIdentifier(anchor_title.to_owned()));
    }
    let automation = UIAutomation::new().map_err(platform_error)?;
    let tabs = match automation
        .create_matcher()
        .name(anchor_title)
        .control_type(ControlType::TabItem)
        .timeout(0)
        .find_all()
    {
        Ok(tabs) => tabs,
        Err(error) if is_empty_match_error(&error.to_string()) => Vec::new(),
        Err(error) => return Err(platform_error(error)),
    };
    let anchor_tab_match_count = u32::try_from(tabs.len()).map_err(|_| {
        VisualError::Platform("UIA anchor count does not fit the receipt contract".to_owned())
    })?;
    let walker = automation
        .get_control_view_walker()
        .map_err(platform_error)?;
    let mut windows = std::collections::BTreeMap::<isize, UIElement>::new();
    for tab in tabs {
        let mut current = tab;
        for _ in 0..16 {
            current = walker.get_parent(&current).map_err(platform_error)?;
            if current.get_control_type().map_err(platform_error)? == ControlType::Window {
                let handle = current.get_native_window_handle().map_err(platform_error)?;
                let hwnd: HWND = handle.into();
                windows.entry(hwnd.0 as isize).or_insert(current);
                break;
            }
        }
    }
    let target_window_match_count = u32::try_from(windows.len()).map_err(|_| {
        VisualError::Platform("UIA window count does not fit the receipt contract".to_owned())
    })?;
    let (native_window_id, window) = if windows.len() == 1 {
        let (hwnd, window) = windows
            .into_iter()
            .next()
            .expect("one exact entry was checked");
        (Some(hwnd), Some(window))
    } else {
        (None, None)
    };
    Ok((
        ExactWindowObservation {
            anchor_tab_match_count,
            target_window_match_count,
            native_window_id,
        },
        window,
    ))
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
    // Normal fixtures embed the full run token. G59 instead uses a fixed-size,
    // run-bound alias because the product title alias grammar is deliberately
    // shorter than a full evidence ID. In both cases, match the resulting
    // expected title exactly at the window and tab levels: several short-lived
    // fixture windows can coexist while their children finish their bounded
    // hold, and a run-token-only match could otherwise select an older window.
    if !title_is_run_correlated(run_id, expected_title) {
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
    if expected_titles.is_empty()
        || expected_titles
            .iter()
            .any(|title| !title_is_run_correlated(run_id, title))
    {
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

fn title_is_run_correlated(run_id: &str, expected_title: &str) -> bool {
    if expected_title.contains(run_id) {
        return true;
    }
    root_workspace_anchor_fixture_alias(run_id).is_ok_and(|root_alias| {
        expected_title
            .split_once(' ')
            .is_some_and(|(status, alias)| !status.is_empty() && alias == root_alias)
    })
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

/// `uiautomation` reports an empty `find_all` as an error on some Windows
/// builds. For an exact owned-anchor lookup, that is a zero-cardinality fact,
/// not an infrastructure failure; callers can then retry or refuse cleanup
/// without broadening the selector.
fn is_empty_match_error(detail: &str) -> bool {
    let detail = detail.to_ascii_lowercase();
    detail.contains("can not find element")
        || detail.contains("cannot find element")
        || detail.contains("element not found")
        || detail.contains("0x80070490")
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{creator_process_started_unix_ms, is_empty_match_error, title_is_run_correlated};
    use crate::visual::root_workspace_anchor_fixture_alias;

    #[test]
    fn root_workspace_fixture_title_is_correlated_without_exceeding_alias_bounds() {
        let run_id = "TB-V051-G59-anchor";
        let alias = root_workspace_anchor_fixture_alias(run_id).expect("safe fixture alias");

        assert!(title_is_run_correlated(run_id, &format!("⠋ {alias}")));
        assert!(!title_is_run_correlated(run_id, "⠋ TB59-not-this-run"));
    }

    #[test]
    fn creator_process_probe_binds_the_current_process_instance() {
        let started = creator_process_started_unix_ms(std::process::id())
            .expect("current process state is queryable")
            .expect("current process is active");
        let now = u64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("test clock follows Unix epoch")
                .as_millis(),
        )
        .expect("current Unix milliseconds fit u64");

        assert!(started <= now);
    }

    #[test]
    fn creator_process_probe_distinguishes_an_absent_pid() {
        assert_eq!(
            creator_process_started_unix_ms(u32::MAX).expect("absent PID is positively observed"),
            None
        );
    }

    #[test]
    fn empty_uia_find_all_is_classified_as_zero_cardinality() {
        assert!(is_empty_match_error("can not find element"));
        assert!(is_empty_match_error("HRESULT 0x80070490"));
        assert!(!is_empty_match_error("access denied"));
    }
}
