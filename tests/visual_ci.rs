use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use tabbeacon::visual::{
    AnimationOutcome, AnimationThreshold, AssertionKind, AssertionResult, Availability,
    ColorClassification, ColorSemantic, ColorTolerance, DesktopPreflight, EvidenceBundle,
    EvidenceManifest, EvidenceWriter, MachineEnvironment, PreflightBlocker, PreflightProbe, Rgb,
    RgbaFrame, Roi, ScreenRect, SessionKind, UiaDump, VisualDisposition, VisualError,
    assess_animation, classify_color, color_metrics, frame_delta, matches_baseline,
};

fn frame(width: u32, height: u32, color: Rgb) -> RgbaFrame {
    RgbaFrame::solid(width, height, color).expect("synthetic frame dimensions are valid")
}

fn preflight(disposition: VisualDisposition) -> DesktopPreflight {
    DesktopPreflight {
        disposition,
        blockers: Vec::new(),
        detail: "synthetic test preflight".to_owned(),
    }
}

fn environment() -> MachineEnvironment {
    MachineEnvironment {
        machine: "test-machine".to_owned(),
        windows_version: "test-windows".to_owned(),
        terminal_version: "test-terminal".to_owned(),
        session_id: "1".to_owned(),
        session_kind: "INTERACTIVE".to_owned(),
        desktop: "Default".to_owned(),
        dpi_scaling: "96dpi/100%".to_owned(),
        display_geometry: Some(ScreenRect::new(0, 0, 100, 100)),
        rust_toolchain: "rustc test".to_owned(),
    }
}

fn manifest(disposition: VisualDisposition, visual_head: Option<&str>) -> EvidenceManifest {
    EvidenceManifest {
        goal_id: "TB-G03".to_owned(),
        expected_head: "a".repeat(40),
        checked_out_head: "a".repeat(40),
        visual_head: visual_head.map(str::to_owned),
        run_id: "TB03TEST-0001".to_owned(),
        observed_at_unix_seconds: 1,
        capture_backend: "synthetic".to_owned(),
        preflight: preflight(disposition),
        environment: environment(),
        window_geometry: Some(ScreenRect::new(0, 0, 100, 100)),
        fixtures: vec!["ready".to_owned()],
        disposition,
    }
}

#[test]
fn roi_clipping_crop_and_bounds_are_deterministic() {
    let source = frame(4, 3, Rgb::new(1, 2, 3));
    assert_eq!(Roi::new(2, 1, 4, 4).clip(4, 3), Some(Roi::new(2, 1, 2, 2)));
    let crop = source
        .crop(Roi::new(2, 1, 4, 4))
        .expect("intersecting ROI crops");
    assert_eq!((crop.width(), crop.height()), (2, 2));
    assert!(matches!(
        source.crop(Roi::new(4, 0, 1, 1)),
        Err(VisualError::InvalidRoi)
    ));
    assert!(matches!(source.pixel(4, 0), Err(VisualError::InvalidRoi)));
}

#[test]
fn color_aggregation_uses_deterministic_mean_median_and_tolerance() {
    let green = ColorSemantic::Working
        .palette_rgb()
        .expect("non-default palette");
    let source = frame(5, 4, green);
    let metrics = color_metrics(&source, Roi::new(0, 0, 5, 4)).expect("valid ROI");
    assert_eq!(metrics.sample_count, 20);
    assert_eq!(metrics.median, green);
    assert_eq!(metrics.mean_milli.red, u64::from(green.red) * 1_000);
    assert_eq!(
        classify_color(&metrics, ColorTolerance::default()),
        ColorClassification::Match(ColorSemantic::Working)
    );
}

#[test]
fn contaminated_roi_is_not_accepted_as_palette_color() {
    let mut pixels = Vec::new();
    for index in 0_u8..16 {
        let color = if index % 2 == 0 {
            Rgb::new(0, 0, 0)
        } else {
            Rgb::new(255, 255, 255)
        };
        pixels.extend_from_slice(&[color.red, color.green, color.blue, u8::MAX]);
    }
    let source = RgbaFrame::new(4, 4, pixels).expect("valid synthetic frame");
    let metrics = color_metrics(&source, Roi::new(0, 0, 4, 4)).expect("valid ROI");
    assert_eq!(
        classify_color(&metrics, ColorTolerance::default()),
        ColorClassification::ContaminatedRoi
    );
}

#[test]
fn ready_and_reset_use_same_run_default_baseline() {
    let baseline = color_metrics(&frame(3, 3, Rgb::new(35, 35, 35)), Roi::new(0, 0, 3, 3))
        .expect("baseline metrics");
    let reset = color_metrics(&frame(3, 3, Rgb::new(37, 35, 35)), Roi::new(0, 0, 3, 3))
        .expect("reset metrics");
    assert!(matches_baseline(
        &reset,
        &baseline,
        ColorTolerance::default()
    ));
}

#[test]
fn animation_oracle_accepts_substantial_roi_motion() {
    let first = frame(10, 4, Rgb::new(0, 0, 0));
    let mut pixels = first.pixels().to_vec();
    for offset in (0..24).step_by(4) {
        pixels[offset] = 255;
        pixels[offset + 1] = 255;
        pixels[offset + 2] = 255;
    }
    let second = RgbaFrame::new(10, 4, pixels).expect("valid synthetic frame");
    let (outcome, deltas) = assess_animation(
        &[first, second],
        Roi::new(0, 0, 10, 4),
        AnimationThreshold::default(),
    )
    .expect("comparable frames");
    assert_eq!(outcome, AnimationOutcome::AnimationPresent);
    assert_eq!(deltas.len(), 1);
    assert!(deltas[0].changed_pixel_ratio_milli >= 20);
}

#[test]
fn stationary_and_low_noise_frames_do_not_false_positive_animation() {
    let stationary = frame(10, 4, Rgb::new(20, 20, 20));
    let mut noisy_pixels = stationary.pixels().to_vec();
    for offset in (0..16).step_by(4) {
        noisy_pixels[offset] = noisy_pixels[offset].saturating_add(2);
    }
    let noisy = RgbaFrame::new(10, 4, noisy_pixels).expect("valid synthetic frame");
    let (outcome, _) = assess_animation(
        &[stationary, noisy],
        Roi::new(0, 0, 10, 4),
        AnimationThreshold::default(),
    )
    .expect("comparable frames");
    assert_eq!(outcome, AnimationOutcome::AnimationAbsent);
}

#[test]
fn animation_rejects_inconsistent_frame_dimensions() {
    let result = frame_delta(
        &frame(2, 2, Rgb::new(0, 0, 0)),
        &frame(3, 2, Rgb::new(0, 0, 0)),
        Roi::new(0, 0, 2, 2),
        8,
    );
    assert!(matches!(
        result,
        Err(VisualError::InconsistentFrames { .. })
    ));
}

#[test]
fn preflight_distinguishes_environment_blockers_from_assertions() {
    let report = DesktopPreflight::assess(PreflightProbe {
        session: SessionKind::SessionZero,
        desktop: Availability::Unavailable,
        terminal: Availability::Available,
        uia: Availability::Available,
        capture: Availability::Unavailable,
    });
    assert_eq!(report.disposition, VisualDisposition::Blocked);
    assert_eq!(
        report.blockers,
        vec![
            PreflightBlocker::SessionZero,
            PreflightBlocker::DesktopUnavailable,
            PreflightBlocker::CaptureUnavailable,
        ]
    );
}

#[test]
fn evidence_manifest_serializes_and_requires_exact_visual_head_for_pass() {
    let pass_manifest = manifest(VisualDisposition::Pass, Some(&"a".repeat(40)));
    pass_manifest
        .validate_exact_heads_for_pass()
        .expect("matching exact heads permit PASS");
    let serialized = serde_json::to_vec(&pass_manifest).expect("manifest serializes");
    let decoded: EvidenceManifest = serde_json::from_slice(&serialized).expect("manifest parses");
    assert_eq!(decoded, pass_manifest);

    let mismatch = manifest(VisualDisposition::Pass, Some(&"b".repeat(40)));
    assert!(matches!(
        mismatch.validate_exact_heads_for_pass(),
        Err(VisualError::ExactHeadMismatch { .. })
    ));
}

#[test]
fn evidence_writer_refuses_overwrite_and_writes_only_owned_bundle_files() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after Unix epoch")
        .as_nanos();
    let root = PathBuf::from("target")
        .join("g03-test-evidence")
        .join(nonce.to_string());
    let writer = EvidenceWriter::create(&root, "TB03TEST-0002").expect("fresh owned evidence dir");
    let bundle = EvidenceBundle {
        manifest: manifest(VisualDisposition::Blocked, None),
        assertions: vec![AssertionResult {
            kind: AssertionKind::Capture,
            disposition: VisualDisposition::Blocked,
            fixture: Some("working".to_owned()),
            detail: "synthetic capture blocker".to_owned(),
        }],
        environment: environment(),
        uia: UiaDump {
            window_name: "owned-window".to_owned(),
            tab_name: "owned-tab".to_owned(),
            window_bounds: None,
            tab_bounds: None,
            native_window_handle: None,
            window_has_keyboard_focus: None,
            detail: "synthetic".to_owned(),
        },
        color_metrics: Vec::new(),
    };
    writer
        .write_bundle(&bundle)
        .expect("writes owned JSON evidence");
    writer
        .write_png("tab-working", &frame(2, 2, Rgb::new(1, 2, 3)))
        .expect("writes lossless owned PNG");
    assert!(writer.directory().join("manifest.json").is_file());
    assert!(writer.directory().join("tab-working.png").is_file());
    assert!(matches!(
        EvidenceWriter::create(&root, "TB03TEST-0002"),
        Err(VisualError::EvidenceDirectoryExists(_))
    ));
}
