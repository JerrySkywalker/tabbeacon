//! Windows Terminal visual-test infrastructure.
//!
//! This module is deliberately above the presentation layer. It drives the
//! deterministic G02 fixture and observes an owned Windows Terminal window; it
//! neither imports provider events nor exposes desktop concepts to `core`.

mod error;
mod evidence;
mod fixture;
mod image;
mod oracle;
mod preflight;

pub use error::{VisualError, VisualResult};
pub use evidence::{
    AssertionKind, AssertionResult, EvidenceBundle, EvidenceManifest, EvidenceWriter,
    MachineEnvironment, UiaDump,
};
pub use fixture::{FixtureDriver, FixtureReplay, VisualTestCase};
pub use image::{Rgb, RgbaFrame, Roi, ScreenRect};
pub use oracle::{
    AnimationOutcome, AnimationThreshold, ColorClassification, ColorMetrics, ColorSemantic,
    ColorTolerance, FrameDeltaMetrics, assess_animation, classify_color, color_metrics,
    frame_delta, matches_baseline,
};
pub use preflight::{
    Availability, DesktopPreflight, PreflightBlocker, PreflightProbe, SessionKind,
    VisualDisposition,
};

#[cfg(windows)]
pub mod capture;
#[cfg(windows)]
pub mod session;
#[cfg(windows)]
pub mod uia;
