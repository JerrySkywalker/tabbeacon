//! Bounded, in-memory Control Center state and Ratatui buffer renderer.

use std::io;

use crossterm::{
    event::{self, Event, KeyCode},
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
    management::{ManagementHealth, ManagementSnapshot},
    settings::{PresentationSettings, PresentationTheme},
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

/// In-memory frontend state. No mutation authority is stored here.
#[derive(Clone, Debug)]
pub struct ControlCenterApp {
    screen: Screen,
    snapshot: ManagementSnapshot,
    current: PresentationSettings,
    draft: PresentationSettings,
    dirty: bool,
    confirm_discard: bool,
}

impl ControlCenterApp {
    /// Creates a staged management frontend from already-collected state.
    #[must_use]
    pub fn new(current: PresentationSettings, snapshot: ManagementSnapshot) -> Self {
        Self {
            screen: Screen::Overview,
            snapshot,
            current,
            draft: current,
            dirty: false,
            confirm_discard: false,
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

    /// Applies a keyboard command without performing external writes.
    pub fn handle_key(&mut self, key: char) {
        if self.confirm_discard {
            match key {
                'd' => {
                    self.draft = self.current;
                    self.dirty = false;
                    self.confirm_discard = false;
                }
                'k' | 'q' => self.confirm_discard = false,
                _ => {}
            }
            return;
        }
        match key {
            'j' => self.step(1),
            'k' => self.step(-1),
            'r' => {
                self.draft = self.current;
                self.dirty = false;
            }
            'a' => {
                self.current = self.draft;
                self.dirty = false;
            }
            'q' if self.dirty => self.confirm_discard = true,
            _ => {}
        }
    }

    /// Changes the draft theme for immediate in-memory preview; no store is touched.
    pub fn toggle_theme(&mut self) {
        self.draft = self
            .draft
            .with_theme(if self.draft.theme() == PresentationTheme::MutedDark {
                PresentationTheme::Classic
            } else {
                PresentationTheme::MutedDark
            });
        self.dirty = self.draft != self.current;
    }

    fn step(&mut self, offset: isize) {
        let index = Screen::ALL
            .iter()
            .position(|screen| *screen == self.screen)
            .unwrap_or(0);
        let next = if offset.is_negative() {
            (index + Screen::ALL.len() - 1) % Screen::ALL.len()
        } else {
            (index + 1) % Screen::ALL.len()
        };
        self.screen = Screen::ALL[next];
    }
}

/// Owns alternate-screen, raw-mode, cursor, draw, and normal restoration.
///
/// # Errors
///
/// Returns terminal I/O errors while entering, drawing, reading input, or restoring state.
pub fn run(mut app: ControlCenterApp) -> io::Result<()> {
    let mut session = TerminalSession::enter()?;
    loop {
        session.terminal.draw(|frame| render(frame, &app))?;
        if let Event::Key(key) = event::read()? {
            let was_confirming = app.confirm_discard();
            match key.code {
                KeyCode::Up => app.handle_key('k'),
                KeyCode::Down => app.handle_key('j'),
                KeyCode::Left | KeyCode::Right | KeyCode::Char('t') => app.toggle_theme(),
                KeyCode::Char(character @ ('a' | 'r' | 'q' | 'd' | 'k')) => {
                    app.handle_key(character);
                }
                _ => {}
            }
            if key.code == KeyCode::Char('q') && !app.dirty() {
                break;
            }
            if was_confirming && key.code == KeyCode::Char('d') && !app.confirm_discard() {
                break;
            }
        }
    }
    session.restore()
}

struct TerminalSession {
    terminal: ratatui::Terminal<CrosstermBackend<io::Stdout>>,
    restored: bool,
}
impl TerminalSession {
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let terminal = ratatui::Terminal::new(CrosstermBackend::new(stdout))?;
        Ok(Self {
            terminal,
            restored: false,
        })
    }
    fn restore(&mut self) -> io::Result<()> {
        if !self.restored {
            disable_raw_mode()?;
            execute!(self.terminal.backend_mut(), LeaveAlternateScreen)?;
            self.terminal.show_cursor()?;
            self.restored = true;
        }
        Ok(())
    }
}
impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

/// Renders all Control Center screens into the active Ratatui frame.
pub fn render(frame: &mut Frame, app: &ControlCenterApp) {
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
    } else {
        format!(
            "j/k navigate  t toggle theme  a apply  r revert  q quit{}",
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
    let lines = match app.screen {
        Screen::Overview => vec![
            "Overall management health",
            health_label(app.snapshot.health),
            "Presentation",
            app.current.theme().as_str(),
            "Worker state and integration actions are shown without raw provider data.",
        ],
        Screen::Appearance => vec![
            "Draft appearance (staged only)",
            "Title, tab color, activity, spinner, theme",
            app.draft.theme().as_str(),
            "Press t to change the live in-memory preview.",
        ],
        Screen::Integration => vec![
            "Codex Integration",
            "Hook trust remains a manual action.",
            "Recommended actions are read-only plans.",
        ],
        Screen::Diagnostics => app
            .snapshot
            .issues
            .iter()
            .map(|issue| issue.title.as_str())
            .collect::<Vec<_>>(),
        Screen::Preview => vec![
            "Ready",
            "Working",
            "Result ready",
            "Approval",
            app.draft.theme().as_str(),
        ],
    };
    Paragraph::new(lines.join("\n"))
}

fn health_label(health: ManagementHealth) -> &'static str {
    match health {
        ManagementHealth::Healthy => "Healthy",
        ManagementHealth::Warning => "Attention",
        ManagementHealth::Error => "Failure",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};
    fn app() -> ControlCenterApp {
        ControlCenterApp::new(
            PresentationSettings::default(),
            ManagementSnapshot {
                health: ManagementHealth::Healthy,
                issues: Vec::new(),
                recommended_actions: Vec::new(),
                change_plans: Vec::new(),
            },
        )
    }
    #[test]
    fn buffer_renders_every_screen_at_normal_and_narrow_width() {
        let mut app = app();
        for _ in 0..5 {
            for width in [80, 24] {
                let mut terminal = Terminal::new(TestBackend::new(width, 12)).unwrap();
                terminal.draw(|frame| render(frame, &app)).unwrap();
                assert!(format!("{:?}", terminal.backend().buffer()).contains(app.screen.title()));
            }
            app.handle_key('j');
        }
    }
    #[test]
    fn edits_are_staged_revertible_and_dirty_quit_is_lossless() {
        let mut app = app();
        let before = app.current();
        app.toggle_theme();
        assert!(app.dirty());
        assert_eq!(app.current(), before);
        app.handle_key('q');
        assert!(app.confirm_discard());
        app.handle_key('k');
        assert!(app.dirty());
        app.handle_key('r');
        assert!(!app.dirty());
    }
    #[test]
    fn apply_promotes_only_the_existing_in_memory_draft() {
        let mut app = app();
        app.toggle_theme();
        app.handle_key('a');
        assert!(!app.dirty());
        assert_eq!(app.current(), app.draft());
    }
}
