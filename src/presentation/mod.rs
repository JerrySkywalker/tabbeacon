//! Provider-neutral semantic presentation policy and Windows Terminal encoding.
//!
//! This module consumes typed semantic state, then produces typed visual state
//! and bytes. It intentionally contains no provider transport, repository
//! identity, process, terminal-launch, or UI-automation code.

use crate::{
    core::{Attention, Health, Phase, SessionSnapshot},
    settings::{ActivityMode, PresentationSettings, PresentationTheme, TabColorMode, TitleMode},
};

const ESC: u8 = 0x1b;
const STRING_TERMINATOR: [u8; 2] = [ESC, b'\\'];
const FRAME_BACKGROUND_COLOR_INDEX: u16 = 264;

/// Maximum number of Unicode scalar values emitted in one terminal title.
pub const MAX_TITLE_SCALARS: usize = 80;

const STATUS_IDENTITY_SEPARATOR: char = ' ';
const READY_STATUS_SLOT: &str = "○";
const STATIC_WORKING_STATUS_SLOT: &str = "•";
const RESULT_READY_STATUS_SLOT: &str = "✓";
const APPROVAL_STATUS_SLOT: &str = "!";
const QUESTION_STATUS_SLOT: &str = "?";
const WARNING_STATUS_SLOT: &str = "!";
const INTERRUPTED_STATUS_SLOT: &str = "⊘";
const FAILED_STATUS_SLOT: &str = "×";

/// Semantic presentation input independent from a provider or terminal backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SemanticPresentationInput<'a> {
    phase: Phase,
    attention: Attention,
    health: Health,
    workspace_alias: &'a str,
}

impl<'a> SemanticPresentationInput<'a> {
    /// Creates semantic presentation input without a provider integration.
    #[must_use]
    pub const fn new(
        phase: Phase,
        attention: Attention,
        health: Health,
        workspace_alias: &'a str,
    ) -> Self {
        Self {
            phase,
            attention,
            health,
            workspace_alias,
        }
    }

    /// Extracts the semantic axes from a reconciled core session snapshot.
    #[must_use]
    pub fn from_snapshot(snapshot: &SessionSnapshot, workspace_alias: &'a str) -> Self {
        Self::new(
            snapshot.phase(),
            snapshot.attention(),
            snapshot.health(),
            workspace_alias,
        )
    }

    /// Returns the semantic phase.
    #[must_use]
    pub const fn phase(self) -> Phase {
        self.phase
    }

    /// Returns the semantic attention state.
    #[must_use]
    pub const fn attention(self) -> Attention {
        self.attention
    }

    /// Returns the semantic health state.
    #[must_use]
    pub const fn health(self) -> Health {
        self.health
    }

    /// Returns the stable workspace identity before title-policy sanitization.
    #[must_use]
    pub const fn workspace_alias(self) -> &'a str {
        self.workspace_alias
    }

    /// Returns the stable workspace identity using the v0.1 compatibility name.
    #[must_use]
    pub const fn repository_alias(self) -> &'a str {
        self.workspace_alias()
    }
}

/// A presentation-safe workspace identity kept separate from semantic status.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TitleIdentity(String);

impl TitleIdentity {
    /// Replaces controls without applying the final composed-title limit.
    #[must_use]
    pub fn new(value: &str) -> Self {
        Self(sanitize_title_text(value))
    }

    /// Returns the control-free workspace identity.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A terminal title that is safe to insert into a fixed OSC envelope.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalTitle(String);

impl TerminalTitle {
    /// Replaces control characters and applies the deterministic title limit.
    #[must_use]
    pub fn new(value: &str) -> Self {
        let sanitized = sanitize_title_text(value);
        let value = if sanitized.chars().count() > MAX_TITLE_SCALARS {
            let mut truncated = sanitized
                .chars()
                .take(MAX_TITLE_SCALARS - 1)
                .collect::<String>();
            truncated.push('…');
            truncated
        } else {
            sanitized
        };

        Self(value)
    }

    /// Returns the sanitized, bounded title payload.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Provider-neutral semantic selection for the mutable left title slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TitleStatus {
    /// Neutral/ready presentation.
    Ready,
    /// Active work presentation.
    Working,
    /// A result is ready for inspection.
    ResultReady,
    /// Approval is required.
    Approval,
    /// An answer is required.
    Question,
    /// Evidence-backed warning.
    Warning,
    /// The session was interrupted.
    Interrupted,
    /// The session failed.
    Failed,
}

/// Semantic tab/frame color, separate from the renderer's RGB palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TabColor {
    /// Restore the terminal's default frame color.
    Default,
    /// Active work is in progress.
    Working,
    /// A result is ready for the user.
    ResultReady,
    /// The agent needs an approval.
    Approval,
    /// The agent needs an answer to a question.
    Question,
    /// Evidence-backed warning.
    Warning,
    /// The session was interrupted.
    Interrupted,
    /// The session failed.
    Failed,
}

impl TabColor {
    /// Returns the classic G02 compatibility palette value for this semantic color.
    ///
    /// New presentation code should resolve colors through [`PresentationTheme`]
    /// so semantic state remains independent from user palette choice.
    #[must_use]
    pub const fn rgb(self) -> Option<Rgb> {
        PresentationTheme::Classic.rgb(self)
    }
}

impl PresentationTheme {
    /// Resolves one provider-neutral semantic tab color to a terminal RGB value.
    #[must_use]
    pub const fn rgb(self, color: TabColor) -> Option<Rgb> {
        match (self, color) {
            (_, TabColor::Default) => None,
            (Self::Classic, TabColor::Working) => Some(Rgb::new(0x2e, 0xcc, 0x71)),
            (Self::Classic, TabColor::ResultReady) => Some(Rgb::new(0x34, 0x98, 0xdb)),
            (Self::Classic, TabColor::Approval | TabColor::Question) => {
                Some(Rgb::new(0xf1, 0xc4, 0x0f))
            }
            (Self::Classic, TabColor::Warning) => Some(Rgb::new(0xe6, 0x7e, 0x22)),
            (Self::Classic, TabColor::Interrupted) => Some(Rgb::new(0x9b, 0x59, 0xb6)),
            (Self::Classic, TabColor::Failed) => Some(Rgb::new(0xe7, 0x4c, 0x3c)),
            // Muted dark deliberately uses low-saturation dark fills. It is a
            // semantic palette, not a constant multiplier of the classic RGBs.
            (Self::MutedDark, TabColor::Working) => Some(Rgb::new(0x1b, 0x4e, 0x3a)),
            (Self::MutedDark, TabColor::ResultReady) => Some(Rgb::new(0x1e, 0x3e, 0x88)),
            (Self::MutedDark, TabColor::Approval | TabColor::Question) => {
                Some(Rgb::new(0x77, 0x68, 0x24))
            }
            (Self::MutedDark, TabColor::Warning) => Some(Rgb::new(0x81, 0x34, 0x0e)),
            (Self::MutedDark, TabColor::Interrupted) => Some(Rgb::new(0x48, 0x39, 0x5f)),
            (Self::MutedDark, TabColor::Failed) => Some(Rgb::new(0x5e, 0x1e, 0x35)),
        }
    }
}

/// A typed RGB color used only by a terminal backend palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Rgb {
    red: u8,
    green: u8,
    blue: u8,
}

impl Rgb {
    /// Creates an RGB value.
    #[must_use]
    pub const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }

    /// Returns the red channel.
    #[must_use]
    pub const fn red(self) -> u8 {
        self.red
    }

    /// Returns the green channel.
    #[must_use]
    pub const fn green(self) -> u8 {
        self.green
    }

    /// Returns the blue channel.
    #[must_use]
    pub const fn blue(self) -> u8 {
        self.blue
    }
}

/// Semantic progress state supported by Windows Terminal's OSC `9;4` path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Progress {
    /// Hide any existing progress ring/taskbar state.
    Clear,
    /// Show work without a completion value.
    Indeterminate,
    /// Show a warning state.
    Warning,
    /// Show an error state.
    Error,
}

/// Fully typed visual state that can be applied to a terminal backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisualState {
    workspace_alias: TitleIdentity,
    title_status: TitleStatus,
    tab_color: TabColor,
    progress: Progress,
    reset_semantics: ResetSemantics,
}

impl VisualState {
    /// Creates a typed visual state.
    #[must_use]
    pub const fn new(
        workspace_alias: TitleIdentity,
        title_status: TitleStatus,
        tab_color: TabColor,
        progress: Progress,
    ) -> Self {
        Self {
            workspace_alias,
            title_status,
            tab_color,
            progress,
            reset_semantics: ResetSemantics::NoReset,
        }
    }

    const fn reset(workspace_alias: TitleIdentity) -> Self {
        Self {
            workspace_alias,
            title_status: TitleStatus::Ready,
            tab_color: TabColor::Default,
            progress: Progress::Clear,
            reset_semantics: ResetSemantics::ClearProgressAndFrameColor,
        }
    }

    /// Returns the stable, presentation-safe workspace identity.
    #[must_use]
    pub const fn workspace_alias(&self) -> &TitleIdentity {
        &self.workspace_alias
    }

    /// Returns the workspace alias using the v0.1 compatibility name.
    #[must_use]
    pub const fn repository_alias(&self) -> &TitleIdentity {
        self.workspace_alias()
    }

    /// Returns the semantic status used by the mutable left title slot.
    #[must_use]
    pub const fn title_status(&self) -> TitleStatus {
        self.title_status
    }

    /// Returns the semantic tab/frame color.
    #[must_use]
    pub const fn tab_color(&self) -> TabColor {
        self.tab_color
    }

    /// Returns the progress semantic.
    #[must_use]
    pub const fn progress(&self) -> Progress {
        self.progress
    }

    /// Returns whether this visual state applies or resets terminal presentation.
    #[must_use]
    pub const fn reset_semantics(&self) -> ResetSemantics {
        self.reset_semantics
    }
}

/// Typed cleanup behavior for a completed presentation session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ResetSemantics {
    /// Apply the visual state without terminal cleanup.
    NoReset,
    /// Reapply the safe title, clear progress, and reset dynamic frame color.
    ClearProgressAndFrameColor,
}

/// The complete policy result: apply typed state or clean it up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresentationAction {
    /// Apply a typed visual state.
    Apply(VisualState),
    /// Reset terminal presentation for an ordinary ended session.
    Reset(VisualState),
}

/// Deterministic policy that maps G01 semantic axes to presentation semantics.
#[derive(Debug, Clone, Copy, Default)]
pub struct PresentationPolicy;

impl PresentationPolicy {
    /// Resolves semantic input according to the normative G02 precedence order.
    #[must_use]
    pub fn resolve(input: SemanticPresentationInput<'_>) -> PresentationAction {
        let workspace_alias = TitleIdentity::new(input.workspace_alias());

        if input.health() == Health::Failed {
            return PresentationAction::Apply(VisualState::new(
                workspace_alias,
                TitleStatus::Failed,
                TabColor::Failed,
                Progress::Error,
            ));
        }
        if input.health() == Health::Interrupted {
            return PresentationAction::Apply(VisualState::new(
                workspace_alias,
                TitleStatus::Interrupted,
                TabColor::Interrupted,
                Progress::Clear,
            ));
        }
        if input.health() == Health::Warning {
            let progress = if input.phase() == Phase::Working {
                Progress::Indeterminate
            } else {
                Progress::Warning
            };
            return PresentationAction::Apply(VisualState::new(
                workspace_alias,
                TitleStatus::Warning,
                TabColor::Warning,
                progress,
            ));
        }
        if input.attention() == Attention::Approval {
            return PresentationAction::Apply(VisualState::new(
                workspace_alias,
                TitleStatus::Approval,
                TabColor::Approval,
                Progress::Warning,
            ));
        }
        if input.attention() == Attention::Question {
            return PresentationAction::Apply(VisualState::new(
                workspace_alias,
                TitleStatus::Question,
                TabColor::Question,
                Progress::Warning,
            ));
        }
        if input.attention() == Attention::ResultReady {
            return PresentationAction::Apply(VisualState::new(
                workspace_alias,
                TitleStatus::ResultReady,
                TabColor::ResultReady,
                Progress::Clear,
            ));
        }
        if input.phase() == Phase::Working {
            return PresentationAction::Apply(VisualState::new(
                workspace_alias,
                TitleStatus::Working,
                TabColor::Working,
                Progress::Indeterminate,
            ));
        }
        if input.phase() == Phase::Ended {
            return PresentationAction::Reset(VisualState::reset(workspace_alias));
        }

        PresentationAction::Apply(VisualState::new(
            workspace_alias,
            TitleStatus::Ready,
            TabColor::Default,
            Progress::Clear,
        ))
    }
}

/// Windows Terminal enhancements that can be disabled without losing core output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowsTerminalCapabilities {
    frame_color: bool,
}

impl WindowsTerminalCapabilities {
    /// Creates a capability declaration for the Windows Terminal renderer.
    #[must_use]
    pub const fn new(frame_color: bool) -> Self {
        Self { frame_color }
    }

    /// Returns whether dynamic tab/frame color may be emitted.
    #[must_use]
    pub const fn frame_color_supported(self) -> bool {
        self.frame_color
    }
}

impl Default for WindowsTerminalCapabilities {
    fn default() -> Self {
        Self::new(false)
    }
}

/// Typed `PresentationAction` to Windows Terminal VT byte renderer.
#[derive(Debug, Clone, Copy, Default)]
pub struct WindowsTerminalRenderer {
    capabilities: WindowsTerminalCapabilities,
    settings: PresentationSettings,
}

impl WindowsTerminalRenderer {
    /// Creates a renderer using the supplied explicit terminal capabilities.
    #[must_use]
    pub const fn new(capabilities: WindowsTerminalCapabilities) -> Self {
        // Preserve the G02 renderer contract for deterministic fixture and
        // visual-infrastructure callers. The live Codex runtime supplies the
        // user-selected v0.1 settings explicitly.
        Self::with_settings(
            capabilities,
            PresentationSettings::new(
                TitleMode::TabBeacon,
                TabColorMode::TabBeacon,
                ActivityMode::WindowsTerminalRing,
                crate::settings::SpinnerPreset::Codex,
                PresentationTheme::Classic,
            ),
        )
    }

    /// Creates a renderer with explicitly selected user presentation settings.
    #[must_use]
    pub const fn with_settings(
        capabilities: WindowsTerminalCapabilities,
        settings: PresentationSettings,
    ) -> Self {
        Self {
            capabilities,
            settings,
        }
    }

    /// Returns the renderer capabilities.
    #[must_use]
    pub const fn capabilities(self) -> WindowsTerminalCapabilities {
        self.capabilities
    }

    /// Returns the typed channel and theme preferences used by this renderer.
    #[must_use]
    pub const fn settings(self) -> PresentationSettings {
        self.settings
    }

    /// Produces deterministic VT bytes for the action without writing to a terminal.
    #[must_use]
    pub fn render(&self, action: &PresentationAction) -> Vec<u8> {
        let mut bytes = Vec::new();
        match action {
            PresentationAction::Apply(state) | PresentationAction::Reset(state) => {
                if let Some(title) = self.title_for(state) {
                    append_title(&mut bytes, &title);
                }
                append_progress(&mut bytes, configured_progress(state, self.settings));
                if self.capabilities.frame_color_supported() {
                    let color = if self.settings.tab_color() == TabColorMode::TabBeacon {
                        state.tab_color()
                    } else {
                        // Native/off never reapply a dynamic color. The clear is
                        // harmlessly idempotent and removes a prior owned color.
                        TabColor::Default
                    };
                    append_frame_color(&mut bytes, color, self.settings.theme());
                }
            }
        }
        bytes
    }

    /// Resolves the title channel without encoding any terminal control bytes.
    ///
    /// A `None` result means native/off mode deliberately leaves title output
    /// to `Codex` rather than emitting an OSC title from `TabBeacon`.
    #[must_use]
    pub fn title_for(&self, state: &VisualState) -> Option<TerminalTitle> {
        (self.settings.title() == TitleMode::TabBeacon)
            .then(|| configured_title(state, self.settings, 0))
    }

    /// Resolves one deterministic configured spinner frame for title previews/tests.
    ///
    /// The one-shot hook path continues to use frame zero. A later admitted
    /// animator may advance this index without changing repository identity or
    /// the status-first grammar.
    #[must_use]
    pub fn title_for_spinner_frame(
        &self,
        state: &VisualState,
        frame_index: usize,
    ) -> Option<TerminalTitle> {
        (self.settings.title() == TitleMode::TabBeacon)
            .then(|| configured_title(state, self.settings, frame_index))
    }

    /// Whether an indeterminate visual state uses Windows Terminal animation.
    #[must_use]
    pub const fn uses_progress_animation(self) -> bool {
        self.settings.activity().uses_windows_terminal_ring()
    }
}

/// One provider-free semantic fixture case for future visual replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresentationFixtureCase {
    name: &'static str,
    phase: Phase,
    attention: Attention,
    health: Health,
    workspace_alias: &'static str,
}

impl PresentationFixtureCase {
    /// Returns the stable fixture name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Returns provider-free semantic input for this fixture state.
    #[must_use]
    pub const fn input(&self) -> SemanticPresentationInput<'static> {
        SemanticPresentationInput::new(
            self.phase,
            self.attention,
            self.health,
            self.workspace_alias,
        )
    }

    /// Returns this provider-free fixture input with a caller-owned title.
    ///
    /// G03 uses this narrow seam to correlate one dedicated Windows Terminal
    /// test tab. It does not alter the fixture's phase, attention, health,
    /// policy, semantic palette, or VT renderer contract.
    #[must_use]
    pub fn input_with_title<'a>(&self, title: &'a str) -> SemanticPresentationInput<'a> {
        SemanticPresentationInput::new(self.phase, self.attention, self.health, title)
    }

    /// Resolves the fixture through the production presentation policy.
    #[must_use]
    pub fn action(&self) -> PresentationAction {
        PresentationPolicy::resolve(self.input())
    }

    /// Resolves this fixture using a caller-owned title for visual-test tab
    /// correlation without changing its semantic fixture state.
    #[must_use]
    pub fn action_with_title(&self, title: &str) -> PresentationAction {
        PresentationPolicy::resolve(self.input_with_title(title))
    }
}

/// A fixture action rendered to deterministic bytes by a chosen renderer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedPresentationFixture {
    name: &'static str,
    action: PresentationAction,
    bytes: Vec<u8>,
}

impl RenderedPresentationFixture {
    /// Returns the stable fixture name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Returns the policy action replayed by this fixture.
    #[must_use]
    pub const fn action(&self) -> &PresentationAction {
        &self.action
    }

    /// Returns the deterministic renderer bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

const PRESENTATION_FIXTURES: [PresentationFixtureCase; 10] = [
    PresentationFixtureCase {
        name: "ready",
        phase: Phase::Ready,
        attention: Attention::None,
        health: Health::Normal,
        workspace_alias: "JPC",
    },
    PresentationFixtureCase {
        name: "working",
        phase: Phase::Working,
        attention: Attention::None,
        health: Health::Normal,
        workspace_alias: "OWH",
    },
    PresentationFixtureCase {
        name: "result-ready",
        phase: Phase::WaitingUser,
        attention: Attention::ResultReady,
        health: Health::Normal,
        workspace_alias: "WM",
    },
    PresentationFixtureCase {
        name: "approval",
        phase: Phase::WaitingUser,
        attention: Attention::Approval,
        health: Health::Normal,
        workspace_alias: "JPC",
    },
    PresentationFixtureCase {
        name: "question",
        phase: Phase::WaitingUser,
        attention: Attention::Question,
        health: Health::Normal,
        workspace_alias: "OWH",
    },
    PresentationFixtureCase {
        name: "warning-working",
        phase: Phase::Working,
        attention: Attention::None,
        health: Health::Warning,
        workspace_alias: "WM",
    },
    PresentationFixtureCase {
        name: "warning-idle",
        phase: Phase::WaitingUser,
        attention: Attention::None,
        health: Health::Warning,
        workspace_alias: "JPC",
    },
    PresentationFixtureCase {
        name: "interrupted",
        phase: Phase::Ready,
        attention: Attention::None,
        health: Health::Interrupted,
        workspace_alias: "OWH",
    },
    PresentationFixtureCase {
        name: "failed",
        phase: Phase::Ready,
        attention: Attention::None,
        health: Health::Failed,
        workspace_alias: "WM",
    },
    PresentationFixtureCase {
        name: "reset",
        phase: Phase::Ended,
        attention: Attention::None,
        health: Health::Normal,
        workspace_alias: "JPC",
    },
];

/// Returns the complete deterministic G02 fixture table.
#[must_use]
pub const fn presentation_fixture() -> &'static [PresentationFixtureCase] {
    &PRESENTATION_FIXTURES
}

/// Resolves and renders every fixture without a provider, network, or terminal launch.
#[must_use]
pub fn replay_presentation_fixture(
    renderer: &WindowsTerminalRenderer,
) -> Vec<RenderedPresentationFixture> {
    presentation_fixture()
        .iter()
        .map(|case| {
            let action = case.action();
            let bytes = renderer.render(&action);
            RenderedPresentationFixture {
                name: case.name(),
                action,
                bytes,
            }
        })
        .collect()
}

fn append_title(bytes: &mut Vec<u8>, title: &TerminalTitle) {
    bytes.extend_from_slice(&[ESC, b']', b'0', b';']);
    bytes.extend_from_slice(title.as_str().as_bytes());
    bytes.extend_from_slice(&STRING_TERMINATOR);
}

fn append_progress(bytes: &mut Vec<u8>, progress: Progress) {
    let payload = match progress {
        Progress::Clear => b"9;4;0;0".as_slice(),
        Progress::Indeterminate => b"9;4;3;0".as_slice(),
        Progress::Warning => b"9;4;4;100".as_slice(),
        Progress::Error => b"9;4;2;100".as_slice(),
    };
    append_osc(bytes, payload);
}

fn configured_title(
    state: &VisualState,
    settings: PresentationSettings,
    frame_index: usize,
) -> TerminalTitle {
    let status_slot = match state.title_status() {
        TitleStatus::Ready => READY_STATUS_SLOT,
        TitleStatus::Working => match settings.activity() {
            ActivityMode::TitleSpinner => {
                let frames = settings.spinner().frames();
                frames[frame_index % frames.len()]
            }
            ActivityMode::TitleIndicator | ActivityMode::Both => STATIC_WORKING_STATUS_SLOT,
            ActivityMode::WindowsTerminalRing | ActivityMode::Native | ActivityMode::Off => {
                READY_STATUS_SLOT
            }
        },
        TitleStatus::ResultReady => RESULT_READY_STATUS_SLOT,
        TitleStatus::Approval => APPROVAL_STATUS_SLOT,
        TitleStatus::Question => QUESTION_STATUS_SLOT,
        TitleStatus::Warning => WARNING_STATUS_SLOT,
        TitleStatus::Interrupted => INTERRUPTED_STATUS_SLOT,
        TitleStatus::Failed => FAILED_STATUS_SLOT,
    };
    let mut title = String::with_capacity(
        status_slot.len()
            + STATUS_IDENTITY_SEPARATOR.len_utf8()
            + state.workspace_alias().as_str().len(),
    );
    title.push_str(status_slot);
    title.push(STATUS_IDENTITY_SEPARATOR);
    title.push_str(state.workspace_alias().as_str());
    TerminalTitle::new(&title)
}

fn sanitize_title_text(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_control() {
                '\u{fffd}'
            } else {
                character
            }
        })
        .collect()
}

fn configured_progress(state: &VisualState, settings: PresentationSettings) -> Progress {
    if settings.activity().uses_windows_terminal_ring() {
        state.progress()
    } else {
        // Explicitly clear a ring left by a prior config or lifecycle state.
        Progress::Clear
    }
}

fn append_frame_color(bytes: &mut Vec<u8>, tab_color: TabColor, theme: PresentationTheme) {
    let Some(color) = theme.rgb(tab_color) else {
        let mut payload = Vec::from(b"104;".as_slice());
        append_decimal(&mut payload, FRAME_BACKGROUND_COLOR_INDEX);
        append_osc(bytes, &payload);
        return;
    };

    let mut payload = Vec::from(b"4;".as_slice());
    append_decimal(&mut payload, FRAME_BACKGROUND_COLOR_INDEX);
    payload.extend_from_slice(b";rgb:");
    append_hex_byte(&mut payload, color.red());
    payload.push(b'/');
    append_hex_byte(&mut payload, color.green());
    payload.push(b'/');
    append_hex_byte(&mut payload, color.blue());
    append_osc(bytes, &payload);
}

fn append_hex_byte(bytes: &mut Vec<u8>, value: u8) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    bytes.push(HEX[usize::from(value >> 4)]);
    bytes.push(HEX[usize::from(value & 0x0f)]);
}

fn append_decimal(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(value.to_string().as_bytes());
}

fn append_osc(bytes: &mut Vec<u8>, payload: &[u8]) {
    bytes.extend_from_slice(&[ESC, b']']);
    bytes.extend_from_slice(payload);
    bytes.extend_from_slice(&STRING_TERMINATOR);
}
