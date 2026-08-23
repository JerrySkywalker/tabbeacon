//! Frozen compatibility facts and exact admission for Codex Hook releases.

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

/// Frozen compatibility contract for one source-audited Codex Hook release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodexHookProfile {
    id: &'static str,
    version: (u64, u64, u64),
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
    /// Backward-compatible exact lookup delegated to the authoritative registry.
    #[must_use]
    pub fn for_version(version: (u64, u64, u64)) -> Option<Self> {
        CodexCompatibilityRegistry::classify(Some(version)).supported_profile()
    }

    /// Stable diagnostic profile identifier.
    #[must_use]
    pub const fn id(self) -> &'static str {
        self.id
    }

    /// Exact admitted release version.
    #[must_use]
    pub const fn version(self) -> (u64, u64, u64) {
        self.version
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

    /// Whether this exact source-admitted profile uses a session-scoped MCP
    /// tool transport instead of a command process for normal Hook delivery.
    #[must_use]
    pub const fn uses_mcp_hook_transport(self) -> bool {
        matches!(self.transport, HookTransport::McpTool)
    }

    /// Title ownership behavior of the admitted Codex release.
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

/// A bounded diagnostic record for a version intentionally not admitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KnownUnadmittedCodexVersion {
    version: (u64, u64, u64),
}

impl KnownUnadmittedCodexVersion {
    /// Version that is deliberately not treated as compatible.
    #[must_use]
    pub const fn version(self) -> (u64, u64, u64) {
        self.version
    }
}

/// Exact, offline compatibility classification for the bounded registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexCompatibilityState {
    /// The detected version has an exact source-audited production profile.
    Supported(CodexHookProfile),
    /// The detected version is tracked, but its Hook profile is not audited.
    Experimental(KnownUnadmittedCodexVersion),
    /// The detected version is not represented in the bounded registry.
    Unknown,
    /// The detected version is source-audited but incompatible with this contract.
    Unsupported(KnownUnadmittedCodexVersion),
}

impl CodexCompatibilityState {
    /// Stable diagnostic spelling with no inferred compatibility.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Supported(_) => "supported",
            Self::Experimental(_) => "experimental",
            Self::Unknown => "unknown",
            Self::Unsupported(_) => "unsupported",
        }
    }

    /// Exact admitted profile when, and only when, this state is supported.
    #[must_use]
    pub const fn supported_profile(self) -> Option<CodexHookProfile> {
        match self {
            Self::Supported(profile) => Some(profile),
            Self::Experimental(_) | Self::Unknown | Self::Unsupported(_) => None,
        }
    }

    /// Whether this state authorizes the existing production Hook contract.
    #[must_use]
    pub const fn is_supported(self) -> bool {
        matches!(self, Self::Supported(_))
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

const RUST_V0_147_0_PROFILE: CodexHookProfile = CodexHookProfile {
    id: "codex-hooks-rust-v0.147.0",
    version: (0, 147, 0),
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
const RUST_V0_149_0_PROFILE: CodexHookProfile = CodexHookProfile {
    id: "codex-hooks-rust-v0.149.0",
    version: (0, 149, 0),
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

const ADMITTED_PROFILES: [CodexHookProfile; 2] = [RUST_V0_147_0_PROFILE, RUST_V0_149_0_PROFILE];

// This is a bounded diagnostic marker, not a profile admission or a claim of
// wire compatibility. It keeps an observed fixture version distinguishable from
// an entirely unknown version while preserving exact-only support.
const KNOWN_UNADMITTED: [KnownUnadmittedCodexVersion; 1] = [KnownUnadmittedCodexVersion {
    version: (0, 148, 0),
}];

// Reserve a distinct disposition for a version whose incompatible Hook
// contract is source-audited. Do not add entries here without that evidence.
const KNOWN_UNSUPPORTED: [KnownUnadmittedCodexVersion; 0] = [];

/// The one authoritative offline registry for Codex Hook compatibility.
#[derive(Debug, Clone, Copy, Default)]
pub struct CodexCompatibilityRegistry;

impl CodexCompatibilityRegistry {
    /// All and only production-admitted profiles.
    #[must_use]
    pub const fn admitted_profiles() -> &'static [CodexHookProfile] {
        &ADMITTED_PROFILES
    }

    /// Classifies an observed version without a network lookup or version range.
    #[must_use]
    pub fn classify(version: Option<(u64, u64, u64)>) -> CodexCompatibilityState {
        let Some(version) = version else {
            return CodexCompatibilityState::Unknown;
        };
        let mut index = 0;
        while index < ADMITTED_PROFILES.len() {
            let profile = ADMITTED_PROFILES[index];
            if profile.version == version {
                return CodexCompatibilityState::Supported(profile);
            }
            index += 1;
        }
        let mut index = 0;
        while index < KNOWN_UNADMITTED.len() {
            let entry = KNOWN_UNADMITTED[index];
            if entry.version == version {
                return CodexCompatibilityState::Experimental(entry);
            }
            index += 1;
        }
        let mut index = 0;
        while index < KNOWN_UNSUPPORTED.len() {
            let entry = KNOWN_UNSUPPORTED[index];
            if entry.version == version {
                return CodexCompatibilityState::Unsupported(entry);
            }
            index += 1;
        }
        CodexCompatibilityState::Unknown
    }
}
