//! Staged Control Center frontend and bounded Ratatui renderer.

use std::{io, time::Duration};

use crossterm::{
    cursor::Show,
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::{
    core::{Attention, Health, Phase},
    management::{ActionSafety, ManagementHealth, ManagementOverview, ManagementSnapshot},
    presentation::{
        PresentationAction, PresentationPolicy, SemanticPresentationInput,
        WindowsTerminalCapabilities, WindowsTerminalRenderer,
    },
    settings::{ActivityMode, PresentationSettings, SpinnerPreset, TabColorMode, TitleMode},
};

/// One bounded daily-management screen.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Screen {
    Overview,
    Appearance,
    Integration,
    Diagnostics,
    Preview,
}

impl Screen {
    const ALL: [Self; 5] = [
        Self::Overview,
        Self::Appearance,
        Self::Integration,
        Self::Diagnostics,
        Self::Preview,
    ];

    const fn title(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Appearance => "Appearance",
            Self::Integration => "Codex Integration",
            Self::Diagnostics => "Diagnostics",
            Self::Preview => "Preview",
        }
    }
}

/// One human-labelled appearance setting that can be selected in the TUI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AppearanceField {
    Title,
    TabColor,
    Activity,
    Spinner,
    Theme,
}

impl AppearanceField {
    const ALL: [Self; 5] = [
        Self::Title,
        Self::TabColor,
        Self::Activity,
        Self::Spinner,
        Self::Theme,
    ];

    const fn label(self) -> &'static str {
        match self {
            Self::Title => "Title",
            Self::TabColor => "Tab color",
            Self::Activity => "Activity",
            Self::Spinner => "Spinner",
            Self::Theme => "Theme",
        }
    }
}

/// A frontend request that must be executed by an existing ownership-aware API.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlCenterCommand {
    /// No persistent operation was requested.
    None,
    /// End the interactive session without persisting a draft.
    Quit,
    /// Persist one staged settings draft through the caller-owned operation.
    Apply {
        /// Current typed settings expected by the frontend.
        before: PresentationSettings,
        /// Staged typed settings to apply.
        after: PresentationSettings,
    },
}

/// In-memory frontend state. No mutation authority is stored here.
#[derive(Clone, Debug)]
pub struct ControlCenterApp {
    screen: Screen,
    snapshot: ManagementSnapshot,
    overview: ManagementOverview,
    current: PresentationSettings,
    draft: PresentationSettings,
    dirty: bool,
    confirm_discard: bool,
    appearance_field: Option<AppearanceField>,
}

impl ControlCenterApp {
    /// Creates a staged management frontend from already-collected state.
    #[must_use]
    pub fn new(
        current: PresentationSettings,
        snapshot: ManagementSnapshot,
        overview: ManagementOverview,
    ) -> Self {
        Self {
            screen: Screen::Overview,
            snapshot,
            overview,
            current,
            draft: current,
            dirty: false,
            confirm_discard: false,
            appearance_field: None,
        }
    }

    /// Current active screen.
    #[must_use]
    pub const fn screen(&self) -> Screen {
        self.screen
    }

    /// Whether appearance changes are staged only.
    #[must_use]
    pub const fn dirty(&self) -> bool {
        self.dirty
    }

    /// Current persisted/effective presentation settings.
    #[must_use]
    pub const fn current(&self) -> PresentationSettings {
        self.current
    }

    /// In-memory draft used by appearance and preview.
    #[must_use]
    pub const fn draft(&self) -> PresentationSettings {
        self.draft
    }

    /// Whether quit requires an explicit discard response.
    #[must_use]
    pub const fn confirm_discard(&self) -> bool {
        self.confirm_discard
    }

    /// Applies one event to staged state and returns a caller-owned action request.
    pub fn handle_key(&mut self, key: KeyCode) -> ControlCenterCommand {
        if self.confirm_discard {
            return self.handle_discard_key(key);
        }
        match key {
            KeyCode::Up | KeyCode::Char('k') => self.step(-1),
            KeyCode::Down | KeyCode::Char('j') => self.step(1),
            KeyCode::Left => self.change_focused(-1),
            KeyCode::Right => self.change_focused(1),
            KeyCode::Enter if self.screen == Screen::Appearance => self.toggle_appearance_focus(),
            KeyCode::Char('r') => self.revert(),
            KeyCode::Char('a') => return self.request_apply(),
            KeyCode::Char('q') if self.dirty => self.confirm_discard = true,
            _ => {}
        }
        ControlCenterCommand::None
    }

    /// Applies a terminal key event, including the interrupt key path.
    pub fn handle_event(&mut self, key: KeyEvent) -> ControlCenterCommand {
        if key.kind != KeyEventKind::Press {
            return ControlCenterCommand::None;
        }
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            if self.dirty {
                self.confirm_discard = true;
                return ControlCenterCommand::None;
            }
            return ControlCenterCommand::Quit;
        }
        let command = self.handle_key(key.code);
        if key.code == KeyCode::Char('q') && !self.dirty && !self.confirm_discard {
            ControlCenterCommand::Quit
        } else {
            command
        }
    }

    /// Marks a successfully caller-owned persistence operation as accepted.
    pub fn apply_succeeded(&mut self) {
        self.current = self.draft;
        self.dirty = false;
    }

    fn handle_discard_key(&mut self, key: KeyCode) -> ControlCenterCommand {
        match key {
            KeyCode::Char('d') => {
                self.revert();
                self.confirm_discard = false;
            }
            KeyCode::Char('k' | 'q') | KeyCode::Esc => self.confirm_discard = false,
            _ => {}
        }
        ControlCenterCommand::None
    }

    fn request_apply(&self) -> ControlCenterCommand {
        if self.dirty {
            ControlCenterCommand::Apply {
                before: self.current,
                after: self.draft,
            }
        } else {
            ControlCenterCommand::None
        }
    }

    fn revert(&mut self) {
        self.draft = self.current;
        self.dirty = false;
        self.appearance_field = None;
    }

    fn toggle_appearance_focus(&mut self) {
        self.appearance_field = self
            .appearance_field
            .map_or(Some(AppearanceField::Title), |_| None);
    }

    fn step(&mut self, offset: isize) {
        if self.appearance_field.is_some() {
            self.step_appearance_field(offset);
            return;
        }
        let index = Screen::ALL
            .iter()
            .position(|screen| *screen == self.screen)
            .unwrap_or(0);
        let next = shifted_index(index, Screen::ALL.len(), offset);
        self.screen = Screen::ALL[next];
    }

    fn step_appearance_field(&mut self, offset: isize) {
        let index = AppearanceField::ALL
            .iter()
            .position(|field| Some(*field) == self.appearance_field)
            .unwrap_or(0);
        self.appearance_field =
            Some(AppearanceField::ALL[shifted_index(index, AppearanceField::ALL.len(), offset)]);
    }

    fn change_focused(&mut self, offset: isize) {
        let Some(field) = self.appearance_field else {
            return;
        };
        self.draft = match field {
            AppearanceField::Title => self
                .draft
                .with_title(cycle_title(self.draft.title(), offset)),
            AppearanceField::TabColor => self
                .draft
                .with_tab_color(cycle_tab_color(self.draft.tab_color(), offset)),
            AppearanceField::Activity => self
                .draft
                .with_activity(cycle_activity(self.draft.activity(), offset)),
            AppearanceField::Spinner => self
                .draft
                .with_spinner(cycle_spinner(self.draft.spinner(), offset)),
            AppearanceField::Theme => self
                .draft
                .with_theme(cycle_theme(self.draft.theme(), offset)),
        };
        self.dirty = self.draft != self.current;
    }
}

fn shifted_index(index: usize, length: usize, offset: isize) -> usize {
    if offset.is_negative() {
        (index + length - 1) % length
    } else {
        (index + 1) % length
    }
}

fn cycle_title(value: TitleMode, offset: isize) -> TitleMode {
    cycle(
        [TitleMode::TabBeacon, TitleMode::Native, TitleMode::Off],
        value,
        offset,
    )
}

fn cycle_tab_color(value: TabColorMode, offset: isize) -> TabColorMode {
    cycle(
        [
            TabColorMode::TabBeacon,
            TabColorMode::Native,
            TabColorMode::Off,
        ],
        value,
        offset,
    )
}

fn cycle_activity(value: ActivityMode, offset: isize) -> ActivityMode {
    cycle(
        [
            ActivityMode::TitleSpinner,
            ActivityMode::TitleIndicator,
            ActivityMode::WindowsTerminalRing,
            ActivityMode::Both,
            ActivityMode::Native,
            ActivityMode::Off,
        ],
        value,
        offset,
    )
}

fn cycle_spinner(value: SpinnerPreset, offset: isize) -> SpinnerPreset {
    cycle(
        [
            SpinnerPreset::Codex,
            SpinnerPreset::Braille,
            SpinnerPreset::Quadrant,
            SpinnerPreset::Line,
            SpinnerPreset::Pulse,
        ],
        value,
        offset,
    )
}

fn cycle_theme(
    value: crate::settings::PresentationTheme,
    offset: isize,
) -> crate::settings::PresentationTheme {
    cycle(
        [
            crate::settings::PresentationTheme::MutedDark,
            crate::settings::PresentationTheme::Classic,
        ],
        value,
        offset,
    )
}

fn cycle<T: Copy + Eq>(values: impl AsRef<[T]>, value: T, offset: isize) -> T {
    let values = values.as_ref();
    let index = values.iter().position(|item| *item == value).unwrap_or(0);
    values[shifted_index(index, values.len(), offset)]
}

/// Runs the TUI and delegates Apply to the caller's existing typed operation.
///
/// # Errors
///
/// Returns terminal I/O errors or an Apply error after the terminal has been restored.
pub fn run<F>(mut app: ControlCenterApp, mut apply: F) -> io::Result<()>
where
    F: FnMut(PresentationSettings, PresentationSettings) -> io::Result<()>,
{
    let mut session = TerminalSession::enter()?;
    loop {
        session.terminal.draw(|frame| render(frame, &app))?;
        if !event::poll(Duration::from_millis(250))? {
            continue;
        }
        if let Event::Key(key) = event::read()? {
            let was_confirming = app.confirm_discard();
            let command = app.handle_event(key);
            match command {
                ControlCenterCommand::Apply { before, after } => {
                    apply(before, after)?;
                    app.apply_succeeded();
                }
                ControlCenterCommand::Quit => break,
                ControlCenterCommand::None => {}
            }
            if was_confirming && key.code == KeyCode::Char('d') && !app.confirm_discard() {
                break;
            }
        }
    }
    session.restore()
}

/// Result of the feature-gated real-terminal lifecycle fixture.
#[cfg(feature = "terminal-smoke-fixture")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalSmokeReport {
    /// The fixture rendered Overview, Appearance, and Integration in order.
    pub screens_visited: usize,
    /// An appearance value changed in the in-memory draft.
    pub draft_changed: bool,
    /// Revert restored the draft to the original settings without Apply.
    pub draft_reverted: bool,
    /// The normal application event handler returned its clean quit command.
    pub clean_quit: bool,
}

/// Runs a deterministic Control Center path through the real Crossterm terminal.
///
/// This function exists only with the `terminal-smoke-fixture` Cargo feature. It
/// has no Apply callback and therefore cannot write settings. The standalone
/// fixture binary uses it in a disposable Windows Terminal tab to prove the
/// production alternate-screen, raw-mode, renderer, navigation, Revert, and
/// cleanup path without synthetic operating-system input.
///
/// # Errors
///
/// Returns terminal I/O errors or an invariant error if the scripted path stops
/// exercising the intended production state transitions.
#[cfg(feature = "terminal-smoke-fixture")]
pub fn run_terminal_smoke_fixture(mut app: ControlCenterApp) -> io::Result<TerminalSmokeReport> {
    fn invariant(condition: bool, message: &'static str) -> io::Result<()> {
        condition
            .then_some(())
            .ok_or_else(|| io::Error::other(message))
    }

    fn draw(session: &mut TerminalSession, app: &ControlCenterApp) -> io::Result<()> {
        session.terminal.draw(|frame| render(frame, app))?;
        std::thread::sleep(Duration::from_millis(50));
        Ok(())
    }

    let original = app.current();
    let mut session = TerminalSession::enter()?;
    let result = (|| {
        invariant(
            app.screen() == Screen::Overview,
            "fixture did not start on Overview",
        )?;
        draw(&mut session, &app)?;

        let _ = app.handle_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        invariant(
            app.screen() == Screen::Appearance,
            "fixture did not reach Appearance",
        )?;
        draw(&mut session, &app)?;

        let _ = app.handle_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        let _ = app.handle_event(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        let draft_changed = app.dirty() && app.draft() != original;
        invariant(draft_changed, "fixture did not create an in-memory draft")?;
        draw(&mut session, &app)?;

        let _ = app.handle_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));
        let draft_reverted = !app.dirty() && app.draft() == original && app.current() == original;
        invariant(draft_reverted, "fixture did not revert its in-memory draft")?;

        let _ = app.handle_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        invariant(
            app.screen() == Screen::Integration,
            "fixture did not reach Integration",
        )?;
        draw(&mut session, &app)?;

        let clean_quit = app.handle_event(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE))
            == ControlCenterCommand::Quit;
        invariant(
            clean_quit,
            "fixture did not use the production clean-quit path",
        )?;

        Ok(TerminalSmokeReport {
            screens_visited: 3,
            draft_changed,
            draft_reverted,
            clean_quit,
        })
    })();
    let restore = session.restore();
    match (result, restore) {
        (Err(error), _) | (Ok(_), Err(error)) => Err(error),
        (Ok(report), Ok(())) => Ok(report),
    }
}

/// Central owner of raw mode, alternate screen, drawing, and restoration.
struct TerminalSession {
    terminal: ratatui::Terminal<CrosstermBackend<io::Stdout>>,
    guard: TerminalGuard<CrosstermTerminalLifecycle>,
}

impl TerminalSession {
    fn enter() -> io::Result<Self> {
        let guard = TerminalGuard::enter(CrosstermTerminalLifecycle)?;
        let stdout = io::stdout();
        let terminal = ratatui::Terminal::new(CrosstermBackend::new(stdout))?;
        Ok(Self { terminal, guard })
    }

    fn restore(&mut self) -> io::Result<()> {
        self.guard.restore()
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

/// The terminal-state operations that must have one owner and one cleanup path.
trait TerminalLifecycle {
    fn enable_raw_mode(&mut self) -> io::Result<()>;
    fn disable_raw_mode(&mut self) -> io::Result<()>;
    fn enter_alternate_screen(&mut self) -> io::Result<()>;
    fn leave_alternate_screen(&mut self) -> io::Result<()>;
    fn show_cursor(&mut self) -> io::Result<()>;
}

/// Concrete Windows-terminal lifecycle used only by the Control Center session.
struct CrosstermTerminalLifecycle;

impl TerminalLifecycle for CrosstermTerminalLifecycle {
    fn enable_raw_mode(&mut self) -> io::Result<()> {
        enable_raw_mode()
    }

    fn disable_raw_mode(&mut self) -> io::Result<()> {
        disable_raw_mode()
    }

    fn enter_alternate_screen(&mut self) -> io::Result<()> {
        execute!(io::stdout(), EnterAlternateScreen)
    }

    fn leave_alternate_screen(&mut self) -> io::Result<()> {
        execute!(io::stdout(), LeaveAlternateScreen)
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        execute!(io::stdout(), Show)
    }
}

/// RAII terminal-state guard that restores every entered state on all exit paths.
struct TerminalGuard<L: TerminalLifecycle> {
    lifecycle: L,
    raw_mode_enabled: bool,
    alternate_screen_entered: bool,
    restored: bool,
}

impl<L: TerminalLifecycle> TerminalGuard<L> {
    fn enter(lifecycle: L) -> io::Result<Self> {
        let mut guard = Self {
            lifecycle,
            raw_mode_enabled: true,
            alternate_screen_entered: false,
            restored: false,
        };
        guard.lifecycle.enable_raw_mode()?;
        // Mark before the command so a partially completed terminal write also
        // receives a best-effort LeaveAlternateScreen during unwinding.
        guard.alternate_screen_entered = true;
        guard.lifecycle.enter_alternate_screen()?;
        Ok(guard)
    }

    fn restore(&mut self) -> io::Result<()> {
        if self.restored {
            return Ok(());
        }
        let mut first_error: Option<io::Error> = None;
        if self.alternate_screen_entered {
            match self.lifecycle.leave_alternate_screen() {
                Ok(()) => self.alternate_screen_entered = false,
                Err(error) => first_error = Some(error),
            }
        }
        if self.raw_mode_enabled {
            match self.lifecycle.disable_raw_mode() {
                Ok(()) => self.raw_mode_enabled = false,
                Err(error) if first_error.is_none() => first_error = Some(error),
                Err(_) => {}
            }
        }
        match self.lifecycle.show_cursor() {
            Err(error) if first_error.is_none() => first_error = Some(error),
            Ok(()) | Err(_) => {}
        }
        self.restored =
            !self.raw_mode_enabled && !self.alternate_screen_entered && first_error.is_none();
        first_error.map_or(Ok(()), Err)
    }
}

impl<L: TerminalLifecycle> Drop for TerminalGuard<L> {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

const MIN_TERMINAL_WIDTH: u16 = 24;
const MIN_TERMINAL_HEIGHT: u16 = 10;

/// Renders all Control Center screens into the active Ratatui frame.
pub fn render(frame: &mut Frame, app: &ControlCenterApp) {
    let area = frame.area();
    if area.width < MIN_TERMINAL_WIDTH || area.height < MIN_TERMINAL_HEIGHT {
        frame.render_widget(
            Paragraph::new(format!(
                "Terminal too small\nMinimum size: {MIN_TERMINAL_WIDTH}x{MIN_TERMINAL_HEIGHT}\nResize, then reopen TabBeacon Control Center."
            ))
            .block(Block::default().borders(Borders::ALL).title("TabBeacon")),
            area,
        );
        return;
    }
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(3),
            Constraint::Length(2),
        ])
        .split(frame.area());
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "TabBeacon Control Center",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(" — {}", health_label(app.snapshot.health))),
        ])),
        areas[0],
    );
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(21), Constraint::Min(20)])
        .split(areas[1]);
    let nav = Screen::ALL
        .iter()
        .map(|screen| {
            ListItem::new(if *screen == app.screen {
                format!("> {}", screen.title())
            } else {
                format!("  {}", screen.title())
            })
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        List::new(nav).block(Block::default().borders(Borders::ALL).title("Sections")),
        body[0],
    );
    frame.render_widget(
        content(app).block(
            Block::default()
                .borders(Borders::ALL)
                .title(app.screen.title()),
        ),
        body[1],
    );
    let footer = if app.confirm_discard {
        "Unsaved changes — [k] Keep editing  [d] Discard changes".to_owned()
    } else if app.appearance_field.is_some() {
        "↑↓ select setting  ←→ change draft  Enter done  a Apply  r Revert".to_owned()
    } else {
        format!(
            "↑↓ navigate  Enter edit Appearance  a Apply  r Revert  q Quit{}",
            if app.dirty {
                "  • unsaved changes"
            } else {
                ""
            }
        )
    };
    frame.render_widget(Paragraph::new(footer), areas[2]);
}

fn content(app: &ControlCenterApp) -> Paragraph<'static> {
    Paragraph::new(match app.screen {
        Screen::Overview => overview_lines(app),
        Screen::Appearance => appearance_lines(app),
        Screen::Integration => integration_lines(app),
        Screen::Diagnostics => diagnostics_lines(app),
        Screen::Preview => preview_lines(app),
    })
}

fn overview_lines(app: &ControlCenterApp) -> String {
    format!(
        "Overall health: {}\n\nTabBeacon  {}\nCodex      {} · {}\nHooks      {} · trust {}\nTitle      {}\n\nPresentation\n  Title      {}\n  Tab color  {}\n  Activity   {}\n  Spinner    {}\n  Theme      {}\n\nWorkers    {} · active {} · stale {}",
        health_label(app.snapshot.health),
        app.overview.tabbeacon_version,
        app.overview.codex_version,
        app.overview.codex_profile,
        app.overview.hooks,
        app.overview.hook_trust,
        app.overview.title_ownership,
        human_title(app.current.title()),
        human_tab_color(app.current.tab_color()),
        human_activity(app.current.activity()),
        human_spinner(app.current.spinner()),
        human_theme(app.current.theme()),
        app.overview.worker_health,
        app.overview.active_workers,
        app.overview.stale_workers,
    )
}

fn appearance_lines(app: &ControlCenterApp) -> String {
    let field_line = |field: AppearanceField, value: String| {
        let marker = if app.appearance_field == Some(field) {
            ">"
        } else {
            " "
        };
        format!("{marker} {:10} {value}", field.label())
    };
    format!(
        "Draft appearance — staged only\n\n{}\n{}\n{}\n{}\n{}\n\n{}",
        field_line(
            AppearanceField::Title,
            human_title(app.draft.title()).to_owned()
        ),
        field_line(
            AppearanceField::TabColor,
            human_tab_color(app.draft.tab_color()).to_owned()
        ),
        field_line(
            AppearanceField::Activity,
            human_activity(app.draft.activity()).to_owned()
        ),
        field_line(
            AppearanceField::Spinner,
            human_spinner(app.draft.spinner()).to_owned()
        ),
        field_line(
            AppearanceField::Theme,
            human_theme(app.draft.theme()).to_owned()
        ),
        if app.appearance_field.is_some() {
            "Use ← → to change this in-memory draft."
        } else {
            "Press Enter to select a setting; no enum typing is required."
        }
    )
}

fn integration_lines(app: &ControlCenterApp) -> String {
    let actions = if app.snapshot.recommended_actions.is_empty() {
        "No action required.".to_owned()
    } else {
        app.snapshot
            .recommended_actions
            .iter()
            .map(|action| {
                format!(
                    "• {} [{}]\n  {}",
                    action.title,
                    safety_label(action.safety),
                    action.instruction
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        "Codex       {} · profile {}\nHooks       {}\nCurrentness {}\nTrust       {} (manual only)\nTitle       {}\n\nRecommended actions\n{}",
        app.overview.codex_version,
        app.overview.codex_profile,
        app.overview.hooks,
        health_label(app.snapshot.health),
        app.overview.hook_trust,
        app.overview.title_ownership,
        actions
    )
}

fn diagnostics_lines(app: &ControlCenterApp) -> String {
    if app.snapshot.issues.is_empty() {
        return "✓ Healthy\n\nNo action required.".to_owned();
    }
    app.snapshot
        .issues
        .iter()
        .map(|issue| {
            let next = issue.remediation.as_ref().map_or_else(
                || "No automated action is available.".to_owned(),
                |action| {
                    format!(
                        "Next: {} [{}]",
                        action.instruction,
                        safety_label(action.safety)
                    )
                },
            );
            format!(
                "{} {}\n{}\n{}",
                severity_label(issue.severity),
                issue.title,
                issue.explanation,
                next
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn preview_lines(app: &ControlCenterApp) -> String {
    let renderer =
        WindowsTerminalRenderer::with_settings(WindowsTerminalCapabilities::new(true), app.draft);
    [
        ("Ready", Phase::Ready, Attention::None),
        ("Working", Phase::Working, Attention::None),
        ("Result ready", Phase::WaitingUser, Attention::ResultReady),
        ("Approval", Phase::WaitingUser, Attention::Approval),
    ]
    .into_iter()
    .map(|(label, phase, attention)| {
        let action = PresentationPolicy::resolve(SemanticPresentationInput::new(
            phase,
            attention,
            Health::Normal,
            "TabBeacon preview",
        ));
        let (title, progress) = match action {
            PresentationAction::Apply(state) | PresentationAction::Reset(state) => (
                renderer.title_for(&state).map_or_else(
                    || "Native title".to_owned(),
                    |title| title.as_str().to_owned(),
                ),
                format!("{:?}", state.progress()),
            ),
        };
        format!("{label:12} {title} · {progress}")
    })
    .collect::<Vec<_>>()
    .join("\n")
}

fn health_label(health: ManagementHealth) -> &'static str {
    match health {
        ManagementHealth::Healthy => "Healthy",
        ManagementHealth::Warning => "Attention",
        ManagementHealth::Error => "Failure",
    }
}

fn severity_label(severity: crate::management::HealthSeverity) -> &'static str {
    match severity {
        crate::management::HealthSeverity::Warning => "! Attention:",
        crate::management::HealthSeverity::Error => "× Failure:",
    }
}

fn safety_label(safety: ActionSafety) -> &'static str {
    match safety {
        ActionSafety::ReadOnly => "Read only",
        ActionSafety::ManualAction => "Manual action",
        ActionSafety::PreviewableSafeRepair => "Previewable repair",
        ActionSafety::OwnerExplicitRequired => "Owner apply required",
        ActionSafety::UnsupportedAutomation => "Not automated",
    }
}

fn human_title(value: TitleMode) -> &'static str {
    match value {
        TitleMode::TabBeacon => "TabBeacon",
        TitleMode::Native => "Native",
        TitleMode::Off => "Off",
    }
}

fn human_tab_color(value: TabColorMode) -> &'static str {
    match value {
        TabColorMode::TabBeacon => "TabBeacon colors",
        TabColorMode::Native => "Native colors",
        TabColorMode::Off => "Off",
    }
}

fn human_activity(value: ActivityMode) -> &'static str {
    match value {
        ActivityMode::TitleSpinner => "Title spinner",
        ActivityMode::TitleIndicator => "Title indicator",
        ActivityMode::WindowsTerminalRing => "Windows Terminal ring",
        ActivityMode::Both => "Title spinner + ring",
        ActivityMode::Native => "Native",
        ActivityMode::Off => "Off",
    }
}

fn human_spinner(value: SpinnerPreset) -> &'static str {
    match value {
        SpinnerPreset::Codex => "Codex",
        SpinnerPreset::Braille => "Braille",
        SpinnerPreset::Quadrant => "Quadrant",
        SpinnerPreset::Line => "Line",
        SpinnerPreset::Pulse => "Pulse",
    }
}

fn human_theme(value: crate::settings::PresentationTheme) -> &'static str {
    match value {
        crate::settings::PresentationTheme::MutedDark => "Muted Dark",
        crate::settings::PresentationTheme::Classic => "Classic",
    }
}

#[cfg(test)]
mod tests {
    use std::{
        cell::RefCell,
        panic::{AssertUnwindSafe, catch_unwind},
        rc::Rc,
    };

    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    #[derive(Clone)]
    struct RecordingLifecycle {
        calls: Rc<RefCell<Vec<&'static str>>>,
        fail_once: Rc<RefCell<Option<&'static str>>>,
    }

    impl RecordingLifecycle {
        fn new(fail_once: Option<&'static str>) -> Self {
            Self {
                calls: Rc::new(RefCell::new(Vec::new())),
                fail_once: Rc::new(RefCell::new(fail_once)),
            }
        }

        fn record(&mut self, operation: &'static str) -> io::Result<()> {
            self.calls.borrow_mut().push(operation);
            let mut failure = self.fail_once.borrow_mut();
            if *failure == Some(operation) {
                *failure = None;
                return Err(io::Error::other(format!("{operation} failed")));
            }
            Ok(())
        }
    }

    impl TerminalLifecycle for RecordingLifecycle {
        fn enable_raw_mode(&mut self) -> io::Result<()> {
            self.record("enable_raw")
        }

        fn disable_raw_mode(&mut self) -> io::Result<()> {
            self.record("disable_raw")
        }

        fn enter_alternate_screen(&mut self) -> io::Result<()> {
            self.record("enter_alternate")
        }

        fn leave_alternate_screen(&mut self) -> io::Result<()> {
            self.record("leave_alternate")
        }

        fn show_cursor(&mut self) -> io::Result<()> {
            self.record("show_cursor")
        }
    }

    fn app() -> ControlCenterApp {
        ControlCenterApp::new(
            PresentationSettings::default(),
            ManagementSnapshot {
                health: ManagementHealth::Healthy,
                issues: Vec::new(),
                recommended_actions: Vec::new(),
                change_plans: Vec::new(),
            },
            ManagementOverview::default(),
        )
    }

    #[test]
    fn buffer_renders_every_screen_at_normal_and_narrow_width() {
        let mut app = app();
        for _ in 0..Screen::ALL.len() {
            for width in [80, 24] {
                let mut terminal = Terminal::new(TestBackend::new(width, 12)).unwrap();
                terminal.draw(|frame| render(frame, &app)).unwrap();
                assert!(format!("{:?}", terminal.backend().buffer()).contains(app.screen.title()));
            }
            app.handle_key(KeyCode::Down);
        }
    }

    #[test]
    fn buffer_renders_minimum_size_and_explains_below_minimum_size() {
        let app = app();
        let mut minimum = Terminal::new(TestBackend::new(MIN_TERMINAL_WIDTH, MIN_TERMINAL_HEIGHT))
            .expect("minimum test terminal starts");
        minimum
            .draw(|frame| render(frame, &app))
            .expect("minimum terminal renders");
        assert!(format!("{:?}", minimum.backend().buffer()).contains("Overview"));

        let mut below_minimum = Terminal::new(TestBackend::new(
            MIN_TERMINAL_WIDTH - 1,
            MIN_TERMINAL_HEIGHT - 1,
        ))
        .expect("below-minimum test terminal starts");
        below_minimum
            .draw(|frame| render(frame, &app))
            .expect("below-minimum terminal renders");
        let rendered = format!("{:?}", below_minimum.backend().buffer());
        assert!(rendered.contains("Terminal too small"));
        assert!(rendered.contains("Minimum size"));
    }

    #[test]
    fn appearance_edits_are_staged_and_all_values_are_keyboard_selectable() {
        let mut app = app();
        app.screen = Screen::Appearance;
        let before = app.current();
        app.handle_key(KeyCode::Enter);
        for _ in 0..AppearanceField::ALL.len() {
            app.handle_key(KeyCode::Right);
            app.handle_key(KeyCode::Down);
        }
        assert!(app.dirty());
        assert_eq!(app.current(), before);
        app.handle_key(KeyCode::Char('r'));
        assert!(!app.dirty());
        assert_eq!(app.current(), app.draft());
    }

    #[test]
    fn apply_is_a_request_until_the_typed_owner_confirms_success() {
        let mut app = app();
        app.screen = Screen::Appearance;
        app.handle_key(KeyCode::Enter);
        app.handle_key(KeyCode::Right);
        let before = app.current();
        let after = app.draft();
        assert_eq!(
            app.handle_key(KeyCode::Char('a')),
            ControlCenterCommand::Apply { before, after }
        );
        assert!(app.dirty());
        assert_eq!(app.current(), before);
        app.apply_succeeded();
        assert!(!app.dirty());
        assert_eq!(app.current(), after);
    }

    #[test]
    fn dirty_quit_requires_explicit_discard_and_preserves_draft_when_cancelled() {
        let mut app = app();
        app.screen = Screen::Appearance;
        app.handle_key(KeyCode::Enter);
        app.handle_key(KeyCode::Right);
        app.handle_key(KeyCode::Char('q'));
        assert!(app.confirm_discard());
        app.handle_key(KeyCode::Char('k'));
        assert!(app.dirty());
        app.handle_key(KeyCode::Char('q'));
        app.handle_key(KeyCode::Char('d'));
        assert!(!app.dirty());
    }

    #[test]
    fn ctrl_c_quits_cleanly_or_requests_an_explicit_dirty_discard() {
        let mut app = app();
        assert_eq!(
            app.handle_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            ControlCenterCommand::Quit
        );

        app.screen = Screen::Appearance;
        app.handle_key(KeyCode::Enter);
        app.handle_key(KeyCode::Right);
        assert_eq!(
            app.handle_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            ControlCenterCommand::None
        );
        assert!(app.confirm_discard());
        assert!(app.dirty());
    }

    #[test]
    fn navigation_and_value_changes_are_edge_triggered() {
        let mut app = app();
        assert_eq!(
            app.handle_event(KeyEvent::new_with_kind(
                KeyCode::Down,
                KeyModifiers::NONE,
                KeyEventKind::Press,
            )),
            ControlCenterCommand::None
        );
        assert_eq!(app.screen(), Screen::Appearance);

        for kind in [KeyEventKind::Repeat, KeyEventKind::Release] {
            let _ = app.handle_event(KeyEvent::new_with_kind(
                KeyCode::Down,
                KeyModifiers::NONE,
                kind,
            ));
        }
        assert_eq!(app.screen(), Screen::Appearance);

        let _ = app.handle_event(KeyEvent::new_with_kind(
            KeyCode::Enter,
            KeyModifiers::NONE,
            KeyEventKind::Press,
        ));
        let before = app.draft();
        let _ = app.handle_event(KeyEvent::new_with_kind(
            KeyCode::Right,
            KeyModifiers::NONE,
            KeyEventKind::Press,
        ));
        let changed = app.draft();
        assert_ne!(changed, before);

        for kind in [KeyEventKind::Repeat, KeyEventKind::Release] {
            let _ = app.handle_event(KeyEvent::new_with_kind(
                KeyCode::Right,
                KeyModifiers::NONE,
                kind,
            ));
        }
        assert_eq!(app.draft(), changed);
    }

    #[test]
    fn terminal_guard_restores_every_terminal_state_on_normal_drop() {
        let lifecycle = RecordingLifecycle::new(None);
        let calls = Rc::clone(&lifecycle.calls);
        {
            let _guard = TerminalGuard::enter(lifecycle).expect("terminal enters");
        }
        assert_eq!(
            calls.borrow().as_slice(),
            [
                "enable_raw",
                "enter_alternate",
                "leave_alternate",
                "disable_raw",
                "show_cursor",
            ]
        );
    }

    #[test]
    fn terminal_guard_restores_partial_setup_and_unwind_paths() {
        let setup_lifecycle = RecordingLifecycle::new(Some("enter_alternate"));
        let setup_calls = Rc::clone(&setup_lifecycle.calls);
        assert!(TerminalGuard::enter(setup_lifecycle).is_err());
        assert_eq!(
            setup_calls.borrow().as_slice(),
            [
                "enable_raw",
                "enter_alternate",
                "leave_alternate",
                "disable_raw",
                "show_cursor",
            ]
        );

        let unwind_lifecycle = RecordingLifecycle::new(None);
        let unwind_calls = Rc::clone(&unwind_lifecycle.calls);
        let unwind = catch_unwind(AssertUnwindSafe(|| {
            let _guard = TerminalGuard::enter(unwind_lifecycle).expect("terminal enters");
            panic!("controlled event-loop failure");
        }));
        assert!(unwind.is_err());
        assert_eq!(
            unwind_calls.borrow().as_slice(),
            [
                "enable_raw",
                "enter_alternate",
                "leave_alternate",
                "disable_raw",
                "show_cursor",
            ]
        );
    }

    #[test]
    fn terminal_guard_attempts_all_cleanup_steps_after_a_cleanup_error() {
        let lifecycle = RecordingLifecycle::new(Some("leave_alternate"));
        let calls = Rc::clone(&lifecycle.calls);
        let mut guard = TerminalGuard::enter(lifecycle).expect("terminal enters");
        assert!(guard.restore().is_err());
        assert_eq!(
            &calls.borrow()[..5],
            [
                "enable_raw",
                "enter_alternate",
                "leave_alternate",
                "disable_raw",
                "show_cursor",
            ]
        );
    }

    #[test]
    fn monochrome_text_keeps_health_and_manual_action_meaning() {
        let action = crate::management::RecommendedAction {
            id: "review-hooks".to_owned(),
            title: "Review hooks".to_owned(),
            instruction: "Launch codex and open /hooks.".to_owned(),
            safety: ActionSafety::ManualAction,
        };
        let mut app = ControlCenterApp::new(
            PresentationSettings::default(),
            ManagementSnapshot {
                health: ManagementHealth::Warning,
                issues: vec![crate::management::HealthIssue {
                    id: "hook-review".to_owned(),
                    severity: crate::management::HealthSeverity::Warning,
                    title: "Hook review required".to_owned(),
                    explanation: "Trust remains manual.".to_owned(),
                    remediation: Some(action),
                }],
                recommended_actions: Vec::new(),
                change_plans: Vec::new(),
            },
            ManagementOverview::default(),
        );
        app.screen = Screen::Diagnostics;
        let mut terminal = Terminal::new(TestBackend::new(80, 12)).expect("test terminal starts");
        terminal.draw(|frame| render(frame, &app)).expect("renders");
        let rendered = format!("{:?}", terminal.backend().buffer());
        assert!(rendered.contains("Attention"));
        assert!(rendered.contains("Manual action"));
        assert!(rendered.contains("Launch codex"));
    }

    #[test]
    fn preview_uses_the_production_presentation_policy_without_terminal_output() {
        let mut app = app();
        app.screen = Screen::Preview;
        let mut terminal = Terminal::new(TestBackend::new(80, 12)).unwrap();
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let rendered = format!("{:?}", terminal.backend().buffer());
        assert!(rendered.contains("Working"));
        assert!(rendered.contains("Result ready"));
        assert!(rendered.contains("Approval"));
    }
}
