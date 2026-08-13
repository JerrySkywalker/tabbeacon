//! Replaceable capture backends for a positively owned Windows Terminal window.

use win_screenshot::{
    capture::{Area, Using, capture_window_ex},
    utils::find_window,
};

use super::{RgbaFrame, ScreenRect, VisualError, VisualResult};

/// An exact top-level window title admitted through UIA run-token/title matching.
///
/// This stays in the visual-test layer and is never exposed to `core` or a
/// provider. The title is accepted only from the owned target's UIA dump and
/// is resolved through `FindWindowW`, never by enumerating desktop windows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedWindowCaptureTarget {
    window_title: String,
    /// UIA geometry used only for diagnostic ROI transformation.
    pub window_bounds: ScreenRect,
}

impl OwnedWindowCaptureTarget {
    /// Creates a target from the UIA-observed, uniquely correlated window title.
    ///
    /// # Errors
    ///
    /// Returns a capture-precondition error for a missing title. It never falls
    /// back to a screen rectangle or enumerates arbitrary windows.
    pub fn new(window_title: &str, window_bounds: ScreenRect) -> VisualResult<Self> {
        (!window_title.is_empty())
            .then(|| Self {
                window_title: window_title.to_owned(),
                window_bounds,
            })
            .ok_or_else(|| {
                VisualError::Platform("owned UIA target did not provide a window title".to_owned())
            })
    }
}

/// Captures pixels without exposing a platform API to visual-oracle callers.
pub trait CaptureBackend {
    /// Stable name recorded in evidence.
    fn name(&self) -> &'static str;

    /// Captures the complete positively owned window identified by an admitted
    /// exact title.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError::Platform`] when the backend cannot produce an
    /// RGBA frame. Callers must classify that as capture/preflight evidence,
    /// never as a presentation color failure.
    fn capture(&self, target: &OwnedWindowCaptureTarget) -> VisualResult<RgbaFrame>;
}

/// Window-only `PrintWindow(PW_RENDERFULLCONTENT)` capture through the safe
/// `win-screenshot` wrapper.
///
/// Unlike desktop-rectangle GDI copying, this backend asks Windows to render
/// only the HWND found by the admitted exact title. It therefore does not
/// sample pixels behind a transparent Terminal window or from unrelated desktop
/// windows. Windows Terminal/Windows 11 are within the wrapper's documented
/// Windows 8.1+ support contract.
#[derive(Debug, Default, Clone, Copy)]
pub struct PrintWindowCaptureBackend;

impl CaptureBackend for PrintWindowCaptureBackend {
    fn name(&self) -> &'static str {
        "win-screenshot-printwindow-full-owned-window"
    }

    fn capture(&self, target: &OwnedWindowCaptureTarget) -> VisualResult<RgbaFrame> {
        let hwnd = find_window(&target.window_title).map_err(|error| {
            VisualError::Platform(format!("owned window title no longer resolved: {error:?}"))
        })?;
        if hwnd == 0 {
            return Err(VisualError::Platform(
                "owned window title resolved to a zero HWND".to_owned(),
            ));
        }
        let frame = capture_window_ex(hwnd, Using::PrintWindow, Area::Full, None, None).map_err(
            |error| VisualError::Platform(format!("PrintWindow capture unavailable: {error}")),
        )?;
        RgbaFrame::new(frame.width, frame.height, frame.pixels)
    }
}

#[cfg(test)]
mod tests {
    use super::{OwnedWindowCaptureTarget, ScreenRect};

    #[test]
    fn owned_capture_target_requires_a_nonempty_uia_window_title() {
        let bounds = ScreenRect::new(0, 0, 10, 10);
        assert!(OwnedWindowCaptureTarget::new("TB03-owned", bounds).is_ok());
        assert!(OwnedWindowCaptureTarget::new("", bounds).is_err());
    }
}
