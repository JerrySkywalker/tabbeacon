//! Staged Control Center frontend and bounded Ratatui renderer.

use std::{
    io,
    time::{Duration, Instant},
};

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
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::{
    activity::SessionsOverview,
    core::{Attention, Health, Phase},
    hook_inventory::HookInventory,
    human_presentation::{
        HumanMessageKey, ManagementTextKind, ResolvedLocale, catalog, color_enabled,
        health_label as shared_health_label, management_action_text, management_text,
        pad_display_width, render_human_text,
    },
    interface_preferences::{HumanColor, InterfaceLanguage, InterfacePreferences},
    management::{ActionSafety, ChangePlan, ManagementOverview, ManagementSnapshot},
    presentation::{
        PresentationAction, PresentationPolicy, SemanticPresentationInput,
        WindowsTerminalCapabilities, WindowsTerminalRenderer,
    },
    providers::registry::ProviderRegistry,
    repo::WorkspaceAliasInspection,
    settings::{
        ActivityMode, PresentationSettings, ProviderBadgePolicy, SpinnerPreset, TabColorMode,
        TitleMode,
    },
    title_explanation::TitleExplanation,
};

/// Bounded local refresh cadence for the daemonless Control Center.
pub const CONTROL_CENTER_REFRESH_INTERVAL: Duration = Duration::from_millis(750);

/// One bounded daily-management screen.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Screen {
    Overview,
    Appearance,
    Workspace,
    Sessions,
    Integration,
    Hooks,
    Diagnostics,
    Interface,
    Preview,
}

impl Screen {
    const ALL: [Self; 9] = [
        Self::Overview,
        Self::Appearance,
        Self::Workspace,
        Self::Sessions,
        Self::Integration,
        Self::Hooks,
        Self::Diagnostics,
        Self::Interface,
        Self::Preview,
    ];

    #[cfg(test)]
    const fn title(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Appearance => "Appearance",
            Self::Workspace => "Workspace",
            Self::Sessions => "Sessions",
            Self::Integration => "Integrations",
            Self::Hooks => "Hooks",
            Self::Diagnostics => "Diagnostics",
            Self::Interface => "Interface",
            Self::Preview => "Preview",
        }
    }

    const fn message_key(self) -> HumanMessageKey {
        match self {
            Self::Overview => HumanMessageKey::Overview,
            Self::Appearance => HumanMessageKey::Appearance,
            Self::Workspace => HumanMessageKey::Workspace,
            Self::Sessions => HumanMessageKey::Sessions,
            Self::Integration => HumanMessageKey::Integrations,
            Self::Hooks => HumanMessageKey::Hooks,
            Self::Diagnostics => HumanMessageKey::Diagnostics,
            Self::Interface => HumanMessageKey::Interface,
            Self::Preview => HumanMessageKey::Preview,
        }
    }

    fn localized_title(self, locale: ResolvedLocale) -> &'static str {
        catalog(locale, self.message_key())
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
    ProviderBadge,
}

impl AppearanceField {
    const ALL: [Self; 6] = [
        Self::Title,
        Self::TabColor,
        Self::Activity,
        Self::Spinner,
        Self::Theme,
        Self::ProviderBadge,
    ];

    const fn message_key(self) -> HumanMessageKey {
        match self {
            Self::Title => HumanMessageKey::Title,
            Self::TabColor => HumanMessageKey::TabColor,
            Self::Activity => HumanMessageKey::Activity,
            Self::Spinner => HumanMessageKey::Spinner,
            Self::Theme => HumanMessageKey::Theme,
            Self::ProviderBadge => HumanMessageKey::ProviderBadgePolicy,
        }
    }
}

/// One staged Interface preference that can be selected in the TUI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InterfaceField {
    Language,
    Color,
    ReducedMotion,
}

impl InterfaceField {
    const ALL: [Self; 3] = [Self::Language, Self::Color, Self::ReducedMotion];

    const fn message_key(self) -> HumanMessageKey {
        match self {
            Self::Language => HumanMessageKey::Language,
            Self::Color => HumanMessageKey::Color,
            Self::ReducedMotion => HumanMessageKey::ReducedMotion,
        }
    }
}

/// One aggregate in-memory Control Center draft.
///
/// Persistence remains separately ownership-aware per user-local store. This
/// type only makes both staged domains explicit to the UI caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControlCenterDraft {
    /// Presentation settings staged for the existing settings store.
    pub presentation: PresentationSettings,
    /// Human Interface preferences staged for the separate Interface store.
    pub interface: InterfacePreferences,
}

/// One bounded, read-only observation merged into the live Control Center.
///
/// It deliberately contains only already-approved management, workspace, and
/// session projections. Collecting it never writes user settings, preferences,
/// repository state, Hook configuration, or terminal state.
#[derive(Clone, Debug)]
pub struct ControlCenterRefresh {
    /// Latest read-only Presentation baseline.
    pub presentation: PresentationSettings,
    /// Latest read-only Interface baseline.
    pub interface: InterfacePreferences,
    /// Shared bounded management projection.
    pub snapshot: ManagementSnapshot,
    /// Compact operational overview derived from the same diagnostic pass.
    pub overview: ManagementOverview,
    /// Privacy-safe current-workspace naming projection, when available.
    pub workspace: Option<WorkspaceAliasInspection>,
    /// Read-only, content-minimal activity lease projection.
    pub sessions: SessionsOverview,
    /// Provider-neutral, command-redacted Hook inventory.
    pub hooks: HookInventory,
    /// Registered provider capability and admission projections.
    pub integrations: ProviderRegistry,
    /// Read-only safe provenance behind a potential title.
    pub title_explanation: TitleExplanation,
}

/// A frontend request that must be executed by an existing ownership-aware API.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ControlCenterCommand {
    /// No persistent operation was requested.
    None,
    /// End the interactive session without persisting a draft.
    Quit,
    /// Persist one staged settings draft through the caller-owned operation.
    Apply {
        /// Current typed state expected by the frontend.
        before: ControlCenterDraft,
        /// Staged typed state to apply.
        after: ControlCenterDraft,
    },
    /// Persist one staged, device-local workspace alias override through the
    /// caller-owned collision-safe resolver.
    ApplyWorkspace {
        /// Custom alias observed when the workspace screen was opened.
        before: Option<String>,
        /// Explicit custom alias, or `None` to use the generated default.
        after: Option<String>,
    },
    /// Apply one already-previewed, ownership-scoped repair through the
    /// caller-owned operation. Only a `PreviewableSafeRepair` can produce it.
    ApplyRepair {
        /// Stable action identity from the shared management snapshot.
        action_id: String,
    },
}

/// A focused modal surface that owns its keyboard events until dismissed.
#[derive(Clone, Debug, Eq, PartialEq)]
enum ControlCenterOverlay {
    None,
    Help,
    RepairPreview(ChangePlan),
    TitleExplanation,
}

impl ControlCenterOverlay {
    const fn is_open(&self) -> bool {
        !matches!(self, Self::None)
    }

    fn title(&self, locale: ResolvedLocale) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::Help => Some(catalog(locale, HumanMessageKey::Help)),
            Self::RepairPreview(_) => Some(catalog(locale, HumanMessageKey::RepairPreview)),
            Self::TitleExplanation => Some(catalog(locale, HumanMessageKey::WhyThisTitle)),
        }
    }
}

/// In-memory frontend state. No mutation authority is stored here.
#[derive(Clone, Debug)]
#[allow(clippy::struct_excessive_bools)] // Independent dirty/conflict/interaction safety state is auditable.
pub struct ControlCenterApp {
    base_locale: ResolvedLocale,
    screen: Screen,
    snapshot: ManagementSnapshot,
    overview: ManagementOverview,
    current: PresentationSettings,
    draft: PresentationSettings,
    current_interface: InterfacePreferences,
    interface_draft: InterfacePreferences,
    workspace: Option<WorkspaceAliasInspection>,
    sessions: Option<SessionsOverview>,
    hooks: HookInventory,
    integrations: ProviderRegistry,
    title_explanation: TitleExplanation,
    current_workspace_override: Option<String>,
    workspace_draft: Option<String>,
    workspace_editor: Option<String>,
    workspace_explaining: bool,
    overlay: ControlCenterOverlay,
    dirty: bool,
    presentation_conflict: bool,
    interface_conflict: bool,
    workspace_conflict: bool,
    confirm_discard: bool,
    appearance_field: Option<AppearanceField>,
    interface_field: Option<InterfaceField>,
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
            base_locale: ResolvedLocale::EnUs,
            screen: Screen::Overview,
            snapshot,
            overview,
            current,
            draft: current,
            current_interface: InterfacePreferences::default(),
            interface_draft: InterfacePreferences::default(),
            workspace: None,
            sessions: None,
            hooks: HookInventory::default(),
            integrations: ProviderRegistry::default(),
            title_explanation: TitleExplanation::default(),
            current_workspace_override: None,
            workspace_draft: None,
            workspace_editor: None,
            workspace_explaining: false,
            overlay: ControlCenterOverlay::None,
            dirty: false,
            presentation_conflict: false,
            interface_conflict: false,
            workspace_conflict: false,
            confirm_discard: false,
            appearance_field: None,
            interface_field: None,
        }
    }

    /// Selects one resolved Human locale for the bounded Control Center surface.
    #[must_use]
    pub fn with_locale(mut self, locale: ResolvedLocale) -> Self {
        self.base_locale = locale;
        self
    }

    /// Supplies the read-only Interface baseline used for the staged screen.
    #[must_use]
    pub fn with_interface_preferences(mut self, preferences: InterfacePreferences) -> Self {
        self.current_interface = preferences;
        self.interface_draft = preferences;
        self
    }

    /// Supplies one already-parsed, read-only provider Hook projection.
    #[must_use]
    pub fn with_hook_inventory(mut self, hooks: HookInventory) -> Self {
        self.hooks = hooks;
        self
    }

    /// Supplies the registered, read-only provider integration projection.
    #[must_use]
    pub fn with_integrations(mut self, integrations: ProviderRegistry) -> Self {
        self.integrations = integrations;
        self
    }

    /// Seeds the frontend with one already-collected read-only live snapshot.
    #[must_use]
    pub fn with_refresh(mut self, refresh: ControlCenterRefresh) -> Self {
        self.merge_refresh(refresh);
        self
    }

    /// The selected Human locale for the bounded Control Center surface.
    #[must_use]
    pub fn locale(&self) -> ResolvedLocale {
        match self.interface_draft.language() {
            InterfaceLanguage::EnUs => ResolvedLocale::EnUs,
            InterfaceLanguage::ZhCn => ResolvedLocale::ZhCn,
            InterfaceLanguage::Auto => self.base_locale,
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

    /// Current persisted/effective Interface preferences.
    #[must_use]
    pub const fn current_interface(&self) -> InterfacePreferences {
        self.current_interface
    }

    /// In-memory Interface preferences used for live Human rendering.
    #[must_use]
    pub const fn interface_draft(&self) -> InterfacePreferences {
        self.interface_draft
    }

    /// Current aggregate state bound to the caller's snapshots.
    #[must_use]
    pub const fn current_draft(&self) -> ControlCenterDraft {
        ControlCenterDraft {
            presentation: self.current,
            interface: self.current_interface,
        }
    }

    /// Aggregate staged state requested for an Apply operation.
    #[must_use]
    pub const fn staged_draft(&self) -> ControlCenterDraft {
        ControlCenterDraft {
            presentation: self.draft,
            interface: self.interface_draft,
        }
    }

    /// Whether quit requires an explicit discard response.
    #[must_use]
    pub const fn confirm_discard(&self) -> bool {
        self.confirm_discard
    }

    /// Whether a refresh observed an externally changed baseline while its
    /// related local draft remained dirty. Apply is refused until Revert.
    #[must_use]
    pub const fn has_concurrent_conflict(&self) -> bool {
        self.presentation_conflict || self.interface_conflict || self.workspace_conflict
    }

    /// Whether a help or repair surface is receiving all normal key events.
    #[must_use]
    pub fn overlay_open(&self) -> bool {
        self.overlay.is_open()
    }

    /// Merges a new bounded observation without ever persisting state.
    ///
    /// A clean draft follows its current baseline. A dirty draft is retained;
    /// if the matching persisted baseline moved, the app marks a visible
    /// conflict and refuses stale Apply until the user Reverts.
    pub fn merge_refresh(&mut self, refresh: ControlCenterRefresh) {
        self.snapshot = refresh.snapshot;
        self.overview = refresh.overview;
        self.hooks = refresh.hooks;
        self.integrations = refresh.integrations;
        self.title_explanation = refresh.title_explanation;
        if let Some(workspace) = refresh.workspace {
            let override_alias = workspace
                .custom_alias()
                .map(|alias| alias.as_str().to_owned());
            if self.workspace_draft == self.current_workspace_override {
                self.current_workspace_override = override_alias.clone();
                self.workspace_draft = override_alias;
                self.workspace_conflict = false;
            } else if override_alias != self.current_workspace_override {
                self.current_workspace_override = override_alias;
                self.workspace_conflict = true;
            }
            self.workspace = Some(workspace);
        }
        self.sessions = Some(refresh.sessions);

        if self.draft == self.current {
            self.current = refresh.presentation;
            self.draft = refresh.presentation;
            self.presentation_conflict = false;
        } else if refresh.presentation != self.current {
            self.current = refresh.presentation;
            self.presentation_conflict = true;
        }

        if self.interface_draft == self.current_interface {
            self.current_interface = refresh.interface;
            self.interface_draft = refresh.interface;
            self.interface_conflict = false;
        } else if refresh.interface != self.current_interface {
            self.current_interface = refresh.interface;
            self.interface_conflict = true;
        }
        self.update_dirty();
    }

    fn editing(&self) -> bool {
        self.appearance_field.is_some()
            || self.interface_field.is_some()
            || self.workspace_editor.is_some()
    }

    /// Applies one event to staged state and returns a caller-owned action request.
    pub fn handle_key(&mut self, key: KeyCode) -> ControlCenterCommand {
        if self.confirm_discard {
            return self.handle_discard_key(key);
        }
        if self.overlay.is_open() {
            return self.handle_overlay_key(key);
        }
        if self.workspace_editor.is_some() {
            self.handle_workspace_editor_key(key);
            return ControlCenterCommand::None;
        }
        if key == KeyCode::Char('t') {
            self.overlay = ControlCenterOverlay::TitleExplanation;
            return ControlCenterCommand::None;
        }
        if key == KeyCode::Char('?') {
            self.overlay = ControlCenterOverlay::Help;
            return ControlCenterCommand::None;
        }
        if self.screen == Screen::Workspace {
            match key {
                KeyCode::Char('d' | 'x') => {
                    self.workspace_draft = None;
                    self.update_dirty();
                    return ControlCenterCommand::None;
                }
                KeyCode::Char('c') => {
                    self.workspace_editor = Some(self.workspace_draft.clone().unwrap_or_default());
                    return ControlCenterCommand::None;
                }
                KeyCode::Char('e') => {
                    self.workspace_explaining = !self.workspace_explaining;
                    return ControlCenterCommand::None;
                }
                KeyCode::Char(candidate @ '1'..='4') => {
                    if let Some(alias) = self.workspace.as_ref().and_then(|workspace| {
                        let index = usize::from(candidate as u8 - b'1');
                        workspace
                            .candidates()
                            .get(index)
                            .map(|item| item.alias().as_str())
                    }) {
                        self.workspace_draft = Some(alias.to_owned());
                        self.update_dirty();
                    }
                    return ControlCenterCommand::None;
                }
                _ => {}
            }
        }
        match key {
            KeyCode::Up | KeyCode::Char('k') => self.step(-1),
            KeyCode::Down | KeyCode::Char('j') => self.step(1),
            KeyCode::Left => self.change_focused(-1),
            KeyCode::Right => self.change_focused(1),
            KeyCode::Enter if self.screen == Screen::Appearance => self.toggle_appearance_focus(),
            KeyCode::Enter if self.screen == Screen::Interface => self.toggle_interface_focus(),
            KeyCode::Char('r') => self.revert(),
            KeyCode::Char('a') => return self.request_apply(),
            KeyCode::Char('p')
                if matches!(self.screen, Screen::Integration | Screen::Diagnostics) =>
            {
                self.open_previewable_repair();
            }
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
        let overlay_was_open = self.overlay.is_open();
        let command = self.handle_key(key.code);
        if key.code == KeyCode::Char('q')
            && !overlay_was_open
            && !self.dirty
            && !self.confirm_discard
        {
            ControlCenterCommand::Quit
        } else {
            command
        }
    }

    /// Marks a successfully caller-owned persistence operation as accepted.
    pub fn apply_succeeded(&mut self) {
        self.current = self.draft;
        self.current_interface = self.interface_draft;
        self.presentation_conflict = false;
        self.interface_conflict = false;
        self.update_dirty();
    }

    /// Marks a successfully caller-owned workspace preference Apply accepted.
    pub fn workspace_apply_succeeded(&mut self) {
        self.current_workspace_override = self.workspace_draft.clone();
        self.workspace_conflict = false;
        self.workspace_editor = None;
        self.update_dirty();
    }

    /// Completes a caller-owned repair and leaves all drafts untouched.
    pub fn repair_apply_succeeded(&mut self) {
        self.overlay = ControlCenterOverlay::None;
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

    fn handle_overlay_key(&mut self, key: KeyCode) -> ControlCenterCommand {
        match &self.overlay {
            ControlCenterOverlay::Help => {
                if matches!(key, KeyCode::Esc | KeyCode::Char('?' | 'q')) {
                    self.overlay = ControlCenterOverlay::None;
                }
            }
            ControlCenterOverlay::RepairPreview(plan) => match key {
                KeyCode::Char('a') => {
                    let action_id = plan.action_id.clone();
                    self.overlay = ControlCenterOverlay::None;
                    return ControlCenterCommand::ApplyRepair { action_id };
                }
                KeyCode::Esc | KeyCode::Char('p' | 'q') => {
                    self.overlay = ControlCenterOverlay::None;
                }
                _ => {}
            },
            ControlCenterOverlay::TitleExplanation => {
                if matches!(key, KeyCode::Esc | KeyCode::Char('t' | 'q')) {
                    self.overlay = ControlCenterOverlay::None;
                }
            }
            ControlCenterOverlay::None => {}
        }
        ControlCenterCommand::None
    }

    fn open_previewable_repair(&mut self) {
        if let Some(plan) = self
            .snapshot
            .change_plans
            .iter()
            .find(|plan| plan.safety == ActionSafety::PreviewableSafeRepair)
            .cloned()
        {
            self.overlay = ControlCenterOverlay::RepairPreview(plan);
        }
    }

    fn request_apply(&self) -> ControlCenterCommand {
        if self.screen == Screen::Workspace
            && self.workspace_draft != self.current_workspace_override
            && !self.has_concurrent_conflict()
        {
            return ControlCenterCommand::ApplyWorkspace {
                before: self.current_workspace_override.clone(),
                after: self.workspace_draft.clone(),
            };
        }
        if self.settings_or_interface_dirty() && !self.has_concurrent_conflict() {
            ControlCenterCommand::Apply {
                before: self.current_draft(),
                after: self.staged_draft(),
            }
        } else {
            ControlCenterCommand::None
        }
    }

    fn revert(&mut self) {
        self.draft = self.current;
        self.interface_draft = self.current_interface;
        self.presentation_conflict = false;
        self.interface_conflict = false;
        self.workspace_draft = self.current_workspace_override.clone();
        self.workspace_editor = None;
        self.workspace_conflict = false;
        self.appearance_field = None;
        self.interface_field = None;
        self.update_dirty();
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
        if self.interface_field.is_some() {
            self.step_interface_field(offset);
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

    fn step_interface_field(&mut self, offset: isize) {
        let index = InterfaceField::ALL
            .iter()
            .position(|field| Some(*field) == self.interface_field)
            .unwrap_or(0);
        self.interface_field =
            Some(InterfaceField::ALL[shifted_index(index, InterfaceField::ALL.len(), offset)]);
    }

    fn change_focused(&mut self, offset: isize) {
        if self.interface_field.is_some() {
            self.change_focused_interface(offset);
            return;
        }
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
            AppearanceField::ProviderBadge => self
                .draft
                .with_provider_badge(cycle_provider_badge(self.draft.provider_badge(), offset)),
        };
        self.update_dirty();
    }

    fn toggle_interface_focus(&mut self) {
        self.interface_field = self
            .interface_field
            .map_or(Some(InterfaceField::Language), |_| None);
    }

    fn change_focused_interface(&mut self, offset: isize) {
        let Some(field) = self.interface_field else {
            return;
        };
        self.interface_draft = match field {
            InterfaceField::Language => self.interface_draft.with_language(
                cycle_interface_language(self.interface_draft.language(), offset),
            ),
            InterfaceField::Color => self
                .interface_draft
                .with_color(cycle_human_color(self.interface_draft.color(), offset)),
            InterfaceField::ReducedMotion => self
                .interface_draft
                .with_reduced_motion(!self.interface_draft.reduced_motion()),
        };
        self.update_dirty();
    }

    fn update_dirty(&mut self) {
        self.dirty = self.settings_or_interface_dirty()
            || self.workspace_draft != self.current_workspace_override;
    }

    fn settings_or_interface_dirty(&self) -> bool {
        self.draft != self.current || self.interface_draft != self.current_interface
    }

    fn handle_workspace_editor_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Enter => {
                let submitted = self.workspace_editor.take().unwrap_or_default();
                self.workspace_draft = (!submitted.trim().is_empty()).then_some(submitted);
                self.update_dirty();
            }
            KeyCode::Esc => self.workspace_editor = None,
            KeyCode::Backspace => {
                if let Some(editor) = self.workspace_editor.as_mut() {
                    editor.pop();
                }
            }
            KeyCode::Char(character) if !character.is_control() => {
                if let Some(editor) = self.workspace_editor.as_mut()
                    && editor.chars().count() < 20
                {
                    editor.push(character);
                }
            }
            _ => {}
        }
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

fn cycle_provider_badge(value: ProviderBadgePolicy, offset: isize) -> ProviderBadgePolicy {
    cycle(
        [
            ProviderBadgePolicy::Auto,
            ProviderBadgePolicy::Always,
            ProviderBadgePolicy::Off,
        ],
        value,
        offset,
    )
}

fn cycle_interface_language(value: InterfaceLanguage, offset: isize) -> InterfaceLanguage {
    cycle(
        [
            InterfaceLanguage::Auto,
            InterfaceLanguage::ZhCn,
            InterfaceLanguage::EnUs,
        ],
        value,
        offset,
    )
}

fn cycle_human_color(value: HumanColor, offset: isize) -> HumanColor {
    cycle(
        [HumanColor::Auto, HumanColor::Always, HumanColor::Never],
        value,
        offset,
    )
}

fn cycle<T: Copy + Eq>(values: impl AsRef<[T]>, value: T, offset: isize) -> T {
    let values = values.as_ref();
    let index = values.iter().position(|item| *item == value).unwrap_or(0);
    values[shifted_index(index, values.len(), offset)]
}

/// Runs the live TUI, delegating Apply and bounded read-only refresh to the
/// caller's existing ownership-aware operations.
///
/// # Errors
///
/// Returns terminal I/O errors or an Apply error after the terminal has been restored.
pub fn run<F, W, R, P>(
    mut app: ControlCenterApp,
    mut apply: F,
    mut apply_workspace: W,
    mut refresh: R,
    mut repair: P,
) -> io::Result<()>
where
    F: FnMut(ControlCenterDraft, ControlCenterDraft) -> io::Result<()>,
    W: FnMut(Option<String>, Option<String>) -> io::Result<()>,
    R: FnMut() -> io::Result<ControlCenterRefresh>,
    P: FnMut(&str) -> io::Result<()>,
{
    let mut session = TerminalSession::enter()?;
    let mut next_refresh = Instant::now() + CONTROL_CENTER_REFRESH_INTERVAL;
    loop {
        session.terminal.draw(|frame| render(frame, &app))?;
        let wait = next_refresh.saturating_duration_since(Instant::now());
        if !event::poll(wait)? {
            app.merge_refresh(refresh()?);
            next_refresh = Instant::now() + CONTROL_CENTER_REFRESH_INTERVAL;
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
                ControlCenterCommand::ApplyWorkspace { before, after } => {
                    apply_workspace(before, after)?;
                    app.workspace_apply_succeeded();
                }
                ControlCenterCommand::ApplyRepair { action_id } => {
                    repair(&action_id)?;
                    app.repair_apply_succeeded();
                    app.merge_refresh(refresh()?);
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
#[allow(clippy::struct_excessive_bools)] // Receipt fields stay independently auditable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalSmokeReport {
    /// The fixture rendered the required live Control Center screens in order.
    pub screens_visited: usize,
    /// A bounded operational refresh was merged without persistence authority.
    pub live_refresh_merged: bool,
    /// Workspace and Sessions navigation reached their production render paths.
    pub workspace_and_sessions_visited: bool,
    /// The command-redacted Hook inventory screen reached its production renderer.
    pub hook_inventory_visited: bool,
    /// The registered provider integration and its capability matrix rendered.
    pub integrations_visited: bool,
    /// `?` opened and `Esc` dismissed the event-isolating help overlay.
    pub help_overlay_exercised: bool,
    /// `t` opened and `Esc` dismissed the read-only title provenance overlay.
    pub title_explanation_exercised: bool,
    /// An appearance value changed in the in-memory draft.
    pub draft_changed: bool,
    /// Revert restored the draft to the original settings without Apply.
    pub draft_reverted: bool,
    /// The provider badge control changed only the in-memory draft, then reverted.
    pub provider_badge_staged: bool,
    /// A concrete Interface language changed the following rendered frame.
    pub interface_locale_switched: bool,
    /// Revert restored the Interface language draft before any persistence request.
    pub interface_draft_reverted: bool,
    /// Apply remained a staged caller request with no fixture mutation authority.
    pub interface_apply_staged: bool,
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
#[allow(clippy::too_many_lines)] // The deterministic smoke trace keeps its receipt assertions in one auditable sequence.
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
    let original_interface = app.current_interface();
    let original_locale = app.locale();
    let refresh = ControlCenterRefresh {
        presentation: original,
        interface: original_interface,
        snapshot: app.snapshot.clone(),
        overview: app.overview.clone(),
        workspace: None,
        sessions: SessionsOverview::default(),
        hooks: app.hooks.clone(),
        integrations: app.integrations.clone(),
        title_explanation: TitleExplanation::default(),
    };
    app.merge_refresh(refresh);
    let live_refresh_merged = app.sessions.is_some() && !app.dirty();
    let mut session = TerminalSession::enter()?;
    let result = (|| {
        invariant(
            live_refresh_merged,
            "fixture did not merge a bounded live refresh",
        )?;
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

        let _ = app.handle_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        for _ in 0..5 {
            let _ = app.handle_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        }
        let _ = app.handle_event(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        let provider_badge_staged = app.dirty()
            && app.draft.provider_badge() == ProviderBadgePolicy::Always
            && app.current().provider_badge() == ProviderBadgePolicy::Auto;
        invariant(
            provider_badge_staged,
            "fixture did not stage the provider badge policy",
        )?;
        draw(&mut session, &app)?;
        let _ = app.handle_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));
        invariant(
            !app.dirty() && app.draft() == original && app.current() == original,
            "fixture did not revert the provider badge draft",
        )?;

        let _ = app.handle_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        invariant(
            app.screen() == Screen::Workspace,
            "fixture did not reach Workspace",
        )?;
        draw(&mut session, &app)?;

        let _ = app.handle_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        invariant(
            app.screen() == Screen::Sessions,
            "fixture did not reach Sessions",
        )?;
        draw(&mut session, &app)?;
        let workspace_and_sessions_visited = app.sessions.is_some();
        invariant(
            workspace_and_sessions_visited,
            "fixture did not retain the refreshed Sessions projection",
        )?;

        let _ = app.handle_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        invariant(
            app.screen() == Screen::Integration,
            "fixture did not reach Integration",
        )?;
        draw(&mut session, &app)?;
        let integrations_visited = app.integrations.providers.len() == 1
            && app.integrations.providers[0].id.as_str() == "codex"
            && app.integrations.providers[0].label == "Codex"
            && app.integrations.providers[0]
                .capability_profile
                .capabilities
                .iter()
                .any(|status| {
                    status.capability == crate::providers::registry::ProviderCapability::Phase
                        && status.availability
                            == crate::providers::registry::CapabilityAvailability::Proven
                });
        invariant(
            integrations_visited,
            "fixture did not render the admitted provider capability projection",
        )?;

        let _ = app.handle_event(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
        let help_overlay_exercised = app.overlay_open();
        invariant(help_overlay_exercised, "fixture did not open Help")?;
        draw(&mut session, &app)?;
        let _ = app.handle_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        invariant(!app.overlay_open(), "fixture did not dismiss Help")?;

        let _ = app.handle_event(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE));
        let title_explanation_exercised = app.overlay_open();
        invariant(
            title_explanation_exercised,
            "fixture did not open Why this title",
        )?;
        draw(&mut session, &app)?;
        let _ = app.handle_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        invariant(
            !app.overlay_open(),
            "fixture did not dismiss Why this title",
        )?;

        let _ = app.handle_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        invariant(app.screen() == Screen::Hooks, "fixture did not reach Hooks")?;
        draw(&mut session, &app)?;
        let hook_inventory_visited = true;

        let _ = app.handle_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        invariant(
            app.screen() == Screen::Diagnostics,
            "fixture did not reach Diagnostics",
        )?;
        draw(&mut session, &app)?;

        let _ = app.handle_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        invariant(
            app.screen() == Screen::Interface,
            "fixture did not reach Interface",
        )?;
        draw(&mut session, &app)?;

        let _ = app.handle_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        let _ = app.handle_event(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        let interface_locale_switched = app.interface_draft().language() == InterfaceLanguage::ZhCn
            && app.locale() == ResolvedLocale::ZhCn
            && app.current_interface() == original_interface;
        invariant(
            interface_locale_switched,
            "fixture did not live-switch the Interface locale",
        )?;
        draw(&mut session, &app)?;

        let _ = app.handle_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));
        let interface_draft_reverted = !app.dirty()
            && app.interface_draft() == original_interface
            && app.current_interface() == original_interface
            && app.locale() == original_locale;
        invariant(
            interface_draft_reverted,
            "fixture did not revert its Interface language draft",
        )?;

        let _ = app.handle_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        let _ = app.handle_event(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        let interface_apply_staged = matches!(
            app.handle_event(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE)),
            ControlCenterCommand::Apply { before, after }
                if before.interface == original_interface
                    && after.interface.language() == InterfaceLanguage::ZhCn
                    && before.presentation == after.presentation
        );
        invariant(
            interface_apply_staged,
            "fixture did not request a staged Interface apply",
        )?;
        let _ = app.handle_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));

        let _ = app.handle_event(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        let _ = app.handle_event(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        let _ = app.handle_event(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
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
            screens_visited: 8,
            live_refresh_merged,
            workspace_and_sessions_visited,
            hook_inventory_visited,
            integrations_visited,
            help_overlay_exercised,
            title_explanation_exercised,
            draft_changed,
            draft_reverted,
            provider_badge_staged,
            interface_locale_switched,
            interface_draft_reverted,
            interface_apply_staged,
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
#[allow(clippy::too_many_lines)] // The compact layout keeps terminal-size, focus, and overlay safety together.
pub fn render(frame: &mut Frame, app: &ControlCenterApp) {
    let style = tui_human_style(
        app.interface_draft.color(),
        std::env::var_os("NO_COLOR").is_some(),
    );
    let area = frame.area();
    if area.width < MIN_TERMINAL_WIDTH || area.height < MIN_TERMINAL_HEIGHT {
        frame.render_widget(
            Paragraph::new(format!(
                "{}\n{}: {MIN_TERMINAL_WIDTH}x{MIN_TERMINAL_HEIGHT}\n{}",
                catalog(app.locale(), HumanMessageKey::TerminalTooSmall),
                catalog(app.locale(), HumanMessageKey::MinimumSize),
                catalog(app.locale(), HumanMessageKey::ResizeAndReopen),
            ))
            .style(style)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("TabBeacon")
                    .style(style),
            ),
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
                catalog(app.locale(), HumanMessageKey::ControlCenter),
                style.add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(
                " — {}",
                shared_health_label(app.locale(), app.snapshot.health)
            )),
        ]))
        .style(style),
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
                format!("> {}", screen.localized_title(app.locale()))
            } else {
                format!("  {}", screen.localized_title(app.locale()))
            })
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        List::new(nav).style(style).block(
            Block::default()
                .borders(Borders::ALL)
                .title(catalog(app.locale(), HumanMessageKey::Sections))
                .style(style),
        ),
        body[0],
    );
    let content_title = app
        .overlay
        .title(app.locale())
        .unwrap_or_else(|| app.screen.localized_title(app.locale()))
        .to_owned();
    frame.render_widget(
        content(app).style(style).block(
            Block::default()
                .borders(Borders::ALL)
                .title(content_title)
                .style(style),
        ),
        body[1],
    );
    let footer = if app.overlay.is_open() {
        catalog(app.locale(), HumanMessageKey::OverlayDismiss).to_owned()
    } else if app.confirm_discard {
        catalog(app.locale(), HumanMessageKey::FooterDiscard).to_owned()
    } else if app.has_concurrent_conflict() {
        catalog(app.locale(), HumanMessageKey::RefreshConflict).to_owned()
    } else if app.editing() {
        catalog(app.locale(), HumanMessageKey::FooterEditing).to_owned()
    } else {
        let footer = catalog(app.locale(), HumanMessageKey::FooterNavigation);
        format!(
            "{footer}{}",
            if app.dirty {
                format!(
                    "  • {}",
                    catalog(app.locale(), HumanMessageKey::UnsavedChanges)
                )
            } else {
                String::new()
            }
        )
    };
    frame.render_widget(Paragraph::new(footer).style(style), areas[2]);
}

fn tui_human_style(color: HumanColor, no_color_is_set: bool) -> Style {
    if color_enabled(color, true, no_color_is_set) {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    }
}

fn content(app: &ControlCenterApp) -> Paragraph<'static> {
    if app.overlay.is_open() {
        return Paragraph::new(overlay_lines(app));
    }
    Paragraph::new(match app.screen {
        Screen::Overview => overview_lines(app),
        Screen::Appearance => appearance_lines(app),
        Screen::Workspace => workspace_lines(app),
        Screen::Sessions => sessions_lines(app),
        Screen::Integration => integration_lines(app),
        Screen::Hooks => hooks_lines(app),
        Screen::Diagnostics => diagnostics_lines(app),
        Screen::Interface => interface_lines(app),
        Screen::Preview => preview_lines(app),
    })
}

fn overlay_lines(app: &ControlCenterApp) -> String {
    match &app.overlay {
        ControlCenterOverlay::Help => format!(
            "{}\n\n{}\n{}\n{}\n{}\n{}",
            catalog(app.locale(), HumanMessageKey::HelpNavigation),
            catalog(app.locale(), HumanMessageKey::HelpSettings),
            catalog(app.locale(), HumanMessageKey::HelpWorkspaceSessions),
            catalog(app.locale(), HumanMessageKey::HelpAccessibility),
            catalog(app.locale(), HumanMessageKey::HelpRepair),
            catalog(app.locale(), HumanMessageKey::OverlayDismiss),
        ),
        ControlCenterOverlay::RepairPreview(plan) => {
            let issue = app.snapshot.issues.iter().find(|issue| {
                issue
                    .remediation
                    .as_ref()
                    .is_some_and(|action| action.id == plan.action_id)
            });
            let (title, explanation, instruction) = issue.map_or_else(
                || ("—".to_owned(), "—".to_owned(), "—".to_owned()),
                |issue| {
                    let instruction = issue.remediation.as_ref().map_or_else(
                        || "—".to_owned(),
                        |action| {
                            render_human_text(
                                app.locale(),
                                &management_action_text(
                                    &issue.id,
                                    &action.id,
                                    action.instruction.clone(),
                                ),
                            )
                        },
                    );
                    (
                        render_human_text(
                            app.locale(),
                            &management_text(
                                ManagementTextKind::IssueTitle,
                                &issue.id,
                                issue.title.clone(),
                            ),
                        ),
                        render_human_text(
                            app.locale(),
                            &management_text(
                                ManagementTextKind::IssueExplanation,
                                &issue.id,
                                issue.explanation.clone(),
                            ),
                        ),
                        instruction,
                    )
                },
            );
            format!(
                "{}: {}\n{}: {}\n{}: {}\n{}: {}\n{}: {}\n{}\n{}",
                catalog(app.locale(), HumanMessageKey::WhatIsWrong),
                title,
                catalog(app.locale(), HumanMessageKey::WhyItMatters),
                explanation,
                catalog(app.locale(), HumanMessageKey::RecommendedAction),
                instruction,
                catalog(app.locale(), HumanMessageKey::WhatWillChange),
                catalog(app.locale(), HumanMessageKey::RepairTitleScope),
                catalog(app.locale(), HumanMessageKey::WhatWillNotChange),
                catalog(app.locale(), HumanMessageKey::RepairTitlePreserved),
                catalog(app.locale(), HumanMessageKey::RepairApplyHint),
                catalog(app.locale(), HumanMessageKey::OverlayDismiss),
            )
        }
        ControlCenterOverlay::TitleExplanation => title_explanation_lines(app),
        ControlCenterOverlay::None => String::new(),
    }
}

fn title_explanation_lines(app: &ControlCenterApp) -> String {
    let explanation = &app.title_explanation;
    let workspace = title_explanation_workspace_lines(app);
    let facts = [
        format!(
            "{}: {}",
            catalog(app.locale(), HumanMessageKey::Provider),
            explanation.provider
        ),
        format!(
            "{}: {}",
            catalog(app.locale(), HumanMessageKey::SemanticPhase),
            explanation.semantic_phase
        ),
        format!(
            "{}: {}",
            catalog(app.locale(), HumanMessageKey::Attention),
            explanation.attention
        ),
        format!(
            "{}: {}",
            catalog(app.locale(), HumanMessageKey::ActivityHealth),
            explanation.activity_health
        ),
        format!(
            "{}: {}",
            catalog(app.locale(), HumanMessageKey::ActivityChannel),
            explanation.activity_channel
        ),
        format!(
            "{}: {}",
            catalog(app.locale(), HumanMessageKey::SessionCorrelation),
            explanation.session_correlation
        ),
        format!(
            "{}: {}",
            catalog(app.locale(), HumanMessageKey::TitleOwner),
            explanation.title_owner
        ),
        format!(
            "{}: {}",
            catalog(app.locale(), HumanMessageKey::CodexWriterState),
            explanation.codex_writer_state
        ),
        format!(
            "{}: {}",
            catalog(app.locale(), HumanMessageKey::TitleAuthority),
            explanation.title_authority
        ),
        format!(
            "{}: {}",
            catalog(app.locale(), HumanMessageKey::TitleConflict),
            explanation.title_conflict
        ),
        format!(
            "{}: {}",
            catalog(app.locale(), HumanMessageKey::ProviderBadgePolicy),
            explanation.provider_badge_policy
        ),
        format!(
            "{}: {}",
            catalog(app.locale(), HumanMessageKey::ProviderBadgeValue),
            explanation.provider_badge_value
        ),
    ];
    format!(
        "{}\n\n{}\n\n{}",
        facts.join("\n"),
        workspace,
        catalog(app.locale(), HumanMessageKey::OverlayDismiss),
    )
}

fn title_explanation_workspace_lines(app: &ControlCenterApp) -> String {
    app.title_explanation.workspace.as_ref().map_or_else(
        || {
            format!(
                "{}: unavailable",
                catalog(app.locale(), HumanMessageKey::Workspace)
            )
        },
        |workspace| {
            format!(
                "{}: {}\n{}: {}\n{}: {}\n{}: {}\n{}: {}\n{}: {}\n{}: {}\n{}: {}\n{}: {}\n{}: {}",
                catalog(app.locale(), HumanMessageKey::ProjectDisplayHint),
                workspace.display_hint,
                catalog(app.locale(), HumanMessageKey::IdentityClass),
                workspace.identity_class,
                catalog(app.locale(), HumanMessageKey::RootBindingSource),
                workspace.root_binding_source,
                catalog(app.locale(), HumanMessageKey::RootBindingStatus),
                workspace.root_binding_status,
                catalog(app.locale(), HumanMessageKey::WorkspaceMismatch),
                workspace.workspace_mismatch_observation,
                catalog(app.locale(), HumanMessageKey::AutomaticAlias),
                workspace.automatic_alias,
                catalog(app.locale(), HumanMessageKey::CustomAlias),
                workspace.override_alias.as_deref().unwrap_or("—"),
                catalog(app.locale(), HumanMessageKey::EffectiveAlias),
                workspace.effective_alias,
                catalog(app.locale(), HumanMessageKey::AliasSource),
                workspace.alias_source,
                catalog(app.locale(), HumanMessageKey::NamingPolicy),
                workspace.naming_policy,
            )
        },
    )
}

fn overview_lines(app: &ControlCenterApp) -> String {
    format!(
        "{}: {}\n\n{}  {}\n{}      {} · {}\n{}      {} · {} {}\n{}      {}\n\n{}\n  {}      {}\n  {}  {}\n  {}   {}\n  {}    {}\n  {}      {}\n\n{}    {} · {} {} · {} {}",
        catalog(app.locale(), HumanMessageKey::OverallHealth),
        shared_health_label(app.locale(), app.snapshot.health),
        catalog(app.locale(), HumanMessageKey::TabBeacon),
        app.overview.tabbeacon_version,
        catalog(app.locale(), HumanMessageKey::Codex),
        app.overview.codex_version,
        app.overview.codex_profile,
        catalog(app.locale(), HumanMessageKey::Hooks),
        app.overview.hooks,
        catalog(app.locale(), HumanMessageKey::Trust),
        app.overview.hook_trust,
        catalog(app.locale(), HumanMessageKey::Title),
        app.overview.title_ownership,
        catalog(app.locale(), HumanMessageKey::Presentation),
        catalog(app.locale(), HumanMessageKey::Title),
        human_title(app.locale(), app.current.title()),
        catalog(app.locale(), HumanMessageKey::TabColor),
        human_tab_color(app.locale(), app.current.tab_color()),
        catalog(app.locale(), HumanMessageKey::Activity),
        human_activity(app.locale(), app.current.activity()),
        catalog(app.locale(), HumanMessageKey::Spinner),
        human_spinner(app.locale(), app.current.spinner()),
        catalog(app.locale(), HumanMessageKey::Theme),
        human_theme(app.locale(), app.current.theme()),
        catalog(app.locale(), HumanMessageKey::Workers),
        app.overview.worker_health,
        catalog(app.locale(), HumanMessageKey::Active),
        app.overview.active_workers,
        catalog(app.locale(), HumanMessageKey::Stale),
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
        format!(
            "{marker} {} {value}",
            pad_display_width(catalog(app.locale(), field.message_key()), 12)
        )
    };
    format!(
        "{}\n\n{}\n{}\n{}\n{}\n{}\n{}\n\n{}",
        catalog(app.locale(), HumanMessageKey::DraftAppearance),
        field_line(
            AppearanceField::Title,
            human_title(app.locale(), app.draft.title()).to_owned()
        ),
        field_line(
            AppearanceField::TabColor,
            human_tab_color(app.locale(), app.draft.tab_color()).to_owned()
        ),
        field_line(
            AppearanceField::Activity,
            human_activity(app.locale(), app.draft.activity()).to_owned()
        ),
        field_line(
            AppearanceField::Spinner,
            human_spinner(app.locale(), app.draft.spinner()).to_owned()
        ),
        field_line(
            AppearanceField::Theme,
            human_theme(app.locale(), app.draft.theme()).to_owned()
        ),
        field_line(
            AppearanceField::ProviderBadge,
            human_provider_badge(app.locale(), app.draft.provider_badge()).to_owned()
        ),
        if app.editing() {
            catalog(app.locale(), HumanMessageKey::UseArrowsToChange)
        } else {
            catalog(app.locale(), HumanMessageKey::PressEnterToSelect)
        }
    )
}

fn workspace_lines(app: &ControlCenterApp) -> String {
    let Some(workspace) = app.workspace.as_ref() else {
        return format!(
            "{}\n\n{}",
            catalog(app.locale(), HumanMessageKey::Workspace),
            catalog(app.locale(), HumanMessageKey::NoAutomatedActionAvailable),
        );
    };
    let candidates = workspace
        .candidates()
        .iter()
        .take(4)
        .enumerate()
        .map(|(index, candidate)| {
            format!(
                "  {}. {} · {} · {}",
                index + 1,
                candidate.alias().as_str(),
                candidate.strategy().as_str(),
                candidate.score(),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let guidance = if let Some(editor) = app.workspace_editor.as_deref() {
        format!(
            "{}\n> {editor}",
            catalog(app.locale(), HumanMessageKey::WorkspaceCustomAliasInput)
        )
    } else if app.workspace_explaining {
        catalog(app.locale(), HumanMessageKey::WorkspaceExplain).to_owned()
    } else {
        catalog(app.locale(), HumanMessageKey::WorkspaceActions).to_owned()
    };
    let score_explanation = if app.workspace_explaining {
        workspace.selected_candidate().map_or_else(
            || "unavailable".to_owned(),
            |candidate| {
                let components = candidate.components();
                format!(
                    "{}: {}\n{}: {}\n{}: {}\n{}: {}\n{}: {}\n{}: {}\n{}: {}\n{}: {}\n{}: {}\n{}: {}\n{}: {}",
                    catalog(app.locale(), HumanMessageKey::TokenCoverage), components.token_coverage,
                    catalog(app.locale(), HumanMessageKey::AcronymPreservation), components.acronym_preservation,
                    catalog(app.locale(), HumanMessageKey::RecognizablePrefix), components.recognizable_prefix,
                    catalog(app.locale(), HumanMessageKey::BalancedRepresentation), components.balanced_representation,
                    catalog(app.locale(), HumanMessageKey::DisplayWidth), components.display_width,
                    catalog(app.locale(), HumanMessageKey::InformationLoss), components.information_loss,
                    catalog(app.locale(), HumanMessageKey::TrivialAliasPenalty), components.trivial_alias,
                    catalog(app.locale(), HumanMessageKey::RedundancyPenalty), components.redundancy,
                    catalog(app.locale(), HumanMessageKey::CollisionPressure), components.collision_pressure,
                    catalog(app.locale(), HumanMessageKey::StrategyAdjustment), components.strategy_adjustment,
                    catalog(app.locale(), HumanMessageKey::Total), components.total(),
                )
            },
        )
    } else {
        String::new()
    };
    format!(
        "{}: {}\n{}: {}\n{}: {}\n{}: {}\n{}: {}\n\n{}:\n{}\n\n{}\n{}\n{}",
        catalog(app.locale(), HumanMessageKey::ProjectDisplayHint),
        workspace.workspace().as_str(),
        catalog(app.locale(), HumanMessageKey::AutomaticAlias),
        workspace.automatic_alias().as_str(),
        catalog(app.locale(), HumanMessageKey::EffectiveAlias),
        app.workspace_draft
            .as_deref()
            .unwrap_or_else(|| workspace.automatic_alias().as_str()),
        catalog(app.locale(), HumanMessageKey::CustomAlias),
        app.workspace_draft.as_deref().unwrap_or("—"),
        catalog(app.locale(), HumanMessageKey::NamingPolicy),
        workspace.policy_version(),
        catalog(app.locale(), HumanMessageKey::Candidates),
        if candidates.is_empty() {
            "—"
        } else {
            &candidates
        },
        guidance,
        score_explanation,
        catalog(app.locale(), HumanMessageKey::WorkspaceLocalOnly),
    )
}

fn sessions_lines(app: &ControlCenterApp) -> String {
    let Some(sessions) = app.sessions.as_ref() else {
        return catalog(app.locale(), HumanMessageKey::NoInspectableSessionLeases).to_owned();
    };
    let rows = if sessions.sessions.is_empty() {
        catalog(app.locale(), HumanMessageKey::NoInspectableSessionLeases).to_owned()
    } else {
        sessions
            .sessions
            .iter()
            .take(12)
            .map(|session| {
                format!(
                    "{} — {} — {} — {}s — {}",
                    session.workspace_alias,
                    app.integrations.label_for(&session.provider),
                    session.semantic_state,
                    session.age_seconds,
                    session.worker_health.as_str().replace('_', " "),
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        "{}: {} · {}: {} · {}: {}\n\n{}\n\n{}",
        catalog(app.locale(), HumanMessageKey::Active),
        sessions.active_sessions,
        catalog(app.locale(), HumanMessageKey::Stale),
        sessions.stale_sessions,
        catalog(app.locale(), HumanMessageKey::InvalidLeases),
        sessions.invalid_leases,
        rows,
        catalog(app.locale(), HumanMessageKey::LeaseObservationOnly),
    )
}

fn interface_lines(app: &ControlCenterApp) -> String {
    let field_line = |field: InterfaceField, value: &str| {
        let marker = if app.interface_field == Some(field) {
            ">"
        } else {
            " "
        };
        format!(
            "{marker} {} {value}",
            pad_display_width(catalog(app.locale(), field.message_key()), 12)
        )
    };
    format!(
        "{}\n\n{}\n{}\n{}\n\n{}",
        catalog(app.locale(), HumanMessageKey::DraftInterface),
        field_line(
            InterfaceField::Language,
            human_language(app.locale(), app.interface_draft.language()),
        ),
        field_line(
            InterfaceField::Color,
            human_color(app.locale(), app.interface_draft.color()),
        ),
        field_line(
            InterfaceField::ReducedMotion,
            human_boolean(app.locale(), app.interface_draft.reduced_motion()),
        ),
        if app.editing() {
            catalog(app.locale(), HumanMessageKey::UseArrowsToChange)
        } else {
            catalog(app.locale(), HumanMessageKey::PressEnterToSelect)
        }
    )
}

fn integration_lines(app: &ControlCenterApp) -> String {
    app.integrations
        .providers
        .iter()
        .map(|provider| {
            let version = provider
                .version
                .as_deref()
                .unwrap_or_else(|| catalog(app.locale(), HumanMessageKey::Unavailable));
            let capabilities = provider
                .capability_profile
                .capabilities
                .iter()
                .map(|capability| {
                    format!(
                        "{} {} [{}]",
                        capability.capability.as_str(),
                        capability.availability.as_str(),
                        capability.authority
                    )
                })
                .collect::<Vec<_>>()
                .join(" · ");
            let actions = if provider.manual_actions.is_empty() {
                catalog(app.locale(), HumanMessageKey::NoAutomatedAction).to_owned()
            } else {
                provider
                    .manual_actions
                    .iter()
                    .map(|action| action.as_str())
                    .collect::<Vec<_>>()
                    .join(" · ")
            };
            format!(
                "{} · {}\n{}: {} · {}: {}\n{}: {}\n{}: {}\n{}: {}\n{}: {}",
                provider.label,
                provider.id,
                catalog(app.locale(), HumanMessageKey::Version),
                version,
                catalog(app.locale(), HumanMessageKey::Admission),
                provider.admission.as_str(),
                catalog(app.locale(), HumanMessageKey::ObservationBackend),
                provider.observation_backend,
                catalog(app.locale(), HumanMessageKey::Hooks),
                provider.hooks.as_str(),
                catalog(app.locale(), HumanMessageKey::Capabilities),
                capabilities,
                catalog(app.locale(), HumanMessageKey::ManualActions),
                actions,
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn hooks_lines(app: &ControlCenterApp) -> String {
    app.hooks.human_table(app.locale())
}

fn diagnostics_lines(app: &ControlCenterApp) -> String {
    if app.snapshot.issues.is_empty() {
        return format!(
            "✓ {}\n\n{}",
            catalog(app.locale(), HumanMessageKey::Healthy),
            catalog(app.locale(), HumanMessageKey::NoAutomatedAction)
        );
    }
    let issues = app
        .snapshot
        .issues
        .iter()
        .map(|issue| {
            let next = issue.remediation.as_ref().map_or_else(
                || catalog(app.locale(), HumanMessageKey::NoAutomatedActionAvailable).to_owned(),
                |action| {
                    format!(
                        "{}: {} [{}]",
                        catalog(app.locale(), HumanMessageKey::Next),
                        render_human_text(
                            app.locale(),
                            &management_action_text(
                                &issue.id,
                                &action.id,
                                action.instruction.clone(),
                            ),
                        ),
                        safety_label(app.locale(), action.safety)
                    )
                },
            );
            format!(
                "{} {}\n{}\n{}",
                severity_label(app.locale(), issue.severity),
                render_human_text(
                    app.locale(),
                    &management_text(
                        ManagementTextKind::IssueTitle,
                        &issue.id,
                        issue.title.clone(),
                    ),
                ),
                render_human_text(
                    app.locale(),
                    &management_text(
                        ManagementTextKind::IssueExplanation,
                        &issue.id,
                        issue.explanation.clone(),
                    ),
                ),
                next
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    if app
        .snapshot
        .change_plans
        .iter()
        .any(|plan| plan.safety == ActionSafety::PreviewableSafeRepair)
    {
        format!(
            "{issues}\n\n{}",
            catalog(app.locale(), HumanMessageKey::RepairPreviewHint)
        )
    } else {
        issues
    }
}

fn preview_lines(app: &ControlCenterApp) -> String {
    let renderer =
        WindowsTerminalRenderer::with_settings(WindowsTerminalCapabilities::new(true), app.draft);
    [
        (HumanMessageKey::Ready, Phase::Ready, Attention::None),
        (HumanMessageKey::Working, Phase::Working, Attention::None),
        (
            HumanMessageKey::ResultReady,
            Phase::WaitingUser,
            Attention::ResultReady,
        ),
        (
            HumanMessageKey::Approval,
            Phase::WaitingUser,
            Attention::Approval,
        ),
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
                    || catalog(app.locale(), HumanMessageKey::NativeTitle).to_owned(),
                    |title| title.as_str().to_owned(),
                ),
                format!("{:?}", state.progress()),
            ),
        };
        format!(
            "{} {title} · {progress}",
            pad_display_width(catalog(app.locale(), label), 12)
        )
    })
    .collect::<Vec<_>>()
    .join("\n")
}

fn severity_label(
    locale: ResolvedLocale,
    severity: crate::management::HealthSeverity,
) -> &'static str {
    match severity {
        crate::management::HealthSeverity::Warning => catalog(locale, HumanMessageKey::Attention),
        crate::management::HealthSeverity::Error => catalog(locale, HumanMessageKey::Failure),
    }
}

fn safety_label(locale: ResolvedLocale, safety: ActionSafety) -> &'static str {
    match safety {
        ActionSafety::ReadOnly => catalog(locale, HumanMessageKey::ReadOnly),
        ActionSafety::ManualAction => catalog(locale, HumanMessageKey::ManualAction),
        ActionSafety::PreviewableSafeRepair => catalog(locale, HumanMessageKey::PreviewableRepair),
        ActionSafety::OwnerExplicitRequired => catalog(locale, HumanMessageKey::OwnerApplyRequired),
        ActionSafety::UnsupportedAutomation => catalog(locale, HumanMessageKey::NotAutomated),
    }
}

fn human_title(locale: ResolvedLocale, value: TitleMode) -> &'static str {
    match value {
        TitleMode::TabBeacon => catalog(locale, HumanMessageKey::TabBeacon),
        TitleMode::Native => catalog(locale, HumanMessageKey::Native),
        TitleMode::Off => catalog(locale, HumanMessageKey::Disabled),
    }
}

fn human_tab_color(locale: ResolvedLocale, value: TabColorMode) -> &'static str {
    match value {
        TabColorMode::TabBeacon => catalog(locale, HumanMessageKey::TabBeaconColors),
        TabColorMode::Native => catalog(locale, HumanMessageKey::NativeColors),
        TabColorMode::Off => catalog(locale, HumanMessageKey::Disabled),
    }
}

fn human_activity(locale: ResolvedLocale, value: ActivityMode) -> &'static str {
    match value {
        ActivityMode::TitleSpinner => catalog(locale, HumanMessageKey::TitleSpinner),
        ActivityMode::TitleIndicator => catalog(locale, HumanMessageKey::TitleIndicator),
        ActivityMode::WindowsTerminalRing => catalog(locale, HumanMessageKey::TerminalRing),
        ActivityMode::Both => catalog(locale, HumanMessageKey::TitleSpinnerAndRing),
        ActivityMode::Native => catalog(locale, HumanMessageKey::Native),
        ActivityMode::Off => catalog(locale, HumanMessageKey::Disabled),
    }
}

fn human_spinner(locale: ResolvedLocale, value: SpinnerPreset) -> &'static str {
    match value {
        SpinnerPreset::Codex => catalog(locale, HumanMessageKey::Codex),
        SpinnerPreset::Braille => catalog(locale, HumanMessageKey::BrailleSpinner),
        SpinnerPreset::Quadrant => catalog(locale, HumanMessageKey::QuadrantSpinner),
        SpinnerPreset::Line => catalog(locale, HumanMessageKey::LineSpinner),
        SpinnerPreset::Pulse => catalog(locale, HumanMessageKey::PulseSpinner),
    }
}

fn human_theme(locale: ResolvedLocale, value: crate::settings::PresentationTheme) -> &'static str {
    match value {
        crate::settings::PresentationTheme::MutedDark => {
            catalog(locale, HumanMessageKey::MutedDark)
        }
        crate::settings::PresentationTheme::Classic => {
            catalog(locale, HumanMessageKey::ClassicTheme)
        }
    }
}

fn human_provider_badge(locale: ResolvedLocale, value: ProviderBadgePolicy) -> &'static str {
    match value {
        ProviderBadgePolicy::Auto => catalog(locale, HumanMessageKey::Auto),
        ProviderBadgePolicy::Always => catalog(locale, HumanMessageKey::Always),
        ProviderBadgePolicy::Off => catalog(locale, HumanMessageKey::Disabled),
    }
}

fn human_language(locale: ResolvedLocale, value: InterfaceLanguage) -> &'static str {
    match value {
        InterfaceLanguage::Auto => catalog(locale, HumanMessageKey::Auto),
        InterfaceLanguage::EnUs => catalog(locale, HumanMessageKey::English),
        InterfaceLanguage::ZhCn => catalog(locale, HumanMessageKey::SimplifiedChinese),
    }
}

fn human_color(locale: ResolvedLocale, value: HumanColor) -> &'static str {
    match value {
        HumanColor::Auto => catalog(locale, HumanMessageKey::Auto),
        HumanColor::Always => catalog(locale, HumanMessageKey::Always),
        HumanColor::Never => catalog(locale, HumanMessageKey::Never),
    }
}

fn human_boolean(locale: ResolvedLocale, value: bool) -> &'static str {
    if value {
        catalog(locale, HumanMessageKey::Enabled)
    } else {
        catalog(locale, HumanMessageKey::Disabled)
    }
}

#[cfg(test)]
mod tests {
    use crate::activity::SessionWorkspaceObservability;
    use std::{
        cell::RefCell,
        panic::{AssertUnwindSafe, catch_unwind},
        rc::Rc,
    };

    use super::*;
    use crate::activity::{
        ActivityLeaseHealth, SessionOverview, SessionRecency, SessionWorkerHealth,
        SessionsBoundaries,
    };
    use crate::management::ManagementHealth;
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

    fn refresh(
        presentation: PresentationSettings,
        interface: InterfacePreferences,
    ) -> ControlCenterRefresh {
        ControlCenterRefresh {
            presentation,
            interface,
            snapshot: ManagementSnapshot {
                health: ManagementHealth::Healthy,
                issues: Vec::new(),
                recommended_actions: Vec::new(),
                change_plans: Vec::new(),
            },
            overview: ManagementOverview::default(),
            workspace: None,
            sessions: SessionsOverview::default(),
            hooks: HookInventory::default(),
            integrations: ProviderRegistry::default(),
            title_explanation: TitleExplanation::default(),
        }
    }

    #[test]
    fn read_only_refresh_updates_clean_baselines_without_creating_a_draft() {
        let mut app = app();
        let refreshed = app
            .current()
            .with_theme(crate::settings::PresentationTheme::Classic);

        app.merge_refresh(refresh(refreshed, app.current_interface()));

        assert_eq!(app.current(), refreshed);
        assert_eq!(app.draft(), refreshed);
        assert!(!app.dirty());
        assert!(!app.has_concurrent_conflict());
    }

    #[test]
    fn refresh_preserves_dirty_draft_and_refuses_stale_apply_until_revert() {
        let mut app = app();
        app.screen = Screen::Appearance;
        app.handle_key(KeyCode::Enter);
        app.handle_key(KeyCode::Right);
        let draft = app.draft();
        let externally_changed = app
            .current()
            .with_theme(crate::settings::PresentationTheme::Classic);

        app.merge_refresh(refresh(externally_changed, app.current_interface()));

        assert_eq!(app.draft(), draft, "refresh never overwrites a dirty draft");
        assert!(app.has_concurrent_conflict());
        assert!(matches!(
            app.handle_key(KeyCode::Char('a')),
            ControlCenterCommand::None
        ));

        app.handle_key(KeyCode::Char('r'));
        assert_eq!(app.current(), externally_changed);
        assert_eq!(app.draft(), externally_changed);
        assert!(!app.has_concurrent_conflict());
        assert!(!app.dirty());
    }

    #[test]
    fn workspace_custom_alias_is_staged_and_requires_explicit_apply() {
        let mut app = app();
        app.screen = Screen::Workspace;
        app.handle_key(KeyCode::Char('c'));
        app.handle_key(KeyCode::Char('T'));
        app.handle_key(KeyCode::Char('B'));
        app.handle_key(KeyCode::Enter);

        assert!(app.dirty());
        assert_eq!(
            app.handle_key(KeyCode::Char('a')),
            ControlCenterCommand::ApplyWorkspace {
                before: None,
                after: Some("TB".to_owned()),
            }
        );
        assert!(app.dirty(), "frontend Apply remains a request");
        app.workspace_apply_succeeded();
        assert!(!app.dirty());
    }

    #[test]
    fn sessions_screen_is_localized_and_never_renders_prohibited_fields() {
        let mut app = app().with_interface_preferences(
            InterfacePreferences::default().with_language(InterfaceLanguage::ZhCn),
        );
        app.merge_refresh(ControlCenterRefresh {
            presentation: app.current(),
            interface: app.current_interface(),
            snapshot: ManagementSnapshot {
                health: ManagementHealth::Healthy,
                issues: Vec::new(),
                recommended_actions: Vec::new(),
                change_plans: Vec::new(),
            },
            overview: ManagementOverview::default(),
            workspace: None,
            sessions: SessionsOverview {
                schema_version: 2,
                observation: "ephemeral_lease_snapshot",
                health: ActivityLeaseHealth::Healthy,
                active_sessions: 1,
                stale_sessions: 0,
                invalid_leases: 0,
                sessions: vec![SessionOverview {
                    workspace_alias: "TB".to_owned(),
                    provider: "codex".to_owned(),
                    semantic_state: "working".to_owned(),
                    age_seconds: 3,
                    recency: SessionRecency::JustNow,
                    worker_health: SessionWorkerHealth::RecentlyAuthorized,
                    workspace_observability: SessionWorkspaceObservability::default(),
                }],
                read_only: true,
                boundaries: SessionsBoundaries {
                    raw_native_session_ids: false,
                    prompt_content: false,
                    remote_control: false,
                },
            },
            hooks: HookInventory::default(),
            integrations: ProviderRegistry::default(),
            title_explanation: TitleExplanation::default(),
        });
        app.screen = Screen::Sessions;
        let mut terminal = Terminal::new(TestBackend::new(100, 20)).expect("test terminal starts");
        terminal.draw(|frame| render(frame, &app)).expect("renders");
        let rendered = format!("{:?}", terminal.backend().buffer());

        assert!(rendered.contains("会话"));
        assert!(rendered.contains("TB"));
        assert!(!rendered.contains("native_session"));
        assert!(!rendered.contains("prompt"));
        assert!(!rendered.contains("turn_id"));
    }

    #[test]
    fn integrations_screen_is_localized_compact_and_never_invents_an_unregistered_provider() {
        let mut app = app().with_interface_preferences(
            InterfacePreferences::default().with_language(InterfaceLanguage::ZhCn),
        );
        let mut snapshot = refresh(app.current(), app.current_interface());
        snapshot.integrations =
            ProviderRegistry::codex_observation(Some("0.149.0"), true, true, true);
        app.merge_refresh(snapshot);
        app.screen = Screen::Integration;

        let mut terminal = Terminal::new(TestBackend::new(28, 20)).expect("narrow terminal starts");
        terminal
            .draw(|frame| render(frame, &app))
            .expect("Integrations render");
        let rendered = format!("{:?}", terminal.backend().buffer());

        assert!(rendered.contains("集成"));
        assert!(rendered.contains("Codex"));
        assert!(rendered.contains("phase"));
        assert!(!rendered.contains("Agy"));
        assert!(!rendered.contains("native_session"));
    }

    #[test]
    fn hooks_screen_is_localized_narrow_safe_and_command_redacted() {
        use crate::hook_inventory::{
            HookCurrentness, HookHandlerKind, HookInventoryEntry, HookOwner, HookSourceKind,
            HookTrustState,
        };

        let mut english_app = app();
        let mut snapshot = refresh(english_app.current(), english_app.current_interface());
        snapshot.hooks = HookInventory::available(vec![HookInventoryEntry::new(
            "codex",
            "pre_tool_use",
            HookOwner::TabBeacon,
            true,
            HookTrustState::Trusted,
            HookCurrentness::Current,
            HookSourceKind::ProviderUserGlobal,
            HookHandlerKind::Command,
            Some(1),
            "sha256:fixture",
        )]);
        english_app.merge_refresh(snapshot);
        english_app.screen = Screen::Hooks;
        let mut terminal = Terminal::new(TestBackend::new(24, 12)).expect("narrow terminal starts");
        terminal
            .draw(|frame| render(frame, &english_app))
            .expect("English Hooks render");
        let rendered = format!("{:?}", terminal.backend().buffer());
        assert!(rendered.contains("Provider"));
        assert!(rendered.contains("PreToolUse"));
        assert!(!rendered.contains("powershell.exe"));
        assert!(!rendered.contains("commandWindows"));

        let mut chinese_app = app().with_interface_preferences(
            InterfacePreferences::default().with_language(InterfaceLanguage::ZhCn),
        );
        let mut snapshot = refresh(chinese_app.current(), chinese_app.current_interface());
        snapshot.hooks = HookInventory::available(vec![HookInventoryEntry::new(
            "codex",
            "pre_tool_use",
            HookOwner::TabBeacon,
            true,
            HookTrustState::Trusted,
            HookCurrentness::Current,
            HookSourceKind::ProviderUserGlobal,
            HookHandlerKind::Command,
            Some(1),
            "sha256:fixture",
        )]);
        chinese_app.merge_refresh(snapshot);
        chinese_app.screen = Screen::Hooks;
        let mut terminal =
            Terminal::new(TestBackend::new(24, 12)).expect("Chinese terminal starts");
        terminal
            .draw(|frame| render(frame, &chinese_app))
            .expect("Chinese Hooks render");
        let rendered = format!("{:?}", terminal.backend().buffer());
        assert!(rendered.contains("提供方"));
        assert!(rendered.contains("可信"));
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
    fn control_center_localizes_the_header_overview_and_footer_path() {
        let app = app().with_locale(ResolvedLocale::ZhCn);
        let mut terminal = Terminal::new(TestBackend::new(100, 18)).expect("test terminal starts");
        terminal.draw(|frame| render(frame, &app)).expect("renders");
        let rendered = format!("{:?}", terminal.backend().buffer());

        assert!(rendered.contains("TabBeacon 控制中心"));
        assert!(rendered.contains("概览"));
        assert!(rendered.contains("总体状态"));
        assert!(rendered.contains("↑↓ 导航"));
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
        let before = app.current_draft();
        let after = app.staged_draft();
        assert_eq!(
            app.handle_key(KeyCode::Char('a')),
            ControlCenterCommand::Apply { before, after }
        );
        assert!(app.dirty());
        assert_eq!(app.current_draft(), before);
        app.apply_succeeded();
        assert!(!app.dirty());
        assert_eq!(app.current_draft(), after);
    }

    #[test]
    fn interface_language_changes_the_live_frame_then_revert_and_apply_remain_staged() {
        let mut app = app()
            .with_locale(ResolvedLocale::EnUs)
            .with_interface_preferences(InterfacePreferences::default());
        app.screen = Screen::Interface;
        let before = app.current_draft();

        app.handle_key(KeyCode::Enter);
        app.handle_key(KeyCode::Right);
        assert_eq!(app.interface_draft().language(), InterfaceLanguage::ZhCn);
        assert_eq!(app.locale(), ResolvedLocale::ZhCn);
        assert!(app.dirty());
        let mut terminal = Terminal::new(TestBackend::new(80, 12)).expect("test terminal starts");
        terminal
            .draw(|frame| render(frame, &app))
            .expect("Chinese frame renders");
        let rendered = format!("{:?}", terminal.backend().buffer());
        assert!(rendered.contains("界面草稿"));
        assert!(rendered.contains("简体中文"));

        app.handle_key(KeyCode::Char('r'));
        assert_eq!(app.current_draft(), before);
        assert_eq!(app.locale(), ResolvedLocale::EnUs);
        assert!(!app.dirty());

        app.handle_key(KeyCode::Enter);
        app.handle_key(KeyCode::Right);
        assert!(matches!(
            app.handle_key(KeyCode::Char('a')),
            ControlCenterCommand::Apply { before: requested_before, after }
                if requested_before == before
                    && after.interface.language() == InterfaceLanguage::ZhCn
                    && after.presentation == before.presentation
        ));
        assert_eq!(
            app.current_draft(),
            before,
            "request does not persist itself"
        );
    }

    #[test]
    fn interface_color_and_reduced_motion_cycle_without_changing_domain_settings() {
        let mut app = app();
        let presentation = app.current();
        app.screen = Screen::Interface;
        app.handle_key(KeyCode::Enter);
        app.handle_key(KeyCode::Down);
        for expected in [HumanColor::Always, HumanColor::Never, HumanColor::Auto] {
            app.handle_key(KeyCode::Right);
            assert_eq!(app.interface_draft().color(), expected);
        }
        app.handle_key(KeyCode::Down);
        app.handle_key(KeyCode::Right);
        assert!(app.interface_draft().reduced_motion());
        assert_eq!(app.draft(), presentation);
        assert!(app.dirty());
    }

    #[test]
    fn interface_color_changes_the_live_tui_style_without_persistence() {
        assert_eq!(
            tui_human_style(HumanColor::Never, false),
            Style::default(),
            "never disables the TUI accent"
        );
        assert_eq!(
            tui_human_style(HumanColor::Auto, true),
            Style::default(),
            "NO_COLOR disables automatic terminal styling"
        );
        assert_ne!(
            tui_human_style(HumanColor::Always, false),
            Style::default(),
            "always enables the visible TUI accent"
        );

        let mut app = app().with_interface_preferences(
            InterfacePreferences::default().with_color(HumanColor::Always),
        );
        let before = app.current_interface();
        let mut terminal = Terminal::new(TestBackend::new(80, 12)).expect("test terminal starts");
        terminal.draw(|frame| render(frame, &app)).expect("renders");
        assert!(
            terminal
                .backend()
                .buffer()
                .content
                .iter()
                .any(|cell| cell.fg == Color::Cyan),
            "the current frame receives the staged Always color"
        );

        app.interface_draft = app.interface_draft.with_color(HumanColor::Never);
        terminal
            .draw(|frame| render(frame, &app))
            .expect("rerenders");
        assert!(
            terminal
                .backend()
                .buffer()
                .content
                .iter()
                .all(|cell| cell.fg != Color::Cyan),
            "the current frame removes the accent immediately for Never"
        );
        assert_eq!(
            app.current_interface(),
            before,
            "rendering never persists a draft"
        );
    }

    #[test]
    fn every_screen_renders_a_narrow_chinese_frame_without_broken_borders() {
        let mut app = app()
            .with_locale(ResolvedLocale::EnUs)
            .with_interface_preferences(
                InterfacePreferences::default().with_language(InterfaceLanguage::ZhCn),
            );
        for _ in 0..Screen::ALL.len() {
            let mut terminal =
                Terminal::new(TestBackend::new(24, 12)).expect("test terminal starts");
            terminal
                .draw(|frame| render(frame, &app))
                .expect("narrow Chinese frame renders");
            let rendered = format!("{:?}", terminal.backend().buffer());
            assert!(rendered.contains("TabBeacon"));
            app.handle_key(KeyCode::Down);
        }
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
    fn known_management_diagnostics_render_from_the_chinese_catalog() {
        let mut app = ControlCenterApp::new(
            PresentationSettings::default(),
            ManagementSnapshot {
                health: ManagementHealth::Warning,
                issues: vec![crate::management::HealthIssue {
                    id: "integration.not_installed".to_owned(),
                    severity: crate::management::HealthSeverity::Warning,
                    title: "Codex integration is not installed".to_owned(),
                    explanation: "TabBeacon did not find an owned Codex integration.".to_owned(),
                    remediation: Some(crate::management::RecommendedAction {
                        id: "integration.setup_codex".to_owned(),
                        title: "Install Codex integration".to_owned(),
                        instruction: "Run tabbeacon setup codex.".to_owned(),
                        safety: ActionSafety::ManualAction,
                    }),
                }],
                recommended_actions: Vec::new(),
                change_plans: Vec::new(),
            },
            ManagementOverview::default(),
        )
        .with_locale(ResolvedLocale::ZhCn);
        app.screen = Screen::Diagnostics;
        let mut terminal = Terminal::new(TestBackend::new(100, 16)).expect("test terminal starts");
        terminal
            .draw(|frame| render(frame, &app))
            .expect("Chinese diagnostics render");
        let rendered = format!("{:?}", terminal.backend().buffer());
        assert!(rendered.contains("尚未安装 TabBeacon 集成"));
        assert!(rendered.contains("请运行 tabbeacon setup codex"));
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

    #[test]
    fn help_overlay_is_localized_event_isolated_and_draft_lossless() {
        let mut app = app().with_interface_preferences(
            InterfacePreferences::default().with_language(InterfaceLanguage::ZhCn),
        );
        app.screen = Screen::Appearance;
        app.handle_key(KeyCode::Enter);
        app.handle_key(KeyCode::Right);
        let draft = app.staged_draft();

        assert_eq!(
            app.handle_key(KeyCode::Char('?')),
            ControlCenterCommand::None
        );
        assert!(app.overlay_open());
        let _ = app.handle_key(KeyCode::Down);
        assert_eq!(
            app.screen(),
            Screen::Appearance,
            "overlay blocks page navigation"
        );
        assert_eq!(app.staged_draft(), draft, "help never changes a draft");

        let mut terminal = Terminal::new(TestBackend::new(100, 20)).expect("terminal starts");
        terminal
            .draw(|frame| render(frame, &app))
            .expect("help renders");
        let rendered = format!("{:?}", terminal.backend().buffer());
        assert!(rendered.contains("帮助"));
        assert!(rendered.contains("Hook 信任"));

        assert_eq!(
            app.handle_event(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)),
            ControlCenterCommand::None,
            "overlay dismissal must not fall through to global quit"
        );
        assert!(!app.overlay_open());
        let _ = app.handle_key(KeyCode::Char('?'));
        assert_eq!(app.handle_key(KeyCode::Esc), ControlCenterCommand::None);
        assert!(!app.overlay_open());
        assert_eq!(app.staged_draft(), draft, "dismiss remains lossless");
    }

    #[test]
    fn why_this_title_overlay_is_localized_read_only_and_draft_lossless() {
        let mut app = app().with_interface_preferences(
            InterfacePreferences::default().with_language(InterfaceLanguage::ZhCn),
        );
        app.screen = Screen::Appearance;
        app.handle_key(KeyCode::Enter);
        app.handle_key(KeyCode::Right);
        let draft = app.staged_draft();

        assert_eq!(
            app.handle_key(KeyCode::Char('t')),
            ControlCenterCommand::None
        );
        assert!(app.overlay_open());
        assert_eq!(
            app.staged_draft(),
            draft,
            "title explanation cannot alter a draft"
        );
        let mut terminal = Terminal::new(TestBackend::new(44, 22)).expect("terminal starts");
        terminal
            .draw(|frame| render(frame, &app))
            .expect("title explanation renders on a narrow terminal");
        let rendered = format!("{:?}", terminal.backend().buffer());
        assert!(rendered.contains("为何使用此标题"));
        assert!(rendered.contains("提供方"));
        assert!(!rendered.contains("C:\\Users"));

        assert_eq!(
            app.handle_event(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)),
            ControlCenterCommand::None,
            "overlay dismissal must not fall through to global quit"
        );
        assert!(!app.overlay_open());
        assert_eq!(app.staged_draft(), draft, "dismiss remains lossless");
    }

    #[test]
    fn only_previewable_repair_can_request_apply_after_preview_and_cancel_is_lossless() {
        let title_action = crate::management::RecommendedAction {
            id: "terminal.title_policy_repair".to_owned(),
            title: "Preview title policy repair".to_owned(),
            instruction: "Inspect the scoped repair first.".to_owned(),
            safety: ActionSafety::PreviewableSafeRepair,
        };
        let manual_action = crate::management::RecommendedAction {
            id: "hooks.review_in_codex".to_owned(),
            title: "Review hooks in Codex".to_owned(),
            instruction: "Launch codex and open /hooks.".to_owned(),
            safety: ActionSafety::ManualAction,
        };
        let mut app = ControlCenterApp::new(
            PresentationSettings::default(),
            ManagementSnapshot {
                health: ManagementHealth::Warning,
                issues: vec![
                    crate::management::HealthIssue {
                        id: "terminal.title_repair_available".to_owned(),
                        severity: crate::management::HealthSeverity::Warning,
                        title: title_action.title.clone(),
                        explanation: "One active profile is safely scoped.".to_owned(),
                        remediation: Some(title_action.clone()),
                    },
                    crate::management::HealthIssue {
                        id: "hooks.review_required".to_owned(),
                        severity: crate::management::HealthSeverity::Warning,
                        title: manual_action.title.clone(),
                        explanation: "Trust remains manual.".to_owned(),
                        remediation: Some(manual_action),
                    },
                ],
                recommended_actions: vec![title_action.clone()],
                change_plans: vec![ChangePlan {
                    action_id: title_action.id.clone(),
                    safety: ActionSafety::PreviewableSafeRepair,
                    proposed_changes: vec!["active profile only".to_owned()],
                    protected_state: vec!["unrelated settings".to_owned()],
                    manual_follow_up: Vec::new(),
                }],
            },
            ManagementOverview::default(),
        );
        app.screen = Screen::Diagnostics;
        let draft = app.staged_draft();

        assert_eq!(
            app.handle_key(KeyCode::Char('p')),
            ControlCenterCommand::None
        );
        assert!(app.overlay_open());
        assert_eq!(app.handle_key(KeyCode::Esc), ControlCenterCommand::None);
        assert!(!app.overlay_open());
        assert_eq!(app.staged_draft(), draft, "repair cancel is lossless");

        let _ = app.handle_key(KeyCode::Char('p'));
        assert_eq!(
            app.handle_key(KeyCode::Char('a')),
            ControlCenterCommand::ApplyRepair {
                action_id: "terminal.title_policy_repair".to_owned(),
            }
        );
        assert!(!app.overlay_open());
        assert_eq!(
            app.staged_draft(),
            draft,
            "repair request cannot alter drafts"
        );
    }

    #[test]
    fn action_safety_labels_and_monochrome_focus_remain_textual() {
        let mut app = app();
        app.screen = Screen::Interface;
        app.handle_key(KeyCode::Enter);
        let mut terminal = Terminal::new(TestBackend::new(80, 14)).expect("terminal starts");
        terminal.draw(|frame| render(frame, &app)).expect("renders");
        let rendered = format!("{:?}", terminal.backend().buffer());
        assert!(rendered.contains('>'), "focus is not color-only");
        assert_eq!(tui_human_style(HumanColor::Never, false), Style::default());
        assert_eq!(
            safety_label(ResolvedLocale::EnUs, ActionSafety::ReadOnly),
            "Read only"
        );
        assert_eq!(
            safety_label(ResolvedLocale::EnUs, ActionSafety::ManualAction),
            "Manual action"
        );
        assert_eq!(
            safety_label(ResolvedLocale::EnUs, ActionSafety::PreviewableSafeRepair),
            "Previewable repair"
        );
        assert_eq!(
            safety_label(ResolvedLocale::EnUs, ActionSafety::OwnerExplicitRequired),
            "Owner apply required"
        );
        assert_eq!(
            safety_label(ResolvedLocale::EnUs, ActionSafety::UnsupportedAutomation),
            "Not automated"
        );
    }
}
