//! Bounded, version-independent Codex Hook capability contracts.

/// Forward-compatibility policy for Hook events outside an admitted profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnknownEventPolicy {
    /// Ignore the event without returning a blocking decision to Codex.
    IgnoreFailOpen,
}

/// Hook events proven from the admitted Codex release source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CodexHookEvent {
    /// A tool is about to execute.
    PreToolUse,
    /// Codex is requesting user permission.
    PermissionRequest,
    /// A tool finished executing.
    PostToolUse,
    /// Context compaction is about to start.
    PreCompact,
    /// Context compaction finished.
    PostCompact,
    /// A root session started or resumed.
    SessionStart,
    /// A root session ended.
    SessionEnd,
    /// A user prompt opened a turn.
    UserPromptSubmit,
    /// A thread-spawned subagent started.
    SubagentStart,
    /// A thread-spawned subagent stopped.
    SubagentStop,
    /// A root turn produced its final response.
    Stop,
}

impl CodexHookEvent {
    /// Parses the exact wire spelling used by Codex Hooks.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "PreToolUse" => Self::PreToolUse,
            "PermissionRequest" => Self::PermissionRequest,
            "PostToolUse" => Self::PostToolUse,
            "PreCompact" => Self::PreCompact,
            "PostCompact" => Self::PostCompact,
            "SessionStart" => Self::SessionStart,
            "SessionEnd" => Self::SessionEnd,
            "UserPromptSubmit" => Self::UserPromptSubmit,
            "SubagentStart" => Self::SubagentStart,
            "SubagentStop" => Self::SubagentStop,
            "Stop" => Self::Stop,
            _ => return None,
        })
    }

    /// Returns the exact wire spelling used by Codex Hooks.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PreToolUse => "PreToolUse",
            Self::PermissionRequest => "PermissionRequest",
            Self::PostToolUse => "PostToolUse",
            Self::PreCompact => "PreCompact",
            Self::PostCompact => "PostCompact",
            Self::SessionStart => "SessionStart",
            Self::SessionEnd => "SessionEnd",
            Self::UserPromptSubmit => "UserPromptSubmit",
            Self::SubagentStart => "SubagentStart",
            Self::SubagentStop => "SubagentStop",
            Self::Stop => "Stop",
        }
    }

    /// Whether the admitted wire schema requires a turn identifier.
    #[must_use]
    pub const fn requires_turn_id(self) -> bool {
        !matches!(self, Self::SessionStart | Self::SessionEnd)
    }

    /// Whether the admitted wire schema can identify a thread-spawned subagent.
    #[must_use]
    pub const fn supports_subagent_context(self) -> bool {
        !matches!(self, Self::SessionStart | Self::SessionEnd | Self::Stop)
    }

    /// Whether this is an explicit subagent lifecycle event.
    #[must_use]
    pub const fn is_subagent_lifecycle(self) -> bool {
        matches!(self, Self::SubagentStart | Self::SubagentStop)
    }
}

/// Source-audited identity and ordering requirements for one Hook surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HookIdentitySemantics {
    session_id_required: bool,
    turn_id_required_outside_session_lifecycle: bool,
    subagent_identity_required_for_subagent_lifecycle: bool,
}

impl HookIdentitySemantics {
    /// Whether every admitted event has a durable session identity.
    #[must_use]
    pub const fn session_id_required(self) -> bool {
        self.session_id_required
    }

    /// Whether non-session lifecycle events carry the ordering turn identity.
    #[must_use]
    pub const fn turn_id_required_outside_session_lifecycle(self) -> bool {
        self.turn_id_required_outside_session_lifecycle
    }

    /// Whether explicit subagent lifecycle events require agent identity and type.
    #[must_use]
    pub const fn subagent_identity_required_for_subagent_lifecycle(self) -> bool {
        self.subagent_identity_required_for_subagent_lifecycle
    }
}

/// Bounded Hook execution facts proven for one admitted source release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HookTimeoutSemantics {
    synchronous_required: bool,
    declaration_timeout_seconds: u8,
    maximum_timeout_seconds: u8,
    timeout_blocks_operation: bool,
}

impl HookTimeoutSemantics {
    /// Whether handlers must remain synchronous for the admitted release.
    #[must_use]
    pub const fn synchronous_required(self) -> bool {
        self.synchronous_required
    }

    /// The minimum owned declaration timeout.
    #[must_use]
    pub const fn declaration_timeout_seconds(self) -> u8 {
        self.declaration_timeout_seconds
    }

    /// The source-audited upper timeout bound for the terminal lifecycle hook.
    #[must_use]
    pub const fn maximum_timeout_seconds(self) -> u8 {
        self.maximum_timeout_seconds
    }

    /// Whether a timeout can block ordinary Codex progression.
    #[must_use]
    pub const fn timeout_blocks_operation(self) -> bool {
        self.timeout_blocks_operation
    }
}

/// Terminal-title ownership behavior proven for one admitted source release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalTitleOwnershipSemantics {
    /// Codex owns titles normally; `[tui].terminal_title = []` delegates them.
    CodexDefaultWithExplicitTabBeaconDelegation,
}

/// Source-audited normal-event delivery transport for one Codex profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HookTransport {
    Command,
    McpTool,
}

/// Source-audited, bounded Hook wire contract shared by exact profiles.
///
/// This is intentionally smaller than an upstream source tree: future source
/// admission compares only the declared Hook root, command-group fields,
/// trust-state addressing, and title delegation semantics represented here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodexHookWireShape {
    id: &'static str,
    root_key: &'static str,
    handler_type: &'static str,
    command_field: &'static str,
    windows_command_field: &'static str,
    timeout_field: &'static str,
    async_field: &'static str,
    trust_state_field: &'static str,
}

impl CodexHookWireShape {
    /// Stable bounded protocol identifier for audit evidence and diagnostics.
    #[must_use]
    pub const fn id(self) -> &'static str {
        self.id
    }

    /// Root field that contains lifecycle-event Hook groups.
    #[must_use]
    pub const fn root_key(self) -> &'static str {
        self.root_key
    }

    /// Exact handler type owned by `TabBeacon`.
    #[must_use]
    pub const fn handler_type(self) -> &'static str {
        self.handler_type
    }

    /// Required command binding fields for an owned Windows command Hook.
    #[must_use]
    pub const fn command_fields(self) -> (&'static str, &'static str) {
        (self.command_field, self.windows_command_field)
    }

    /// Required execution-bound fields for an owned command Hook.
    #[must_use]
    pub const fn execution_fields(self) -> (&'static str, &'static str) {
        (self.timeout_field, self.async_field)
    }

    /// Trusted normalized-definition field used by the known Codex shape.
    #[must_use]
    pub const fn trust_state_field(self) -> &'static str {
        self.trust_state_field
    }
}

impl TerminalTitleOwnershipSemantics {
    /// Whether Codex is the ordinary default title owner.
    #[must_use]
    pub const fn codex_owns_by_default(self) -> bool {
        true
    }

    /// The supported configuration mechanism when `TabBeacon` owns the title.
    #[must_use]
    pub const fn tabbeacon_delegation_key(self) -> &'static str {
        "[tui].terminal_title = []"
    }
}

/// Frozen bounded compatibility contract for one Codex Hook wire surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodexHookProfile {
    id: &'static str,
    lifecycle_events: &'static [CodexHookEvent],
    identity: HookIdentitySemantics,
    turn_aware: bool,
    agent_aware: bool,
    compact_aware: bool,
    timeout: HookTimeoutSemantics,
    transport: HookTransport,
    terminal_title_ownership: TerminalTitleOwnershipSemantics,
    unknown_event_policy: UnknownEventPolicy,
    wire_shape: CodexHookWireShape,
    reconciliation_note: &'static str,
}

impl CodexHookProfile {
    /// Stable capability-contract identifier.
    #[must_use]
    pub const fn id(self) -> &'static str {
        self.id
    }

    /// Complete Hook event surface proven for this profile.
    #[must_use]
    pub const fn lifecycle_events(self) -> &'static [CodexHookEvent] {
        self.lifecycle_events
    }

    /// Required identity and ordering fields.
    #[must_use]
    pub const fn identity(self) -> HookIdentitySemantics {
        self.identity
    }

    /// Whether reliable turn identity is available on turn-scoped events.
    #[must_use]
    pub const fn turn_aware(self) -> bool {
        self.turn_aware
    }

    /// Whether thread-spawned subagents are explicitly distinguishable.
    #[must_use]
    pub const fn agent_aware(self) -> bool {
        self.agent_aware
    }

    /// Whether pre/post compact lifecycle is explicitly available.
    #[must_use]
    pub const fn compact_aware(self) -> bool {
        self.compact_aware
    }

    /// Synchronous and timeout behavior of the admitted hook surface.
    #[must_use]
    pub const fn timeout(self) -> HookTimeoutSemantics {
        self.timeout
    }

    /// Whether this bounded profile uses a session-scoped MCP
    /// tool transport instead of a command process for normal Hook delivery.
    #[must_use]
    pub const fn uses_mcp_hook_transport(self) -> bool {
        matches!(self.transport, HookTransport::McpTool)
    }

    /// Title ownership behavior of the bounded Codex Hook contract.
    #[must_use]
    pub const fn terminal_title_ownership(self) -> TerminalTitleOwnershipSemantics {
        self.terminal_title_ownership
    }

    /// Policy for events not declared by this profile.
    #[must_use]
    pub const fn unknown_event_policy(self) -> UnknownEventPolicy {
        self.unknown_event_policy
    }

    /// Bounded source-audited wire contract used for future delta review.
    #[must_use]
    pub const fn wire_shape(self) -> CodexHookWireShape {
        self.wire_shape
    }

    /// Bounded note for reconciling exact owned declarations on this release.
    #[must_use]
    pub const fn reconciliation_note(self) -> &'static str {
        self.reconciliation_note
    }
}

const RUST_V0_147_0_EVENTS: [CodexHookEvent; 11] = [
    CodexHookEvent::PreToolUse,
    CodexHookEvent::PermissionRequest,
    CodexHookEvent::PostToolUse,
    CodexHookEvent::PreCompact,
    CodexHookEvent::PostCompact,
    CodexHookEvent::SessionStart,
    CodexHookEvent::SessionEnd,
    CodexHookEvent::UserPromptSubmit,
    CodexHookEvent::SubagentStart,
    CodexHookEvent::SubagentStop,
    CodexHookEvent::Stop,
];

const RUST_COMMAND_HOOK_WIRE_V1: CodexHookWireShape = CodexHookWireShape {
    id: "codex-command-hooks-wire-v1",
    root_key: "hooks",
    handler_type: "command",
    command_field: "command",
    windows_command_field: "commandWindows",
    timeout_field: "timeout",
    async_field: "async",
    trust_state_field: "trusted_hash",
};

const COMMAND_HOOKS_V1_PROFILE: CodexHookProfile = CodexHookProfile {
    id: "codex-hooks-command-v1",
    lifecycle_events: &RUST_V0_147_0_EVENTS,
    identity: HookIdentitySemantics {
        session_id_required: true,
        turn_id_required_outside_session_lifecycle: true,
        subagent_identity_required_for_subagent_lifecycle: true,
    },
    turn_aware: true,
    agent_aware: true,
    compact_aware: true,
    timeout: HookTimeoutSemantics {
        synchronous_required: true,
        declaration_timeout_seconds: 1,
        maximum_timeout_seconds: 3,
        timeout_blocks_operation: false,
    },
    transport: HookTransport::Command,
    terminal_title_ownership:
        TerminalTitleOwnershipSemantics::CodexDefaultWithExplicitTabBeaconDelegation,
    unknown_event_policy: UnknownEventPolicy::IgnoreFailOpen,
    wire_shape: RUST_COMMAND_HOOK_WIRE_V1,
    reconciliation_note: "owned-command-hooks-only",
};

// The source audit found an added `mcp_tool` handler family, a session-owned
// prewarmed runtime, and an explicit rejection of `SessionEnd` mcp_tool
// handlers. TabBeacon therefore retains MCP only for normal events and uses
// one owned command Hook for SessionEnd. All third-party MCP servers remain
// outside its ownership boundary.
const MCP_HYBRID_V1_PROFILE: CodexHookProfile = CodexHookProfile {
    id: "codex-hooks-mcp-hybrid-v1",
    lifecycle_events: &RUST_V0_147_0_EVENTS,
    identity: HookIdentitySemantics {
        session_id_required: true,
        turn_id_required_outside_session_lifecycle: true,
        subagent_identity_required_for_subagent_lifecycle: true,
    },
    turn_aware: true,
    agent_aware: true,
    compact_aware: true,
    timeout: HookTimeoutSemantics {
        synchronous_required: true,
        declaration_timeout_seconds: 1,
        maximum_timeout_seconds: 3,
        timeout_blocks_operation: false,
    },
    transport: HookTransport::McpTool,
    terminal_title_ownership:
        TerminalTitleOwnershipSemantics::CodexDefaultWithExplicitTabBeaconDelegation,
    unknown_event_policy: UnknownEventPolicy::IgnoreFailOpen,
    wire_shape: RUST_COMMAND_HOOK_WIRE_V1,
    reconciliation_note: "10-owned-mcp-tool-hooks;1-owned-session-end-command;session-eof-fallback;external-mcp-preserved",
};

impl CodexHookProfile {
    /// The conservative command-hook contract used for a newly discovered,
    /// compatible Codex installation. It is independent of release ordering.
    #[must_use]
    pub const fn command_v1() -> Self {
        COMMAND_HOOKS_V1_PROFILE
    }

    /// The existing hybrid MCP contract. It is selected only for a manifest
    /// that already proves TabBeacon owns this exact declaration family.
    #[must_use]
    pub const fn mcp_hybrid_v1() -> Self {
        MCP_HYBRID_V1_PROFILE
    }
}

/// Capability-derived compatibility for the bounded Codex Hook contract.
///
/// A Codex version is carried elsewhere as diagnostics only. None of these
/// variants is selected from version ordering or a version registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexCompatibilityState {
    /// Required Hook capabilities and an optional local schema fingerprint are
    /// positively established.
    Full(CodexHookProfile),
    /// Required Hooks are established, while an optional enhanced discovery
    /// surface (such as App Server schema generation) is unavailable.
    Degraded(CodexHookProfile),
    /// The installed CLI positively reports that a required Hook capability is
    /// absent or disabled.
    Incompatible,
    /// Discovery failed or did not produce a safe, bounded conclusion.
    Unproven,
}

impl CodexCompatibilityState {
    /// Stable capability-first diagnostic spelling.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Full(_) => "full",
            Self::Degraded(_) => "degraded",
            Self::Incompatible => "incompatible",
            Self::Unproven => "unproven",
        }
    }

    /// The safe Hook contract available to setup and repair.
    #[must_use]
    pub const fn supported_profile(self) -> Option<CodexHookProfile> {
        match self {
            Self::Full(profile) | Self::Degraded(profile) => Some(profile),
            Self::Incompatible | Self::Unproven => None,
        }
    }

    /// Whether the observed capability evidence permits an ownership-gated
    /// configuration mutation.
    #[must_use]
    pub const fn is_supported(self) -> bool {
        matches!(self, Self::Full(_) | Self::Degraded(_))
    }
}
