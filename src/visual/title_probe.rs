//! Explicit, owned Windows Terminal visible-title authority probe.
//!
//! This is deliberately separate from normal operational diagnostics. It
//! starts a dedicated fixture window, emits a bounded production-renderer
//! title, samples only the exact correlated tab, then lets its own fixture
//! restore terminal presentation and exit.

use std::{
    env, fs,
    io::{self, Write},
    process, thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use crate::{
    title_authority::{ActiveTitleProbeResult, TitleProbeBoundary, classify_visible_title_samples},
    visual::{
        FixtureDriver, OwnedTabActivation, OwnedTabTitleReader, OwnedWindowTabReader,
        TemporaryWindowProductDisposition, TerminalTestSessionLauncher, WindowsUiaLocator,
    },
};

// Match the visual harness's established static-fixture budget. UIA startup
// and activation can consume materially more time than title sampling itself.
const PROBE_FIXTURE_HOLD: Duration = Duration::from_secs(10);
const PROBE_TARGET_RESOLUTION_BUDGET: Duration = Duration::from_secs(5);
const PROBE_TARGET_RETRY_INTERVAL: Duration = Duration::from_millis(50);
const PROBE_CLEANUP_GRACE: Duration = Duration::from_millis(350);
const PROBE_CLEANUP_BUDGET: Duration = Duration::from_secs(10);
const TITLE_SAMPLE_OFFSETS: [Duration; 5] = [
    Duration::from_millis(0),
    Duration::from_millis(50),
    Duration::from_millis(150),
    Duration::from_millis(300),
    Duration::from_millis(750),
];

/// Runs a bounded, explicit active probe against a dedicated owned Windows
/// Terminal fixture. Raw UIA title values are immediately reduced to the
/// shared classifier and never serialized by diagnostics.
#[must_use]
pub fn run_title_authority_probe() -> ActiveTitleProbeResult {
    let Ok(executable) = env::current_exe() else {
        return ActiveTitleProbeResult::unavailable(TitleProbeBoundary::FixturePreparation);
    };
    let run_id = probe_run_id();
    let Ok(replay) = FixtureDriver::default().reset(&run_id) else {
        return ActiveTitleProbeResult::unavailable(TitleProbeBoundary::FixturePreparation);
    };
    let anchor_run_id = format!("{run_id}-anchor");
    let Ok(anchor_replay) = FixtureDriver::default().reset(&anchor_run_id) else {
        return ActiveTitleProbeResult::unavailable(TitleProbeBoundary::FixturePreparation);
    };
    let anchor_title = &anchor_replay.case.expected_title;
    let launcher = TerminalTestSessionLauncher::default();
    let lifecycle_root = env::temp_dir().join("tabbeacon-temporary-wt-lifecycle");
    if fs::create_dir_all(&lifecycle_root).is_err() {
        return ActiveTitleProbeResult::unavailable(TitleProbeBoundary::FixturePreparation);
    }
    let anchor_started_at = Instant::now();
    let anchor = launcher.launch_title_authority_anchor(
        &executable,
        &run_id,
        &anchor_run_id,
        anchor_title,
        millis(PROBE_FIXTURE_HOLD),
        &lifecycle_root,
    );
    let Ok(anchor) = anchor else {
        return ActiveTitleProbeResult::unavailable(TitleProbeBoundary::AnchorLaunch);
    };

    let mut cleanup_deadline =
        anchor_started_at + PROBE_FIXTURE_HOLD + PROBE_CLEANUP_GRACE + PROBE_CLEANUP_BUDGET;
    let mut cleanup_window_reader = None;
    let probe = match resolve_owned_window_reader(&run_id, anchor_title) {
        Some(window_reader) => {
            let probe_started_at = Instant::now();
            cleanup_deadline =
                probe_started_at + PROBE_FIXTURE_HOLD + PROBE_CLEANUP_GRACE + PROBE_CLEANUP_BUDGET;
            if launcher
                .launch_title_authority_probe(
                    &executable,
                    &anchor.window_name,
                    &run_id,
                    millis(PROBE_FIXTURE_HOLD),
                )
                .is_err()
            {
                ActiveTitleProbeResult::unavailable(TitleProbeBoundary::ProbeTabLaunch)
            } else {
                let observed = match resolve_probe_tab_reader(&window_reader, anchor_title) {
                    Some(title_reader) => title_reader
                        .observe_title_samples(&replay.case.expected_title, &TITLE_SAMPLE_OFFSETS)
                        .map_or_else(
                            |_| {
                                ActiveTitleProbeResult::unavailable(
                                    TitleProbeBoundary::VisibleObservation,
                                )
                            },
                            |samples| {
                                ActiveTitleProbeResult::complete(classify_visible_title_samples(
                                    &samples,
                                ))
                            },
                        ),
                    None => {
                        ActiveTitleProbeResult::unavailable(TitleProbeBoundary::ProbeTabCorrelation)
                    }
                };
                cleanup_window_reader = Some(window_reader);
                observed
            }
        }
        None => ActiveTitleProbeResult::unavailable(TitleProbeBoundary::AnchorCorrelation),
    };

    // A correlated probe tab must visibly disappear before returning. This is
    // stronger than assuming `wt.exe` dispatched the child immediately. If the
    // anchor itself could not be correlated, wait through its bounded lifetime
    // before classifying the active result as unavailable.
    let cleaned_up = cleanup_window_reader.map_or_else(
        || {
            sleep_until(cleanup_deadline);
            true
        },
        |window_reader| wait_for_probe_tab_cleanup(&window_reader, anchor_title, cleanup_deadline),
    );
    let product_disposition = if probe.boundary == TitleProbeBoundary::Complete {
        TemporaryWindowProductDisposition::Pass
    } else {
        TemporaryWindowProductDisposition::Blocked
    };
    let exact_cleanup = anchor
        .cleanup(product_disposition)
        .is_ok_and(|receipt| receipt.temporary_wt_cleanup == "PASS");
    if cleaned_up && exact_cleanup {
        probe
    } else {
        ActiveTitleProbeResult::unavailable(TitleProbeBoundary::FixtureCleanup)
    }
}

fn resolve_owned_window_reader(run_id: &str, anchor_title: &str) -> Option<OwnedWindowTabReader> {
    let deadline = Instant::now() + PROBE_TARGET_RESOLUTION_BUDGET;
    let locator = WindowsUiaLocator;
    let expected_titles = vec![anchor_title.to_owned()];
    loop {
        match locator.locate_and_activate_any_with_title_reader(run_id, &expected_titles) {
            Ok(OwnedTabActivation::Activated { window_reader, .. }) => return Some(window_reader),
            Ok(OwnedTabActivation::Refused { .. }) => return None,
            Err(_) if Instant::now() >= deadline => return None,
            Err(_) => sleep_until((Instant::now() + PROBE_TARGET_RETRY_INTERVAL).min(deadline)),
        }
    }
}

fn resolve_probe_tab_reader(
    window_reader: &OwnedWindowTabReader,
    anchor_title: &str,
) -> Option<OwnedTabTitleReader> {
    let deadline = Instant::now() + PROBE_TARGET_RESOLUTION_BUDGET;
    loop {
        match window_reader.non_anchor_tab(anchor_title) {
            Ok(Some(title_reader)) => return Some(title_reader),
            Ok(None) | Err(_) if Instant::now() >= deadline => return None,
            Ok(None) | Err(_) => {
                sleep_until((Instant::now() + PROBE_TARGET_RETRY_INTERVAL).min(deadline));
            }
        }
    }
}

fn wait_for_probe_tab_cleanup(
    window_reader: &OwnedWindowTabReader,
    anchor_title: &str,
    deadline: Instant,
) -> bool {
    loop {
        let tab_is_absent = match window_reader.non_anchor_tab(anchor_title) {
            Ok(None) => true,
            Ok(Some(_)) => false,
            Err(_) => Instant::now() >= deadline,
        };
        if tab_is_absent {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        sleep_until((Instant::now() + PROBE_TARGET_RETRY_INTERVAL).min(deadline));
    }
}

fn sleep_until(deadline: Instant) {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if !remaining.is_zero() {
        thread::sleep(remaining);
    }
}

/// Emits one short-lived fixture title through the production presentation
/// renderer. This private command is launched only in an owned fixture window
/// by [`run_title_authority_probe`].
///
/// # Errors
///
/// Returns an I/O error when the fixture cannot emit or flush the bounded
/// production-renderer bytes to its owned terminal session.
pub fn emit_title_authority_fixture(run_id: &str, hold_millis: u64) -> io::Result<()> {
    let driver = FixtureDriver::default();
    let replay = driver
        .reset(run_id)
        .map_err(|error| io::Error::other(error.to_string()))?;
    let mut stdout = io::stdout().lock();
    stdout.write_all(&replay.vt_bytes)?;
    stdout.flush()?;
    thread::sleep(Duration::from_millis(hold_millis));
    let reset = driver
        .reset(run_id)
        .map_err(|error| io::Error::other(error.to_string()))?;
    stdout.write_all(&reset.vt_bytes)?;
    stdout.flush()
}

fn probe_run_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0_u128, |duration| duration.as_millis());
    format!("G15-{}-{millis}", process::id())
}

fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{
        PROBE_CLEANUP_BUDGET, PROBE_CLEANUP_GRACE, PROBE_FIXTURE_HOLD,
        PROBE_TARGET_RESOLUTION_BUDGET, TITLE_SAMPLE_OFFSETS, millis, probe_run_id,
    };

    #[test]
    fn probe_schedule_is_bounded_and_monotonic() {
        assert_eq!(TITLE_SAMPLE_OFFSETS.len(), 5);
        assert!(
            TITLE_SAMPLE_OFFSETS
                .windows(2)
                .all(|pair| pair[0] <= pair[1])
        );
        assert_eq!(TITLE_SAMPLE_OFFSETS[0].as_millis(), 0);
        assert_eq!(
            TITLE_SAMPLE_OFFSETS.last().map(Duration::as_millis),
            Some(750)
        );
        assert!(PROBE_FIXTURE_HOLD > *TITLE_SAMPLE_OFFSETS.last().expect("schedule exists"));
        assert!(PROBE_CLEANUP_GRACE.as_millis() > 0);
        assert!(PROBE_CLEANUP_BUDGET >= PROBE_TARGET_RESOLUTION_BUDGET);
    }

    #[test]
    fn probe_run_id_is_safe_for_fixture_correlation() {
        let run_id = probe_run_id();
        assert!(run_id.starts_with("G15-"));
        assert!(run_id.len() <= 64);
        assert!(
            run_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        );
    }

    #[test]
    fn fixture_hold_is_representable_for_the_launcher() {
        assert_eq!(millis(PROBE_FIXTURE_HOLD), 10_000);
    }
}
