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
    AssertionKind, AssertionResult, EvidenceBundle, EvidenceFileDigest, EvidenceIntegrity,
    EvidenceManifest, EvidenceWriter, FailureCategory, MachineEnvironment, UiaDump,
    WindowActivation,
};
pub use fixture::{
    FixtureDriver, FixtureReplay, ROOT_WORKSPACE_ANCHOR_FIXTURE_NAME, VisualTestCase,
    root_workspace_anchor_fixture_alias,
};
pub use image::{Rgb, RgbaFrame, Roi, ScreenRect};
pub use oracle::{
    AnimationOutcome, AnimationThreshold, ColorClassification, ColorMetrics, ColorSemantic,
    ColorTolerance, FrameDeltaMetrics, RgbMilli, RgbVariance, assess_animation, classify_color,
    classify_color_for_theme, color_metrics, frame_delta, matches_baseline, select_background_roi,
};
pub use preflight::{
    Availability, DesktopPreflight, PreflightBlocker, PreflightProbe, SessionKind,
    VisualDisposition,
};

#[cfg(windows)]
pub mod capture;
#[cfg(windows)]
pub mod runner;
#[cfg(windows)]
pub mod session;
#[cfg(windows)]
mod title_probe;
#[cfg(windows)]
pub mod uia;

#[cfg(windows)]
pub use capture::{CaptureBackend, OwnedWindowCaptureTarget, PrintWindowCaptureBackend};
#[cfg(windows)]
pub use runner::{LiveVisualRunRequest, LiveVisualRunSummary};
#[cfg(windows)]
pub use session::{TerminalTestSession, TerminalTestSessionLauncher};
#[cfg(windows)]
pub use title_probe::{emit_title_authority_fixture, run_title_authority_probe};
#[cfg(windows)]
pub use uia::{
    ExactOwnedWindow, OwnedTabActivation, OwnedTabTitleReader, OwnedWindowTabReader, TargetLocator,
    WindowsUiaLocator,
};

#[cfg(not(windows))]
/// Reports that the explicit Windows Terminal probe cannot run on this platform.
#[must_use]
pub fn run_title_authority_probe() -> crate::title_authority::ActiveTitleProbeResult {
    crate::title_authority::ActiveTitleProbeResult::unavailable(
        crate::title_authority::TitleProbeBoundary::PlatformUnavailable,
    )
}
