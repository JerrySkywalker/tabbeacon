//! Product action identities and execution; key parsing belongs to TUC.
use super::{
    CliTarget, ControlCenterApp, ControlCenterCommand, ControlCenterOverlay, DiscardDecision,
    Event, KeyCode, KeyEvent, KeyModifiers, QuitDisposition, Screen,
};
use terminal_ui_contract::{
    document::ViewState,
    focus::{FocusPath, FocusState},
    keymap::{
        Action, ActionDescriptor, ActionEffect, ActionRegistry, Binding, Capture,
        Direction as MoveDirection, InputMode, KeymapProfile, KeymapRegistry, NavigationPolicy,
        Resolution, ResolveContext, Resolver, SystemClock,
    },
    viewport::BoundedViewport,
};
use terminal_ui_ratatui::geometry::{HitMap, PointerEffect};

pub(super) struct Interaction {
    pub map: KeymapRegistry,
    resolver: Resolver<SystemClock>,
    pub mode: InputMode,
    pub focus: FocusState,
    pub hitmap: HitMap,
    pub generation: u64,
    pub nav: BoundedViewport,
    pub details: BoundedViewport,
    pub overlay: BoundedViewport,
    pub view: ViewState,
    pub keymap_error: Option<String>,
    modal_restore_visible: Option<Vec<FocusPath>>,
}
impl Clone for Interaction {
    fn clone(&self) -> Self {
        Self {
            map: self.map.clone(),
            resolver: Resolver::new(SystemClock::default()),
            mode: self.mode,
            focus: self.focus.clone(),
            hitmap: self.hitmap.clone(),
            generation: self.generation,
            nav: self.nav,
            details: self.details,
            overlay: self.overlay,
            view: self.view,
            keymap_error: self.keymap_error.clone(),
            modal_restore_visible: self.modal_restore_visible.clone(),
        }
    }
}
impl std::fmt::Debug for Interaction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Interaction")
            .field("profile", &self.map.profile)
            .field("focus", &self.focus)
            .field("mode", &self.mode)
            .finish_non_exhaustive()
    }
}
impl Default for Interaction {
    fn default() -> Self {
        let mut focus = FocusState::default();
        focus.reconcile(vec![nav_path(), details_path()]);
        Self {
            map: keymap(KeymapProfile::Standard, None).expect("valid product bindings"),
            resolver: Resolver::new(SystemClock::default()),
            mode: InputMode::Normal,
            focus,
            hitmap: HitMap::new(0, vec![]),
            generation: 0,
            nav: BoundedViewport::default(),
            details: BoundedViewport::default(),
            overlay: BoundedViewport::default(),
            view: ViewState::default(),
            keymap_error: None,
            modal_restore_visible: None,
        }
    }
}
impl Interaction {
    pub(super) fn sync_modal(&mut self, modal: bool) {
        let changed = if modal && self.focus.modal_root().is_none() {
            self.modal_restore_visible = Some(self.focus.visible().to_vec());
            self.focus.push_modal(modal_path());
            self.overlay.home();
            true
        } else if !modal && self.focus.modal_root().is_some() {
            if let Some(visible) = self.modal_restore_visible.take() {
                self.focus.reconcile(visible);
            }
            self.focus.pop_modal();
            true
        } else {
            false
        };
        if changed {
            self.resolver.cancel();
            self.generation = self.generation.wrapping_add(1);
            self.hitmap = HitMap::new(self.generation, vec![]);
        }
    }
}
pub(super) fn nav_path() -> FocusPath {
    FocusPath::new(&["control", "navigation"])
}
pub(super) fn details_path() -> FocusPath {
    FocusPath::new(&["control", "details"])
}
pub(super) fn modal_path() -> FocusPath {
    FocusPath::new(&["modal"])
}

#[derive(Clone, Copy, Debug)]
enum BusinessAction {
    Apply,
    Revert,
    ExplainTitle,
    RepairPreview,
    ProviderPreview,
    ProviderReset,
    Provider(u8),
    DiagnosticProvider(u8),
    DiagnosticMode,
    WorkspaceClear,
    WorkspaceEdit,
    WorkspaceExplain,
    WorkspaceCandidate(u8),
    Discard,
    Keep,
    Dismiss,
    ConfirmApply,
}
impl BusinessAction {
    fn id(self) -> String {
        format!(
            "tabbeacon.{}",
            match self {
                Self::Apply => "apply".into(),
                Self::Revert => "revert".into(),
                Self::ExplainTitle => "title".into(),
                Self::RepairPreview => "repair.preview".into(),
                Self::ProviderPreview => "provider.preview".into(),
                Self::ProviderReset => "provider.reset".into(),
                Self::Provider(n) => format!("provider.{n}"),
                Self::DiagnosticProvider(n) => format!("diagnostics.provider.{n}"),
                Self::DiagnosticMode => "diagnostics.mode".into(),
                Self::WorkspaceClear => "workspace.clear".into(),
                Self::WorkspaceEdit => "workspace.edit".into(),
                Self::WorkspaceExplain => "workspace.explain".into(),
                Self::WorkspaceCandidate(n) => format!("workspace.candidate.{n}"),
                Self::Discard => "discard".into(),
                Self::Keep => "keep".into(),
                Self::Dismiss => "dismiss".into(),
                Self::ConfirmApply => "confirm.apply".into(),
            }
        )
    }
}
#[allow(clippy::too_many_lines)] // One table binds product action identities to their owning contexts.
fn declarations(
    profile: KeymapProfile,
) -> (
    Vec<ActionDescriptor>,
    Vec<Binding>,
    Vec<(String, BusinessAction)>,
) {
    let mut actions = vec![];
    let mut bindings = vec![];
    let mut lookup = vec![];
    let mut add = |action: BusinessAction, contexts: &[&str], keys: &[&str], effect| {
        let id = action.id();
        if !lookup.iter().any(|(existing, _)| existing == &id) {
            actions.push(ActionDescriptor {
                id: id.clone(),
                label: id.trim_start_matches("tabbeacon.").replace('.', " "),
                action: Action::Product(id.clone()),
                effect,
                contexts: contexts.iter().map(|s| (*s).into()).collect(),
            });
            lookup.push((id.clone(), action));
        }
        for context in contexts {
            for key in keys {
                bindings.push(Binding::new(context, &[*key], &id).expect("product chord"));
            }
        }
    };
    add(
        BusinessAction::Apply,
        &["global"],
        &["a"],
        ActionEffect::ProductWriteRequest,
    );
    add(
        BusinessAction::Revert,
        &["global"],
        &["r"],
        ActionEffect::View,
    );
    add(
        BusinessAction::ExplainTitle,
        &["global"],
        &["t"],
        ActionEffect::View,
    );
    add(
        BusinessAction::RepairPreview,
        &["integrations", "diagnostics"],
        &["p"],
        ActionEffect::View,
    );
    add(
        BusinessAction::ProviderPreview,
        &["integrations"],
        &["v"],
        ActionEffect::View,
    );
    add(
        BusinessAction::ProviderReset,
        &["integrations"],
        &["r"],
        ActionEffect::View,
    );
    add(
        BusinessAction::DiagnosticMode,
        &["diagnostics"],
        &["m"],
        ActionEffect::ReadOnlyQuery,
    );
    for n in 1..=3 {
        let key = if profile == KeymapProfile::Vim {
            format!("Alt+{n}")
        } else {
            n.to_string()
        };
        add(
            BusinessAction::Provider(n),
            &["integrations"],
            &[&key],
            ActionEffect::ReadOnlyQuery,
        );
        add(
            BusinessAction::DiagnosticProvider(n),
            &["diagnostics"],
            &[&key],
            ActionEffect::ReadOnlyQuery,
        );
    }
    add(
        BusinessAction::WorkspaceClear,
        &["workspace"],
        &["d", "x"],
        ActionEffect::View,
    );
    add(
        BusinessAction::WorkspaceEdit,
        &["workspace"],
        &["c"],
        ActionEffect::View,
    );
    add(
        BusinessAction::WorkspaceExplain,
        &["workspace"],
        &["e"],
        ActionEffect::View,
    );
    for n in 1..=4 {
        let key = if profile == KeymapProfile::Vim {
            format!("Alt+{n}")
        } else {
            n.to_string()
        };
        add(
            BusinessAction::WorkspaceCandidate(n),
            &["workspace"],
            &[&key],
            ActionEffect::View,
        );
    }
    add(
        BusinessAction::Discard,
        &["confirm"],
        &["d"],
        ActionEffect::View,
    );
    add(
        BusinessAction::Keep,
        &["confirm"],
        &["k", "q"],
        ActionEffect::View,
    );
    add(
        BusinessAction::Dismiss,
        &["help", "title", "repair_preview", "provider_preview"],
        &["q"],
        ActionEffect::View,
    );
    // Local alternative dismissal keys; the same action explicitly owns each context.
    add(
        BusinessAction::ConfirmApply,
        &["repair_preview", "provider_preview"],
        &["a"],
        ActionEffect::ProductWriteRequest,
    );
    for (context, key) in [
        ("help", "?"),
        ("title", "t"),
        ("repair_preview", "p"),
        ("provider_preview", "v"),
    ] {
        bindings.push(Binding::new(context, &[key], &BusinessAction::Dismiss.id()).unwrap());
    }
    (actions, bindings, lookup)
}
fn keymap(
    profile: KeymapProfile,
    text: Option<&str>,
) -> Result<KeymapRegistry, terminal_ui_contract::keymap::KeymapError> {
    // A custom preset determines product count-safe digit bindings as well.
    let profile = if let Some(text) = text {
        let document = text
            .parse::<toml_edit::DocumentMut>()
            .map_err(|_| terminal_ui_contract::keymap::KeymapError("invalid keymap TOML".into()))?;
        match document.get("preset") {
            None => KeymapProfile::Standard,
            Some(item) => match item.as_str() {
                Some("standard") => KeymapProfile::Standard,
                Some("vim") => KeymapProfile::Vim,
                _ => {
                    return Err(terminal_ui_contract::keymap::KeymapError(
                        "unsupported keymap preset".into(),
                    ));
                }
            },
        }
    } else {
        profile
    };
    let (product, bindings, _) = declarations(profile);
    let map = if let Some(text) = text {
        KeymapRegistry::from_toml(text, product, bindings)?
    } else {
        let mut actions = ActionRegistry::standard(NavigationPolicy::Pane);
        for action in product {
            actions.register_product(action)?;
        }
        KeymapRegistry::new(profile, NavigationPolicy::Pane, actions, bindings)?
    };
    map.with_modal_write_requests(&["repair_preview", "provider_preview"])
}

impl ControlCenterApp {
    pub(super) fn effective_hints_text(&self, context: &ResolveContext) -> String {
        self.interaction
            .borrow()
            .map
            .effective_hints(context)
            .iter()
            .map(|hint| format!("{} — {}", hint.keys, hint_label(hint, self.locale())))
            .collect::<Vec<_>>()
            .join("\n")
    }
    pub(super) fn input_view_fingerprint(&self) -> String {
        let i = self.interaction.borrow();
        format!(
            "{:?}",
            (
                (
                    self.screen,
                    self.current_draft(),
                    self.staged_draft(),
                    &self.workspace_editor,
                    &self.workspace_draft,
                    self.workspace_explaining,
                    &self.overlay,
                    self.confirm_discard,
                    self.appearance_field,
                    self.interface_field
                ),
                (
                    self.selected_provider,
                    self.selected_provider_mode,
                    self.diagnostic_provider,
                    self.diagnostic_mode,
                    i.focus.current(),
                    i.mode,
                    i.view,
                    i.nav,
                    i.details,
                    i.overlay
                )
            )
        )
    }
    /// Sets a session-only profile; never persists user configuration.
    ///
    /// # Errors
    /// Returns an error if product declarations conflict with the shared profile.
    pub fn set_keymap_profile(
        &mut self,
        profile: KeymapProfile,
    ) -> Result<(), terminal_ui_contract::keymap::KeymapError> {
        self.interaction.get_mut().map = keymap(profile, None)?;
        self.interaction.get_mut().resolver.cancel();
        Ok(())
    }
    /// Validates an explicitly supplied user file. Error retains the accepted map.
    ///
    /// # Errors
    /// Returns shared validation errors for unsupported or conflicting bindings.
    pub fn load_keymap_toml(
        &mut self,
        text: &str,
    ) -> Result<(), terminal_ui_contract::keymap::KeymapError> {
        match keymap(KeymapProfile::Standard, Some(text)) {
            Ok(map) => {
                let i = self.interaction.get_mut();
                i.map = map;
                i.resolver.cancel();
                i.keymap_error = None;
                Ok(())
            }
            Err(error) => {
                self.interaction.get_mut().keymap_error =
                    Some("Invalid keymap; accepted map retained / 键位无效，保留原映射".into());
                Err(error)
            }
        }
    }
    pub(super) fn input_context(&self) -> ResolveContext {
        let owner = if self.confirm_discard {
            "confirm"
        } else {
            match self.overlay {
                ControlCenterOverlay::Help => "help",
                ControlCenterOverlay::TitleExplanation => "title",
                ControlCenterOverlay::RepairPreview(_) => "repair_preview",
                ControlCenterOverlay::ProviderPreview { .. } => "provider_preview",
                ControlCenterOverlay::None => {
                    if self.workspace_editor.is_some() {
                        "editor"
                    } else {
                        match self.screen {
                            Screen::Integration => "integrations",
                            Screen::Diagnostics => "diagnostics",
                            Screen::Workspace => "workspace",
                            _ => "details",
                        }
                    }
                }
            }
        };
        let i = self.interaction.borrow();
        let mut context = ResolveContext::normal(&[owner]);
        context.mode = i.mode;
        context.editable = self.workspace_editor.is_some()
            || (i.map.profile == KeymapProfile::Vim
                && !self.overlay.is_open()
                && !self.confirm_discard);
        context.capture = if self.confirm_discard {
            Capture::Confirm
        } else if self.overlay.is_open() {
            Capture::Overlay
        } else if self.workspace_editor.is_some() {
            Capture::Editor
        } else {
            Capture::Normal
        };
        context.focus_token = (Screen::ALL
            .iter()
            .position(|s| *s == self.screen)
            .unwrap_or(0) as u64)
            * 10000
            + i.focus.revision();
        context
    }
    /// Convenience fixture entry; uses the same shared normalizer and resolver.
    pub fn handle_key(&mut self, key: KeyCode) -> ControlCenterCommand {
        self.handle_event(KeyEvent::new(key, KeyModifiers::NONE))
    }
    pub fn handle_event(&mut self, key: KeyEvent) -> ControlCenterCommand {
        self.handle_input(Event::Key(key))
    }
    /// Unified keyboard/paste/mouse input. Focus transitions never execute actions.
    pub fn handle_input(&mut self, event: Event) -> ControlCenterCommand {
        use terminal_ui_contract::input::InputEvent;
        let modal =
            self.confirm_discard || self.overlay.is_open() || self.workspace_editor.is_some();
        self.interaction.get_mut().sync_modal(modal);
        let normalized = terminal_ui_ratatui::input::normalize(event);
        if let InputEvent::Mouse(mouse) = normalized {
            let modal =
                self.confirm_discard || self.overlay.is_open() || self.workspace_editor.is_some();
            let i = self.interaction.get_mut();
            if modal && i.focus.modal_root().is_none() {
                return ControlCenterCommand::None;
            }
            match i.hitmap.pointer(mouse, i.generation, &mut i.focus) {
                PointerEffect::Scroll { path, lines } if modal && path.is_within(&modal_path()) => {
                    i.overlay.lines(lines);
                }
                PointerEffect::Scroll { path, lines } if path.is_within(&details_path()) => {
                    i.details.lines(lines);
                }
                PointerEffect::Scroll { path, lines } if path.is_within(&nav_path()) => {
                    i.nav.lines(lines);
                }
                PointerEffect::Focus(_) if i.focus.current() == Some(&nav_path()) => {
                    self.appearance_field = None;
                    self.interface_field = None;
                }
                _ => {}
            }
            i.resolver.cancel();
            return ControlCenterCommand::None;
        }
        if matches!(normalized, InputEvent::Resize { .. }) {
            let i = self.interaction.get_mut();
            i.generation = i.generation.wrapping_add(1);
            i.hitmap = HitMap::new(i.generation, vec![]);
            i.resolver.cancel();
            return ControlCenterCommand::None;
        }
        let context = self.input_context();
        let resolution = {
            let i = self.interaction.get_mut();
            i.resolver.resolve(&i.map, &context, &normalized)
        };
        let command = match resolution {
            Resolution::Intent(intent) => self.execute_intent(intent.action, intent.count),
            Resolution::Text(text) => {
                if let Some(editor) = self.workspace_editor.as_mut() {
                    for c in text.chars().filter(|c| !c.is_control()) {
                        if editor.chars().count() < 20 {
                            editor.push(c);
                        }
                    }
                }
                ControlCenterCommand::None
            }
            Resolution::EditorKey(key) => {
                use terminal_ui_contract::input::KeyCode as Code;
                if !key.chord.modifiers.is_text() {
                    return ControlCenterCommand::None;
                }
                match key.chord.code {
                    Code::Enter => {
                        let submitted = self.workspace_editor.take().unwrap_or_default();
                        self.workspace_draft = (!submitted.trim().is_empty()).then_some(submitted);
                        self.update_dirty();
                        self.interaction.get_mut().mode = InputMode::Normal;
                    }
                    Code::Backspace => {
                        if let Some(editor) = self.workspace_editor.as_mut() {
                            editor.pop();
                        }
                    }
                    _ => {}
                }
                ControlCenterCommand::None
            }
            _ => ControlCenterCommand::None,
        };
        let modal =
            self.confirm_discard || self.overlay.is_open() || self.workspace_editor.is_some();
        self.interaction.get_mut().sync_modal(modal);
        command
    }
    #[allow(clippy::too_many_lines)] // Typed shared intents dispatch to product boundaries without a second key parser.
    fn execute_intent(&mut self, action: Action, count: u32) -> ControlCenterCommand {
        // Shared View intent is not permission to run a background product reducer.
        if (self.overlay.is_open() || self.confirm_discard)
            && matches!(
                action,
                Action::Move(MoveDirection::Left | MoveDirection::Right)
                    | Action::MoveScreen(_)
                    | Action::Open
                    | Action::FocusNext
                    | Action::FocusPrevious
                    | Action::EnterInsert
            )
        {
            return ControlCenterCommand::None;
        }
        let count = isize::try_from(count).unwrap_or(9999);
        match action {
            Action::RequestQuit => {
                if self.overlay.is_open() {
                    self.overlay = ControlCenterOverlay::None;
                } else if self.confirm_discard {
                    self.keep_draft();
                } else if self.dirty {
                    self.confirm_discard = matches!(
                        self.discard_editor.request_quit(true),
                        QuitDisposition::ConfirmDiscard
                    );
                } else {
                    return ControlCenterCommand::Quit;
                }
            }
            Action::Cancel => {
                let i = self.interaction.get_mut();
                if i.mode == InputMode::Insert {
                    i.mode = InputMode::Normal;
                } else if self.confirm_discard {
                    self.keep_draft();
                } else if self.overlay.is_open() {
                    self.overlay = ControlCenterOverlay::None;
                } else if self.workspace_editor.is_some() {
                    self.workspace_editor = None;
                } else if self.appearance_field.is_some() {
                    self.toggle_appearance_focus();
                } else if self.interface_field.is_some() {
                    self.toggle_interface_focus();
                }
            }
            Action::Help => self.overlay = ControlCenterOverlay::Help,
            Action::Refresh => return ControlCenterCommand::Refresh,
            Action::ToggleDensity => {
                let i = self.interaction.get_mut();
                i.view.toggle();
                i.details.home();
            }
            Action::EnterInsert => self.interaction.get_mut().mode = InputMode::Insert,
            Action::FocusNext | Action::FocusPrevious => {
                self.interaction
                    .get_mut()
                    .focus
                    .cycle(action == Action::FocusPrevious);
                if self.interaction.borrow().focus.current() == Some(&nav_path()) {
                    self.appearance_field = None;
                    self.interface_field = None;
                }
            }
            Action::Move(MoveDirection::Up | MoveDirection::Down) => {
                let delta = if action == Action::Move(MoveDirection::Up) {
                    -count
                } else {
                    count
                };
                if self.overlay.is_open() || self.confirm_discard {
                    self.interaction.get_mut().overlay.lines(delta);
                } else if self.appearance_field.is_some()
                    || self.interface_field.is_some()
                    || self.interaction.get_mut().focus.current() == Some(&nav_path())
                {
                    self.step(delta);
                } else {
                    self.interaction.get_mut().details.lines(delta);
                }
            }
            Action::Move(MoveDirection::Left | MoveDirection::Right) => {
                let delta = if action == Action::Move(MoveDirection::Left) {
                    -count
                } else {
                    count
                };
                if self.screen == Screen::Integration {
                    self.cycle_provider_mode(delta);
                } else {
                    self.change_focused(delta);
                }
            }
            Action::MoveScreen(delta) => self.step(isize::from(delta) * count),
            Action::Page(delta) | Action::HalfPage(delta) => {
                let half = matches!(action, Action::HalfPage(_));
                let i = self.interaction.get_mut();
                let v = if self.overlay.is_open() || self.confirm_discard {
                    &mut i.overlay
                } else if i.focus.current() == Some(&nav_path()) {
                    &mut i.nav
                } else {
                    &mut i.details
                };
                if half {
                    v.half_page(isize::from(delta) * count);
                } else {
                    v.page(isize::from(delta) * count);
                }
            }
            Action::Home | Action::End => {
                let i = self.interaction.get_mut();
                let v = if self.overlay.is_open() || self.confirm_discard {
                    &mut i.overlay
                } else if i.focus.current() == Some(&nav_path()) {
                    &mut i.nav
                } else {
                    &mut i.details
                };
                if action == Action::Home {
                    v.home();
                } else {
                    v.end();
                }
            }
            Action::Open => match self.screen {
                Screen::Appearance => {
                    self.interaction.get_mut().focus.focus(&details_path());
                    self.toggle_appearance_focus();
                }
                Screen::Interface => {
                    self.interaction.get_mut().focus.focus(&details_path());
                    self.toggle_interface_focus();
                }
                _ => {
                    self.interaction.get_mut().focus.focus(&details_path());
                }
            },
            Action::Product(id) => {
                let lookup = declarations(self.interaction.get_mut().map.profile).2;
                if let Some((_, action)) =
                    lookup.into_iter().find(|(candidate, _)| *candidate == id)
                {
                    return self.execute_business(action);
                }
            }
            Action::Search => {}
        }
        ControlCenterCommand::None
    }
    fn keep_draft(&mut self) {
        let _ = self.discard_editor.resolve_discard(DiscardDecision::Cancel);
        self.confirm_discard = self.discard_editor.awaiting_discard_confirmation();
    }
    fn execute_business(&mut self, action: BusinessAction) -> ControlCenterCommand {
        match action {
            BusinessAction::Apply => return self.request_apply(),
            BusinessAction::Revert => self.revert(),
            BusinessAction::ExplainTitle => self.overlay = ControlCenterOverlay::TitleExplanation,
            BusinessAction::RepairPreview => self.open_previewable_repair(),
            BusinessAction::ProviderPreview => self.open_provider_preview(),
            BusinessAction::ProviderReset => self.selected_provider_mode = None,
            BusinessAction::Provider(n) => self.select_provider(provider(n)),
            BusinessAction::DiagnosticProvider(n) => {
                let p = provider(n);
                if p != self.diagnostic_provider {
                    self.diagnostic_provider = p;
                    self.diagnostic_mode = crate::doctor_matrix::ExecutionMode::Unknown;
                }
                self.interaction.get_mut().details.home();
            }
            BusinessAction::DiagnosticMode => {
                use crate::doctor_matrix::ExecutionMode as Mode;
                let modes = [
                    Mode::Unknown,
                    if self.diagnostic_provider == CliTarget::Codex {
                        Mode::Embedded
                    } else {
                        Mode::Direct
                    },
                    Mode::SharedDaemon,
                ];
                let idx = modes
                    .iter()
                    .position(|m| *m == self.diagnostic_mode)
                    .unwrap_or(0);
                self.diagnostic_mode = modes[(idx + 1) % 3];
                self.interaction.get_mut().details.home();
            }
            BusinessAction::WorkspaceClear => {
                self.workspace_draft = None;
                self.update_dirty();
            }
            BusinessAction::WorkspaceEdit => {
                self.workspace_editor = Some(self.workspace_draft.clone().unwrap_or_default());
                self.interaction.get_mut().mode = InputMode::Insert;
            }
            BusinessAction::WorkspaceExplain => {
                self.workspace_explaining = !self.workspace_explaining;
            }
            BusinessAction::WorkspaceCandidate(n) => {
                if let Some(alias) = self
                    .workspace
                    .as_ref()
                    .and_then(|w| w.candidates().get(usize::from(n - 1)))
                    .map(|a| a.alias().as_str().to_owned())
                {
                    self.workspace_draft = Some(alias);
                    self.update_dirty();
                }
            }
            BusinessAction::Discard => {
                if self
                    .discard_editor
                    .resolve_discard(DiscardDecision::Discard)
                {
                    self.revert();
                }
                self.confirm_discard = self.discard_editor.awaiting_discard_confirmation();
                return ControlCenterCommand::Quit;
            }
            BusinessAction::Keep => self.keep_draft(),
            BusinessAction::Dismiss => self.overlay = ControlCenterOverlay::None,
            BusinessAction::ConfirmApply => {
                let command = match &self.overlay {
                    ControlCenterOverlay::RepairPreview(plan) => {
                        ControlCenterCommand::ApplyRepair {
                            expected: plan.expected_repair.clone(),
                            action_id: plan.action_id.clone(),
                        }
                    }
                    ControlCenterOverlay::ProviderPreview {
                        provider,
                        expected_global,
                        before,
                        after,
                        ..
                    } => ControlCenterCommand::ApplyProvider {
                        provider: *provider,
                        expected_global: *expected_global,
                        before: *before,
                        after: *after,
                    },
                    _ => ControlCenterCommand::None,
                };
                self.overlay = ControlCenterOverlay::None;
                return command;
            }
        }
        ControlCenterCommand::None
    }
}
fn provider(n: u8) -> CliTarget {
    match n {
        2 => CliTarget::Agy,
        3 => CliTarget::Cursor,
        _ => CliTarget::Codex,
    }
}

pub(super) fn hint_label(
    hint: &terminal_ui_contract::keymap::EffectiveHint,
    locale: super::ResolvedLocale,
) -> String {
    if locale == super::ResolvedLocale::EnUs {
        return hint.label.clone();
    }
    let label = match hint.action_id.as_str() {
        "ui.view.toggle_density" => "Smart/Raw 显示密度",
        "ui.refresh" => "只读刷新",
        "ui.help" => "帮助",
        "ui.cancel" => "返回/取消",
        "ui.quit" => "请求退出",
        "ui.focus.next" => "下一面板",
        "ui.focus.previous" => "上一面板",
        "ui.move.up" => "向上",
        "ui.move.down" => "向下",
        "ui.move.left" => "向左",
        "ui.move.right" => "向右",
        "ui.open" => "选择/编辑",
        "ui.mode.insert" => "插入模式",
        "ui.viewport.page_up" => "上一页",
        "ui.viewport.page_down" => "下一页",
        "ui.viewport.half_up" => "上半页",
        "ui.viewport.half_down" => "下半页",
        "ui.viewport.home" => "首行",
        "ui.viewport.end" => "末行",
        "ui.viewport.line_up" => "向上滚动",
        "ui.viewport.line_down" => "向下滚动",
        "tabbeacon.apply" | "tabbeacon.confirm.apply" => "明确请求应用",
        "tabbeacon.revert" => "还原草稿",
        "tabbeacon.title" => "标题解释",
        "tabbeacon.repair.preview" => "修复预览",
        "tabbeacon.provider.preview" => "偏好预览",
        "tabbeacon.provider.reset" => "继承偏好",
        "tabbeacon.diagnostics.mode" => "查询执行模式",
        "tabbeacon.discard" => "丢弃草稿并退出",
        "tabbeacon.keep" => "保留草稿",
        "tabbeacon.dismiss" => "关闭预览",
        "tabbeacon.workspace.clear" => "清除别名草稿",
        "tabbeacon.workspace.edit" => "编辑别名",
        "tabbeacon.workspace.explain" => "解释别名",
        _ => {
            return hint
                .label
                .replace("diagnostics provider", "诊断提供方")
                .replace("provider", "提供方")
                .replace("workspace candidate", "别名候选");
        }
    };
    label.into()
}
