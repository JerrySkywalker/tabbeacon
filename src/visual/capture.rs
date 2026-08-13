//! Replaceable capture backends for a positively owned Windows Terminal window.

use win_screenshot::capture::{Area, Using, capture_window_ex};

use super::{RgbaFrame, ScreenRect, VisualError, VisualResult};

/// A native window handle admitted through exact UIA run-token/title matching.
///
/// This stays in the visual-test layer and is never exposed to `core` or a
/// provider. The handle is accepted only from the owned target's UIA dump.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnedWindowCaptureTarget {
    native_handle: isize,
    /// UIA geometry used only for diagnostic ROI transformation.
    pub window_bounds: ScreenRect,
}

impl OwnedWindowCaptureTarget {
    /// Converts the maintained UIA binding's opaque `HANDLE(0x...)` diagnostic
    /// format into the native handle accepted by the window-only capture API.
    ///
    /// # Errors
    ///
    /// Returns a capture-precondition error for a missing, invalid, or zero
    /// handle. It never falls back to a screen rectangle.
    pub fn new(native_handle: &str, window_bounds: ScreenRect) -> VisualResult<Self> {
        let native_handle = parse_handle(native_handle).ok_or_else(|| {
            VisualError::Platform(
                "owned UIA target did not provide a usable native window handle".to_owned(),
            )
        })?;
        (native_handle != 0)
            .then_some(Self {
                native_handle,
                window_bounds,
            })
            .ok_or_else(|| {
                VisualError::Platform(
                    "owned UIA target provided a zero native window handle".to_owned(),
                )
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
    fn capture(&self, target: OwnedWindowCaptureTarget) -> VisualResult<RgbaFrame>;
}

/// Window-only `PrintWindow(PW_RENDERFULLCONTENT)` capture through the safe
/// `win-screenshot` wrapper.
///
/// Unlike desktop-rectangle GDI copying, this backend asks Windows to render
/// only the admitted HWND. It therefore does not sample pixels behind a
/// transparent Terminal window or from unrelated desktop windows. Windows
/// Terminal/Windows 11 are within the wrapper's documented Windows 8.1+
/// support contract.
#[derive(Debug, Default, Clone, Copy)]
pub struct PrintWindowCaptureBackend;

impl CaptureBackend for PrintWindowCaptureBackend {
    fn name(&self) -> &'static str {
        "win-screenshot-printwindow-full-owned-window"
    }

    fn capture(&self, target: OwnedWindowCaptureTarget) -> VisualResult<RgbaFrame> {
        let frame = capture_window_ex(
            target.native_handle,
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

fn parse_handle(value: &str) -> Option<isize> {
    let hexadecimal = value.strip_prefix("HANDLE(0x")?.strip_suffix(')')?;
    isize::from_str_radix(hexadecimal, 16).ok()
}

#[cfg(test)]
mod tests {
    use super::{OwnedWindowCaptureTarget, ScreenRect};

    #[test]
    fn owned_capture_target_accepts_only_a_nonzero_uia_handle() {
        let bounds = ScreenRect::new(0, 0, 10, 10);
        assert!(OwnedWindowCaptureTarget::new("HANDLE(0x10)", bounds).is_ok());
        assert!(OwnedWindowCaptureTarget::new("HANDLE(0x0)", bounds).is_err());
        assert!(OwnedWindowCaptureTarget::new("not-a-handle", bounds).is_err());
    }
}
