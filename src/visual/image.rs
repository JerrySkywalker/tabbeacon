//! Typed, deterministic pixel and geometry primitives.

use serde::{Deserialize, Serialize};

use super::{VisualError, VisualResult};

/// An RGB color sampled from a lossless captured frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rgb {
    /// Red channel.
    pub red: u8,
    /// Green channel.
    pub green: u8,
    /// Blue channel.
    pub blue: u8,
}

impl Rgb {
    /// Creates an RGB color.
    #[must_use]
    pub const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }
}

/// A rectangle relative to a captured frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Roi {
    /// Left coordinate in source-frame pixels.
    pub x: u32,
    /// Top coordinate in source-frame pixels.
    pub y: u32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl Roi {
    /// Creates a frame-relative rectangle.
    #[must_use]
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Clips this ROI to a frame. Empty intersections are rejected.
    #[must_use]
    pub fn clip(self, frame_width: u32, frame_height: u32) -> Option<Self> {
        let right = self.x.saturating_add(self.width).min(frame_width);
        let bottom = self.y.saturating_add(self.height).min(frame_height);
        (right > self.x && bottom > self.y).then_some(Self {
            x: self.x,
            y: self.y,
            width: right - self.x,
            height: bottom - self.y,
        })
    }
}

/// A rectangle in desktop screen coordinates. Negative origin coordinates are
/// valid for multi-monitor Windows desktops.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenRect {
    /// Left desktop coordinate.
    pub left: i32,
    /// Top desktop coordinate.
    pub top: i32,
    /// Width in physical pixels.
    pub width: u32,
    /// Height in physical pixels.
    pub height: u32,
}

impl ScreenRect {
    /// Creates a desktop rectangle.
    #[must_use]
    pub const fn new(left: i32, top: i32, width: u32, height: u32) -> Self {
        Self {
            left,
            top,
            width,
            height,
        }
    }
}

/// An owned RGBA frame with a validated four-byte-per-pixel layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbaFrame {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl RgbaFrame {
    /// Validates and creates an RGBA frame.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError::InvalidFrame`] when `pixels` does not contain
    /// exactly four bytes for every pixel.
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> VisualResult<Self> {
        let expected = expected_len(width, height)?;
        if pixels.len() != expected {
            return Err(VisualError::InvalidFrame {
                width,
                height,
                bytes: pixels.len(),
            });
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    /// Creates a uniformly colored synthetic frame.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError::InvalidFrame`] when the requested dimensions
    /// cannot be represented safely in memory.
    pub fn solid(width: u32, height: u32, color: Rgb) -> VisualResult<Self> {
        let pixel_count = expected_len(width, height)? / 4;
        let mut pixels = Vec::with_capacity(pixel_count * 4);
        for _ in 0..pixel_count {
            pixels.extend_from_slice(&[color.red, color.green, color.blue, u8::MAX]);
        }
        Self::new(width, height, pixels)
    }

    /// Returns the frame width in pixels.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Returns the frame height in pixels.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Returns the frame's top-down RGBA bytes.
    #[must_use]
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// Returns the RGB value at a valid frame coordinate.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError::InvalidRoi`] when the coordinate is outside the
    /// frame.
    pub fn pixel(&self, x: u32, y: u32) -> VisualResult<Rgb> {
        if x >= self.width || y >= self.height {
            return Err(VisualError::InvalidRoi);
        }
        let width = usize::try_from(self.width).map_err(|_| VisualError::InvalidRoi)?;
        let x = usize::try_from(x).map_err(|_| VisualError::InvalidRoi)?;
        let y = usize::try_from(y).map_err(|_| VisualError::InvalidRoi)?;
        let offset = (y * width + x) * 4;
        Ok(Rgb::new(
            self.pixels[offset],
            self.pixels[offset + 1],
            self.pixels[offset + 2],
        ))
    }

    /// Returns an owned crop clipped to this frame.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError::InvalidRoi`] when the ROI has no intersection
    /// with this frame.
    pub fn crop(&self, roi: Roi) -> VisualResult<Self> {
        let clipped = roi
            .clip(self.width, self.height)
            .ok_or(VisualError::InvalidRoi)?;
        let row_width = usize::try_from(clipped.width).map_err(|_| VisualError::InvalidRoi)?;
        let rows = usize::try_from(clipped.height).map_err(|_| VisualError::InvalidRoi)?;
        let source_width = usize::try_from(self.width).map_err(|_| VisualError::InvalidRoi)?;
        let source_x = usize::try_from(clipped.x).map_err(|_| VisualError::InvalidRoi)?;
        let source_y = usize::try_from(clipped.y).map_err(|_| VisualError::InvalidRoi)?;
        let mut pixels = Vec::with_capacity(row_width * rows * 4);
        for row in 0..rows {
            let start = ((source_y + row) * source_width + source_x) * 4;
            let end = start + row_width * 4;
            pixels.extend_from_slice(&self.pixels[start..end]);
        }
        Self::new(clipped.width, clipped.height, pixels)
    }
}

fn expected_len(width: u32, height: u32) -> VisualResult<usize> {
    let pixels = width
        .checked_mul(height)
        .and_then(|count| count.checked_mul(4))
        .ok_or(VisualError::InvalidFrame {
            width,
            height,
            bytes: 0,
        })?;
    usize::try_from(pixels).map_err(|_| VisualError::InvalidFrame {
        width,
        height,
        bytes: 0,
    })
}
