use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use tabbeacon::settings::PresentationTheme;
use tabbeacon::visual::{
    AnimationOutcome, AnimationThreshold, AssertionKind, AssertionResult, Availability,
    ColorClassification, ColorSemantic, ColorTolerance, DesktopPreflight, EvidenceBundle,
    EvidenceManifest, EvidenceWriter, FailureCategory, FixtureDriver, MachineEnvironment,
    PreflightBlocker, PreflightProbe, Rgb, RgbaFrame, Roi, ScreenRect, SessionKind, UiaDump,
    VisualDisposition, VisualError, WindowActivation, assess_animation, classify_color,
    classify_color_for_theme, color_metrics, frame_delta, matches_baseline, select_background_roi,
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
fn muted_dark_color_oracle_uses_the_selected_semantic_palette() {
    let muted = ColorSemantic::Working
        .palette_rgb_for(PresentationTheme::MutedDark)
        .expect("working color exists");
    let classic = ColorSemantic::Working
        .palette_rgb_for(PresentationTheme::Classic)
        .expect("classic working color exists");
    assert_ne!(muted, classic);
    let metrics = color_metrics(&frame(5, 4, muted), Roi::new(0, 0, 5, 4))
        .expect("valid muted frame metrics");
    assert_eq!(
        classify_color_for_theme(
            &metrics,
            ColorTolerance::default(),
            PresentationTheme::MutedDark
        ),
        ColorClassification::Match(ColorSemantic::Working)
    );
    for semantic in [
        ColorSemantic::ResultReady,
        ColorSemantic::Warning,
        ColorSemantic::Interrupted,
        ColorSemantic::Failed,
    ] {
        let color = semantic
            .palette_rgb_for(PresentationTheme::MutedDark)
            .expect("non-default semantic color exists");
        let metrics = color_metrics(&frame(5, 4, color), Roi::new(0, 0, 5, 4))
            .expect("valid muted frame metrics");
        assert_eq!(
            classify_color_for_theme(
                &metrics,
                ColorTolerance::default(),
                PresentationTheme::MutedDark
            ),
            ColorClassification::Match(semantic)
        );
    }
    let approval = ColorSemantic::Approval
        .palette_rgb_for(PresentationTheme::MutedDark)
        .expect("approval color exists");
    let metrics = color_metrics(&frame(5, 4, approval), Roi::new(0, 0, 5, 4))
        .expect("valid approval frame metrics");
    assert!(matches!(
        classify_color_for_theme(
            &metrics,
            ColorTolerance::default(),
            PresentationTheme::MutedDark
        ),
        ColorClassification::Match(ColorSemantic::Approval) | ColorClassification::Ambiguous(_)
    ));
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
fn background_roi_selection_avoids_a_high_variance_text_like_tile() {
    let green = Rgb::new(0x2e, 0xcc, 0x71);
    let mut pixels = frame(12, 4, green).pixels().to_vec();
    for y in 1..3 {
        let offset = usize::try_from((y * 12 + 1) * 4).expect("small synthetic offset");
        pixels[offset..offset + 3].copy_from_slice(&[255, 255, 255]);
    }
    let source = RgbaFrame::new(12, 4, pixels).expect("valid synthetic frame");
    let (roi, metrics) = select_background_roi(&source, Roi::new(0, 0, 12, 4))
        .expect("an interior background tile is available");
    assert!(roi.x > 1);
    assert_eq!(
        metrics.variance,
        tabbeacon::visual::RgbVariance {
            red: 0,
            green: 0,
            blue: 0
        }
    );
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
fn assertion_results_classify_exact_head_and_capture_failures() {
    let mismatch = AssertionResult::new(
        AssertionKind::ExactHead,
        VisualDisposition::Fail,
        None,
        "wrong SHA".to_owned(),
    );
    assert_eq!(
        mismatch.failure_category,
        Some(FailureCategory::EvidenceMismatch)
    );
    let capture = AssertionResult::new(
        AssertionKind::Capture,
        VisualDisposition::Blocked,
        Some("working".to_owned()),
        "window occluded".to_owned(),
    );
    assert_eq!(
        capture.failure_category,
        Some(FailureCategory::RunnerEnvironmentDefect)
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
    let invalid_sha = EvidenceManifest {
        expected_head: "not-a-sha".to_owned(),
        checked_out_head: "not-a-sha".to_owned(),
        visual_head: Some("not-a-sha".to_owned()),
        ..manifest(VisualDisposition::Pass, Some(&"a".repeat(40)))
    };
    assert!(matches!(
        invalid_sha.validate_exact_heads_for_pass(),
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
        assertions: vec![AssertionResult::new(
            AssertionKind::Capture,
            VisualDisposition::Blocked,
            Some("working".to_owned()),
            "synthetic capture blocker".to_owned(),
        )],
        environment: environment(),
        uia: UiaDump {
            window_name: "owned-window".to_owned(),
            tab_name: "owned-tab".to_owned(),
            window_bounds: None,
            tab_bounds: None,
            native_window_handle: None,
            window_has_keyboard_focus: None,
            activation: Some(WindowActivation {
                set_foreground: true,
                set_focus: true,
            }),
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
    let integrity = writer
        .write_integrity_manifest()
        .expect("writes deterministic owned evidence integrity");
    assert!(writer.directory().join("manifest.json").is_file());
    assert!(writer.directory().join("tab-working.png").is_file());
    assert!(writer.directory().join("integrity.json").is_file());
    assert_eq!(integrity.algorithm, "SHA-256");
    assert_eq!(integrity.tree_sha256.len(), 64);
    let names = integrity
        .files
        .iter()
        .map(|file| file.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        vec![
            "assertions.json",
            "color-metrics.json",
            "environment.json",
            "manifest.json",
            "tab-working.png",
            "uia.json",
        ]
    );
    let integrity_json = std::fs::read_to_string(writer.directory().join("integrity.json"))
        .expect("owned integrity evidence reads");
    assert!(integrity_json.contains(&integrity.tree_sha256));
    assert!(matches!(
        writer.write_integrity_manifest(),
        Err(VisualError::EvidenceArtifactExists(_))
    ));
    let uia = std::fs::read_to_string(writer.directory().join("uia.json"))
        .expect("owned UIA evidence reads");
    assert!(uia.contains("set_foreground"));
    assert!(matches!(
        EvidenceWriter::create(&root, "TB03TEST-0002"),
        Err(VisualError::EvidenceDirectoryExists(_))
    ));
    assert!(matches!(
        writer.write_png("tab-working", &frame(2, 2, Rgb::new(1, 2, 3))),
        Err(VisualError::EvidenceArtifactExists(_))
    ));
}

#[test]
fn fixture_driver_uses_a_unique_title_without_changing_g02_semantics() {
    let driver = FixtureDriver::default();
    let cases = driver.all_cases("TB03TEST-unique").expect("safe run token");
    assert_eq!(cases.len(), 10);
    assert!(cases.iter().all(|case| {
        case.case
            .expected_title
            .split_once(' ')
            .is_some_and(|(_, identity)| identity.starts_with("TB03-TB03TEST-unique-"))
    }));
    assert!(cases.iter().all(|case| !case.vt_bytes.is_empty()));
    for case in &cases {
        let expected_slot = match case.case.fixture_name.as_str() {
            "ready" | "reset" => "○",
            "working" => "⠋",
            "result-ready" => "✓",
            "approval" | "warning-working" | "warning-idle" => "!",
            "question" => "?",
            "interrupted" => "⊘",
            "failed" => "×",
            other => panic!("unexpected fixture: {other}"),
        };
        let (slot, identity) = case
            .case
            .expected_title
            .split_once(' ')
            .expect("status-first grammar has one separator");
        assert_eq!(slot, expected_slot);
        assert_eq!(
            identity,
            format!("TB03-TB03TEST-unique-{}", case.case.fixture_name)
        );
    }
    let working = cases
        .iter()
        .find(|case| case.case.fixture_name == "working")
        .expect("working fixture exists");
    assert!(working.case.expected_title.starts_with("⠋ "));
    assert!(working.case.expected_title.ends_with("-working"));
    assert_eq!(working.case.theme, PresentationTheme::MutedDark);
    assert!(working.case.expects_animation);
    assert!(working.case.expects_title_animation);
    assert!(working.case.expected_title_frames.len() >= 2);
    assert!(working.title_frame_bytes.len() >= 2);
    let aliases = working
        .case
        .expected_title_frames
        .iter()
        .map(|title| title.split_once(' ').expect("status-first title").1)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(aliases.len(), 1, "title animation keeps the alias stable");
    let reset = driver
        .reset("TB03TEST-unique")
        .expect("reset fixture exists");
    assert_eq!(reset.case.fixture_name, "reset");
    assert!(reset.case.expected_title.starts_with("○ "));
}
