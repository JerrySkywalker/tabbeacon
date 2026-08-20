use tabbeacon::{
    core::{
        AgentEvidence, AgentProvider, AgentSessionKey, Attention, EvidenceAuthority,
        EvidenceConfidence, EvidenceSource, EvidenceTieBreak, FieldUpdate, Health, Phase,
        SessionReconciler, StatePatch,
    },
    presentation::{
        MAX_TITLE_SCALARS, PresentationAction, PresentationPolicy, Progress, ResetSemantics,
        SemanticPresentationInput, TabColor, TerminalTitle, TitleIdentity, TitleStatus,
        WindowsTerminalCapabilities, WindowsTerminalRenderer, presentation_fixture,
        replay_presentation_fixture,
    },
    settings::{
        ActivityMode, PresentationSettings, PresentationTheme, SpinnerPreset, TabColorMode,
        TitleMode,
    },
};

fn input(
    phase: Phase,
    attention: Attention,
    health: Health,
    title: &str,
) -> SemanticPresentationInput<'_> {
    SemanticPresentationInput::new(phase, attention, health, title)
}

fn resolve(phase: Phase, attention: Attention, health: Health) -> PresentationAction {
    PresentationPolicy::resolve(input(phase, attention, health, "JPC semantic fixture"))
}

fn visual_state(action: PresentationAction) -> tabbeacon::presentation::VisualState {
    match action {
        PresentationAction::Apply(state) => state,
        PresentationAction::Reset(_) => panic!("test expected an apply action"),
    }
}

#[test]
fn title_encoding_uses_one_st_terminated_osc_envelope() {
    let action = PresentationPolicy::resolve(input(
        Phase::Ready,
        Attention::None,
        Health::Normal,
        "JPC ready",
    ));
    let renderer = WindowsTerminalRenderer::new(WindowsTerminalCapabilities::new(false));

    assert_eq!(
        renderer.render(&action),
        "\x1b]0;○ JPC ready\x1b\\\x1b]9;4;0;0\x1b\\".as_bytes()
    );
}

#[test]
fn title_sanitization_blocks_control_sequence_injection() {
    let hostile = "JPC\x1b]9;4;2;100\x07\x1b\\\u{009c}done";
    let action = PresentationPolicy::resolve(input(
        Phase::Ready,
        Attention::None,
        Health::Normal,
        hostile,
    ));
    let renderer = WindowsTerminalRenderer::new(WindowsTerminalCapabilities::new(false));
    let bytes = renderer.render(&action);
    let expected_title = "JPC�]9;4;2;100��\\�done";

    assert_eq!(
        visual_state(action).repository_alias().as_str(),
        expected_title
    );
    assert_eq!(
        bytes,
        format!("\x1b]0;○ {expected_title}\x1b\\\x1b]9;4;0;0\x1b\\").into_bytes()
    );
    assert!(!bytes.contains(&0x07));
    assert!(!bytes.contains(&0x9c));
}

#[test]
fn title_length_uses_unicode_scalar_limit_and_ellipsis() {
    let long_title = "界".repeat(MAX_TITLE_SCALARS + 1);
    let title = TerminalTitle::new(&long_title);

    assert_eq!(title.as_str().chars().count(), MAX_TITLE_SCALARS);
    assert!(title.as_str().ends_with('…'));
    assert_eq!(
        title.as_str().chars().take(MAX_TITLE_SCALARS - 1).count(),
        MAX_TITLE_SCALARS - 1
    );
}

#[test]
fn renderer_encodes_each_progress_semantic() {
    let renderer = WindowsTerminalRenderer::new(WindowsTerminalCapabilities::new(false));
    let repository_alias = TitleIdentity::new("OWH");
    let cases = [
        (Progress::Clear, b"9;4;0;0".as_slice()),
        (Progress::Indeterminate, b"9;4;3;0".as_slice()),
        (Progress::Warning, b"9;4;4;100".as_slice()),
        (Progress::Error, b"9;4;2;100".as_slice()),
    ];

    for (progress, payload) in cases {
        let action = PresentationAction::Apply(tabbeacon::presentation::VisualState::new(
            repository_alias.clone(),
            TitleStatus::Ready,
            TabColor::Default,
            progress,
        ));
        let bytes = renderer.render(&action);
        let mut expected_suffix = Vec::from(b"\x1b]".as_slice());
        expected_suffix.extend_from_slice(payload);
        expected_suffix.extend_from_slice(b"\x1b\\");
        assert!(bytes.ends_with(&expected_suffix));
    }
}

#[test]
fn renderer_sets_and_resets_frame_color_when_capable() {
    let renderer = WindowsTerminalRenderer::new(WindowsTerminalCapabilities::new(true));
    let working = resolve(Phase::Working, Attention::None, Health::Normal);
    let ready = resolve(Phase::Ready, Attention::None, Health::Normal);

    assert!(
        renderer
            .render(&working)
            .ends_with(b"\x1b]4;264;rgb:2e/cc/71\x1b\\")
    );
    assert!(renderer.render(&ready).ends_with(b"\x1b]104;264\x1b\\"));
}

#[test]
fn failed_interrupted_warning_and_attention_precedence_is_fixed() {
    let failed = visual_state(resolve(Phase::Working, Attention::Approval, Health::Failed));
    assert_eq!(
        (failed.tab_color(), failed.progress()),
        (TabColor::Failed, Progress::Error)
    );

    let interrupted = visual_state(resolve(
        Phase::Working,
        Attention::Approval,
        Health::Interrupted,
    ));
    assert_eq!(
        (interrupted.tab_color(), interrupted.progress()),
        (TabColor::Interrupted, Progress::Clear)
    );

    let warning = visual_state(resolve(
        Phase::Working,
        Attention::Approval,
        Health::Warning,
    ));
    assert_eq!(
        (warning.tab_color(), warning.progress()),
        (TabColor::Warning, Progress::Indeterminate)
    );

    let approval = visual_state(resolve(
        Phase::WaitingUser,
        Attention::Approval,
        Health::Normal,
    ));
    assert_eq!(
        (approval.tab_color(), approval.progress()),
        (TabColor::Approval, Progress::Warning)
    );

    let question = visual_state(resolve(
        Phase::WaitingUser,
        Attention::Question,
        Health::Normal,
    ));
    assert_eq!(
        (question.tab_color(), question.progress()),
        (TabColor::Question, Progress::Warning)
    );

    let result = visual_state(resolve(
        Phase::WaitingUser,
        Attention::ResultReady,
        Health::Normal,
    ));
    assert_eq!(
        (result.tab_color(), result.progress()),
        (TabColor::ResultReady, Progress::Clear)
    );
}

#[test]
fn working_and_ready_policy_fallbacks_are_deterministic() {
    let working = visual_state(resolve(Phase::Working, Attention::None, Health::Normal));
    assert_eq!(
        (working.tab_color(), working.progress()),
        (TabColor::Working, Progress::Indeterminate)
    );

    let ready = visual_state(resolve(Phase::Ready, Attention::None, Health::Normal));
    assert_eq!(
        (ready.tab_color(), ready.progress()),
        (TabColor::Default, Progress::Clear)
    );
}

#[test]
fn policy_consumes_normalized_session_snapshot_without_provider_raw_types() {
    let session =
        AgentSessionKey::new(AgentProvider::new("future-agent").unwrap(), "session-1").unwrap();
    let evidence = AgentEvidence::new(
        session,
        EvidenceSource::new("normalized", "test").unwrap(),
        EvidenceAuthority::Lifecycle,
        EvidenceConfidence::Standard,
        std::time::UNIX_EPOCH,
        EvidenceTieBreak::new("1").unwrap(),
        StatePatch {
            phase: FieldUpdate::set(Phase::WaitingUser),
            attention: FieldUpdate::set(Attention::Approval),
            health: FieldUpdate::set(Health::Normal),
        },
    );
    let snapshot = SessionReconciler::default().apply(&evidence);
    let state = visual_state(PresentationPolicy::resolve(
        SemanticPresentationInput::from_snapshot(&snapshot, "JPC snapshot"),
    ));

    assert_eq!(state.repository_alias().as_str(), "JPC snapshot");
    assert_eq!(state.title_status(), TitleStatus::Approval);
    assert_eq!(
        (state.tab_color(), state.progress()),
        (TabColor::Approval, Progress::Warning)
    );
}

#[test]
fn ended_produces_cleanup_reset_instead_of_ready() {
    let action = resolve(Phase::Ended, Attention::None, Health::Normal);

    match action {
        PresentationAction::Reset(reset) => {
            assert_eq!(
                reset.reset_semantics(),
                ResetSemantics::ClearProgressAndFrameColor
            );
            assert_eq!(reset.repository_alias().as_str(), "JPC semantic fixture");
            assert_eq!(reset.title_status(), TitleStatus::Ready);
            assert_eq!(reset.tab_color(), TabColor::Default);
            assert_eq!(reset.progress(), Progress::Clear);
        }
        PresentationAction::Apply(_) => panic!("ended state must reset presentation"),
    }
}

#[test]
fn representative_orthogonal_state_chains_match_policy_contract() {
    let warning_working = visual_state(resolve(Phase::Working, Attention::None, Health::Warning));
    assert_eq!(
        (warning_working.tab_color(), warning_working.progress()),
        (TabColor::Warning, Progress::Indeterminate)
    );

    let approval_waiting = visual_state(resolve(
        Phase::WaitingUser,
        Attention::Approval,
        Health::Normal,
    ));
    assert_eq!(
        (approval_waiting.tab_color(), approval_waiting.progress()),
        (TabColor::Approval, Progress::Warning)
    );

    let result_waiting = visual_state(resolve(
        Phase::WaitingUser,
        Attention::ResultReady,
        Health::Normal,
    ));
    assert_eq!(
        (result_waiting.tab_color(), result_waiting.progress()),
        (TabColor::ResultReady, Progress::Clear)
    );
}

#[test]
fn renderer_is_repeatable_and_reset_is_idempotent() {
    let renderer = WindowsTerminalRenderer::new(WindowsTerminalCapabilities::new(true));
    let working = resolve(Phase::Working, Attention::None, Health::Normal);
    let reset = resolve(Phase::Ended, Attention::None, Health::Normal);

    assert_eq!(renderer.render(&working), renderer.render(&working));
    assert_eq!(renderer.render(&reset), renderer.render(&reset));
    assert!(renderer.render(&reset).ends_with(b"\x1b]104;264\x1b\\"));
}

#[test]
fn absent_frame_color_capability_preserves_title_and_progress_only() {
    let action = resolve(Phase::Working, Attention::None, Health::Normal);
    let enabled =
        WindowsTerminalRenderer::new(WindowsTerminalCapabilities::new(true)).render(&action);
    let disabled =
        WindowsTerminalRenderer::new(WindowsTerminalCapabilities::new(false)).render(&action);

    assert_eq!(
        disabled,
        "\x1b]0;○ JPC semantic fixture\x1b\\\x1b]9;4;3;0\x1b\\".as_bytes()
    );
    assert!(enabled.starts_with(&disabled));
    assert!(
        !disabled
            .windows(b"264".len())
            .any(|window| window == b"264")
    );
}

#[test]
fn fixture_covers_every_named_state_and_replays_without_external_input() {
    let names = presentation_fixture()
        .iter()
        .map(tabbeacon::presentation::PresentationFixtureCase::name)
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        vec![
            "ready",
            "working",
            "result-ready",
            "approval",
            "question",
            "warning-working",
            "warning-idle",
            "interrupted",
            "failed",
            "reset",
        ]
    );

    let renderer = WindowsTerminalRenderer::new(WindowsTerminalCapabilities::new(true));
    let first = replay_presentation_fixture(&renderer);
    let second = replay_presentation_fixture(&renderer);
    assert_eq!(first, second);
    assert_eq!(
        first
            .iter()
            .filter(|fixture| matches!(fixture.action(), PresentationAction::Reset(_)))
            .count(),
        1
    );
    assert!(first.iter().all(|fixture| !fixture.bytes().is_empty()));
}

#[test]
fn muted_dark_default_uses_the_braille_title_spinner_with_semantic_color() {
    let settings = PresentationSettings::default();
    let terminal_renderer =
        WindowsTerminalRenderer::with_settings(WindowsTerminalCapabilities::new(true), settings);
    let working = resolve(Phase::Working, Attention::None, Health::Normal);
    let terminal_output =
        String::from_utf8(terminal_renderer.render(&working)).expect("terminal bytes are UTF-8");

    assert!(terminal_output.contains("]0;⠋ JPC semantic fixture"));
    assert!(terminal_output.contains("rgb:1b/4e/3a"));
    assert!(terminal_output.contains("]9;4;0;0"));
    assert!(
        !terminal_output.contains("]9;4;3;0"),
        "the default Human preset must not emit the Terminal Ring while its title spinner is active"
    );
    assert_eq!(settings.theme(), PresentationTheme::MutedDark);
}

#[test]
fn title_spinner_frame_zero_is_a_safe_deterministic_fail_open_fallback() {
    let settings = PresentationSettings::default()
        .with_activity(ActivityMode::TitleSpinner)
        .with_spinner(SpinnerPreset::Braille);
    let terminal_renderer =
        WindowsTerminalRenderer::with_settings(WindowsTerminalCapabilities::new(false), settings);
    let working = resolve(Phase::Working, Attention::None, Health::Normal);
    let terminal_output =
        String::from_utf8(terminal_renderer.render(&working)).expect("terminal bytes are UTF-8");

    assert!(terminal_output.contains("]0;⠋ JPC semantic fixture"));
    assert!(!terminal_output.contains("⠙"));
    assert!(terminal_output.contains("]9;4;0;0"));
}

#[test]
fn native_and_off_channels_clear_owned_terminal_state_without_emitting_title() {
    for title_mode in [TitleMode::Native, TitleMode::Off] {
        let settings = PresentationSettings::new(
            title_mode,
            TabColorMode::Off,
            ActivityMode::Off,
            SpinnerPreset::Codex,
            PresentationTheme::MutedDark,
        );
        let terminal_renderer = WindowsTerminalRenderer::with_settings(
            WindowsTerminalCapabilities::new(true),
            settings,
        );
        let working = resolve(Phase::Working, Attention::None, Health::Normal);
        let terminal_bytes = terminal_renderer.render(&working);
        let terminal_output = String::from_utf8(terminal_bytes).expect("terminal bytes are UTF-8");

        assert!(!terminal_output.contains("]0;"));
        assert!(terminal_output.contains("]9;4;0;0"));
        assert!(terminal_output.contains("]104;264"));
        assert!(!terminal_output.contains("rgb:"));
    }
}

#[test]
fn both_mode_combines_title_frame_zero_with_the_native_progress_ring() {
    let settings = PresentationSettings::default().with_activity(ActivityMode::Both);
    let terminal_renderer =
        WindowsTerminalRenderer::with_settings(WindowsTerminalCapabilities::new(false), settings);
    let working = resolve(Phase::Working, Attention::None, Health::Normal);
    let terminal_output =
        String::from_utf8(terminal_renderer.render(&working)).expect("terminal bytes are UTF-8");

    assert!(terminal_output.contains("]0;⠋ JPC semantic fixture"));
    assert!(terminal_output.contains("]9;4;3;0"));
}

#[test]
fn status_first_title_grammar_is_compact_for_every_required_semantic() {
    let settings = PresentationSettings::default()
        .with_activity(ActivityMode::TitleSpinner)
        .with_spinner(SpinnerPreset::Braille);
    let renderer =
        WindowsTerminalRenderer::with_settings(WindowsTerminalCapabilities::new(false), settings);
    let cases = [
        (Phase::Ready, Attention::None, "○ OWH"),
        (Phase::Working, Attention::None, "⠋ OWH"),
        (Phase::WaitingUser, Attention::ResultReady, "✓ OWH"),
        (Phase::WaitingUser, Attention::Approval, "! OWH"),
        (Phase::WaitingUser, Attention::Question, "? OWH"),
    ];

    for (phase, attention, expected) in cases {
        let action = PresentationPolicy::resolve(input(phase, attention, Health::Normal, "OWH"));
        let state = match &action {
            PresentationAction::Apply(state) => state,
            PresentationAction::Reset(_) => panic!("semantic case unexpectedly reset"),
        };
        let title = renderer.title_for(state).expect("TabBeacon owns the title");
        assert_eq!(title.as_str(), expected);
        for prose in [
            "working",
            "result-ready",
            "approval",
            "question",
            "waiting",
            "ready",
            "reset",
        ] {
            assert!(!title.as_str().contains(prose));
        }
    }
}

#[test]
fn every_spinner_frame_changes_only_the_left_status_slot() {
    for preset in [
        SpinnerPreset::Codex,
        SpinnerPreset::Braille,
        SpinnerPreset::Quadrant,
        SpinnerPreset::Line,
        SpinnerPreset::Pulse,
    ] {
        let settings = PresentationSettings::default()
            .with_activity(ActivityMode::TitleSpinner)
            .with_spinner(preset);
        let renderer = WindowsTerminalRenderer::with_settings(
            WindowsTerminalCapabilities::new(false),
            settings,
        );
        let working = resolve(Phase::Working, Attention::None, Health::Normal);
        let state = match &working {
            PresentationAction::Apply(state) => state,
            PresentationAction::Reset(_) => panic!("working unexpectedly reset"),
        };

        for (index, frame) in preset.frames().iter().enumerate() {
            let title = renderer
                .title_for_spinner_frame(state, index)
                .expect("TabBeacon owns the title");
            assert_eq!(title.as_str(), format!("{frame} JPC semantic fixture"));
            assert_eq!(
                title.as_str().chars().skip(2).collect::<String>(),
                "JPC semantic fixture"
            );
        }
    }
}

#[test]
fn worker_title_frames_and_hook_owned_channels_are_encoded_independently() {
    let settings = PresentationSettings::default()
        .with_activity(ActivityMode::Both)
        .with_spinner(SpinnerPreset::Braille);
    let renderer =
        WindowsTerminalRenderer::with_settings(WindowsTerminalCapabilities::new(true), settings);
    let working = resolve(Phase::Working, Attention::None, Health::Normal);
    let state = match &working {
        PresentationAction::Apply(state) => state,
        PresentationAction::Reset(_) => panic!("working unexpectedly reset"),
    };

    let first = String::from_utf8(renderer.render_title_spinner_frame(state, 0))
        .expect("worker title frame is UTF-8");
    let second = String::from_utf8(renderer.render_title_spinner_frame(state, 1))
        .expect("worker title frame is UTF-8");
    assert_eq!(first, "\x1b]0;⠋ JPC semantic fixture\x1b\\");
    assert_eq!(second, "\x1b]0;⠙ JPC semantic fixture\x1b\\");
    assert!(!first.contains("]9;4;"));
    assert!(!first.contains("]4;264;"));

    let hook_channels =
        String::from_utf8(renderer.render_without_title(&working)).expect("Hook bytes are UTF-8");
    assert!(!hook_channels.contains("]0;"));
    assert!(hook_channels.contains("]9;4;3;0"));
    assert!(hook_channels.contains("]4;264;rgb:1b/4e/3a"));
}

#[test]
fn long_repository_alias_is_sanitized_and_bounded_after_status_layout() {
    let mut alias = "界".repeat(MAX_TITLE_SCALARS + 10);
    alias.push('\u{0007}');
    let action =
        PresentationPolicy::resolve(input(Phase::Ready, Attention::None, Health::Normal, &alias));
    let state = match &action {
        PresentationAction::Apply(state) => state,
        PresentationAction::Reset(_) => panic!("ready unexpectedly reset"),
    };
    let renderer = WindowsTerminalRenderer::new(WindowsTerminalCapabilities::new(false));
    let title = renderer.title_for(state).expect("TabBeacon owns the title");

    assert_eq!(title.as_str().chars().count(), MAX_TITLE_SCALARS);
    assert!(title.as_str().starts_with("○ "));
    assert!(title.as_str().ends_with('…'));
    assert!(!title.as_str().chars().any(char::is_control));
}

#[test]
fn title_and_activity_ownership_modes_preserve_their_existing_boundaries() {
    let working = resolve(Phase::Working, Attention::None, Health::Normal);
    let state = match &working {
        PresentationAction::Apply(state) => state,
        PresentationAction::Reset(_) => panic!("working unexpectedly reset"),
    };
    for (activity, expected_title) in [
        (ActivityMode::TitleSpinner, "⠋ JPC semantic fixture"),
        (ActivityMode::TitleIndicator, "• JPC semantic fixture"),
        (ActivityMode::Both, "⠋ JPC semantic fixture"),
        (ActivityMode::WindowsTerminalRing, "○ JPC semantic fixture"),
        (ActivityMode::Native, "○ JPC semantic fixture"),
        (ActivityMode::Off, "○ JPC semantic fixture"),
    ] {
        let renderer = WindowsTerminalRenderer::with_settings(
            WindowsTerminalCapabilities::new(false),
            PresentationSettings::default().with_activity(activity),
        );
        assert_eq!(
            renderer
                .title_for(state)
                .expect("TabBeacon title remains owned")
                .as_str(),
            expected_title
        );
    }

    for title_mode in [TitleMode::Native, TitleMode::Off] {
        let renderer = WindowsTerminalRenderer::with_settings(
            WindowsTerminalCapabilities::new(false),
            PresentationSettings::default().with_title(title_mode),
        );
        assert!(renderer.title_for(state).is_none());
    }
}

#[test]
fn reset_returns_to_the_neutral_status_first_title() {
    let action = resolve(Phase::Ended, Attention::None, Health::Normal);
    let state = match &action {
        PresentationAction::Reset(state) => state,
        PresentationAction::Apply(_) => panic!("ended state must reset"),
    };
    let renderer = WindowsTerminalRenderer::with_settings(
        WindowsTerminalCapabilities::new(true),
        PresentationSettings::default().with_activity(ActivityMode::TitleSpinner),
    );

    assert_eq!(
        renderer
            .title_for(state)
            .expect("TabBeacon owns reset title")
            .as_str(),
        "○ JPC semantic fixture"
    );
    let bytes = String::from_utf8(renderer.render(&action)).expect("terminal bytes are UTF-8");
    assert!(bytes.contains("]9;4;0;0"));
    assert!(bytes.contains("]104;264"));
}
