//! Live Windows Terminal visual-harness orchestration.

use std::{
    env,
    path::{Path, PathBuf},
    process::{self, Command},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::Serialize;

use super::{
    AnimationThreshold, AssertionKind, AssertionResult, Availability, CaptureBackend,
    ColorClassification, ColorMetrics, ColorSemantic, ColorTolerance, DesktopPreflight,
    EvidenceBundle, EvidenceManifest, EvidenceWriter, FixtureDriver, MachineEnvironment,
    OwnedWindowCaptureTarget, PreflightProbe, PrintWindowCaptureBackend, RgbaFrame, Roi,
    ScreenRect, SessionKind, TargetLocator, TerminalTestSessionLauncher, UiaDump,
    VisualDisposition, VisualError, VisualResult, WindowsUiaLocator, assess_animation,
    classify_color_for_theme, matches_baseline, select_background_roi,
};

/// Inputs for one live visual-harness invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveVisualRunRequest {
    /// Candidate SHA that must match the checkout before a visual PASS is
    /// possible.
    pub expected_head: String,
    /// Unique evidence-directory and UIA correlation token.
    pub run_id: String,
    /// Root under which the owned run directory is created.
    pub evidence_root: PathBuf,
    /// Optional one-fixture smoke selection. `None` replays every G02 fixture.
    pub fixture_name: Option<String>,
}

/// Compact machine-readable outcome suitable for workflow-step output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LiveVisualRunSummary {
    /// Overall live visual disposition.
    pub disposition: VisualDisposition,
    /// Candidate SHA supplied to this invocation.
    pub expected_head: String,
    /// SHA read from the checked-out repository.
    pub checked_out_head: String,
    /// SHA supported by visual evidence, present only on PASS.
    pub visual_head: Option<String>,
    /// Owned evidence directory.
    pub evidence_path: PathBuf,
    /// SHA-256 over deterministic evidence artifact records.
    pub evidence_tree_sha256: String,
    /// Preflight lane disposition.
    pub preflight: VisualDisposition,
    /// UIA target-resolution lane disposition.
    pub uia: VisualDisposition,
    /// Pixel-capture lane disposition.
    pub capture: VisualDisposition,
    /// UIA title lane disposition.
    pub title: VisualDisposition,
    /// Color-oracle lane disposition.
    pub color: VisualDisposition,
    /// Animation-oracle lane disposition.
    pub animation: VisualDisposition,
}

/// Runs the trusted local visual fixture path and always preserves a classified
/// owned evidence bundle once its directory is created.
///
/// # Errors
///
/// Returns infrastructure errors for invalid request paths, inability to create
/// the owned evidence directory, or inability to serialize required evidence.
/// Individual desktop/UIA/capture observations are represented in the returned
/// classified summary rather than being converted into product failures.
pub fn run_live(request: &LiveVisualRunRequest) -> VisualResult<LiveVisualRunSummary> {
    let checked_out_head = checked_out_head()?;
    let (environment, base_probe) = inspect_environment();
    let driver = FixtureDriver::default();
    let replays = selected_replays(&driver, request)?;
    let fixture_names = replays
        .iter()
        .map(|replay| replay.case.fixture_name.clone())
        .collect::<Vec<_>>();
    let writer = EvidenceWriter::create(&request.evidence_root, &request.run_id)?;

    let exact_head = is_exact_sha(&request.expected_head)
        && is_exact_sha(&checked_out_head)
        && request.expected_head == checked_out_head;
    let mut observation = Observation::new(DesktopPreflight::assess(base_probe), exact_head);
    observation.record_exact_head(&request.expected_head, &checked_out_head, exact_head);

    if !matches!(
        observation.preflight.disposition,
        VisualDisposition::Blocked
    ) && exact_head
    {
        let fixture_executable = env::current_exe().map_err(VisualError::Io)?;
        for replay in &replays {
            observe_replay(
                &writer,
                &fixture_executable,
                &request.run_id,
                replay,
                &mut observation,
            )?;
        }
    }

    observation.finalize_preflight(base_probe);
    let final_disposition = observation.disposition(exact_head);
    let visual_head =
        matches!(final_disposition, VisualDisposition::Pass).then_some(checked_out_head.clone());
    let manifest = EvidenceManifest {
        goal_id: "TB-G03".to_owned(),
        expected_head: request.expected_head.clone(),
        checked_out_head: checked_out_head.clone(),
        visual_head: visual_head.clone(),
        run_id: request.run_id.clone(),
        observed_at_unix_seconds: unix_seconds(),
        capture_backend: PrintWindowCaptureBackend.name().to_owned(),
        preflight: observation.preflight.clone(),
        environment: environment.clone(),
        window_geometry: observation.uia.window_bounds,
        fixtures: fixture_names,
        disposition: final_disposition,
    };
    let bundle = EvidenceBundle {
        manifest,
        assertions: observation.assertions,
        environment,
        uia: observation.uia,
        color_metrics: observation.metrics,
    };
    writer.write_bundle(&bundle)?;
    let integrity = writer.write_integrity_manifest()?;
    Ok(LiveVisualRunSummary {
        disposition: final_disposition,
        expected_head: request.expected_head.clone(),
        checked_out_head,
        visual_head,
        evidence_path: writer.directory().to_path_buf(),
        evidence_tree_sha256: integrity.tree_sha256,
        preflight: observation.preflight.disposition,
        uia: observation.lanes.uia(),
        capture: observation.lanes.capture(),
        title: observation.lanes.title(),
        color: observation.lanes.color(),
        animation: observation.lanes.animation(),
    })
}

fn observe_replay(
    writer: &EvidenceWriter,
    fixture_executable: &Path,
    run_id: &str,
    replay: &super::FixtureReplay,
    observation: &mut Observation,
) -> VisualResult<()> {
    let launcher = TerminalTestSessionLauncher::default();
    if let Err(error) = launcher.launch(fixture_executable, replay, run_id) {
        observation.record_uia_blocked(&replay.case.fixture_name, error.to_string());
        return Ok(());
    }
    let locator = WindowsUiaLocator;
    let initial_target =
        match locate_capturable_with_retry(locator, run_id, &replay.case.expected_title) {
            Ok(target) => target,
            Err(error) => {
                observation.record_uia_failure(&replay.case.fixture_name, error.to_string());
                return Ok(());
            }
        };
    let activation = match activate_with_retry(locator, run_id, &replay.case.expected_title) {
        Ok(activation) => activation,
        Err(error) => {
            observation.record_target(writer, replay, &initial_target)?;
            observation.record_capture_blocked(
                &replay.case.fixture_name,
                format!("owned-window activation was refused: {error}"),
            );
            return Ok(());
        }
    };
    let mut target = match locate_capturable_with_retry(
        locator,
        run_id,
        &replay.case.expected_title,
    ) {
        Ok(target) => target,
        Err(error) => {
            observation.record_target(writer, replay, &initial_target)?;
            observation.record_capture_blocked(
                &replay.case.fixture_name,
                format!(
                    "owned-window activation did not yield capturable UIA geometry after foreground={} focus={}: {error}",
                    activation.set_foreground, activation.set_focus
                ),
            );
            return Ok(());
        }
    };
    target.activation = Some(activation);
    observation.record_target(writer, replay, &target)?;
    let Some((window_bounds, tab_bounds)) = capture_bounds(&target) else {
        observation.record_capture_blocked(
            &replay.case.fixture_name,
            "UIA did not provide owned window/tab bounds",
        );
        return Ok(());
    };
    if !target
        .activation
        .as_ref()
        .is_some_and(|activation| activation.set_foreground)
    {
        observation.record_capture_blocked(
            &replay.case.fixture_name,
            "owned Terminal window did not confirm foreground activation; visibility-dependent capture refused",
        );
        return Ok(());
    }
    let capture_target = match OwnedWindowCaptureTarget::new(&target.window_name, window_bounds) {
        Ok(target) => target,
        Err(error) => {
            observation.record_capture_blocked(&replay.case.fixture_name, error.to_string());
            return Ok(());
        }
    };
    observe_capture(writer, replay, &capture_target, tab_bounds, observation)
}

fn activate_with_retry(
    locator: WindowsUiaLocator,
    run_id: &str,
    expected_title: &str,
) -> VisualResult<super::WindowActivation> {
    let mut last_activation = None;
    let mut last_error = None;
    for _ in 0..5 {
        match locator.activate_owned_window(run_id, expected_title) {
            Ok(activation) if activation.set_foreground => return Ok(activation),
            Ok(activation) => last_activation = Some(activation),
            Err(error) => last_error = Some(error),
        }
        thread::sleep(Duration::from_millis(200));
    }
    last_activation.ok_or_else(|| {
        last_error.unwrap_or_else(|| {
            VisualError::Platform("owned-window activation produced no observation".to_owned())
        })
    })
}

fn observe_capture(
    writer: &EvidenceWriter,
    replay: &super::FixtureReplay,
    capture_target: &OwnedWindowCaptureTarget,
    tab_bounds: ScreenRect,
    observation: &mut Observation,
) -> VisualResult<()> {
    let capture_backend = PrintWindowCaptureBackend;
    let frames = match capture_frames(&capture_backend, capture_target, 3) {
        Ok(frames) => frames,
        Err(error) => {
            observation.record_capture_blocked(&replay.case.fixture_name, error.to_string());
            return Ok(());
        }
    };
    let first_frame = frames
        .first()
        .cloned()
        .ok_or_else(|| VisualError::Platform("capture returned no frames".to_owned()))?;
    let tab_roi = match relative_roi(capture_target.window_bounds, tab_bounds, &first_frame) {
        Ok(tab_roi) => tab_roi,
        Err(error) => {
            observation.record_capture_blocked(
                &replay.case.fixture_name,
                format!("UIA geometry became non-capturable before frame sampling: {error}"),
            );
            return Ok(());
        }
    };
    write_capture_images(writer, replay, &first_frame, tab_roi)?;
    observation.record_capture_pass(&replay.case.fixture_name, capture_backend.name());
    observe_color(writer, replay, &first_frame, tab_roi, observation)?;
    if replay.case.expects_animation {
        observe_animation(writer, replay, &frames, tab_roi, observation)?;
    }
    Ok(())
}

fn write_capture_images(
    writer: &EvidenceWriter,
    replay: &super::FixtureReplay,
    frame: &RgbaFrame,
    tab_roi: Roi,
) -> VisualResult<()> {
    writer.write_png(
        &format!("full-window-{}-001", replay.case.fixture_name),
        frame,
    )?;
    writer.write_png(
        &format!("tab-{}", replay.case.fixture_name),
        &frame.crop(tab_roi)?,
    )?;
    Ok(())
}

fn observe_color(
    writer: &EvidenceWriter,
    replay: &super::FixtureReplay,
    frame: &RgbaFrame,
    tab_roi: Roi,
    observation: &mut Observation,
) -> VisualResult<()> {
    let (color_roi, color) = select_background_roi(frame, tab_roi)?;
    writer.write_png(
        &format!("tab-color-roi-{}", replay.case.fixture_name),
        &frame.crop(color_roi)?,
    )?;
    writer.write_json_document(
        &format!("color-metrics-{}.json", replay.case.fixture_name),
        &color,
    )?;
    observation.record_color(replay, color_roi, color);
    Ok(())
}

fn observe_animation(
    writer: &EvidenceWriter,
    replay: &super::FixtureReplay,
    frames: &[RgbaFrame],
    tab_roi: Roi,
    observation: &mut Observation,
) -> VisualResult<()> {
    let progress_roi = progress_roi(tab_roi)?;
    for (index, frame) in frames.iter().enumerate() {
        writer.write_png(
            &format!("progress-roi-{}-{:03}", replay.case.fixture_name, index + 1),
            &frame.crop(progress_roi)?,
        )?;
    }
    let (outcome, deltas) = assess_animation(frames, progress_roi, AnimationThreshold::default())?;
    writer.write_json_document(
        &format!("frame-delta-{}.json", replay.case.fixture_name),
        &deltas,
    )?;
    observation.record_animation(&replay.case.fixture_name, outcome, &deltas);
    Ok(())
}

fn capture_bounds(target: &UiaDump) -> Option<(ScreenRect, ScreenRect)> {
    target.window_bounds.zip(target.tab_bounds)
}

fn target_has_capturable_geometry(target: &UiaDump) -> bool {
    capture_bounds(target).is_some_and(|(window, tab)| {
        if window.width == 0 || window.height == 0 || tab.width == 0 || tab.height == 0 {
            return false;
        }
        let window_left = i64::from(window.left);
        let window_top = i64::from(window.top);
        let window_right = window_left + i64::from(window.width);
        let window_bottom = window_top + i64::from(window.height);
        let tab_left = i64::from(tab.left);
        let tab_top = i64::from(tab.top);
        let tab_right = tab_left + i64::from(tab.width);
        let tab_bottom = tab_top + i64::from(tab.height);
        tab_left >= window_left
            && tab_top >= window_top
            && tab_right <= window_right
            && tab_bottom <= window_bottom
    })
}

struct Observation {
    assertions: Vec<AssertionResult>,
    preflight: DesktopPreflight,
    uia: UiaDump,
    metrics: Vec<(String, ColorMetrics)>,
    baseline: Option<ColorMetrics>,
    saw_non_pass: bool,
    lanes: LaneDisposition,
}

impl Observation {
    fn new(preflight: DesktopPreflight, exact_head: bool) -> Self {
        let mut lanes = LaneDisposition::default();
        lanes.observe_preflight(preflight.disposition);
        let assertions = vec![AssertionResult::new(
            AssertionKind::Preflight,
            preflight.disposition,
            None,
            preflight.detail.clone(),
        )];
        Self {
            assertions,
            preflight,
            uia: empty_uia_dump(),
            metrics: Vec::new(),
            baseline: None,
            saw_non_pass: !exact_head,
            lanes,
        }
    }

    fn record_exact_head(&mut self, expected: &str, checked_out: &str, exact: bool) {
        self.assertions.push(AssertionResult::new(
            AssertionKind::ExactHead,
            if exact {
                VisualDisposition::Pass
            } else {
                VisualDisposition::Fail
            },
            None,
            format!("expected={expected} checked_out={checked_out}"),
        ));
    }

    fn record_uia_blocked(&mut self, fixture: &str, detail: String) {
        self.saw_non_pass = true;
        self.lanes.observe_uia(VisualDisposition::Blocked);
        self.assertions.push(AssertionResult::new(
            AssertionKind::UiaTarget,
            VisualDisposition::Blocked,
            Some(fixture.to_owned()),
            detail,
        ));
    }

    fn record_uia_failure(&mut self, fixture: &str, detail: String) {
        self.saw_non_pass = true;
        self.lanes.observe_uia(VisualDisposition::Fail);
        self.assertions.push(AssertionResult::new(
            AssertionKind::UiaTarget,
            VisualDisposition::Fail,
            Some(fixture.to_owned()),
            detail,
        ));
    }

    fn record_target(
        &mut self,
        writer: &EvidenceWriter,
        replay: &super::FixtureReplay,
        target: &UiaDump,
    ) -> VisualResult<()> {
        self.uia = target.clone();
        writer.write_json_document(&format!("uia-{}.json", replay.case.fixture_name), target)?;
        self.lanes.observe_uia(VisualDisposition::Pass);
        self.assertions.push(AssertionResult::new(
            AssertionKind::UiaTarget,
            VisualDisposition::Pass,
            Some(replay.case.fixture_name.clone()),
            "owned Windows Terminal window and exact tab resolved through UIA".to_owned(),
        ));
        let disposition = if target.tab_name == replay.case.expected_title {
            VisualDisposition::Pass
        } else {
            VisualDisposition::Fail
        };
        self.saw_non_pass |= !matches!(disposition, VisualDisposition::Pass);
        self.lanes.observe_title(disposition);
        self.assertions.push(AssertionResult::new(
            AssertionKind::Title,
            disposition,
            Some(replay.case.fixture_name.clone()),
            format!(
                "expected_title={}; uia_title={}",
                replay.case.expected_title, target.tab_name
            ),
        ));
        Ok(())
    }

    fn record_capture_blocked(&mut self, fixture: &str, detail: impl Into<String>) {
        self.saw_non_pass = true;
        self.preflight = capture_blocked_preflight();
        self.lanes.observe_capture(VisualDisposition::Blocked);
        self.assertions
            .push(capture_blocked_assertion(fixture, &detail.into()));
    }

    fn record_capture_pass(&mut self, fixture: &str, backend: &str) {
        self.lanes.observe_capture(VisualDisposition::Pass);
        self.assertions.push(AssertionResult::new(
            AssertionKind::Capture,
            VisualDisposition::Pass,
            Some(fixture.to_owned()),
            backend.to_owned(),
        ));
    }

    fn record_color(&mut self, replay: &super::FixtureReplay, roi: Roi, metrics: ColorMetrics) {
        let is_ready_baseline = matches!(replay.case.expected_color, ColorSemantic::Default)
            && replay.case.fixture_name == "ready";
        let disposition = if is_ready_baseline {
            self.baseline = Some(metrics.clone());
            VisualDisposition::Pass
        } else {
            evaluate_color(
                replay.case.expected_color,
                replay.case.theme,
                &metrics,
                self.baseline.as_ref(),
                ColorTolerance::default(),
            )
        };
        self.saw_non_pass |= !matches!(disposition, VisualDisposition::Pass);
        self.lanes.observe_color(disposition);
        self.assertions.push(AssertionResult::new(
            AssertionKind::Color,
            disposition,
            Some(replay.case.fixture_name.clone()),
            format!("roi={roi:?}; metrics={metrics:?}"),
        ));
        self.metrics
            .push((replay.case.fixture_name.clone(), metrics));
    }

    fn record_animation(
        &mut self,
        fixture: &str,
        outcome: super::AnimationOutcome,
        deltas: &[super::FrameDeltaMetrics],
    ) {
        let disposition = match outcome {
            super::AnimationOutcome::AnimationPresent => VisualDisposition::Pass,
            super::AnimationOutcome::AnimationAbsent => VisualDisposition::Fail,
            super::AnimationOutcome::UnprovenCapture => VisualDisposition::Unproven,
            super::AnimationOutcome::BlockedEnvironment => VisualDisposition::Blocked,
        };
        self.saw_non_pass |= !matches!(disposition, VisualDisposition::Pass);
        self.lanes.observe_animation(disposition);
        self.assertions.push(AssertionResult::new(
            AssertionKind::Animation,
            disposition,
            Some(fixture.to_owned()),
            format!("outcome={outcome:?}; deltas={deltas:?}"),
        ));
    }

    fn finalize_preflight(&mut self, base_probe: PreflightProbe) {
        self.preflight = match self.lanes.capture {
            Some(VisualDisposition::Pass) if !self.lanes.has_blocker() => {
                DesktopPreflight::assess(PreflightProbe {
                    capture: Availability::Available,
                    ..base_probe
                })
            }
            Some(VisualDisposition::Blocked) => capture_blocked_preflight(),
            _ => self.preflight.clone(),
        };
        self.lanes.preflight = Some(self.preflight.disposition);
        if let Some(assertion) = self
            .assertions
            .iter_mut()
            .find(|assertion| matches!(assertion.kind, AssertionKind::Preflight))
        {
            *assertion = AssertionResult::new(
                AssertionKind::Preflight,
                self.preflight.disposition,
                None,
                self.preflight.detail.clone(),
            );
        }
    }

    fn disposition(&self, exact_head: bool) -> VisualDisposition {
        if !exact_head || self.lanes.has_failure() {
            VisualDisposition::Fail
        } else if self.lanes.has_blocker() {
            VisualDisposition::Blocked
        } else if self.saw_non_pass || self.lanes.has_unproven() {
            VisualDisposition::Unproven
        } else {
            VisualDisposition::Pass
        }
    }
}

fn selected_replays(
    driver: &FixtureDriver,
    request: &LiveVisualRunRequest,
) -> VisualResult<Vec<super::FixtureReplay>> {
    let all = driver.all_cases(&request.run_id)?;
    match request.fixture_name.as_deref() {
        Some(name) => all
            .into_iter()
            .filter(|replay| replay.case.fixture_name == name)
            .collect::<Vec<_>>()
            .into_iter()
            .next()
            .map_or_else(
                || {
                    Err(VisualError::Platform(format!(
                        "unknown G02 fixture requested for visual run: {name}"
                    )))
                },
                |replay| Ok(vec![replay]),
            ),
        None => Ok(all),
    }
}

fn locate_capturable_with_retry(
    locator: WindowsUiaLocator,
    run_id: &str,
    expected_title: &str,
) -> VisualResult<UiaDump> {
    const CAPTURABLE_GEOMETRY_ATTEMPTS: usize = 20;
    let mut last_target = None;
    let mut last_error = None;
    for _ in 0..CAPTURABLE_GEOMETRY_ATTEMPTS {
        match locator.locate(run_id, expected_title) {
            Ok(target) if target_has_capturable_geometry(&target) => return Ok(target),
            Ok(target) => last_target = Some(target),
            Err(error) => last_error = Some(error),
        }
        thread::sleep(Duration::from_millis(200));
    }
    let detail = last_target.map_or_else(
        || "no matching UIA target was observed".to_owned(),
        |target| {
            format!(
                "last target had non-capturable geometry: window={:?}; tab={:?}",
                target.window_bounds, target.tab_bounds
            )
        },
    );
    Err(last_error.unwrap_or_else(|| {
        VisualError::Platform(format!(
            "owned Windows Terminal target did not expose capturable geometry within {CAPTURABLE_GEOMETRY_ATTEMPTS} attempts: {detail}"
        ))
    }))
}

fn capture_frames(
    backend: &impl CaptureBackend,
    target: &OwnedWindowCaptureTarget,
    count: usize,
) -> VisualResult<Vec<RgbaFrame>> {
    let mut frames = Vec::with_capacity(count);
    for index in 0..count {
        frames.push(backend.capture(target)?);
        if index + 1 < count {
            thread::sleep(Duration::from_millis(300));
        }
    }
    Ok(frames)
}

fn relative_roi(window: ScreenRect, tab: ScreenRect, frame: &RgbaFrame) -> VisualResult<Roi> {
    let relative_left = tab
        .left
        .checked_sub(window.left)
        .ok_or(VisualError::InvalidRoi)?;
    let relative_top = tab
        .top
        .checked_sub(window.top)
        .ok_or(VisualError::InvalidRoi)?;
    let x = scale_to_frame(
        u32::try_from(relative_left).map_err(|_| VisualError::InvalidRoi)?,
        frame.width(),
        window.width,
    )
    .ok_or(VisualError::InvalidRoi)?;
    let y = scale_to_frame(
        u32::try_from(relative_top).map_err(|_| VisualError::InvalidRoi)?,
        frame.height(),
        window.height,
    )
    .ok_or(VisualError::InvalidRoi)?;
    let width =
        scale_to_frame(tab.width, frame.width(), window.width).ok_or(VisualError::InvalidRoi)?;
    let height =
        scale_to_frame(tab.height, frame.height(), window.height).ok_or(VisualError::InvalidRoi)?;
    Roi::new(x, y, width, height)
        .clip(frame.width(), frame.height())
        .ok_or(VisualError::InvalidRoi)
}

fn scale_to_frame(value: u32, frame_extent: u32, uia_extent: u32) -> Option<u32> {
    let denominator = u64::from(uia_extent);
    if denominator == 0 {
        return None;
    }
    let scaled = u64::from(value)
        .checked_mul(u64::from(frame_extent))?
        .checked_add(denominator / 2)?
        .checked_div(denominator)?;
    u32::try_from(scaled).ok()
}

fn progress_roi(tab: Roi) -> VisualResult<Roi> {
    let size = tab.height.clamp(16, 64) / 2;
    let x = tab.x.saturating_add(tab.height / 2);
    let y = tab.y.saturating_add(tab.height.saturating_sub(size) / 2);
    let width = size;
    let height = size;
    (width > 0 && height > 0)
        .then_some(Roi::new(x, y, width, height))
        .ok_or(VisualError::InvalidRoi)
}

fn evaluate_color(
    expected: ColorSemantic,
    theme: crate::settings::PresentationTheme,
    observed: &ColorMetrics,
    baseline: Option<&ColorMetrics>,
    tolerance: ColorTolerance,
) -> VisualDisposition {
    match expected {
        ColorSemantic::Default => baseline.map_or(VisualDisposition::Unproven, |baseline| {
            if matches_baseline(observed, baseline, tolerance) {
                VisualDisposition::Pass
            } else {
                VisualDisposition::Fail
            }
        }),
        ColorSemantic::Approval | ColorSemantic::Question => {
            match classify_color_for_theme(observed, tolerance, theme) {
                ColorClassification::Match(ColorSemantic::Approval | ColorSemantic::Question)
                | ColorClassification::Ambiguous(_) => VisualDisposition::Pass,
                ColorClassification::ContaminatedRoi | ColorClassification::Unclassified => {
                    VisualDisposition::Fail
                }
                ColorClassification::Match(_) => VisualDisposition::Fail,
            }
        }
        semantic => {
            if classify_color_for_theme(observed, tolerance, theme)
                == ColorClassification::Match(semantic)
            {
                VisualDisposition::Pass
            } else {
                VisualDisposition::Fail
            }
        }
    }
}

fn capture_blocked_preflight() -> DesktopPreflight {
    DesktopPreflight::assess(PreflightProbe {
        session: SessionKind::Interactive,
        desktop: Availability::Available,
        terminal: Availability::Available,
        uia: Availability::Available,
        capture: Availability::Unavailable,
    })
}

fn capture_blocked_assertion(fixture: &str, detail: &str) -> AssertionResult {
    AssertionResult::new(
        AssertionKind::Capture,
        VisualDisposition::Blocked,
        Some(fixture.to_owned()),
        detail.to_owned(),
    )
}

fn empty_uia_dump() -> UiaDump {
    UiaDump {
        window_name: String::new(),
        tab_name: String::new(),
        window_bounds: None,
        tab_bounds: None,
        native_window_handle: None,
        window_has_keyboard_focus: None,
        activation: None,
        detail: "no owned UIA target was resolved".to_owned(),
    }
}

fn checked_out_head() -> VisualResult<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(VisualError::Io)?;
    if !output.status.success() {
        return Err(VisualError::Platform(
            "git rev-parse HEAD failed in visual harness".to_owned(),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn inspect_environment() -> (MachineEnvironment, PreflightProbe) {
    let (session_id, session_kind) = process_session();
    let terminal_available = Command::new("wt.exe").arg("--version").output().is_ok();
    let uia_available = WindowsUiaLocator::is_available();
    let desktop_available = matches!(session_kind, SessionKind::Interactive) && uia_available;
    let desktop_geometry = WindowsUiaLocator::desktop_geometry();
    let dpi = reported_dpi();
    let environment = MachineEnvironment {
        machine: env::var("COMPUTERNAME").unwrap_or_else(|_| "UNAVAILABLE".to_owned()),
        windows_version: powershell_value(
            "(Get-CimInstance Win32_OperatingSystem | ForEach-Object { $_.Caption + ' ' + $_.Version + ' build ' + $_.BuildNumber })",
        ),
        terminal_version: powershell_value(
            "(Get-AppxPackage -Name Microsoft.WindowsTerminal | Select-Object -First 1 -ExpandProperty Version)",
        ),
        session_id,
        session_kind: format!("{session_kind:?}"),
        desktop: if desktop_available {
            "UIA desktop root available".to_owned()
        } else {
            "UIA desktop root unavailable or noninteractive session".to_owned()
        },
        dpi_scaling: dpi.to_string(),
        display_geometry: desktop_geometry,
        rust_toolchain: command_value("rustc", &["--version"]),
    };
    let probe = PreflightProbe {
        session: session_kind,
        desktop: availability(desktop_available),
        terminal: availability(terminal_available),
        uia: availability(uia_available),
        capture: Availability::Unknown,
    };
    (environment, probe)
}

fn reported_dpi() -> u32 {
    powershell_value(
        "(Get-ItemProperty 'HKCU:\\Control Panel\\Desktop\\WindowMetrics' -ErrorAction SilentlyContinue | Select-Object -ExpandProperty AppliedDPI)",
    )
    .parse::<u32>()
    .ok()
    .filter(|dpi| *dpi >= 96)
    .unwrap_or(96)
}

fn process_session() -> (String, SessionKind) {
    let process_id = process::id().to_string();
    let output = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {process_id}"), "/FO", "CSV", "/NH"])
        .output();
    let Ok(output) = output else {
        return ("UNAVAILABLE".to_owned(), SessionKind::Unknown);
    };
    let output = String::from_utf8_lossy(&output.stdout);
    let line = output.lines().find(|line| line.starts_with('"'));
    let Some(line) = line else {
        return ("UNAVAILABLE".to_owned(), SessionKind::Unknown);
    };
    let fields = line
        .trim()
        .trim_matches('"')
        .split("\",\"")
        .collect::<Vec<_>>();
    let session_name = fields.get(2).copied().unwrap_or_default();
    let session_id = fields.get(3).copied().unwrap_or("UNAVAILABLE").to_owned();
    let kind = if session_id == "0" {
        SessionKind::SessionZero
    } else if session_name.eq_ignore_ascii_case("services") {
        SessionKind::Service
    } else if session_id != "UNAVAILABLE" && !session_name.is_empty() {
        SessionKind::Interactive
    } else {
        SessionKind::Unknown
    };
    (session_id, kind)
}

fn powershell_value(command: &str) -> String {
    command_value(
        "powershell.exe",
        &["-NoProfile", "-NonInteractive", "-Command", command],
    )
}

fn command_value(program: &str, arguments: &[&str]) -> String {
    Command::new(program)
        .args(arguments)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "UNAVAILABLE".to_owned())
}

fn availability(value: bool) -> Availability {
    if value {
        Availability::Available
    } else {
        Availability::Unavailable
    }
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

fn is_exact_sha(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Default)]
struct LaneDisposition {
    preflight: Option<VisualDisposition>,
    uia: Option<VisualDisposition>,
    capture: Option<VisualDisposition>,
    title: Option<VisualDisposition>,
    color: Option<VisualDisposition>,
    animation: Option<VisualDisposition>,
}

impl LaneDisposition {
    fn observe_preflight(&mut self, value: VisualDisposition) {
        merge(&mut self.preflight, value);
    }

    fn observe_uia(&mut self, value: VisualDisposition) {
        merge(&mut self.uia, value);
    }

    fn observe_capture(&mut self, value: VisualDisposition) {
        merge(&mut self.capture, value);
    }

    fn observe_title(&mut self, value: VisualDisposition) {
        merge(&mut self.title, value);
    }

    fn observe_color(&mut self, value: VisualDisposition) {
        merge(&mut self.color, value);
    }

    fn observe_animation(&mut self, value: VisualDisposition) {
        merge(&mut self.animation, value);
    }

    fn has_failure(&self) -> bool {
        self.values()
            .any(|value| matches!(value, VisualDisposition::Fail))
    }

    fn has_blocker(&self) -> bool {
        self.values()
            .any(|value| matches!(value, VisualDisposition::Blocked))
    }

    fn has_unproven(&self) -> bool {
        self.values()
            .any(|value| matches!(value, VisualDisposition::Unproven))
    }

    fn uia(&self) -> VisualDisposition {
        self.uia.unwrap_or(VisualDisposition::Unproven)
    }

    fn capture(&self) -> VisualDisposition {
        self.capture.unwrap_or(VisualDisposition::Unproven)
    }

    fn title(&self) -> VisualDisposition {
        self.title.unwrap_or(VisualDisposition::Unproven)
    }

    fn color(&self) -> VisualDisposition {
        self.color.unwrap_or(VisualDisposition::Unproven)
    }

    fn animation(&self) -> VisualDisposition {
        self.animation.unwrap_or(VisualDisposition::Unproven)
    }

    fn values(&self) -> impl Iterator<Item = VisualDisposition> {
        [
            self.preflight,
            self.uia,
            self.capture,
            self.title,
            self.color,
            self.animation,
        ]
        .into_iter()
        .flatten()
    }
}

fn merge(target: &mut Option<VisualDisposition>, incoming: VisualDisposition) {
    let current = target.unwrap_or(VisualDisposition::Pass);
    let rank = |value: VisualDisposition| match value {
        VisualDisposition::Pass => 0,
        VisualDisposition::Unproven => 1,
        VisualDisposition::Blocked => 2,
        VisualDisposition::Fail => 3,
    };
    if rank(incoming) >= rank(current) {
        *target = Some(incoming);
    }
}

#[cfg(test)]
mod tests {
    use crate::visual::Rgb;

    use super::{
        RgbaFrame, Roi, ScreenRect, UiaDump, empty_uia_dump, progress_roi, relative_roi,
        target_has_capturable_geometry,
    };

    #[test]
    fn tab_roi_uses_actual_capture_to_uia_window_mapping() {
        let frame = RgbaFrame::solid(1492, 966, Rgb::new(0, 0, 0)).expect("valid frame");
        let roi = relative_roi(
            ScreenRect::new(34, 40, 746, 483),
            ScreenRect::new(44, 49, 248, 32),
            &frame,
        )
        .expect("logically reported tab maps inside the physical capture");
        assert_eq!(roi, Roi::new(20, 18, 496, 64));

        let physically_reported = relative_roi(
            ScreenRect::new(67, 80, 1492, 966),
            ScreenRect::new(565, 97, 493, 64),
            &frame,
        )
        .expect("physically reported tab maps without duplicate DPI scaling");
        assert_eq!(physically_reported, Roi::new(498, 17, 493, 64));
    }

    #[test]
    fn capturable_geometry_rejects_zero_sized_or_off_window_tabs() {
        let valid = UiaDump {
            window_bounds: Some(ScreenRect::new(67, 80, 1492, 966)),
            tab_bounds: Some(ScreenRect::new(565, 97, 493, 64)),
            ..empty_uia_dump()
        };
        assert!(target_has_capturable_geometry(&valid));

        let zero_window = UiaDump {
            window_bounds: Some(ScreenRect::new(0, 0, 0, 0)),
            tab_bounds: Some(ScreenRect::new(-31_575, -31_983, 433, 64)),
            ..empty_uia_dump()
        };
        assert!(!target_has_capturable_geometry(&zero_window));

        let off_window_tab = UiaDump {
            window_bounds: Some(ScreenRect::new(0, 0, 100, 100)),
            tab_bounds: Some(ScreenRect::new(-1, 0, 50, 20)),
            ..empty_uia_dump()
        };
        assert!(!target_has_capturable_geometry(&off_window_tab));
    }

    #[test]
    fn progress_roi_selects_a_bounded_icon_square_inside_tab() {
        assert_eq!(
            progress_roi(Roi::new(20, 18, 496, 64)).expect("sufficient tab geometry"),
            Roi::new(52, 34, 32, 32)
        );
    }
}
