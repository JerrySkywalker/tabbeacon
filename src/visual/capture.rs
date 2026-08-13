//! Replaceable capture backend for the owned Windows Terminal window.

use uiautomation::{screenshots::Screenshot, types::Rect};

use super::{RgbaFrame, ScreenRect, VisualError, VisualResult};

/// Captures pixels without exposing a platform API to visual-oracle callers.
pub trait CaptureBackend {
    /// Stable name recorded in evidence.
    fn name(&self) -> &'static str;

    /// Captures exactly the supplied owned-window screen rectangle.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError::Platform`] when the backend cannot produce an
    /// RGBA frame. Callers must classify that as capture/preflight evidence,
    /// never as a presentation color failure.
    fn capture(&self, rect: ScreenRect) -> VisualResult<RgbaFrame>;
}

/// GDI desktop-rectangle capture through the safe `uiautomation` screenshot
/// adapter. This backend is visibility-dependent: callers must establish that
/// the dedicated test window is foreground and unoccluded before using it.
#[derive(Debug, Default, Clone, Copy)]
pub struct UiaGdiCaptureBackend;

impl CaptureBackend for UiaGdiCaptureBackend {
    fn name(&self) -> &'static str {
        "uiautomation-gdi-visible-screen-rectangle"
    }

    fn capture(&self, rect: ScreenRect) -> VisualResult<RgbaFrame> {
        let width = i32::try_from(rect.width)
            .map_err(|_| VisualError::Platform("capture width exceeds i32".to_owned()))?;
        let height = i32::try_from(rect.height)
            .map_err(|_| VisualError::Platform("capture height exceeds i32".to_owned()))?;
        let right = rect
            .left
            .checked_add(width)
            .ok_or_else(|| VisualError::Platform("capture right coordinate overflow".to_owned()))?;
        let bottom = rect.top.checked_add(height).ok_or_else(|| {
            VisualError::Platform("capture bottom coordinate overflow".to_owned())
        })?;
        let screenshot = Screenshot::capture_rect(Rect::new(rect.left, rect.top, right, bottom))
            .map_err(|error| VisualError::Platform(format!("GDI capture unavailable: {error}")))?
            .to_rgba();
        RgbaFrame::new(
            screenshot.width(),
            screenshot.height(),
            screenshot.pixels().to_vec(),
        )
    }
}
