//! Typed Human presentation and locale resolution.
//!
//! Product/domain code supplies typed meaning. This module owns the small
//! locale catalog, display-cell width handling, terminal color policy, and the
//! conversion into Human terminal lines. JSON and legacy `--plain` contracts
//! intentionally do not use this module at runtime.

use std::env;

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::{
    human_output::HumanTone,
    interface_preferences::{HumanColor, InterfaceLanguage},
    management::{ActionSafety, ManagementHealth},
};

const ELLIPSIS: &str = "...";

/// One concrete locale supported by Human rendering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedLocale {
    /// English (United States) Human text.
    EnUs,
    /// Simplified Chinese Human text.
    ZhCn,
}

impl ResolvedLocale {
    /// Stable BCP-47-style spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EnUs => "en-US",
            Self::ZhCn => "zh-CN",
        }
    }
}

/// The accepted source that selected a concrete locale.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocaleSource {
    /// An explicit `--lang` argument.
    Cli,
    /// The admitted `TABBEACON_LANG` environment variable.
    Environment,
    /// The user-local Interface preference.
    Preference,
    /// The operating system locale.
    OperatingSystem,
    /// No accepted value was available, so English is the safe default.
    Default,
}

/// A resolved concrete locale with its selecting source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocaleResolution {
    locale: ResolvedLocale,
    source: LocaleSource,
}

impl LocaleResolution {
    /// Selected concrete locale.
    #[must_use]
    pub const fn locale(self) -> ResolvedLocale {
        self.locale
    }

    /// Accepted source that selected the locale.
    #[must_use]
    pub const fn source(self) -> LocaleSource {
        self.source
    }
}

/// Injectable inputs used to resolve one Human locale deterministically.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocaleInputs {
    /// Explicit admitted CLI selection. `auto` continues the chain.
    pub cli: Option<InterfaceLanguage>,
    /// Admitted `TABBEACON_LANG` selection. Unsupported values are `None`.
    pub environment: Option<InterfaceLanguage>,
    /// User-local Interface preference.
    pub preference: InterfaceLanguage,
    /// Operating system locale. Unsupported values are `None`.
    pub operating_system: Option<InterfaceLanguage>,
}

/// Resolves a locale through the documented source precedence.
#[must_use]
pub const fn resolve_locale(inputs: LocaleInputs) -> LocaleResolution {
    if let Some(locale) = concrete_locale(inputs.cli) {
        return LocaleResolution {
            locale,
            source: LocaleSource::Cli,
        };
    }
    if let Some(locale) = concrete_locale(inputs.environment) {
        return LocaleResolution {
            locale,
            source: LocaleSource::Environment,
        };
    }
    if let Some(locale) = concrete_locale(Some(inputs.preference)) {
        return LocaleResolution {
            locale,
            source: LocaleSource::Preference,
        };
    }
    if let Some(locale) = concrete_locale(inputs.operating_system) {
        return LocaleResolution {
            locale,
            source: LocaleSource::OperatingSystem,
        };
    }
    LocaleResolution {
        locale: ResolvedLocale::EnUs,
        source: LocaleSource::Default,
    }
}

const fn concrete_locale(value: Option<InterfaceLanguage>) -> Option<ResolvedLocale> {
    match value {
        Some(InterfaceLanguage::EnUs) => Some(ResolvedLocale::EnUs),
        Some(InterfaceLanguage::ZhCn) => Some(ResolvedLocale::ZhCn),
        Some(InterfaceLanguage::Auto) | None => None,
    }
}

/// Resolves Human locale inputs from the approved process-local sources.
#[must_use]
pub fn resolve_runtime_locale(
    cli: Option<InterfaceLanguage>,
    preference: InterfaceLanguage,
) -> LocaleResolution {
    resolve_locale(LocaleInputs {
        cli,
        environment: env::var("TABBEACON_LANG")
            .ok()
            .and_then(|value| InterfaceLanguage::parse(&value)),
        preference,
        operating_system: operating_system_language(),
    })
}

#[cfg(windows)]
fn operating_system_language() -> Option<InterfaceLanguage> {
    use windows::Win32::Globalization::GetUserDefaultLocaleName;

    // `LOCALE_NAME_MAX_LENGTH` is 85 by the Win32 contract, including the
    // terminating NUL. The buffer is stack-owned for this synchronous call.
    let mut buffer = [0_u16; 85];
    // SAFETY: Win32 writes at most the supplied `buffer.len()` UTF-16 units;
    // the buffer is valid, writable, and remains owned for the full call.
    #[allow(unsafe_code)]
    let written = unsafe { GetUserDefaultLocaleName(&mut buffer) };
    let length = usize::try_from(written).ok()?.checked_sub(1)?;
    let locale = String::from_utf16(&buffer[..length]).ok()?;
    InterfaceLanguage::parse(&locale)
}

#[cfg(not(windows))]
fn operating_system_language() -> Option<InterfaceLanguage> {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .into_iter()
        .find_map(|name| {
            env::var(name)
                .ok()
                .and_then(|value| InterfaceLanguage::parse(&value))
        })
}

/// One catalog key for Human-only wording.
///
/// The catalog is deliberately small in G50. Later Goals may add keys here,
/// rather than growing language conditionals throughout operational code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HumanMessageKey {
    Status,
    Doctor,
    Setup,
    Healthy,
    NeedsAttention,
    ActionNeeded,
    Integration,
    Presentation,
    Runtime,
    TabBeacon,
    Codex,
    Hooks,
    HookTrust,
    Title,
    TabColor,
    Activity,
    Spinner,
    Theme,
    Workers,
    Active,
    Stale,
    ActiveAndStale,
    Supported,
    NotAdmitted,
    Unavailable,
    ActiveCount,
    NeedsAttentionCount,
    NoActionRequired,
    Attention,
    Why,
    Next,
    ProtectedState,
    AdditionalConditions,
    ChecksPassed,
    WarningsAndFailures,
    ControlCenter,
    Sections,
    Overview,
    Appearance,
    CodexIntegration,
    Diagnostics,
    Preview,
    OverallHealth,
    UnsavedChanges,
    FooterNavigation,
    FooterEditing,
    FooterDiscard,
    TerminalTooSmall,
    InterfacePreferences,
    Language,
    Color,
    ReducedMotion,
    InterfacePreferencesUpdated,
    SetupInstalled,
    SetupUpgraded,
    SetupAlreadyInstalled,
    SetupInstalledNext,
    SetupUpgradedNext,
    SetupAlreadyInstalledNext,
    CodexVersionSupported,
    CodexVersionNotAdmitted,
    CodexVersionUnavailable,
    TrustActive,
    TrustReviewRequired,
    TrustNotProven,
    TitleOwnedByTabBeacon,
    TitleNativeOrOff,
    TitleOwnershipConflict,
    CheckCodexVersion,
    CheckCodexHookProfile,
    CheckExecutable,
    CheckOwnershipManifest,
    CheckHookDeclarations,
    CheckHookCurrentness,
    CheckHookTrust,
    CheckTerminalTitle,
    CheckIntegration,
    IssueIntegrationNotInstalledTitle,
    IssueIntegrationNotInstalledExplanation,
    IssueHooksDeclarationsOutOfSyncTitle,
    IssueHooksDeclarationsOutOfSyncExplanation,
    IssueHooksUpgradeRequiredTitle,
    IssueHooksUpgradeRequiredExplanation,
    IssueExecutableUnavailableTitle,
    IssueExecutableUnavailableExplanation,
    IssueCodexProfileUnadmittedTitle,
    IssueCodexProfileUnadmittedExplanation,
    IssueCodexProfileUnavailableTitle,
    IssueCodexProfileUnavailableExplanation,
    IssueHooksReviewRequiredTitle,
    IssueHooksReviewRequiredExplanation,
    IssueHooksTrustUnprovenTitle,
    IssueHooksTrustUnprovenExplanation,
    IssueTitleRepairAvailableTitle,
    IssueTitleRepairAvailableExplanation,
    IssueTitleDiagnoseOnlyTitle,
    IssueTitleDiagnoseOnlyExplanation,
    IssueTitleOwnershipConflictTitle,
    IssueTitleOwnershipConflictExplanation,
    IssueSettingsInvalidTitle,
    IssueSettingsInvalidExplanation,
    IssueSettingsUnavailableTitle,
    IssueSettingsUnavailableExplanation,
    IssueWorkersWarningTitle,
    IssueWorkersWarningExplanation,
    IssueWorkersUnavailableTitle,
    IssueWorkersUnavailableExplanation,
    IssueDiagnosticsAttentionTitle,
    IssueDiagnosticsAttentionExplanation,
    ActionSetupInstall,
    ActionSetupReconcile,
    ActionSetupUpgrade,
    ActionExecutableGuidance,
    ActionProfileGuidance,
    ActionReviewHooks,
    ActionTitleRepair,
    ActionTitleInspect,
    ActionSettingsReset,
    ActionSettingsInspect,
    ActionWorkersInspect,
    ActionDiagnosticsInspect,
    ProtectedReadOnly,
    ProtectedManualAction,
    ProtectedPreviewableRepair,
    ProtectedOwnerExplicit,
    ProtectedUnsupportedAutomation,
}

/// A semantic Human text fragment, independent from a concrete language.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HumanText {
    /// A catalog entry with no dynamic values.
    Message(HumanMessageKey),
    /// A catalog entry with a fixed-order list of dynamic values.
    Template {
        key: HumanMessageKey,
        values: Vec<String>,
    },
    /// A bounded existing detail for which G50 has no catalog key yet.
    Literal(String),
}

/// The semantic role of stable management wording.
///
/// The management projection intentionally retains English-safe source text
/// for JSON and legacy plain contracts. Human rendering selects catalog text
/// by the same stable IDs here instead of translating strings in product code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagementTextKind {
    /// One management issue title.
    IssueTitle,
    /// One management issue explanation.
    IssueExplanation,
    /// One doctor check summary.
    CheckSummary,
}

impl HumanText {
    /// Creates one catalog-backed fragment.
    #[must_use]
    pub const fn message(key: HumanMessageKey) -> Self {
        Self::Message(key)
    }

    /// Creates one catalog-backed fragment with dynamic values.
    #[must_use]
    pub fn template(
        key: HumanMessageKey,
        values: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self::Template {
            key,
            values: values.into_iter().map(Into::into).collect(),
        }
    }

    /// Creates a bounded literal detail emitted by an existing domain model.
    #[must_use]
    pub fn literal(value: impl Into<String>) -> Self {
        Self::Literal(value.into())
    }
}

/// Returns catalog-backed wording for a known management ID, preserving a
/// bounded literal only for an unknown future ID.
#[must_use]
pub fn management_text(
    kind: ManagementTextKind,
    id: &str,
    fallback: impl Into<String>,
) -> HumanText {
    management_message_key(kind, id)
        .map_or_else(|| HumanText::literal(fallback), HumanText::message)
}

/// Returns catalog-backed action guidance for a known issue/action pair.
///
/// `integration.setup_codex` deliberately has distinct safe guidance per
/// condition, so both stable IDs select the precise existing meaning.
#[must_use]
pub fn management_action_text(
    issue_id: &str,
    action_id: &str,
    fallback: impl Into<String>,
) -> HumanText {
    management_action_message_key(issue_id, action_id)
        .map_or_else(|| HumanText::literal(fallback), HumanText::message)
}

/// Returns catalog-backed protected-state language for known action safety.
#[must_use]
pub const fn protected_state_text(safety: ActionSafety) -> HumanText {
    HumanText::message(match safety {
        ActionSafety::ReadOnly => HumanMessageKey::ProtectedReadOnly,
        ActionSafety::ManualAction => HumanMessageKey::ProtectedManualAction,
        ActionSafety::PreviewableSafeRepair => HumanMessageKey::ProtectedPreviewableRepair,
        ActionSafety::OwnerExplicitRequired => HumanMessageKey::ProtectedOwnerExplicit,
        ActionSafety::UnsupportedAutomation => HumanMessageKey::ProtectedUnsupportedAutomation,
    })
}

// Stable management IDs deliberately remain co-located in this translation
// boundary; splitting them would make unknown-ID fallback review less clear.
#[allow(clippy::too_many_lines)]
fn management_message_key(kind: ManagementTextKind, id: &str) -> Option<HumanMessageKey> {
    match (kind, id) {
        (ManagementTextKind::CheckSummary, "codex.version") => {
            Some(HumanMessageKey::CheckCodexVersion)
        }
        (ManagementTextKind::CheckSummary, "codex.hook-profile") => {
            Some(HumanMessageKey::CheckCodexHookProfile)
        }
        (ManagementTextKind::CheckSummary, "tabbeacon.executable") => {
            Some(HumanMessageKey::CheckExecutable)
        }
        (ManagementTextKind::CheckSummary, "ownership.manifest") => {
            Some(HumanMessageKey::CheckOwnershipManifest)
        }
        (ManagementTextKind::CheckSummary, "hooks.declarations") => {
            Some(HumanMessageKey::CheckHookDeclarations)
        }
        (ManagementTextKind::CheckSummary, "hooks.currentness") => {
            Some(HumanMessageKey::CheckHookCurrentness)
        }
        (ManagementTextKind::CheckSummary, "hooks.trust") => Some(HumanMessageKey::CheckHookTrust),
        (ManagementTextKind::CheckSummary, "terminal.title") => {
            Some(HumanMessageKey::CheckTerminalTitle)
        }
        (ManagementTextKind::CheckSummary, "diagnostics.integration") => {
            Some(HumanMessageKey::CheckIntegration)
        }
        (ManagementTextKind::IssueTitle, "integration.not_installed") => {
            Some(HumanMessageKey::IssueIntegrationNotInstalledTitle)
        }
        (ManagementTextKind::IssueExplanation, "integration.not_installed") => {
            Some(HumanMessageKey::IssueIntegrationNotInstalledExplanation)
        }
        (ManagementTextKind::IssueTitle, "hooks.declarations_out_of_sync") => {
            Some(HumanMessageKey::IssueHooksDeclarationsOutOfSyncTitle)
        }
        (ManagementTextKind::IssueExplanation, "hooks.declarations_out_of_sync") => {
            Some(HumanMessageKey::IssueHooksDeclarationsOutOfSyncExplanation)
        }
        (ManagementTextKind::IssueTitle, "hooks.integration_upgrade_required") => {
            Some(HumanMessageKey::IssueHooksUpgradeRequiredTitle)
        }
        (ManagementTextKind::IssueExplanation, "hooks.integration_upgrade_required") => {
            Some(HumanMessageKey::IssueHooksUpgradeRequiredExplanation)
        }
        (ManagementTextKind::IssueTitle, "integration.executable_unavailable") => {
            Some(HumanMessageKey::IssueExecutableUnavailableTitle)
        }
        (ManagementTextKind::IssueExplanation, "integration.executable_unavailable") => {
            Some(HumanMessageKey::IssueExecutableUnavailableExplanation)
        }
        (ManagementTextKind::IssueTitle, "codex.profile_unadmitted") => {
            Some(HumanMessageKey::IssueCodexProfileUnadmittedTitle)
        }
        (ManagementTextKind::IssueExplanation, "codex.profile_unadmitted") => {
            Some(HumanMessageKey::IssueCodexProfileUnadmittedExplanation)
        }
        (ManagementTextKind::IssueTitle, "codex.profile_unavailable") => {
            Some(HumanMessageKey::IssueCodexProfileUnavailableTitle)
        }
        (ManagementTextKind::IssueExplanation, "codex.profile_unavailable") => {
            Some(HumanMessageKey::IssueCodexProfileUnavailableExplanation)
        }
        (ManagementTextKind::IssueTitle, "hooks.review_required") => {
            Some(HumanMessageKey::IssueHooksReviewRequiredTitle)
        }
        (ManagementTextKind::IssueExplanation, "hooks.review_required") => {
            Some(HumanMessageKey::IssueHooksReviewRequiredExplanation)
        }
        (ManagementTextKind::IssueTitle, "hooks.trust_unproven") => {
            Some(HumanMessageKey::IssueHooksTrustUnprovenTitle)
        }
        (ManagementTextKind::IssueExplanation, "hooks.trust_unproven") => {
            Some(HumanMessageKey::IssueHooksTrustUnprovenExplanation)
        }
        (ManagementTextKind::IssueTitle, "terminal.title_repair_available") => {
            Some(HumanMessageKey::IssueTitleRepairAvailableTitle)
        }
        (ManagementTextKind::IssueExplanation, "terminal.title_repair_available") => {
            Some(HumanMessageKey::IssueTitleRepairAvailableExplanation)
        }
        (ManagementTextKind::IssueTitle, "terminal.title_diagnose_only") => {
            Some(HumanMessageKey::IssueTitleDiagnoseOnlyTitle)
        }
        (ManagementTextKind::IssueExplanation, "terminal.title_diagnose_only") => {
            Some(HumanMessageKey::IssueTitleDiagnoseOnlyExplanation)
        }
        (ManagementTextKind::IssueTitle, "terminal.title_ownership_conflict") => {
            Some(HumanMessageKey::IssueTitleOwnershipConflictTitle)
        }
        (ManagementTextKind::IssueExplanation, "terminal.title_ownership_conflict") => {
            Some(HumanMessageKey::IssueTitleOwnershipConflictExplanation)
        }
        (ManagementTextKind::IssueTitle, "settings.invalid") => {
            Some(HumanMessageKey::IssueSettingsInvalidTitle)
        }
        (ManagementTextKind::IssueExplanation, "settings.invalid") => {
            Some(HumanMessageKey::IssueSettingsInvalidExplanation)
        }
        (ManagementTextKind::IssueTitle, "settings.unavailable") => {
            Some(HumanMessageKey::IssueSettingsUnavailableTitle)
        }
        (ManagementTextKind::IssueExplanation, "settings.unavailable") => {
            Some(HumanMessageKey::IssueSettingsUnavailableExplanation)
        }
        (ManagementTextKind::IssueTitle, "workers.warning") => {
            Some(HumanMessageKey::IssueWorkersWarningTitle)
        }
        (ManagementTextKind::IssueExplanation, "workers.warning") => {
            Some(HumanMessageKey::IssueWorkersWarningExplanation)
        }
        (ManagementTextKind::IssueTitle, "workers.unavailable") => {
            Some(HumanMessageKey::IssueWorkersUnavailableTitle)
        }
        (ManagementTextKind::IssueExplanation, "workers.unavailable") => {
            Some(HumanMessageKey::IssueWorkersUnavailableExplanation)
        }
        (ManagementTextKind::IssueTitle, id) if id.starts_with("diagnostics.") => {
            Some(HumanMessageKey::IssueDiagnosticsAttentionTitle)
        }
        (ManagementTextKind::IssueExplanation, id) if id.starts_with("diagnostics.") => {
            Some(HumanMessageKey::IssueDiagnosticsAttentionExplanation)
        }
        _ => None,
    }
}

fn management_action_message_key(issue_id: &str, action_id: &str) -> Option<HumanMessageKey> {
    match (issue_id, action_id) {
        ("integration.not_installed", "integration.setup_codex") => {
            Some(HumanMessageKey::ActionSetupInstall)
        }
        (
            "hooks.declarations_out_of_sync" | "terminal.title_ownership_conflict",
            "integration.setup_codex",
        ) => Some(HumanMessageKey::ActionSetupReconcile),
        ("hooks.integration_upgrade_required", "integration.setup_codex") => {
            Some(HumanMessageKey::ActionSetupUpgrade)
        }
        (_, "integration.executable_guidance") => Some(HumanMessageKey::ActionExecutableGuidance),
        (_, "codex.profile_guidance") => Some(HumanMessageKey::ActionProfileGuidance),
        (_, "hooks.review_in_codex") => Some(HumanMessageKey::ActionReviewHooks),
        (_, "terminal.title_policy_repair") => Some(HumanMessageKey::ActionTitleRepair),
        (_, "terminal.title_policy_inspect") => Some(HumanMessageKey::ActionTitleInspect),
        (_, "settings.reset_explicitly") => Some(HumanMessageKey::ActionSettingsReset),
        (_, "settings.inspect_environment") => Some(HumanMessageKey::ActionSettingsInspect),
        (_, "workers.inspect_status") => Some(HumanMessageKey::ActionWorkersInspect),
        (_, "diagnostics.inspect") => Some(HumanMessageKey::ActionDiagnosticsInspect),
        _ => None,
    }
}

/// One typed field within a Human section.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HumanField {
    marker: Option<String>,
    label: HumanText,
    value: HumanText,
    tone: HumanTone,
}

impl HumanField {
    /// Creates a field with an optional textual state marker.
    #[must_use]
    pub fn new(
        marker: Option<impl Into<String>>,
        label: HumanText,
        value: HumanText,
        tone: HumanTone,
    ) -> Self {
        Self {
            marker: marker.map(Into::into),
            label,
            value,
            tone,
        }
    }
}

/// One typed Human explanatory message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HumanMessage {
    marker: Option<String>,
    prefix: Option<HumanText>,
    text: HumanText,
    tone: HumanTone,
}

impl HumanMessage {
    /// Creates a message with no localized prefix.
    #[must_use]
    pub fn plain(text: HumanText, tone: HumanTone) -> Self {
        Self {
            marker: None,
            prefix: None,
            text,
            tone,
        }
    }

    /// Creates a message with a localized semantic prefix.
    #[must_use]
    pub fn prefixed(prefix: HumanText, text: HumanText, tone: HumanTone) -> Self {
        Self {
            marker: None,
            prefix: Some(prefix),
            text,
            tone,
        }
    }

    /// Creates a message with a language-neutral state marker.
    #[must_use]
    pub fn marked(marker: impl Into<String>, text: HumanText, tone: HumanTone) -> Self {
        Self {
            marker: Some(marker.into()),
            prefix: None,
            text,
            tone,
        }
    }
}

/// One typed next action within a Human section.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HumanAction {
    text: HumanText,
    tone: HumanTone,
}

impl HumanAction {
    /// Creates a next action.
    #[must_use]
    pub fn new(text: HumanText, tone: HumanTone) -> Self {
        Self { text, tone }
    }
}

/// One typed group in a Human document.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HumanSection {
    heading: Option<HumanText>,
    fields: Vec<HumanField>,
    messages: Vec<HumanMessage>,
    actions: Vec<HumanAction>,
}

impl HumanSection {
    /// Creates a section with an optional heading.
    #[must_use]
    pub fn new(heading: Option<HumanText>) -> Self {
        Self {
            heading,
            ..Self::default()
        }
    }

    /// Adds one field.
    #[must_use]
    pub fn with_field(mut self, field: HumanField) -> Self {
        self.fields.push(field);
        self
    }

    /// Adds one explanatory message.
    #[must_use]
    pub fn with_message(mut self, message: HumanMessage) -> Self {
        self.messages.push(message);
        self
    }

    /// Adds one next action.
    #[must_use]
    pub fn with_action(mut self, action: HumanAction) -> Self {
        self.actions.push(action);
        self
    }
}

/// Locale-neutral meaning for one Human terminal surface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HumanDocument {
    title: HumanText,
    status: Option<HumanText>,
    sections: Vec<HumanSection>,
}

impl HumanDocument {
    /// Creates one Human document.
    #[must_use]
    pub fn new(title: HumanText, status: Option<HumanText>) -> Self {
        Self {
            title,
            status,
            sections: Vec::new(),
        }
    }

    /// Adds one ordered section.
    #[must_use]
    pub fn with_section(mut self, section: HumanSection) -> Self {
        self.sections.push(section);
        self
    }

    /// The semantic title for structural tests and alternate renderers.
    #[must_use]
    pub const fn title(&self) -> &HumanText {
        &self.title
    }

    /// The optional semantic status for structural tests and alternate renderers.
    #[must_use]
    pub fn status(&self) -> Option<&HumanText> {
        self.status.as_ref()
    }

    /// Ordered typed content sections.
    #[must_use]
    pub fn sections(&self) -> &[HumanSection] {
        &self.sections
    }
}

/// One fully rendered Human terminal line with explicit semantic treatment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HumanLine {
    text: String,
    tone: HumanTone,
}

impl HumanLine {
    /// Visible text before optional terminal styling.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Semantic visual treatment selected by the shared renderer.
    #[must_use]
    pub const fn tone(&self) -> HumanTone {
        self.tone
    }
}

/// One locale-aware renderer for typed Human documents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HumanRenderer {
    locale: ResolvedLocale,
    width: usize,
}

impl HumanRenderer {
    /// Creates a renderer bounded to the supplied terminal display-cell width.
    #[must_use]
    pub const fn new(locale: ResolvedLocale, width: usize) -> Self {
        Self { locale, width }
    }

    /// Concrete locale selected for this renderer.
    #[must_use]
    pub const fn locale(self) -> ResolvedLocale {
        self.locale
    }

    /// Renders typed semantic content as bounded Human lines.
    #[must_use]
    pub fn render(self, document: &HumanDocument) -> Vec<HumanLine> {
        let mut lines = Vec::new();
        let title = render_text(self.locale, document.title());
        let title = document.status().map_or(title.clone(), |status| {
            format!("{title} — {}", render_text(self.locale, status))
        });
        push_line(&mut lines, self.width, &title, HumanTone::Accent);

        for section in document.sections() {
            lines.push(HumanLine {
                text: String::new(),
                tone: HumanTone::Plain,
            });
            if let Some(heading) = &section.heading {
                let heading = render_text(self.locale, heading);
                push_line(&mut lines, self.width, &heading, HumanTone::Accent);
            }
            for field in &section.fields {
                let marker = field
                    .marker
                    .as_deref()
                    .map_or(String::new(), |value| format!("{value} "));
                let label = render_text(self.locale, &field.label);
                let value = render_text(self.locale, &field.value);
                let field_text = format!("  {marker}{label}  {value}");
                push_line(&mut lines, self.width, &field_text, field.tone);
            }
            for message in &section.messages {
                let marker = message
                    .marker
                    .as_deref()
                    .map_or(String::new(), |value| format!("{value} "));
                let prefix = message.prefix.as_ref().map_or(String::new(), |value| {
                    format!("{}: ", render_text(self.locale, value))
                });
                let message_text = format!(
                    "{marker}{prefix}{}",
                    render_text(self.locale, &message.text)
                );
                push_line(&mut lines, self.width, &message_text, message.tone);
            }
            for action in &section.actions {
                let action_text = format!(
                    "{}: {}",
                    catalog(self.locale, HumanMessageKey::Next),
                    render_text(self.locale, &action.text)
                );
                push_line(&mut lines, self.width, &action_text, action.tone);
            }
        }
        lines
    }
}

/// Determines whether this policy may emit ANSI styling to a target.
#[must_use]
pub const fn color_enabled(policy: HumanColor, is_terminal: bool, no_color_is_set: bool) -> bool {
    if no_color_is_set || matches!(policy, HumanColor::Never) {
        return false;
    }
    matches!(policy, HumanColor::Always) || is_terminal
}

/// Returns the number of terminal display cells occupied by a string.
#[must_use]
pub fn display_width(value: &str) -> usize {
    UnicodeWidthStr::width(value)
}

/// Fits text into a display-cell width without splitting a Unicode scalar.
#[must_use]
pub fn fit_display_width(value: &str, width: usize) -> String {
    if display_width(value) <= width {
        return value.to_owned();
    }
    if width < display_width(ELLIPSIS) {
        return take_display_width(value, width);
    }
    let mut shortened = take_display_width(value, width - display_width(ELLIPSIS));
    shortened.push_str(ELLIPSIS);
    shortened
}

/// Pads text to a display-cell width without assuming scalar-count width.
#[must_use]
pub fn pad_display_width(value: &str, width: usize) -> String {
    let mut padded = fit_display_width(value, width);
    padded.push_str(&" ".repeat(width.saturating_sub(display_width(&padded))));
    padded
}

fn take_display_width(value: &str, width: usize) -> String {
    let mut used = 0_usize;
    let mut result = String::new();
    for character in value.chars() {
        let character_width = UnicodeWidthChar::width(character).unwrap_or(0);
        if used.saturating_add(character_width) > width {
            break;
        }
        result.push(character);
        used += character_width;
    }
    result
}

fn push_line(lines: &mut Vec<HumanLine>, width: usize, text: &str, tone: HumanTone) {
    lines.push(HumanLine {
        text: fit_display_width(text, width),
        tone,
    });
}

/// Returns one shared localized health label.
#[must_use]
pub const fn health_label(locale: ResolvedLocale, health: ManagementHealth) -> &'static str {
    match health {
        ManagementHealth::Healthy => catalog(locale, HumanMessageKey::Healthy),
        ManagementHealth::Warning => catalog(locale, HumanMessageKey::NeedsAttention),
        ManagementHealth::Error => catalog(locale, HumanMessageKey::ActionNeeded),
    }
}

/// Returns static Human catalog wording for a concrete locale.
#[must_use]
// One exhaustive two-locale catalog is intentionally kept together so new
// product code cannot add language branches outside this presentation boundary.
#[allow(clippy::too_many_lines)]
pub const fn catalog(locale: ResolvedLocale, key: HumanMessageKey) -> &'static str {
    match (locale, key) {
        (_, HumanMessageKey::TabBeacon | HumanMessageKey::TitleOwnedByTabBeacon) => "TabBeacon",
        (_, HumanMessageKey::Codex) => "Codex",
        (ResolvedLocale::EnUs, HumanMessageKey::Status) => "TabBeacon Status",
        (ResolvedLocale::EnUs, HumanMessageKey::Doctor) => "TabBeacon Doctor",
        (ResolvedLocale::EnUs, HumanMessageKey::Setup) => "TabBeacon Setup",
        (ResolvedLocale::EnUs, HumanMessageKey::Healthy) => "Healthy",
        (ResolvedLocale::EnUs, HumanMessageKey::NeedsAttention) => "Needs attention",
        (ResolvedLocale::EnUs, HumanMessageKey::ActionNeeded) => "Action needed",
        (ResolvedLocale::EnUs, HumanMessageKey::Integration) => "Integration",
        (ResolvedLocale::EnUs, HumanMessageKey::Presentation) => "Presentation",
        (ResolvedLocale::EnUs, HumanMessageKey::Runtime) => "Runtime",
        (ResolvedLocale::EnUs, HumanMessageKey::Hooks) => "Hooks",
        (ResolvedLocale::EnUs, HumanMessageKey::HookTrust | HumanMessageKey::CheckHookTrust) => {
            "Hook trust"
        }
        (ResolvedLocale::EnUs, HumanMessageKey::Title) => "Title",
        (ResolvedLocale::EnUs, HumanMessageKey::TabColor) => "Tab color",
        (ResolvedLocale::EnUs, HumanMessageKey::Activity) => "Activity",
        (ResolvedLocale::EnUs, HumanMessageKey::Spinner) => "Spinner",
        (ResolvedLocale::EnUs, HumanMessageKey::Theme) => "Theme",
        (ResolvedLocale::EnUs, HumanMessageKey::Workers) => "Workers",
        (ResolvedLocale::EnUs, HumanMessageKey::Active | HumanMessageKey::TrustActive) => "active",
        (ResolvedLocale::EnUs, HumanMessageKey::Stale) => "stale",
        (ResolvedLocale::EnUs, HumanMessageKey::ActiveAndStale) => "Active {0} · Stale {1}",
        (ResolvedLocale::EnUs, HumanMessageKey::Supported) => "Supported",
        (ResolvedLocale::EnUs, HumanMessageKey::NotAdmitted) => "Not admitted",
        (ResolvedLocale::EnUs, HumanMessageKey::Unavailable) => "Unavailable",
        (ResolvedLocale::EnUs, HumanMessageKey::ActiveCount) => "{0} active",
        (ResolvedLocale::EnUs, HumanMessageKey::NeedsAttentionCount) => "{0} need attention",
        (ResolvedLocale::EnUs, HumanMessageKey::NoActionRequired) => "No action required.",
        (ResolvedLocale::EnUs, HumanMessageKey::Attention) => "Attention",
        (ResolvedLocale::EnUs, HumanMessageKey::Why) => "Why",
        (ResolvedLocale::EnUs, HumanMessageKey::Next) => "Next",
        (ResolvedLocale::EnUs, HumanMessageKey::ProtectedState) => "TabBeacon did not change",
        (ResolvedLocale::EnUs, HumanMessageKey::AdditionalConditions) => {
            "{0} additional condition(s): run tabbeacon doctor."
        }
        (ResolvedLocale::EnUs, HumanMessageKey::ChecksPassed) => "{0} checks passed.",
        (ResolvedLocale::EnUs, HumanMessageKey::WarningsAndFailures) => {
            "{0} warning(s), {1} failure(s)."
        }
        (ResolvedLocale::EnUs, HumanMessageKey::ControlCenter) => "TabBeacon Control Center",
        (ResolvedLocale::EnUs, HumanMessageKey::Sections) => "Sections",
        (ResolvedLocale::EnUs, HumanMessageKey::Overview) => "Overview",
        (ResolvedLocale::EnUs, HumanMessageKey::Appearance) => "Appearance",
        (ResolvedLocale::EnUs, HumanMessageKey::CodexIntegration) => "Codex Integration",
        (ResolvedLocale::EnUs, HumanMessageKey::Diagnostics) => "Diagnostics",
        (ResolvedLocale::EnUs, HumanMessageKey::Preview) => "Preview",
        (ResolvedLocale::EnUs, HumanMessageKey::OverallHealth) => "Overall health",
        (ResolvedLocale::EnUs, HumanMessageKey::UnsavedChanges) => "unsaved changes",
        (ResolvedLocale::EnUs, HumanMessageKey::FooterNavigation) => {
            "↑↓ navigate  Enter edit Appearance  a Apply  r Revert  q Quit"
        }
        (ResolvedLocale::EnUs, HumanMessageKey::FooterEditing) => {
            "↑↓ select setting  ←→ change draft  Enter done  a Apply  r Revert"
        }
        (ResolvedLocale::EnUs, HumanMessageKey::FooterDiscard) => {
            "Unsaved changes — [k] Keep editing  [d] Discard changes"
        }
        (ResolvedLocale::EnUs, HumanMessageKey::TerminalTooSmall) => "Terminal too small",
        (ResolvedLocale::EnUs, HumanMessageKey::InterfacePreferences) => "Interface preferences",
        (ResolvedLocale::EnUs, HumanMessageKey::Language) => "Language",
        (ResolvedLocale::EnUs, HumanMessageKey::Color) => "Color",
        (ResolvedLocale::EnUs, HumanMessageKey::ReducedMotion) => "Reduced motion",
        (ResolvedLocale::EnUs, HumanMessageKey::InterfacePreferencesUpdated) => {
            "Interface preferences updated."
        }
        (ResolvedLocale::EnUs, HumanMessageKey::SetupInstalled) => "Codex integration installed.",
        (ResolvedLocale::EnUs, HumanMessageKey::SetupUpgraded) => "Codex integration upgraded.",
        (ResolvedLocale::EnUs, HumanMessageKey::SetupAlreadyInstalled) => {
            "Codex integration is already installed."
        }
        (ResolvedLocale::EnUs, HumanMessageKey::SetupInstalledNext) => {
            "Launch codex, review TabBeacon hooks in /hooks, then run tabbeacon doctor."
        }
        (ResolvedLocale::EnUs, HumanMessageKey::SetupUpgradedNext) => {
            "Launch codex, review the updated TabBeacon hooks in /hooks, then run tabbeacon doctor."
        }
        (ResolvedLocale::EnUs, HumanMessageKey::SetupAlreadyInstalledNext) => {
            "Run tabbeacon doctor to verify hook trust and configuration."
        }
        (ResolvedLocale::EnUs, HumanMessageKey::CodexVersionSupported) => "{0} — Supported",
        (ResolvedLocale::EnUs, HumanMessageKey::CodexVersionNotAdmitted) => "{0} — Not admitted",
        (ResolvedLocale::EnUs, HumanMessageKey::CodexVersionUnavailable) => {
            "Unavailable — Not admitted"
        }
        (ResolvedLocale::EnUs, HumanMessageKey::TrustReviewRequired) => "review required",
        (ResolvedLocale::EnUs, HumanMessageKey::TrustNotProven) => "not proven",
        (ResolvedLocale::EnUs, HumanMessageKey::TitleNativeOrOff) => "Codex native or off",
        (ResolvedLocale::EnUs, HumanMessageKey::TitleOwnershipConflict) => "conflict",
        (ResolvedLocale::EnUs, HumanMessageKey::CheckCodexVersion) => "Codex compatibility",
        (ResolvedLocale::EnUs, HumanMessageKey::CheckCodexHookProfile) => "Codex Hook profile",
        (ResolvedLocale::EnUs, HumanMessageKey::CheckExecutable) => "managed executable",
        (ResolvedLocale::EnUs, HumanMessageKey::CheckOwnershipManifest) => "ownership manifest",
        (ResolvedLocale::EnUs, HumanMessageKey::CheckHookDeclarations) => {
            "managed Hook declarations"
        }
        (ResolvedLocale::EnUs, HumanMessageKey::CheckHookCurrentness) => {
            "Hook integration currentness"
        }
        (ResolvedLocale::EnUs, HumanMessageKey::CheckTerminalTitle) => "terminal-title ownership",
        (ResolvedLocale::EnUs, HumanMessageKey::CheckIntegration) => {
            "Codex integration environment"
        }
        (ResolvedLocale::EnUs, HumanMessageKey::IssueIntegrationNotInstalledTitle) => {
            "TabBeacon integration is not installed"
        }
        (ResolvedLocale::EnUs, HumanMessageKey::IssueIntegrationNotInstalledExplanation) => {
            "TabBeacon cannot prove that its managed Codex hooks are present."
        }
        (ResolvedLocale::EnUs, HumanMessageKey::IssueHooksDeclarationsOutOfSyncTitle) => {
            "Managed Hook declarations need attention"
        }
        (ResolvedLocale::EnUs, HumanMessageKey::IssueHooksDeclarationsOutOfSyncExplanation) => {
            "The installed declarations are missing or modified, so the integration is not proven current."
        }
        (ResolvedLocale::EnUs, HumanMessageKey::IssueHooksUpgradeRequiredTitle) => {
            "TabBeacon integration upgrade required"
        }
        (ResolvedLocale::EnUs, HumanMessageKey::IssueHooksUpgradeRequiredExplanation) => {
            "Owned Hook declarations do not match the current admitted integration shape."
        }
        (ResolvedLocale::EnUs, HumanMessageKey::IssueExecutableUnavailableTitle) => {
            "Managed executable is unavailable"
        }
        (ResolvedLocale::EnUs, HumanMessageKey::IssueExecutableUnavailableExplanation) => {
            "The owned Hook integration cannot find the executable it was configured to invoke."
        }
        (ResolvedLocale::EnUs, HumanMessageKey::IssueCodexProfileUnadmittedTitle) => {
            "Codex profile is not admitted"
        }
        (ResolvedLocale::EnUs, HumanMessageKey::IssueCodexProfileUnadmittedExplanation) => {
            "This detected Codex version has no admitted TabBeacon Hook profile."
        }
        (ResolvedLocale::EnUs, HumanMessageKey::IssueCodexProfileUnavailableTitle) => {
            "Codex compatibility is unavailable"
        }
        (ResolvedLocale::EnUs, HumanMessageKey::IssueCodexProfileUnavailableExplanation) => {
            "TabBeacon cannot safely prove an admitted Codex Hook profile."
        }
        (ResolvedLocale::EnUs, HumanMessageKey::IssueHooksReviewRequiredTitle) => {
            "Codex Hook review is required"
        }
        (ResolvedLocale::EnUs, HumanMessageKey::IssueHooksReviewRequiredExplanation) => {
            "The owned definitions are present, but Codex trust remains a human review boundary."
        }
        (ResolvedLocale::EnUs, HumanMessageKey::IssueHooksTrustUnprovenTitle) => {
            "Codex Hook trust is not proven"
        }
        (ResolvedLocale::EnUs, HumanMessageKey::IssueHooksTrustUnprovenExplanation) => {
            "TabBeacon cannot mark Hook definitions trusted or infer that trust from configuration alone."
        }
        (ResolvedLocale::EnUs, HumanMessageKey::IssueTitleRepairAvailableTitle) => {
            "Windows Terminal title repair is available"
        }
        (ResolvedLocale::EnUs, HumanMessageKey::IssueTitleRepairAvailableExplanation) => {
            "The existing policy subsystem proved one active-profile repair scope without guessing unrelated settings."
        }
        (ResolvedLocale::EnUs, HumanMessageKey::IssueTitleDiagnoseOnlyTitle) => {
            "Windows Terminal title policy needs diagnosis"
        }
        (ResolvedLocale::EnUs, HumanMessageKey::IssueTitleDiagnoseOnlyExplanation) => {
            "The current policy cannot safely identify a repair scope, so TabBeacon will not mutate settings."
        }
        (ResolvedLocale::EnUs, HumanMessageKey::IssueTitleOwnershipConflictTitle) => {
            "Codex title ownership conflicts with the selected preference"
        }
        (ResolvedLocale::EnUs, HumanMessageKey::IssueTitleOwnershipConflictExplanation) => {
            "The existing owned integration cannot prove its terminal-title preference is reconciled."
        }
        (ResolvedLocale::EnUs, HumanMessageKey::IssueSettingsInvalidTitle) => {
            "Presentation settings are invalid"
        }
        (ResolvedLocale::EnUs, HumanMessageKey::IssueSettingsInvalidExplanation) => {
            "TabBeacon did not interpret or overwrite the malformed settings document."
        }
        (ResolvedLocale::EnUs, HumanMessageKey::IssueSettingsUnavailableTitle) => {
            "Presentation settings are unavailable"
        }
        (ResolvedLocale::EnUs, HumanMessageKey::IssueSettingsUnavailableExplanation) => {
            "TabBeacon cannot safely inspect the current settings location."
        }
        (ResolvedLocale::EnUs, HumanMessageKey::IssueWorkersWarningTitle) => {
            "Activity worker state needs attention"
        }
        (ResolvedLocale::EnUs, HumanMessageKey::IssueWorkersWarningExplanation) => {
            "Stale, invalid, or bounded-out activity leases were observed without exposing their contents."
        }
        (ResolvedLocale::EnUs, HumanMessageKey::IssueWorkersUnavailableTitle) => {
            "Activity worker state is unavailable"
        }
        (ResolvedLocale::EnUs, HumanMessageKey::IssueWorkersUnavailableExplanation) => {
            "TabBeacon cannot safely inspect the activity-lease aggregate."
        }
        (ResolvedLocale::EnUs, HumanMessageKey::IssueDiagnosticsAttentionTitle) => {
            "Additional diagnostic attention is required"
        }
        (ResolvedLocale::EnUs, HumanMessageKey::IssueDiagnosticsAttentionExplanation) => {
            "A bounded diagnostic check needs review; its underlying state is not changed by this management projection."
        }
        (ResolvedLocale::EnUs, HumanMessageKey::ActionSetupInstall) => {
            "Run tabbeacon setup codex when you are ready to apply the ownership-aware setup."
        }
        (ResolvedLocale::EnUs, HumanMessageKey::ActionSetupReconcile) => {
            "Run tabbeacon setup codex to request the existing ownership-aware reconciliation."
        }
        (ResolvedLocale::EnUs, HumanMessageKey::ActionSetupUpgrade) => {
            "Run tabbeacon setup codex to request the existing ownership-aware upgrade."
        }
        (ResolvedLocale::EnUs, HumanMessageKey::ActionExecutableGuidance) => {
            "Restore an admitted TabBeacon executable, then inspect status again."
        }
        (ResolvedLocale::EnUs, HumanMessageKey::ActionProfileGuidance) => {
            "Use a supported Codex version or wait for an explicitly admitted TabBeacon profile; no support is fabricated automatically."
        }
        (ResolvedLocale::EnUs, HumanMessageKey::ActionReviewHooks) => {
            "Launch codex, open /hooks, and review the TabBeacon definitions."
        }
        (ResolvedLocale::EnUs, HumanMessageKey::ActionTitleRepair) => {
            "Inspect with tabbeacon title-policy inspect, then explicitly choose tabbeacon title-policy repair if the scoped change is correct."
        }
        (ResolvedLocale::EnUs, HumanMessageKey::ActionTitleInspect) => {
            "Run tabbeacon title-policy inspect for the bounded policy diagnosis."
        }
        (ResolvedLocale::EnUs, HumanMessageKey::ActionSettingsReset) => {
            "Inspect the settings first; run tabbeacon config reset only if you intentionally want the default presentation settings."
        }
        (ResolvedLocale::EnUs, HumanMessageKey::ActionSettingsInspect) => {
            "Restore access to the settings location, then run tabbeacon status again."
        }
        (ResolvedLocale::EnUs, HumanMessageKey::ActionWorkersInspect) => {
            "Run tabbeacon status or tabbeacon doctor to review the bounded worker health summary."
        }
        (ResolvedLocale::EnUs, HumanMessageKey::ActionDiagnosticsInspect) => {
            "Run tabbeacon doctor to review the current bounded diagnostic result."
        }
        (ResolvedLocale::EnUs, HumanMessageKey::ProtectedReadOnly) => {
            "No persistent configuration change is requested."
        }
        (ResolvedLocale::EnUs, HumanMessageKey::ProtectedManualAction) => {
            "TabBeacon does not change application trust state."
        }
        (ResolvedLocale::EnUs, HumanMessageKey::ProtectedPreviewableRepair) => {
            "Unrelated Windows Terminal settings remain untouched."
        }
        (ResolvedLocale::EnUs, HumanMessageKey::ProtectedOwnerExplicit) => {
            "No change occurs until the Owner explicitly applies it."
        }
        (ResolvedLocale::EnUs, HumanMessageKey::ProtectedUnsupportedAutomation) => {
            "TabBeacon will not fabricate an unsupported automation path."
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::Status) => "TabBeacon 状态",
        (ResolvedLocale::ZhCn, HumanMessageKey::Doctor) => "TabBeacon 诊断",
        (ResolvedLocale::ZhCn, HumanMessageKey::Setup) => "TabBeacon 设置",
        (ResolvedLocale::ZhCn, HumanMessageKey::Healthy) => "正常",
        (ResolvedLocale::ZhCn, HumanMessageKey::NeedsAttention) => "需要关注",
        (ResolvedLocale::ZhCn, HumanMessageKey::ActionNeeded) => "需要处理",
        (ResolvedLocale::ZhCn, HumanMessageKey::Integration) => "集成",
        (ResolvedLocale::ZhCn, HumanMessageKey::Presentation) => "显示",
        (ResolvedLocale::ZhCn, HumanMessageKey::Runtime) => "运行时",
        (ResolvedLocale::ZhCn, HumanMessageKey::Hooks) => "钩子",
        (ResolvedLocale::ZhCn, HumanMessageKey::HookTrust | HumanMessageKey::CheckHookTrust) => {
            "钩子信任"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::Title) => "标题",
        (ResolvedLocale::ZhCn, HumanMessageKey::TabColor) => "标签颜色",
        (ResolvedLocale::ZhCn, HumanMessageKey::Activity | HumanMessageKey::Active) => "活动",
        (ResolvedLocale::ZhCn, HumanMessageKey::Spinner) => "旋转指示器",
        (ResolvedLocale::ZhCn, HumanMessageKey::Theme) => "主题",
        (ResolvedLocale::ZhCn, HumanMessageKey::Workers) => "工作器",
        (ResolvedLocale::ZhCn, HumanMessageKey::Stale) => "过期",
        (ResolvedLocale::ZhCn, HumanMessageKey::ActiveAndStale) => "活动 {0} · 过期 {1}",
        (ResolvedLocale::ZhCn, HumanMessageKey::Supported) => "已支持",
        (ResolvedLocale::ZhCn, HumanMessageKey::NotAdmitted) => "未准入",
        (ResolvedLocale::ZhCn, HumanMessageKey::Unavailable) => "不可用",
        (ResolvedLocale::ZhCn, HumanMessageKey::ActiveCount) => "活动 {0}",
        (ResolvedLocale::ZhCn, HumanMessageKey::NeedsAttentionCount) => "{0} 项需要关注",
        (ResolvedLocale::ZhCn, HumanMessageKey::NoActionRequired) => "无需操作。",
        (ResolvedLocale::ZhCn, HumanMessageKey::Attention) => "请关注",
        (ResolvedLocale::ZhCn, HumanMessageKey::Why) => "原因",
        (ResolvedLocale::ZhCn, HumanMessageKey::Next) => "下一步",
        (ResolvedLocale::ZhCn, HumanMessageKey::ProtectedState) => "TabBeacon 未更改",
        (ResolvedLocale::ZhCn, HumanMessageKey::AdditionalConditions) => {
            "另有 {0} 项状况：请运行 tabbeacon doctor。"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::ChecksPassed) => "{0} 项检查通过。",
        (ResolvedLocale::ZhCn, HumanMessageKey::WarningsAndFailures) => "{0} 项警告，{1} 项失败。",
        (ResolvedLocale::ZhCn, HumanMessageKey::ControlCenter) => "TabBeacon 控制中心",
        (ResolvedLocale::ZhCn, HumanMessageKey::Sections) => "分区",
        (ResolvedLocale::ZhCn, HumanMessageKey::Overview) => "概览",
        (ResolvedLocale::ZhCn, HumanMessageKey::Appearance) => "外观",
        (ResolvedLocale::ZhCn, HumanMessageKey::CodexIntegration) => "Codex 集成",
        (ResolvedLocale::ZhCn, HumanMessageKey::Diagnostics) => "诊断",
        (ResolvedLocale::ZhCn, HumanMessageKey::Preview) => "预览",
        (ResolvedLocale::ZhCn, HumanMessageKey::OverallHealth) => "总体状态",
        (ResolvedLocale::ZhCn, HumanMessageKey::UnsavedChanges) => "有未保存的更改",
        (ResolvedLocale::ZhCn, HumanMessageKey::FooterNavigation) => {
            "↑↓ 导航  Enter 编辑外观  a 应用  r 还原  q 退出"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::FooterEditing) => {
            "↑↓ 选择设置  ←→ 调整草稿  Enter 完成  a 应用  r 还原"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::FooterDiscard) => {
            "有未保存的更改 — [k] 继续编辑  [d] 放弃更改"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::TerminalTooSmall) => "终端窗口过小",
        (ResolvedLocale::ZhCn, HumanMessageKey::InterfacePreferences) => "界面偏好",
        (ResolvedLocale::ZhCn, HumanMessageKey::Language) => "语言",
        (ResolvedLocale::ZhCn, HumanMessageKey::Color) => "颜色",
        (ResolvedLocale::ZhCn, HumanMessageKey::ReducedMotion) => "减少动画",
        (ResolvedLocale::ZhCn, HumanMessageKey::InterfacePreferencesUpdated) => "界面偏好已更新。",
        (
            ResolvedLocale::ZhCn,
            HumanMessageKey::SetupInstalled | HumanMessageKey::SetupAlreadyInstalled,
        ) => "Codex 集成已安装。",
        (ResolvedLocale::ZhCn, HumanMessageKey::SetupUpgraded) => "Codex 集成已升级。",
        (ResolvedLocale::ZhCn, HumanMessageKey::SetupInstalledNext) => {
            "启动 codex，在 /hooks 中检查 TabBeacon 钩子，然后运行 tabbeacon doctor。"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::SetupUpgradedNext) => {
            "启动 codex，在 /hooks 中检查更新后的 TabBeacon 钩子，然后运行 tabbeacon doctor。"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::SetupAlreadyInstalledNext) => {
            "运行 tabbeacon doctor 以检查钩子信任和配置。"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::CodexVersionSupported) => "{0} — 已支持",
        (ResolvedLocale::ZhCn, HumanMessageKey::CodexVersionNotAdmitted) => "{0} — 未准入",
        (ResolvedLocale::ZhCn, HumanMessageKey::CodexVersionUnavailable) => "不可用 — 未准入",
        (ResolvedLocale::ZhCn, HumanMessageKey::TrustActive) => "已激活",
        (ResolvedLocale::ZhCn, HumanMessageKey::TrustReviewRequired) => "需要审查",
        (ResolvedLocale::ZhCn, HumanMessageKey::TrustNotProven) => "未获证明",
        (ResolvedLocale::ZhCn, HumanMessageKey::TitleNativeOrOff) => "Codex 原生或关闭",
        (ResolvedLocale::ZhCn, HumanMessageKey::TitleOwnershipConflict) => "冲突",
        (ResolvedLocale::ZhCn, HumanMessageKey::CheckCodexVersion) => "Codex 兼容性",
        (ResolvedLocale::ZhCn, HumanMessageKey::CheckCodexHookProfile) => "Codex 钩子配置档",
        (ResolvedLocale::ZhCn, HumanMessageKey::CheckExecutable) => "受管可执行文件",
        (ResolvedLocale::ZhCn, HumanMessageKey::CheckOwnershipManifest) => "所有权清单",
        (ResolvedLocale::ZhCn, HumanMessageKey::CheckHookDeclarations) => "受管钩子声明",
        (ResolvedLocale::ZhCn, HumanMessageKey::CheckHookCurrentness) => "钩子集成时效",
        (ResolvedLocale::ZhCn, HumanMessageKey::CheckTerminalTitle) => "终端标题所有权",
        (ResolvedLocale::ZhCn, HumanMessageKey::CheckIntegration) => "Codex 集成环境",
        (ResolvedLocale::ZhCn, HumanMessageKey::IssueIntegrationNotInstalledTitle) => {
            "尚未安装 TabBeacon 集成"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::IssueIntegrationNotInstalledExplanation) => {
            "TabBeacon 无法证明其受管 Codex 钩子已存在。"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::IssueHooksDeclarationsOutOfSyncTitle) => {
            "受管钩子声明需要处理"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::IssueHooksDeclarationsOutOfSyncExplanation) => {
            "已安装的声明缺失或被修改，因此无法证明集成仍是最新状态。"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::IssueHooksUpgradeRequiredTitle) => {
            "需要升级 TabBeacon 集成"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::IssueHooksUpgradeRequiredExplanation) => {
            "受管钩子声明与当前准入的集成形态不一致。"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::IssueExecutableUnavailableTitle) => {
            "受管可执行文件不可用"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::IssueExecutableUnavailableExplanation) => {
            "受管钩子集成找不到其配置要调用的可执行文件。"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::IssueCodexProfileUnadmittedTitle) => {
            "Codex 配置档未准入"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::IssueCodexProfileUnadmittedExplanation) => {
            "检测到的 Codex 版本没有准入的 TabBeacon 钩子配置档。"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::IssueCodexProfileUnavailableTitle) => {
            "Codex 兼容性不可用"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::IssueCodexProfileUnavailableExplanation) => {
            "TabBeacon 无法安全地证明存在已准入的 Codex 钩子配置档。"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::IssueHooksReviewRequiredTitle) => {
            "需要审查 Codex 钩子"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::IssueHooksReviewRequiredExplanation) => {
            "受管定义已存在，但 Codex 信任仍是人工审查边界。"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::IssueHooksTrustUnprovenTitle) => {
            "尚未证明 Codex 钩子可信"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::IssueHooksTrustUnprovenExplanation) => {
            "TabBeacon 无法将钩子定义标记为可信，也不会仅凭配置推断信任。"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::IssueTitleRepairAvailableTitle) => {
            "可以修复 Windows Terminal 标题"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::IssueTitleRepairAvailableExplanation) => {
            "现有策略子系统已证明一个活动配置档修复范围，且不会猜测无关设置。"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::IssueTitleDiagnoseOnlyTitle) => {
            "需要诊断 Windows Terminal 标题策略"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::IssueTitleDiagnoseOnlyExplanation) => {
            "当前策略无法安全地确定修复范围，因此 TabBeacon 不会修改设置。"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::IssueTitleOwnershipConflictTitle) => {
            "Codex 标题所有权与所选偏好冲突"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::IssueTitleOwnershipConflictExplanation) => {
            "现有受管集成无法证明其终端标题偏好已经协调。"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::IssueSettingsInvalidTitle) => "显示设置无效",
        (ResolvedLocale::ZhCn, HumanMessageKey::IssueSettingsInvalidExplanation) => {
            "TabBeacon 未解释或覆盖格式错误的设置文档。"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::IssueSettingsUnavailableTitle) => "显示设置不可用",
        (ResolvedLocale::ZhCn, HumanMessageKey::IssueSettingsUnavailableExplanation) => {
            "TabBeacon 无法安全检查当前设置位置。"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::IssueWorkersWarningTitle) => {
            "活动工作器状态需要关注"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::IssueWorkersWarningExplanation) => {
            "发现了过期、无效或超出边界的活动租约，但未暴露其内容。"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::IssueWorkersUnavailableTitle) => {
            "活动工作器状态不可用"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::IssueWorkersUnavailableExplanation) => {
            "TabBeacon 无法安全检查活动租约汇总。"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::IssueDiagnosticsAttentionTitle) => {
            "需要关注其他诊断结果"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::IssueDiagnosticsAttentionExplanation) => {
            "某项受限诊断检查需要审查；此管理投影不会改变其底层状态。"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::ActionSetupInstall) => {
            "准备应用所有权感知设置时，请运行 tabbeacon setup codex。"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::ActionSetupReconcile) => {
            "请运行 tabbeacon setup codex，请求现有的所有权感知协调。"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::ActionSetupUpgrade) => {
            "请运行 tabbeacon setup codex，请求现有的所有权感知升级。"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::ActionExecutableGuidance) => {
            "恢复已准入的 TabBeacon 可执行文件，然后再次检查状态。"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::ActionProfileGuidance) => {
            "使用受支持的 Codex 版本，或等待明确准入的 TabBeacon 配置档；不会自动虚构支持。"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::ActionReviewHooks) => {
            "启动 codex，打开 /hooks，并审查 TabBeacon 定义。"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::ActionTitleRepair) => {
            "使用 tabbeacon title-policy inspect 检查；仅当范围变更正确时，再明确选择 tabbeacon title-policy repair。"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::ActionTitleInspect) => {
            "运行 tabbeacon title-policy inspect，获取受限的策略诊断。"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::ActionSettingsReset) => {
            "请先检查设置；仅在明确需要默认显示设置时运行 tabbeacon config reset。"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::ActionSettingsInspect) => {
            "恢复对设置位置的访问，然后再次运行 tabbeacon status。"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::ActionWorkersInspect) => {
            "运行 tabbeacon status 或 tabbeacon doctor，审查受限的工作器状态摘要。"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::ActionDiagnosticsInspect) => {
            "运行 tabbeacon doctor，审查当前受限诊断结果。"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::ProtectedReadOnly) => "未请求更改持久配置。",
        (ResolvedLocale::ZhCn, HumanMessageKey::ProtectedManualAction) => {
            "TabBeacon 不会更改应用信任状态。"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::ProtectedPreviewableRepair) => {
            "无关的 Windows Terminal 设置保持不变。"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::ProtectedOwnerExplicit) => {
            "Owner 未明确应用前不会发生更改。"
        }
        (ResolvedLocale::ZhCn, HumanMessageKey::ProtectedUnsupportedAutomation) => {
            "TabBeacon 不会虚构不受支持的自动化路径。"
        }
    }
}

fn render_text(locale: ResolvedLocale, value: &HumanText) -> String {
    match value {
        HumanText::Message(key) => catalog(locale, *key).to_owned(),
        HumanText::Template { key, values } => {
            let mut rendered = catalog(locale, *key).to_owned();
            for (index, value) in values.iter().enumerate() {
                rendered = rendered.replace(&format!("{{{index}}}"), value);
            }
            rendered
        }
        HumanText::Literal(value) => value.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        HumanDocument, HumanMessage, HumanMessageKey, HumanRenderer, HumanSection, HumanText,
        InterfaceLanguage, LocaleInputs, LocaleSource, ResolvedLocale, color_enabled,
        display_width, fit_display_width, pad_display_width, resolve_locale,
    };
    use crate::{
        human_output::{HumanTone, style},
        interface_preferences::HumanColor,
    };

    #[test]
    fn locale_resolution_obeys_every_precedence_step_and_auto_continues() {
        let inputs = LocaleInputs {
            cli: Some(InterfaceLanguage::ZhCn),
            environment: Some(InterfaceLanguage::EnUs),
            preference: InterfaceLanguage::EnUs,
            operating_system: Some(InterfaceLanguage::EnUs),
        };
        assert_eq!(resolve_locale(inputs).source(), LocaleSource::Cli);
        assert_eq!(resolve_locale(inputs).locale(), ResolvedLocale::ZhCn);

        let environment = LocaleInputs {
            cli: Some(InterfaceLanguage::Auto),
            ..inputs
        };
        assert_eq!(
            resolve_locale(environment).source(),
            LocaleSource::Environment
        );
        assert_eq!(resolve_locale(environment).locale(), ResolvedLocale::EnUs);

        let preference = LocaleInputs {
            environment: Some(InterfaceLanguage::Auto),
            ..environment
        };
        assert_eq!(
            resolve_locale(preference).source(),
            LocaleSource::Preference
        );
        assert_eq!(resolve_locale(preference).locale(), ResolvedLocale::EnUs);

        let operating_system = LocaleInputs {
            preference: InterfaceLanguage::Auto,
            ..preference
        };
        assert_eq!(
            resolve_locale(operating_system).source(),
            LocaleSource::OperatingSystem
        );

        let fallback = LocaleInputs {
            operating_system: None,
            ..operating_system
        };
        assert_eq!(resolve_locale(fallback).source(), LocaleSource::Default);
        assert_eq!(resolve_locale(fallback).locale(), ResolvedLocale::EnUs);
    }

    #[test]
    fn unsupported_locale_spelling_is_not_partially_admitted() {
        assert_eq!(InterfaceLanguage::parse("fr-FR"), None);
        assert_eq!(
            InterfaceLanguage::parse("zh_CN.UTF-8"),
            Some(InterfaceLanguage::ZhCn)
        );
        let resolved = resolve_locale(LocaleInputs {
            cli: None,
            environment: InterfaceLanguage::parse("fr-FR"),
            preference: InterfaceLanguage::Auto,
            operating_system: InterfaceLanguage::parse("de-DE"),
        });
        assert_eq!(resolved.locale(), ResolvedLocale::EnUs);
        assert_eq!(resolved.source(), LocaleSource::Default);
    }

    #[test]
    fn typed_document_renders_bilingual_semantics_without_color() {
        let document = HumanDocument::new(
            HumanText::message(HumanMessageKey::Status),
            Some(HumanText::message(HumanMessageKey::Healthy)),
        )
        .with_section(
            HumanSection::new(Some(HumanText::message(HumanMessageKey::Integration))).with_message(
                HumanMessage::plain(
                    HumanText::message(HumanMessageKey::NoActionRequired),
                    HumanTone::Success,
                ),
            ),
        );
        let english = HumanRenderer::new(ResolvedLocale::EnUs, 80)
            .render(&document)
            .into_iter()
            .map(|line| line.text().to_owned())
            .collect::<Vec<_>>()
            .join("\n");
        let chinese = HumanRenderer::new(ResolvedLocale::ZhCn, 80)
            .render(&document)
            .into_iter()
            .map(|line| line.text().to_owned())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(english.contains("TabBeacon Status — Healthy"));
        assert!(chinese.contains("TabBeacon 状态 — 正常"));
        assert!(chinese.contains("无需操作。"));
    }

    #[test]
    fn color_policy_preserves_semantic_text_and_respects_redirected_auto() {
        assert!(!color_enabled(HumanColor::Never, true, false));
        assert!(!color_enabled(HumanColor::Auto, false, false));
        assert!(color_enabled(HumanColor::Always, false, false));
        let plain = "需要关注";
        let styled = style(
            HumanTone::Attention,
            plain,
            color_enabled(HumanColor::Always, false, false),
        );
        assert_eq!(
            styled.replace("\u{1b}[33m", "").replace("\u{1b}[0m", ""),
            plain
        );
    }

    #[test]
    fn display_width_primitives_handle_cjk_and_combining_text() {
        assert_eq!(display_width("中文"), 4);
        assert_eq!(display_width("e\u{301}"), 1);
        let fitted = fit_display_width("中文abcdef", 7);
        assert!(display_width(&fitted) <= 7);
        assert_eq!(display_width(&pad_display_width("中", 4)), 4);
    }
}
