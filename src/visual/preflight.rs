//! Machine-readable interactive-desktop preflight decisions.

use serde::{Deserialize, Serialize};

/// An explicit evidence disposition used by visual-test infrastructure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VisualDisposition {
    /// The requirement was observed successfully.
    Pass,
    /// The requirement was observed to fail.
    Fail,
    /// An external or environment precondition prevented observation.
    Blocked,
    /// Available evidence cannot prove either pass or failure.
    Unproven,
}

/// Whether one desktop prerequisite was observed, absent, or not yet tested.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Availability {
    /// The prerequisite was observed to be available.
    Available,
    /// The prerequisite was observed to be unavailable.
    Unavailable,
    /// The prerequisite was not safely observable in this run.
    Unknown,
}

/// The process-session category relevant to trusted GUI observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SessionKind {
    /// A nonzero, user-interactive session.
    Interactive,
    /// Windows Session 0, which cannot provide valid user-desktop evidence.
    SessionZero,
    /// A service/non-user desktop context.
    Service,
    /// The process session could not be classified.
    Unknown,
}

/// A precise reason that prevents a trusted live visual observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PreflightBlocker {
    /// The harness is running in Session 0.
    SessionZero,
    /// The harness is running as a service/non-user session.
    ServiceContext,
    /// The active desktop is locked or otherwise inaccessible.
    DesktopUnavailable,
    /// Windows Terminal could not be found or launched.
    WindowsTerminalUnavailable,
    /// UI Automation could not inspect the owned terminal window.
    UiaUnavailable,
    /// Captured pixels are unavailable, occluded, or otherwise untrustworthy.
    CaptureUnavailable,
    /// A required Windows/runtime capability is unsupported.
    UnsupportedRuntime,
}

/// Inputs collected by the platform adapter before issuing a preflight verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreflightProbe {
    /// Process session category.
    pub session: SessionKind,
    /// Accessibility of the input desktop.
    pub desktop: Availability,
    /// Availability of `wt.exe` and Windows Terminal.
    pub terminal: Availability,
    /// Availability of Windows UI Automation.
    pub uia: Availability,
    /// Trustworthiness of pixels for the positively owned terminal window.
    pub capture: Availability,
}

/// A serializable visual preflight result included in every evidence manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesktopPreflight {
    /// Overall preflight disposition.
    pub disposition: VisualDisposition,
    /// Classified blockers; empty only for `PASS`.
    pub blockers: Vec<PreflightBlocker>,
    /// Human-readable, non-sensitive diagnostics.
    pub detail: String,
}

impl DesktopPreflight {
    /// Classifies the supplied platform observations deterministically.
    #[must_use]
    pub fn assess(probe: PreflightProbe) -> Self {
        let mut blockers = Vec::new();
        match probe.session {
            SessionKind::SessionZero => blockers.push(PreflightBlocker::SessionZero),
            SessionKind::Service => blockers.push(PreflightBlocker::ServiceContext),
            SessionKind::Interactive | SessionKind::Unknown => {}
        }
        append_unavailable(
            &mut blockers,
            probe.desktop,
            PreflightBlocker::DesktopUnavailable,
        );
        append_unavailable(
            &mut blockers,
            probe.terminal,
            PreflightBlocker::WindowsTerminalUnavailable,
        );
        append_unavailable(&mut blockers, probe.uia, PreflightBlocker::UiaUnavailable);
        append_unavailable(
            &mut blockers,
            probe.capture,
            PreflightBlocker::CaptureUnavailable,
        );

        if !blockers.is_empty() {
            return Self {
                disposition: VisualDisposition::Blocked,
                detail: "one or more required interactive-desktop capabilities are unavailable"
                    .to_owned(),
                blockers,
            };
        }

        if matches!(probe.session, SessionKind::Unknown)
            || matches!(probe.desktop, Availability::Unknown)
            || matches!(probe.terminal, Availability::Unknown)
            || matches!(probe.uia, Availability::Unknown)
            || matches!(probe.capture, Availability::Unknown)
        {
            return Self {
                disposition: VisualDisposition::Unproven,
                blockers: Vec::new(),
                detail: "interactive-desktop capability has not been fully observed".to_owned(),
            };
        }

        Self {
            disposition: VisualDisposition::Pass,
            blockers: Vec::new(),
            detail: "interactive desktop, UIA, Terminal, and owned-window pixels are available"
                .to_owned(),
        }
    }
}

fn append_unavailable(
    blockers: &mut Vec<PreflightBlocker>,
    availability: Availability,
    blocker: PreflightBlocker,
) {
    if matches!(availability, Availability::Unavailable) {
        blockers.push(blocker);
    }
}
