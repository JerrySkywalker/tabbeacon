//! Provider-neutral, privacy-safe Hook inventory projections.
//!
//! Provider adapters parse their own configuration and emit this bounded
//! representation.  Callers never receive hook commands, configuration paths,
//! raw state keys, or provider payloads.

use serde::Serialize;

use crate::human_presentation::ResolvedLocale;

/// Stable schema version for [`HookInventory`].
pub const HOOK_INVENTORY_SCHEMA_VERSION: u32 = 1;

/// Read-only availability of one provider Hook inspection.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HookInventoryAvailability {
    /// The adapter parsed the supported source shape safely.
    Available,
    /// The provider state was missing, malformed, symbolic, or unsupported.
    Unavailable,
}

impl HookInventoryAvailability {
    /// Stable machine identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Unavailable => "unavailable",
        }
    }
}

/// Provider-neutral owner classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HookOwner {
    /// The live declaration exactly matches an active `TabBeacon` manifest entry.
    TabBeacon,
    /// A parsed declaration is not owned by `TabBeacon`.
    ThirdParty,
    /// The declaration cannot safely be attributed to `TabBeacon` or a third party.
    UnownedOrAmbiguous,
}

impl HookOwner {
    /// Stable machine identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TabBeacon => "tabbeacon",
            Self::ThirdParty => "third_party",
            Self::UnownedOrAmbiguous => "unowned_or_ambiguous",
        }
    }
}

/// Manual-review trust classification for one Hook entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HookTrustState {
    /// The declared hash is present, current, and enabled.
    Trusted,
    /// Codex must still receive a human trust review.
    ReviewRequired,
    /// The trusted hash differs while the declaration itself is exact.
    HashStaleOrChanged,
    /// The declaration is disabled in provider state.
    Disabled,
    /// Trust cannot be claimed because ownership is not proven.
    UnownedOrAmbiguous,
    /// The provider/profile/source shape cannot safely establish trust.
    UnsupportedOrUnavailable,
}

impl HookTrustState {
    /// Stable machine identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Trusted => "trusted",
            Self::ReviewRequired => "review_required",
            Self::HashStaleOrChanged => "hash_stale_or_changed",
            Self::Disabled => "disabled",
            Self::UnownedOrAmbiguous => "unowned_or_ambiguous",
            Self::UnsupportedOrUnavailable => "unsupported_or_unavailable",
        }
    }
}

/// Currentness classification for one Hook entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HookCurrentness {
    /// The exact declaration matches the currently capability-compatible integration shape.
    Current,
    /// An unproven capability probe retains an exact known installed declaration shape.
    InstalledExactCapabilityUnproven,
    /// The declaration is exact to its manifest but the integration shape is old.
    Stale,
    /// The expected declaration was missing or changed after installation.
    DeclarationModifiedOrMissing,
    /// Currentness cannot be claimed because ownership is not proven.
    UnownedOrAmbiguous,
    /// The provider/profile/source shape cannot safely establish currentness.
    UnsupportedOrUnavailable,
}

impl HookCurrentness {
    /// Stable machine identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::InstalledExactCapabilityUnproven => "installed_exact_capability_unproven",
            Self::Stale => "stale",
            Self::DeclarationModifiedOrMissing => "declaration_modified_or_missing",
            Self::UnownedOrAmbiguous => "unowned_or_ambiguous",
            Self::UnsupportedOrUnavailable => "unsupported_or_unavailable",
        }
    }
}

/// Safe provenance for an inventory row.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HookSourceKind {
    /// A currently parsed provider user-global Hook declaration.
    ProviderUserGlobal,
    /// A manifest expectation retained only to explain a missing declaration.
    OwnedManifestExpectation,
    /// The provider shape did not admit a safe source classification.
    UnsupportedOrUnavailable,
}

impl HookSourceKind {
    /// Stable machine identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ProviderUserGlobal => "provider_user_global",
            Self::OwnedManifestExpectation => "owned_manifest_expectation",
            Self::UnsupportedOrUnavailable => "unsupported_or_unavailable",
        }
    }
}

/// Safe handler classification; no handler text is projected.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HookHandlerKind {
    /// A command Hook with its command text redacted.
    Command,
    /// A session-scoped MCP tool Hook with server and tool text redacted.
    McpTool,
    /// A provider handler shape that is not supported by the adapter.
    Unsupported,
}

impl HookHandlerKind {
    /// Stable machine identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Command => "command",
            Self::McpTool => "mcp_tool",
            Self::Unsupported => "unsupported",
        }
    }
}

/// Explicit command exposure policy for every inventory row.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HookCommandVisibility {
    /// Full commands remain unavailable in Human, JSON, plain, and TUI output.
    Redacted,
}

impl HookCommandVisibility {
    /// Stable machine identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        "redacted"
    }
}

/// One provider-neutral, redacted Hook inventory row.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct HookInventoryEntry {
    /// Stable provider identifier.
    pub provider: String,
    /// Stable provider-normalized event identifier.
    pub event: String,
    /// Proven ownership classification.
    pub owner: HookOwner,
    /// Provider enablement observation, never an instruction to change it.
    pub enabled: bool,
    /// Manual-review trust classification.
    pub trust_state: HookTrustState,
    /// Integration declaration currentness classification.
    pub currentness: HookCurrentness,
    /// Safe provenance class.
    pub source_kind: HookSourceKind,
    /// Safe handler class.
    pub handler_kind: HookHandlerKind,
    /// Declared Hook timeout in seconds when safely parseable.
    pub timeout: Option<u64>,
    /// A non-secret SHA-256 declaration fingerprint.
    pub fingerprint: String,
    /// Explicit command-redaction status.
    pub command_visibility: HookCommandVisibility,
}

impl HookInventoryEntry {
    /// Creates a complete redacted Hook inventory row.
    #[must_use]
    #[allow(clippy::too_many_arguments)] // The stable external projection is intentionally explicit.
    pub fn new(
        provider: impl Into<String>,
        event: impl Into<String>,
        owner: HookOwner,
        enabled: bool,
        trust_state: HookTrustState,
        currentness: HookCurrentness,
        source_kind: HookSourceKind,
        handler_kind: HookHandlerKind,
        timeout: Option<u64>,
        fingerprint: impl Into<String>,
    ) -> Self {
        Self {
            provider: provider.into(),
            event: event.into(),
            owner,
            enabled,
            trust_state,
            currentness,
            source_kind,
            handler_kind,
            timeout,
            fingerprint: fingerprint.into(),
            command_visibility: HookCommandVisibility::Redacted,
        }
    }
}

/// Read-only Hook inventory returned by a provider adapter.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct HookInventory {
    /// Stable projection schema.
    pub schema_version: u32,
    /// Inspection never requests a provider/config mutation.
    pub read_only: bool,
    /// Whether the provider source shape was safely inspectable.
    pub availability: HookInventoryAvailability,
    /// Redacted provider-neutral entries.
    pub entries: Vec<HookInventoryEntry>,
}

impl HookInventory {
    /// Creates an available provider projection.
    #[must_use]
    pub fn available(mut entries: Vec<HookInventoryEntry>) -> Self {
        entries.sort_by(|left, right| {
            left.provider
                .cmp(&right.provider)
                .then_with(|| left.event.cmp(&right.event))
                .then_with(|| left.fingerprint.cmp(&right.fingerprint))
        });
        Self {
            schema_version: HOOK_INVENTORY_SCHEMA_VERSION,
            read_only: true,
            availability: HookInventoryAvailability::Available,
            entries,
        }
    }

    /// Creates an unavailable projection without exposing an underlying path or error.
    #[must_use]
    pub const fn unavailable() -> Self {
        Self {
            schema_version: HOOK_INVENTORY_SCHEMA_VERSION,
            read_only: true,
            availability: HookInventoryAvailability::Unavailable,
            entries: Vec::new(),
        }
    }

    /// Returns safe, stable plain key-value rows.
    #[must_use]
    pub fn plain_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("HOOKS_SCHEMA_VERSION={}", self.schema_version),
            format!("HOOKS_READ_ONLY={}", self.read_only),
            format!("HOOKS_AVAILABILITY={}", self.availability.as_str()),
        ];
        for (index, entry) in self.entries.iter().enumerate() {
            lines.push(format!(
                "HOOK={}|provider={}|event={}|owner={}|enabled={}|trust_state={}|currentness={}|source_kind={}|handler_kind={}|timeout={}|fingerprint={}|command_visibility={}",
                index + 1,
                entry.provider,
                entry.event,
                entry.owner.as_str(),
                entry.enabled,
                entry.trust_state.as_str(),
                entry.currentness.as_str(),
                entry.source_kind.as_str(),
                entry.handler_kind.as_str(),
                entry.timeout.map_or_else(|| "unavailable".to_owned(), |value| value.to_string()),
                entry.fingerprint,
                entry.command_visibility.as_str(),
            ));
        }
        lines.push("ARBITRARY_COMMANDS_REDACTED=true".to_owned());
        lines.push("AUTO_HOOK_TRUST=false".to_owned());
        lines
    }

    /// Renders a compact localized Human/TUI table without command content.
    #[must_use]
    pub fn human_table(&self, locale: ResolvedLocale) -> String {
        let heading = match locale {
            ResolvedLocale::EnUs => "Provider / Event / Hook status",
            ResolvedLocale::ZhCn => "提供方 / 事件 / 钩子状态",
        };
        if self.availability == HookInventoryAvailability::Unavailable {
            return match locale {
                ResolvedLocale::EnUs => {
                    "Hooks — unavailable\n\nThe provider Hook shape could not be inspected safely. No configuration was changed.".to_owned()
                }
                ResolvedLocale::ZhCn => {
                    "钩子 — 不可用\n\n无法安全检查提供方钩子结构。未更改任何配置。".to_owned()
                }
            };
        }
        let rows = self
            .entries
            .iter()
            .take(12)
            .map(|entry| match locale {
                ResolvedLocale::EnUs => format!(
                    "{}  {}\n  Owner: {} · Enabled: {}\n  Trust: {} · Current: {}",
                    entry.provider,
                    event_label(&entry.event),
                    owner_label(locale, entry.owner),
                    enabled_label(locale, entry.enabled),
                    trust_label(locale, entry.trust_state),
                    currentness_label(locale, entry.currentness),
                ),
                ResolvedLocale::ZhCn => format!(
                    "{}  {}\n  所有者: {} · 启用: {}\n  信任: {} · 当前状态: {}",
                    entry.provider,
                    event_label(&entry.event),
                    owner_label(locale, entry.owner),
                    enabled_label(locale, entry.enabled),
                    trust_label(locale, entry.trust_state),
                    currentness_label(locale, entry.currentness),
                ),
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        let footer = match locale {
            ResolvedLocale::EnUs => {
                "Read-only inventory. Commands are redacted; trust review remains manual in Codex /hooks."
            }
            ResolvedLocale::ZhCn => {
                "只读清单。命令已脱敏；信任审查仍需在 Codex /hooks 中手动完成。"
            }
        };
        format!("{heading}\n\n{rows}\n\n{footer}")
    }
}

impl Default for HookInventory {
    fn default() -> Self {
        Self::unavailable()
    }
}

fn event_label(event: &str) -> &str {
    match event {
        "pre_tool_use" => "PreToolUse",
        "permission_request" => "PermissionRequest",
        "post_tool_use" => "PostToolUse",
        "pre_compact" => "PreCompact",
        "post_compact" => "PostCompact",
        "session_start" => "SessionStart",
        "session_end" => "SessionEnd",
        "user_prompt_submit" => "UserPromptSubmit",
        "subagent_start" => "SubagentStart",
        "subagent_stop" => "SubagentStop",
        "stop" => "Stop",
        _ => "Unsupported",
    }
}

fn owner_label(locale: ResolvedLocale, owner: HookOwner) -> &'static str {
    match (locale, owner) {
        (_, HookOwner::TabBeacon) => "TabBeacon",
        (ResolvedLocale::EnUs, HookOwner::ThirdParty) => "Third party",
        (ResolvedLocale::EnUs, HookOwner::UnownedOrAmbiguous) => "Unowned/ambiguous",
        (ResolvedLocale::ZhCn, HookOwner::ThirdParty) => "第三方",
        (ResolvedLocale::ZhCn, HookOwner::UnownedOrAmbiguous) => "未拥有/不明确",
    }
}

fn enabled_label(locale: ResolvedLocale, enabled: bool) -> &'static str {
    match (locale, enabled) {
        (ResolvedLocale::EnUs, true) => "yes",
        (ResolvedLocale::EnUs, false) => "no",
        (ResolvedLocale::ZhCn, true) => "是",
        (ResolvedLocale::ZhCn, false) => "否",
    }
}

fn trust_label(locale: ResolvedLocale, state: HookTrustState) -> &'static str {
    match (locale, state) {
        (ResolvedLocale::EnUs, HookTrustState::Trusted) => "trusted",
        (ResolvedLocale::EnUs, HookTrustState::ReviewRequired) => "review required",
        (ResolvedLocale::EnUs, HookTrustState::HashStaleOrChanged) => "hash stale or changed",
        (ResolvedLocale::EnUs, HookTrustState::Disabled) => "disabled",
        (ResolvedLocale::EnUs, HookTrustState::UnownedOrAmbiguous) => "unowned/ambiguous",
        (ResolvedLocale::EnUs, HookTrustState::UnsupportedOrUnavailable) => "unavailable",
        (ResolvedLocale::ZhCn, HookTrustState::Trusted) => "可信",
        (ResolvedLocale::ZhCn, HookTrustState::ReviewRequired) => "需要审查",
        (ResolvedLocale::ZhCn, HookTrustState::HashStaleOrChanged) => "哈希过期或已变更",
        (ResolvedLocale::ZhCn, HookTrustState::Disabled) => "已停用",
        (ResolvedLocale::ZhCn, HookTrustState::UnownedOrAmbiguous) => "未拥有/不明确",
        (ResolvedLocale::ZhCn, HookTrustState::UnsupportedOrUnavailable) => "不可用",
    }
}

fn currentness_label(locale: ResolvedLocale, state: HookCurrentness) -> &'static str {
    match (locale, state) {
        (ResolvedLocale::EnUs, HookCurrentness::Current) => "current",
        (ResolvedLocale::EnUs, HookCurrentness::InstalledExactCapabilityUnproven) => {
            "installed exact (capability probe unproven)"
        }
        (ResolvedLocale::EnUs, HookCurrentness::Stale) => "stale",
        (ResolvedLocale::EnUs, HookCurrentness::DeclarationModifiedOrMissing) => {
            "declaration modified/missing"
        }
        (ResolvedLocale::EnUs, HookCurrentness::UnownedOrAmbiguous) => "unowned/ambiguous",
        (ResolvedLocale::EnUs, HookCurrentness::UnsupportedOrUnavailable) => "unavailable",
        (ResolvedLocale::ZhCn, HookCurrentness::Current) => "当前",
        (ResolvedLocale::ZhCn, HookCurrentness::InstalledExactCapabilityUnproven) => {
            "已安装且精确（能力探测未证明）"
        }
        (ResolvedLocale::ZhCn, HookCurrentness::Stale) => "过期",
        (ResolvedLocale::ZhCn, HookCurrentness::DeclarationModifiedOrMissing) => "声明已变更/缺失",
        (ResolvedLocale::ZhCn, HookCurrentness::UnownedOrAmbiguous) => "未拥有/不明确",
        (ResolvedLocale::ZhCn, HookCurrentness::UnsupportedOrUnavailable) => "不可用",
    }
}

#[cfg(test)]
mod tests {
    use super::{
        HookCommandVisibility, HookCurrentness, HookHandlerKind, HookInventory,
        HookInventoryAvailability, HookInventoryEntry, HookOwner, HookSourceKind, HookTrustState,
    };
    use crate::human_presentation::ResolvedLocale;

    fn entry() -> HookInventoryEntry {
        HookInventoryEntry::new(
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
        )
    }

    #[test]
    fn machine_and_human_projections_keep_commands_redacted() {
        let inventory = HookInventory::available(vec![entry()]);
        let json = serde_json::to_string(&inventory).expect("inventory serializes");
        let plain = inventory.plain_lines().join("\n");
        let en = inventory.human_table(ResolvedLocale::EnUs);
        let zh = inventory.human_table(ResolvedLocale::ZhCn);
        for output in [&json, &plain, &en, &zh] {
            assert!(!output.contains("commandWindows"));
            assert!(!output.contains("powershell.exe"));
            assert!(!output.contains("C:\\\\"));
        }
        assert_eq!(
            inventory.entries[0].command_visibility,
            HookCommandVisibility::Redacted
        );
        assert!(en.contains("trusted"));
        assert!(zh.contains("可信"));
    }

    #[test]
    fn unavailable_projection_is_explicit_and_read_only() {
        let inventory = HookInventory::unavailable();
        assert_eq!(
            inventory.availability,
            HookInventoryAvailability::Unavailable
        );
        assert!(inventory.read_only);
        assert!(inventory.entries.is_empty());
        assert!(
            inventory
                .human_table(ResolvedLocale::EnUs)
                .contains("No configuration was changed")
        );
    }
}
