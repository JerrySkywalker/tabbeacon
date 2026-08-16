//! Replaceable capture backends for a positively owned Windows Terminal window.

use win_screenshot::capture::{Area, Using, capture_window_ex};

use super::{RgbaFrame, ScreenRect, VisualError, VisualResult};

/// A top-level HWND admitted through UIA run-token/title matching.
///
/// This stays in the visual-test layer and is never exposed to `core` or a
/// provider. The process-local handle is accepted only from the owned target's
/// UIA dump; no title lookup or desktop-window enumeration is performed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedWindowCaptureTarget {
    window_handle: isize,
    /// UIA geometry used only for diagnostic ROI transformation.
    pub window_bounds: ScreenRect,
}

impl OwnedWindowCaptureTarget {
    /// Creates a target from the UIA-observed, uniquely correlated window handle.
    ///
    /// # Errors
    ///
    /// Returns a capture-precondition error for a zero handle. It never falls
    /// back to title lookup, a screen rectangle, or arbitrary window enumeration.
    pub fn new(window_handle: isize, window_bounds: ScreenRect) -> VisualResult<Self> {
        (window_handle != 0)
            .then_some(Self {
                window_handle,
                window_bounds,
            })
            .ok_or_else(|| {
                VisualError::Platform("owned UIA target did not provide a native HWND".to_owned())
            })
    }
}

/// Captures pixels without exposing a platform API to visual-oracle callers.
pub trait CaptureBackend {
    /// Stable name recorded in evidence.
    fn name(&self) -> &'static str;

    /// Captures the complete positively owned window identified by an admitted
    /// native handle.
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
/// only the HWND admitted by UIA. It therefore does not
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
        let frame = capture_window_ex(
            target.window_handle,
            Using::PrintWindow,
            Area::Full,
            None,
            None,
        )
        .map_err(|error| {
            VisualError::Platform(format!("PrintWindow capture unavailable: {error}"))
        })?;
        RgbaFrame::new(frame.width, frame.height, frame.pixels)
    }
}

#[cfg(test)]
mod tests {
    use super::{OwnedWindowCaptureTarget, ScreenRect};

    #[test]
    fn owned_capture_target_requires_a_nonzero_uia_window_handle() {
        let bounds = ScreenRect::new(0, 0, 10, 10);
        assert!(OwnedWindowCaptureTarget::new(1, bounds).is_ok());
        assert!(OwnedWindowCaptureTarget::new(0, bounds).is_err());
    }
}
