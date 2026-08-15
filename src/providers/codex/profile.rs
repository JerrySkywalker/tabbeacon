//! Frozen compatibility facts for admitted Codex Hook releases.

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

/// Frozen compatibility contract for one source-audited Codex Hook release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodexHookProfile {
    id: &'static str,
    version: (u64, u64, u64),
    lifecycle_events: &'static [CodexHookEvent],
    turn_aware: bool,
    agent_aware: bool,
    compact_aware: bool,
    unknown_event_policy: UnknownEventPolicy,
}

impl CodexHookProfile {
    /// Profile proven from the official `openai/codex` `rust-v0.147.0` tag.
    pub const RUST_V0_147_0: Self = Self {
        id: "codex-hooks-rust-v0.147.0",
        version: (0, 147, 0),
        lifecycle_events: &RUST_V0_147_0_EVENTS,
        turn_aware: true,
        agent_aware: true,
        compact_aware: true,
        unknown_event_policy: UnknownEventPolicy::IgnoreFailOpen,
    };

    /// Selects an exact source-audited profile without assuming later releases.
    #[must_use]
    pub const fn for_version(version: (u64, u64, u64)) -> Option<Self> {
        if version.0 == 0 && version.1 == 147 && version.2 == 0 {
            Some(Self::RUST_V0_147_0)
        } else {
            None
        }
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

    /// Policy for events not declared by this profile.
    #[must_use]
    pub const fn unknown_event_policy(self) -> UnknownEventPolicy {
        self.unknown_event_policy
    }
}
