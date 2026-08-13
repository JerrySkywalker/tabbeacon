//! Pure color and animation oracles for deterministic visual evidence.

use serde::{Deserialize, Serialize};

use super::{Rgb, RgbaFrame, Roi, VisualError, VisualResult};

/// A semantic Windows Terminal palette color observable in a tab-background
/// ROI. Default/reset use a captured baseline instead of a hard-coded theme RGB.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ColorSemantic {
    /// The terminal's theme-controlled default color.
    Default,
    /// G02 working green.
    Working,
    /// G02 result-ready blue.
    ResultReady,
    /// G02 approval yellow.
    Approval,
    /// G02 question yellow, semantically distinct from approval.
    Question,
    /// G02 warning orange.
    Warning,
    /// G02 interrupted purple.
    Interrupted,
    /// G02 failed red.
    Failed,
}

impl ColorSemantic {
    /// Returns the fixed G02 palette color where one exists.
    #[must_use]
    pub const fn palette_rgb(self) -> Option<Rgb> {
        match self {
            Self::Default => None,
            Self::Working => Some(Rgb::new(0x2e, 0xcc, 0x71)),
            Self::ResultReady => Some(Rgb::new(0x34, 0x98, 0xdb)),
            Self::Approval | Self::Question => Some(Rgb::new(0xf1, 0xc4, 0x0f)),
            Self::Warning => Some(Rgb::new(0xe6, 0x7e, 0x22)),
            Self::Interrupted => Some(Rgb::new(0x9b, 0x59, 0xb6)),
            Self::Failed => Some(Rgb::new(0xe7, 0x4c, 0x3c)),
        }
    }
}

/// Per-channel aggregate metrics for a sampled ROI. Mean values use a fixed
/// milli-channel scale to avoid nondeterministic floating-point formatting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColorMetrics {
    /// Number of sampled pixels.
    pub sample_count: u64,
    /// Arithmetic mean in channel-milli units.
    pub mean_milli: RgbMilli,
    /// Per-channel median in 8-bit RGB.
    pub median: Rgb,
    /// Per-channel population variance in squared 8-bit channel units.
    pub variance: RgbVariance,
}

/// RGB values stored as one-thousandth channel units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RgbMilli {
    /// Mean red channel times 1,000.
    pub red: u64,
    /// Mean green channel times 1,000.
    pub green: u64,
    /// Mean blue channel times 1,000.
    pub blue: u64,
}

/// Population variance for each RGB channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RgbVariance {
    /// Red variance.
    pub red: u64,
    /// Green variance.
    pub green: u64,
    /// Blue variance.
    pub blue: u64,
}

/// Tolerance for color comparisons. The large mean-distance bound accommodates
/// documented compositor/theme variation while variance rejects text/border
/// contaminated ROIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColorTolerance {
    /// Maximum squared distance in channel-milli space.
    pub max_mean_distance_milli_squared: u64,
    /// Maximum allowed population variance in any channel.
    pub max_channel_variance: u64,
}

impl Default for ColorTolerance {
    fn default() -> Self {
        Self {
            // Equivalent to roughly 29 channel values of Euclidean RGB drift.
            max_mean_distance_milli_squared: 2_500_000_000,
            max_channel_variance: 2_500,
        }
    }
}

/// Classification returned by the palette oracle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ColorClassification {
    /// Exactly one palette semantic met the tolerance.
    Match(ColorSemantic),
    /// More than one semantic met the tolerance; a heuristic must not choose.
    Ambiguous(Vec<ColorSemantic>),
    /// No palette semantic met the tolerance.
    Unclassified,
    /// The ROI has dispersion inconsistent with a tab-background sample.
    ContaminatedRoi,
}

/// Calculates deterministic ROI aggregates.
///
/// # Errors
///
/// Returns [`VisualError::InvalidRoi`] when the ROI does not intersect the
/// source frame.
pub fn color_metrics(frame: &RgbaFrame, roi: Roi) -> VisualResult<ColorMetrics> {
    let crop = frame.crop(roi)?;
    let count = u64::from(crop.width()) * u64::from(crop.height());
    if count == 0 {
        return Err(VisualError::InvalidRoi);
    }

    let mut red_values =
        Vec::with_capacity(usize::try_from(count).map_err(|_| VisualError::InvalidRoi)?);
    let mut green_values =
        Vec::with_capacity(usize::try_from(count).map_err(|_| VisualError::InvalidRoi)?);
    let mut blue_values =
        Vec::with_capacity(usize::try_from(count).map_err(|_| VisualError::InvalidRoi)?);
    let mut sums = [0_u64; 3];
    let mut squares = [0_u64; 3];

    for pixel in crop.pixels().chunks_exact(4) {
        let channels = [pixel[0], pixel[1], pixel[2]];
        red_values.push(channels[0]);
        green_values.push(channels[1]);
        blue_values.push(channels[2]);
        for (index, channel) in channels.into_iter().enumerate() {
            let channel = u64::from(channel);
            sums[index] += channel;
            squares[index] += channel * channel;
        }
    }

    red_values.sort_unstable();
    green_values.sort_unstable();
    blue_values.sort_unstable();
    let midpoint = usize::try_from(count / 2).map_err(|_| VisualError::InvalidRoi)?;
    let mean = |sum: u64| (sum * 1_000 + count / 2) / count;
    let variance = |sum: u64, square_sum: u64| {
        let channel_mean = sum / count;
        (square_sum / count).saturating_sub(channel_mean * channel_mean)
    };

    Ok(ColorMetrics {
        sample_count: count,
        mean_milli: RgbMilli {
            red: mean(sums[0]),
            green: mean(sums[1]),
            blue: mean(sums[2]),
        },
        median: Rgb::new(
            red_values[midpoint],
            green_values[midpoint],
            blue_values[midpoint],
        ),
        variance: RgbVariance {
            red: variance(sums[0], squares[0]),
            green: variance(sums[1], squares[1]),
            blue: variance(sums[2], squares[2]),
        },
    })
}

/// Classifies an ROI against the non-default G02 semantic palette.
#[must_use]
pub fn classify_color(metrics: &ColorMetrics, tolerance: ColorTolerance) -> ColorClassification {
    if metrics.variance.red > tolerance.max_channel_variance
        || metrics.variance.green > tolerance.max_channel_variance
        || metrics.variance.blue > tolerance.max_channel_variance
    {
        return ColorClassification::ContaminatedRoi;
    }

    let mut matches = Vec::new();
    for semantic in [
        ColorSemantic::Working,
        ColorSemantic::ResultReady,
        ColorSemantic::Approval,
        ColorSemantic::Question,
        ColorSemantic::Warning,
        ColorSemantic::Interrupted,
        ColorSemantic::Failed,
    ] {
        if let Some(expected) = semantic.palette_rgb()
            && mean_distance_milli_squared(metrics.mean_milli, expected)
                <= tolerance.max_mean_distance_milli_squared
        {
            matches.push(semantic);
        }
    }
    match matches.len() {
        0 => ColorClassification::Unclassified,
        1 => ColorClassification::Match(matches[0]),
        _ => ColorClassification::Ambiguous(matches),
    }
}

/// Compares a default/reset ROI with a same-run default baseline.
#[must_use]
pub fn matches_baseline(
    metrics: &ColorMetrics,
    baseline: &ColorMetrics,
    tolerance: ColorTolerance,
) -> bool {
    metrics.variance.red <= tolerance.max_channel_variance
        && metrics.variance.green <= tolerance.max_channel_variance
        && metrics.variance.blue <= tolerance.max_channel_variance
        && baseline.variance.red <= tolerance.max_channel_variance
        && baseline.variance.green <= tolerance.max_channel_variance
        && baseline.variance.blue <= tolerance.max_channel_variance
        && mean_delta_milli_squared(metrics.mean_milli, baseline.mean_milli)
            <= tolerance.max_mean_distance_milli_squared
}

/// Selects the least-dispersed deterministic interior background strip from a
/// target tab. Upper and lower strips avoid title text, progress icons, close
/// controls, and borders without assuming one fixed coordinate is always free.
///
/// # Errors
///
/// Returns [`VisualError::InvalidRoi`] when the target tab cannot provide any
/// non-border interior sample tile.
pub fn select_background_roi(frame: &RgbaFrame, tab: Roi) -> VisualResult<(Roi, ColorMetrics)> {
    let tab = tab
        .clip(frame.width(), frame.height())
        .ok_or(VisualError::InvalidRoi)?;
    let horizontal_margin = (tab.width / 10).max(1);
    let vertical_margin = (tab.height / 8).max(1);
    let inner_width = tab.width.saturating_sub(horizontal_margin * 2);
    let strip_height = (tab.height / 8).clamp(1, 16);
    if inner_width < 4 || tab.height <= vertical_margin.saturating_add(strip_height) {
        return Err(VisualError::InvalidRoi);
    }

    let tile_width = (inner_width / 4).max(1);
    let mut candidates = Vec::new();
    let rows = [
        tab.y + vertical_margin,
        tab.y + tab.height - vertical_margin - strip_height,
    ];
    for y in rows {
        for column in 0..4_u32 {
            let x = tab.x + horizontal_margin + column * tile_width;
            let right = (x + tile_width).min(tab.x + tab.width - horizontal_margin);
            if right > x {
                let roi = Roi::new(x, y, right - x, strip_height);
                let metrics = color_metrics(frame, roi)?;
                let score = metrics.variance.red + metrics.variance.green + metrics.variance.blue;
                candidates.push((score, roi, metrics));
            }
        }
    }
    candidates
        .into_iter()
        .min_by_key(|(score, roi, _)| (*score, roi.x, roi.y, roi.width, roi.height))
        .map(|(_, roi, metrics)| (roi, metrics))
        .ok_or(VisualError::InvalidRoi)
}

/// Deterministic metrics comparing two equally sized RGBA frames.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameDeltaMetrics {
    /// Number of compared pixels.
    pub sample_count: u64,
    /// Pixels with at least one meaningful RGB component change.
    pub changed_pixels: u64,
    /// Changed pixels per 1,000 sampled pixels.
    pub changed_pixel_ratio_milli: u64,
    /// Mean absolute RGB component delta in milli-channel units.
    pub mean_absolute_component_delta_milli: u64,
}

/// The bounded threshold used to decide whether indeterminate progress moved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnimationThreshold {
    /// Minimum changed-pixel ratio, in per-1,000 units.
    pub min_changed_pixel_ratio_milli: u64,
    /// Minimum mean RGB component delta, in milli-channel units.
    pub min_mean_absolute_component_delta_milli: u64,
    /// A component change must exceed this value to count a pixel as changed.
    pub component_delta_threshold: u8,
}

impl Default for AnimationThreshold {
    fn default() -> Self {
        Self {
            min_changed_pixel_ratio_milli: 20,
            min_mean_absolute_component_delta_milli: 2_000,
            component_delta_threshold: 8,
        }
    }
}

/// A bounded animation observation result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AnimationOutcome {
    /// At least one successive frame pair crosses the tested motion threshold.
    AnimationPresent,
    /// Enough consistent frames were observed, but none crossed the threshold.
    AnimationAbsent,
    /// Pixel frames could not be trusted or compared.
    UnprovenCapture,
    /// The interactive desktop precondition blocked frame observation.
    BlockedEnvironment,
}

/// Compares two same-sized frames within a frame-relative ROI.
///
/// # Errors
///
/// Returns [`VisualError::InconsistentFrames`] for dimension mismatches and
/// [`VisualError::InvalidRoi`] for empty/out-of-frame ROIs.
pub fn frame_delta(
    before: &RgbaFrame,
    after: &RgbaFrame,
    roi: Roi,
    component_delta_threshold: u8,
) -> VisualResult<FrameDeltaMetrics> {
    if before.width() != after.width() || before.height() != after.height() {
        return Err(VisualError::InconsistentFrames {
            first: (before.width(), before.height()),
            second: (after.width(), after.height()),
        });
    }
    let before = before.crop(roi)?;
    let after = after.crop(roi)?;
    let sample_count = u64::from(before.width()) * u64::from(before.height());
    let mut changed_pixels = 0_u64;
    let mut component_sum = 0_u64;
    for (before_pixel, after_pixel) in before
        .pixels()
        .chunks_exact(4)
        .zip(after.pixels().chunks_exact(4))
    {
        let mut pixel_changed = false;
        for channel in 0..3 {
            let delta = u64::from(before_pixel[channel].abs_diff(after_pixel[channel]));
            component_sum += delta;
            pixel_changed |= delta > u64::from(component_delta_threshold);
        }
        if pixel_changed {
            changed_pixels += 1;
        }
    }
    Ok(FrameDeltaMetrics {
        sample_count,
        changed_pixels,
        changed_pixel_ratio_milli: (changed_pixels * 1_000 + sample_count / 2) / sample_count,
        mean_absolute_component_delta_milli: (component_sum * 1_000 + (sample_count * 3) / 2)
            / (sample_count * 3),
    })
}

/// Assesses a bounded frame sequence for genuine indeterminate-progress motion.
///
/// # Errors
///
/// Returns the same errors as [`frame_delta`] when any frame pair is not
/// comparable. Callers map that to an evidence failure class rather than a
/// product assertion.
pub fn assess_animation(
    frames: &[RgbaFrame],
    roi: Roi,
    threshold: AnimationThreshold,
) -> VisualResult<(AnimationOutcome, Vec<FrameDeltaMetrics>)> {
    if frames.len() < 2 {
        return Ok((AnimationOutcome::UnprovenCapture, Vec::new()));
    }
    let mut deltas = Vec::with_capacity(frames.len() - 1);
    for pair in frames.windows(2) {
        let delta = frame_delta(&pair[0], &pair[1], roi, threshold.component_delta_threshold)?;
        deltas.push(delta);
    }
    let moving = deltas.iter().any(|delta| {
        delta.changed_pixel_ratio_milli >= threshold.min_changed_pixel_ratio_milli
            && delta.mean_absolute_component_delta_milli
                >= threshold.min_mean_absolute_component_delta_milli
    });
    let outcome = if moving {
        AnimationOutcome::AnimationPresent
    } else {
        AnimationOutcome::AnimationAbsent
    };
    Ok((outcome, deltas))
}

fn mean_distance_milli_squared(mean: RgbMilli, expected: Rgb) -> u64 {
    mean_delta_milli_squared(
        mean,
        RgbMilli {
            red: u64::from(expected.red) * 1_000,
            green: u64::from(expected.green) * 1_000,
            blue: u64::from(expected.blue) * 1_000,
        },
    )
}

fn mean_delta_milli_squared(left: RgbMilli, right: RgbMilli) -> u64 {
    let red = left.red.abs_diff(right.red);
    let green = left.green.abs_diff(right.green);
    let blue = left.blue.abs_diff(right.blue);
    red * red + green * green + blue * blue
}
