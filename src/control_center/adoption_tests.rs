//! Consumer proofs use production input/render/adapter paths and isolated children.
use super::{
    BoundedRefreshCollector, ControlCenterApp, ControlCenterCommand, ControlCenterOverlay, Screen,
    input, render, tests,
};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::{Terminal, backend::TestBackend};
use std::{
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};
use terminal_ui_contract::{
    document::ViewDensity,
    focus::FocusPath,
    keymap::{InputMode, KeymapProfile},
};
use terminal_ui_runtime::refresh::{RefreshController, RefreshPolicy, RefreshPoll, RequestKind};

fn draw(app: &ControlCenterApp, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| render(frame, app)).unwrap();
    format!("{:?}", terminal.backend().buffer())
}
fn mouse(
    app: &mut ControlCenterApp,
    kind: MouseEventKind,
    column: u16,
    row: u16,
) -> ControlCenterCommand {
    app.handle_input(Event::Mouse(MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }))
}
fn policy() -> RefreshPolicy {
    RefreshPolicy {
        task_timeout: Duration::from_secs(5),
        poll_interval: Duration::from_secs(1),
        stale_after: Duration::from_secs(3),
    }
}

fn completed(runtime: &mut RefreshController<(), super::ControlCenterRefresh>) -> RefreshPoll {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let result = runtime.poll();
        if result != RefreshPoll::Pending {
            return result;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn accepted_epoch_revision_last_good_and_controlled_collection_timings() {
    let app = tests::app();
    let mut value = tests::refresh(app.current(), app.current_interface());
    let samples = Arc::new(Mutex::new(vec![]));
    let times = samples.clone();
    let (started_tx, started) = mpsc::sync_channel(1);
    let (release, gate) = mpsc::sync_channel(1);
    let mut calls = 0;
    let collector = BoundedRefreshCollector(
        move |control: &terminal_ui_runtime::refresh::CollectionControl| {
            let collection_started = Instant::now();
            calls += 1;
            if calls == 1 {
                started_tx.send(()).unwrap();
                while gate.try_recv().is_err() {
                    control
                        .wait_for(Duration::from_millis(1))
                        .map_err(std::io::Error::other)?;
                }
            }
            control
                .wait_for(Duration::from_millis(20))
                .map_err(std::io::Error::other)?;
            times
                .lock()
                .unwrap()
                .push(collection_started.elapsed().as_secs_f64() * 1000.0);
            if calls == 22 {
                return Err(std::io::Error::other("synthetic error"));
            }
            value.overview.tabbeacon_version = format!("synthetic-{calls}");
            Ok(value.clone())
        },
    );
    let mut runtime = RefreshController::spawn(collector, policy()).unwrap();
    runtime.request((), RequestKind::Manual).unwrap();
    started.recv_timeout(Duration::from_secs(1)).unwrap();
    runtime.advance_epoch().unwrap();
    runtime.request((), RequestKind::Manual).unwrap();
    release.send(()).unwrap();
    assert_eq!(completed(&mut runtime), RefreshPoll::Stale);
    assert!(runtime.snapshot().is_none());
    for sample in 0..20 {
        assert!(
            matches!(completed(&mut runtime),RefreshPoll::Accepted{revision,..} if revision==sample+1)
        );
        assert_eq!(runtime.snapshot().unwrap().epoch(), 1);
        if sample < 19 {
            runtime.request((), RequestKind::Manual).unwrap();
        }
    }
    let retained = runtime.snapshot().unwrap().clone();
    runtime.request((), RequestKind::Manual).unwrap();
    assert!(matches!(completed(&mut runtime), RefreshPoll::Failed(_)));
    assert!(Arc::ptr_eq(runtime.state().accepted().unwrap(), &retained));
    runtime.shutdown(Duration::from_secs(1)).unwrap();
    let mut samples = samples.lock().unwrap().clone();
    samples.remove(0);
    samples.truncate(20);
    samples.sort_by(f64::total_cmp);
    println!(
        "CONTROLLED_COLLECTOR_MS_P50={:.3} P95={:.3} SAMPLES=20 SYNTHETIC=true EPOCH_REJECTION=PASS LAST_GOOD_ERROR=PASS",
        samples[9], samples[18]
    );
}

#[test]
fn standard_context_vim_counts_insert_and_custom_are_unambiguous() {
    let mut app = tests::app();
    app.screen = Screen::Diagnostics;
    app.handle_key(KeyCode::Char('2'));
    assert_eq!(
        app.diagnostic_provider,
        crate::presentation_policy::CliTarget::Agy
    );
    app.handle_event(KeyEvent::new(KeyCode::Char('3'), KeyModifiers::CONTROL));
    assert_eq!(
        app.diagnostic_provider,
        crate::presentation_policy::CliTarget::Agy
    );
    app.handle_key(KeyCode::Char('m'));
    assert_eq!(
        app.diagnostic_mode,
        crate::doctor_matrix::ExecutionMode::Direct
    );
    app.screen = Screen::Integration;
    app.provider_presentation.push((
        crate::presentation_policy::CliTarget::Cursor,
        crate::presentation_policy::resolve_presentation(
            app.current(),
            crate::presentation_policy::PresentationOverride::default(),
            crate::presentation_policy::PresentationCapabilities::NONE,
            crate::presentation_policy::ApplicationStatus::Unproven,
        ),
    ));
    app.handle_key(KeyCode::Char('3'));
    assert_eq!(
        app.selected_provider,
        crate::presentation_policy::CliTarget::Cursor
    );
    app.handle_key(KeyCode::Right);
    app.handle_key(KeyCode::Char('r'));
    assert_eq!(app.selected_provider_mode, None);
    app.handle_key(KeyCode::Char('v'));
    assert!(matches!(
        app.overlay,
        ControlCenterOverlay::ProviderPreview { .. }
    ));
    app.handle_key(KeyCode::Esc);
    app.screen = Screen::Diagnostics;
    app.set_keymap_profile(KeymapProfile::Vim).unwrap();
    app.interaction
        .get_mut()
        .focus
        .focus(&input::details_path());
    draw(&app, 100, 24);
    let provider = app.diagnostic_provider;
    app.handle_key(KeyCode::Char('5'));
    app.handle_key(KeyCode::Char('j'));
    assert_eq!(app.interaction.borrow().details.offset(), 5);
    assert_eq!(app.diagnostic_provider, provider);
    app.handle_key(KeyCode::Char('g'));
    app.handle_key(KeyCode::Char('g'));
    assert_eq!(app.interaction.borrow().details.offset(), 0);
    app.handle_key(KeyCode::Char('G'));
    assert_eq!(
        app.interaction.borrow().details.offset(),
        app.interaction.borrow().details.max_offset()
    );
    app.handle_event(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
    assert!(
        app.interaction.borrow().details.offset() < app.interaction.borrow().details.max_offset()
    );
    app.handle_event(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));
    app.handle_event(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::ALT));
    assert_eq!(
        app.diagnostic_provider,
        crate::presentation_policy::CliTarget::Codex
    );
    app.handle_key(KeyCode::Char('i'));
    assert_eq!(app.interaction.borrow().mode, InputMode::Insert);
    let mode = app.diagnostic_mode;
    for key in ['q', 'm', '3'] {
        assert_eq!(
            app.handle_key(KeyCode::Char(key)),
            ControlCenterCommand::None
        );
    }
    assert_eq!(app.diagnostic_mode, mode);
    assert_eq!(
        app.diagnostic_provider,
        crate::presentation_policy::CliTarget::Codex
    );
    app.handle_key(KeyCode::Esc);
    app.load_keymap_toml(
        "schema_version=1\n[[bindings]]\ncontext='global'\nkeys=['F6']\naction='ui.refresh'",
    )
    .unwrap();
    assert_eq!(app.handle_key(KeyCode::F(6)), ControlCenterCommand::Refresh);
    assert!(
        app.load_keymap_toml("schema_version=1\nexecute='shell'")
            .is_err()
    );
    assert_eq!(app.handle_key(KeyCode::F(6)), ControlCenterCommand::Refresh);
    app.handle_key(KeyCode::Char('?'));
    let help = draw(&app, 120, 50);
    assert!(help.contains("F6"));
}

#[test]
fn mouse_focus_hover_wheel_modal_and_resize_use_committed_geometry() {
    let mut app = tests::app();
    app.screen = Screen::Diagnostics;
    let before = app.current_draft();
    draw(&app, 100, 24);
    assert_eq!(
        app.interaction.borrow().focus.current(),
        Some(&input::nav_path())
    );
    assert_eq!(
        mouse(&mut app, MouseEventKind::ScrollDown, 40, 5),
        ControlCenterCommand::None
    );
    assert!(app.interaction.borrow().details.offset() > 0);
    assert_eq!(
        app.interaction.borrow().focus.current(),
        Some(&input::nav_path())
    );
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), 40, 5);
    assert_eq!(
        app.interaction.borrow().focus.current(),
        Some(&input::details_path())
    );
    assert!(draw(&app, 100, 24).contains("[FOCUS] Diagnostics"));
    let offset = app.interaction.borrow().details.offset();
    app.handle_key(KeyCode::Down);
    assert_eq!(app.interaction.borrow().details.offset(), offset + 1);
    app.handle_key(KeyCode::End);
    let end = app.interaction.borrow().details.offset();
    app.handle_key(KeyCode::PageDown);
    assert_eq!(app.interaction.borrow().details.offset(), end);
    app.handle_key(KeyCode::Home);
    assert_eq!(app.interaction.borrow().details.offset(), 0);
    app.handle_key(KeyCode::BackTab);
    assert_eq!(
        app.interaction.borrow().focus.current(),
        Some(&input::nav_path())
    );
    app.handle_key(KeyCode::Tab);
    assert_eq!(
        app.interaction.borrow().focus.current(),
        Some(&input::details_path())
    );
    app.handle_key(KeyCode::Char('?'));
    draw(&app, 100, 24);
    mouse(&mut app, MouseEventKind::ScrollDown, 40, 5);
    assert!(app.interaction.borrow().overlay.offset() > 0);
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), 2, 5);
    assert_eq!(
        app.interaction.borrow().focus.current(),
        Some(&input::modal_path())
    );
    assert_eq!(app.current_draft(), before);
    app.handle_key(KeyCode::Esc);
    draw(&app, 100, 24);
    assert_eq!(
        app.interaction.borrow().focus.current(),
        Some(&input::details_path())
    );
    app.handle_input(Event::Resize(24, 12));
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), 2, 5);
    assert_eq!(
        app.interaction.borrow().focus.current(),
        Some(&input::details_path())
    );
    draw(&app, 24, 12);
    app.handle_key(KeyCode::End);
    draw(&app, 100, 50);
    assert!(
        app.interaction.borrow().details.offset() <= app.interaction.borrow().details.max_offset()
    );
    draw(&app, 23, 9);
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), 2, 5);
    assert_eq!(
        app.interaction.borrow().focus.current(),
        Some(&input::details_path())
    );
}

#[test]
fn nested_hit_regions_choose_deepest_visible_target_without_writes() {
    use ratatui::layout::Rect;
    use terminal_ui_ratatui::geometry::{HitMap, HitRegion};
    let mut app = tests::app();
    let nested = FocusPath::new(&["control", "details", "nested"]);
    let mut i = app.interaction.borrow_mut();
    i.generation = 5;
    let region = |path, rect, z| HitRegion {
        path,
        rect,
        clip: Rect::new(0, 0, 100, 40),
        z,
        enabled: true,
        focusable: true,
        scrollable: true,
    };
    i.hitmap = HitMap::new(
        5,
        vec![
            region(input::details_path(), Rect::new(21, 2, 79, 36), 0),
            region(nested.clone(), Rect::new(30, 4, 20, 10), 1),
        ],
    );
    i.focus
        .reconcile(vec![input::details_path(), nested.clone()]);
    i.details.reflow(100, 10);
    drop(i);
    assert_eq!(
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), 33, 6),
        ControlCenterCommand::None
    );
    assert_eq!(app.interaction.borrow().focus.current(), Some(&nested));
    mouse(&mut app, MouseEventKind::ScrollDown, 33, 6);
    assert_eq!(app.interaction.borrow().details.offset(), 3);
    assert!(!app.dirty());
}

#[test]
fn slow_worker_does_not_block_first_frame_input_or_density_and_preserves_dirty_cas() {
    let mut app = tests::app();
    let snapshot = tests::refresh(app.current(), app.current_interface());
    let (started_tx, started) = mpsc::sync_channel(1);
    let collector = BoundedRefreshCollector(
        move |control: &terminal_ui_runtime::refresh::CollectionControl| {
            started_tx.send(()).unwrap();
            control
                .wait_for(Duration::from_secs(2))
                .map_err(std::io::Error::other)?;
            Ok(snapshot.clone())
        },
    );
    let mut runtime = RefreshController::spawn(collector, policy()).unwrap();
    runtime.request((), RequestKind::Manual).unwrap();
    started.recv_timeout(Duration::from_secs(1)).unwrap();
    let started = Instant::now();
    draw(&app, 100, 24);
    app.handle_key(KeyCode::Tab);
    assert_eq!(app.handle_key(KeyCode::F(5)), ControlCenterCommand::Refresh);
    app.handle_key(KeyCode::F(2));
    assert_eq!(app.interaction.borrow().view.density, ViewDensity::Raw);
    assert!(started.elapsed() < Duration::from_secs(1));
    for _ in 0..100 {
        runtime.request((), RequestKind::Manual).unwrap();
    }
    assert!(runtime.is_in_flight() && runtime.has_pending());
    runtime.advance_epoch().unwrap();
    runtime.shutdown(Duration::from_secs(1)).unwrap();
    assert!(!runtime.worker_owned());
    app.screen = Screen::Appearance;
    app.handle_key(KeyCode::Enter);
    app.handle_key(KeyCode::Right);
    let draft = app.staged_draft();
    let mut fresh = tests::refresh(app.current(), app.current_interface());
    fresh.presentation = crate::settings::PresentationSettings::preset("native").unwrap();
    app.merge_refresh(fresh);
    assert_eq!(app.staged_draft(), draft);
    assert!(app.has_concurrent_conflict());
    assert_eq!(
        app.handle_key(KeyCode::Char('a')),
        ControlCenterCommand::None
    );
}

#[test]
fn smart_raw_are_same_revision_and_event_frame_timings_are_synthetic() {
    let mut app = tests::app();
    app.snapshot_revision = 42;
    app.sampled_at = std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    app.screen = Screen::Diagnostics;
    let document = app.ui_document();
    let smart = document.project(ViewDensity::Smart);
    let raw = document.project(ViewDensity::Raw);
    assert_eq!(smart.revision(), raw.revision());
    assert_eq!(smart.revision(), 42);
    let before = app.current_draft();
    let mut inputs = vec![];
    let mut frames = vec![];
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    for _ in 0..40 {
        let started = Instant::now();
        app.handle_key(KeyCode::F(2));
        inputs.push(started.elapsed().as_secs_f64() * 1000.0);
        let started = Instant::now();
        terminal.draw(|frame| render(frame, &app)).unwrap();
        frames.push(started.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(app.ui_document().revision(), 42);
    }
    inputs.sort_by(f64::total_cmp);
    frames.sort_by(f64::total_cmp);
    println!(
        "SYNTHETIC_INPUT_MS_P50={:.4} P95={:.4} TESTBACKEND_FRAME_MS_P50={:.4} P95={:.4} REAL_TUI_INPUT_TO_FRAME_P95=UNPROVEN",
        inputs[19], inputs[37], frames[19], frames[37]
    );
    assert_eq!(app.current_draft(), before);
    assert!(!app.dirty());
}

#[test]
#[ignore = "owned observation child invoked only by its cancellation test"]
fn observation_child_fixture() {
    if let Some(path) = std::env::var_os("TB_OBSERVATION_CHILD_FIXTURE") {
        std::fs::write(path, b"started").unwrap();
        std::thread::sleep(Duration::from_mins(1));
    }
}

#[test]
fn product_adapter_shutdown_reaps_its_slow_child_and_joins_worker() {
    let app = tests::app();
    let snapshot = tests::refresh(app.current(), app.current_interface());
    let root = tempfile::tempdir().unwrap();
    let marker = root.path().join("started");
    let child_marker = marker.clone();
    let outcome = Arc::new(Mutex::new(None));
    let result = outcome.clone();
    let collector = BoundedRefreshCollector(
        move |control: &terminal_ui_runtime::refresh::CollectionControl| {
            let error = crate::provider_observation::bounded_output(
                std::process::Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--exact",
                        "control_center::adoption_tests::observation_child_fixture",
                        "--ignored",
                    ])
                    .env("TB_OBSERVATION_CHILD_FIXTURE", &child_marker),
                Duration::from_secs(30),
                Some(control),
            )
            .err();
            *result.lock().unwrap() = error;
            Ok(snapshot.clone())
        },
    );
    let mut runtime = RefreshController::spawn(collector, policy()).unwrap();
    runtime.request((), RequestKind::Manual).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while !marker.exists() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    let started = Instant::now();
    runtime.shutdown(Duration::from_secs(1)).unwrap();
    assert!(!runtime.worker_owned());
    assert_eq!(
        *outcome.lock().unwrap(),
        Some(crate::provider_observation::ProbeError::Cancelled)
    );
    println!(
        "OWNED_CHILD_TERMINATED_AND_REAPED=true WORKER_JOINED=true SHUTDOWN_MS={:.3}",
        started.elapsed().as_secs_f64() * 1000.0
    );
    assert!(started.elapsed() < Duration::from_secs(1));
}
