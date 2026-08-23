//! Codex user-global command-hook provider.
//!
//! Raw Codex payloads stop at this module. Only provider-neutral evidence is
//! exposed to the core reconciler.

mod anchor;
mod config;
mod generation;
mod mcp;
mod profile;
mod runtime;

pub use config::{
    CodexDoctorReport, CodexIntegration, CodexIntegrationError, CodexMutationAuthority,
    CodexRepairDisposition, CodexRepairReport, CodexRuntimeContinuity, DoctorStatus, SetupOutcome,
    TitleOwnershipOutcome, UninstallOutcome,
};
pub use mcp::{
    MCP_HOOK_SERVER_NAME, MCP_HOOK_TOOL_NAME, McpHookSession, hook_input_template,
    run_stdio_hook_server,
};
pub use profile::{
    CodexCompatibilityRegistry, CodexCompatibilityState, CodexHookEvent, CodexHookProfile,
    CodexHookWireShape, HookIdentitySemantics, HookTimeoutSemantics, KnownUnadmittedCodexVersion,
    TerminalTitleOwnershipSemantics, UnknownEventPolicy,
};
pub use runtime::{CodexHookRuntime, HookDispatchOutcome};

use std::{fmt, path::PathBuf, time::SystemTime};

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::core::{
    AgentEvidence, AgentProvider, AgentSessionKey, Attention, AuthoritySet, BackendCapabilities,
    EvidenceAuthority, EvidenceConfidence, EvidenceSource, EvidenceTieBreak, FieldUpdate, Phase,
    StatePatch,
};

const PROVIDER_ID: &str = "codex";
const BACKEND_ID: &str = "codex-hooks";
const SOURCE_INSTANCE: &str = "user-global";

/// Provider result after parsing one raw Codex hook payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodexNormalization {
    /// A supported lifecycle event normalized into core evidence.
    Evidence(NormalizedCodexHook),
    /// A compact event intentionally preserves the currently displayed state.
    PreserveCurrentState(CodexHookContext),
    /// A subagent event cannot mutate the root session presentation.
    IgnoreSubagent(CodexHookContext),
    /// A forward-compatible event that `TabBeacon` does not claim to understand.
    UnsupportedEvent,
}

/// The admitted semantic source of a root `SessionStart` event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexSessionStartSource {
    /// A fresh root session began.
    Startup,
    /// A compatible root session resumed.
    Resume,
    /// The provider cleared prior root session state.
    Clear,
}

/// Non-sensitive identity and ordering fields retained from one Hook payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexHookContext {
    event: CodexHookEvent,
    session_id: String,
    turn_id: Option<String>,
    agent_id: Option<String>,
    agent_type: Option<String>,
    session_start_source: Option<CodexSessionStartSource>,
    cwd: PathBuf,
}

impl CodexHookContext {
    /// Exact admitted Hook event.
    #[must_use]
    pub const fn event(&self) -> CodexHookEvent {
        self.event
    }

    /// Durable Codex session identity.
    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Reliable turn identity when the event schema carries one.
    #[must_use]
    pub fn turn_id(&self) -> Option<&str> {
        self.turn_id.as_deref()
    }

    /// Thread-spawned subagent identity when present.
    #[must_use]
    pub fn agent_id(&self) -> Option<&str> {
        self.agent_id.as_deref()
    }

    /// Thread-spawned subagent role/type when present.
    #[must_use]
    pub fn agent_type(&self) -> Option<&str> {
        self.agent_type.as_deref()
    }

    /// Typed root-session binding authority when this is an admitted start.
    #[must_use]
    pub const fn session_start_source(&self) -> Option<CodexSessionStartSource> {
        self.session_start_source
    }

    /// Local working directory used only for offline repository identity.
    #[must_use]
    pub fn cwd(&self) -> &std::path::Path {
        &self.cwd
    }
}

/// Provider-neutral evidence plus the local cwd binding owned by the adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedCodexHook {
    evidence: AgentEvidence,
    context: CodexHookContext,
}

impl NormalizedCodexHook {
    /// Returns the provider-neutral evidence record.
    #[must_use]
    pub const fn evidence(&self) -> &AgentEvidence {
        &self.evidence
    }

    /// Returns non-sensitive event identity needed for generation admission.
    #[must_use]
    pub const fn context(&self) -> &CodexHookContext {
        &self.context
    }

    /// Returns the local cwd used for repository identity resolution.
    #[must_use]
    pub fn cwd(&self) -> &std::path::Path {
        self.context.cwd()
    }
}

/// Safe classification error that never includes raw prompt or tool content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodexHookError {
    /// The input was not one JSON object.
    MalformedJson,
    /// A supported event omitted a required string field.
    MissingField(&'static str),
    /// A provider-neutral identifier could not be constructed.
    InvalidIdentifier(&'static str),
}

impl fmt::Display for CodexHookError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedJson => formatter.write_str("Codex hook input is not a JSON object"),
            Self::MissingField(field) => write!(formatter, "Codex hook input lacks {field}"),
            Self::InvalidIdentifier(kind) => {
                write!(formatter, "Codex hook input has an invalid {kind}")
            }
        }
    }
}

impl std::error::Error for CodexHookError {}

/// Stateless normalizer for the admitted Codex hook surface.
#[derive(Debug, Clone, Copy, Default)]
pub struct CodexHookNormalizer;

impl CodexHookNormalizer {
    /// Returns the exact release profile implemented by this normalizer.
    #[must_use]
    pub fn profile() -> CodexHookProfile {
        CodexCompatibilityRegistry::admitted_profiles()[0]
    }

    /// Declares exactly the semantic axes and authority available from hooks.
    #[must_use]
    pub const fn capabilities() -> BackendCapabilities {
        BackendCapabilities::new(
            AuthoritySet::LIFECYCLE,
            AuthoritySet::LIFECYCLE,
            AuthoritySet::NONE,
        )
    }

    /// Normalizes a single raw hook object without retaining sensitive fields.
    ///
    /// # Errors
    ///
    /// Returns a content-free classification for malformed input, missing
    /// required fields, or invalid provider-neutral identifiers.
    #[allow(clippy::too_many_lines)]
    pub fn normalize(
        self,
        raw: &[u8],
        observed_at: SystemTime,
    ) -> Result<CodexNormalization, CodexHookError> {
        let value: Value =
            serde_json::from_slice(raw).map_err(|_| CodexHookError::MalformedJson)?;
        let object = value.as_object().ok_or(CodexHookError::MalformedJson)?;
        let event_name = object
            .get("hook_event_name")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or(CodexHookError::MissingField("hook_event_name"))?;
        let Some(event) = CodexHookEvent::parse(event_name) else {
            return Ok(CodexNormalization::UnsupportedEvent);
        };
        let session_id = required_string(object, "session_id")?;
        let cwd = required_string(object, "cwd")?;
        let turn_id = if event.requires_turn_id() {
            Some(required_string(object, "turn_id")?.to_owned())
        } else {
            None
        };
        let mut agent_id = optional_string(object, "agent_id");
        let mut agent_type = optional_string(object, "agent_type");
        if event.is_subagent_lifecycle() {
            agent_id = Some(required_string(object, "agent_id")?.to_owned());
            agent_type = Some(required_string(object, "agent_type")?.to_owned());
        }
        let session_start_source = if event == CodexHookEvent::SessionStart {
            match required_string(object, "source")? {
                "startup" => Some(CodexSessionStartSource::Startup),
                "resume" => Some(CodexSessionStartSource::Resume),
                "clear" => Some(CodexSessionStartSource::Clear),
                "compact" => None,
                _ => return Ok(CodexNormalization::UnsupportedEvent),
            }
        } else {
            None
        };
        let context = CodexHookContext {
            event,
            session_id: session_id.to_owned(),
            turn_id,
            agent_id,
            agent_type,
            session_start_source,
            cwd: PathBuf::from(cwd),
        };

        if context.agent_id.is_some()
            || context.agent_type.is_some()
            || event.is_subagent_lifecycle()
        {
            return Ok(CodexNormalization::IgnoreSubagent(context));
        }

        let patch = match event {
            CodexHookEvent::SessionStart => match context.session_start_source() {
                Some(_) => StatePatch {
                    phase: FieldUpdate::set(Phase::Ready),
                    attention: FieldUpdate::clear(),
                    health: FieldUpdate::unchanged(),
                },
                None => return Ok(CodexNormalization::PreserveCurrentState(context)),
            },
            CodexHookEvent::UserPromptSubmit
            | CodexHookEvent::PreToolUse
            | CodexHookEvent::PostToolUse => StatePatch {
                phase: FieldUpdate::set(Phase::Working),
                attention: FieldUpdate::clear(),
                health: FieldUpdate::unchanged(),
            },
            CodexHookEvent::PermissionRequest => StatePatch {
                phase: FieldUpdate::set(Phase::WaitingUser),
                attention: FieldUpdate::set(Attention::Approval),
                health: FieldUpdate::unchanged(),
            },
            CodexHookEvent::Stop => StatePatch {
                phase: FieldUpdate::set(Phase::WaitingUser),
                attention: FieldUpdate::set(Attention::ResultReady),
                health: FieldUpdate::unchanged(),
            },
            CodexHookEvent::SessionEnd => StatePatch {
                phase: FieldUpdate::set(Phase::Ended),
                attention: FieldUpdate::clear(),
                health: FieldUpdate::unchanged(),
            },
            CodexHookEvent::PreCompact | CodexHookEvent::PostCompact => {
                return Ok(CodexNormalization::PreserveCurrentState(context));
            }
            CodexHookEvent::SubagentStart | CodexHookEvent::SubagentStop => {
                unreachable!("explicit subagent events were classified before semantic mapping")
            }
        };

        let provider = AgentProvider::new(PROVIDER_ID)
            .map_err(|_| CodexHookError::InvalidIdentifier("provider"))?;
        let session = AgentSessionKey::new(provider, context.session_id())
            .map_err(|_| CodexHookError::InvalidIdentifier("session ID"))?;
        let source = EvidenceSource::new(BACKEND_ID, SOURCE_INSTANCE)
            .map_err(|_| CodexHookError::InvalidIdentifier("evidence source"))?;
        let tie_break =
            EvidenceTieBreak::new(format!("{event_name}:{}", identity_digest(&context)))
                .map_err(|_| CodexHookError::InvalidIdentifier("tie-break key"))?;
        let evidence = AgentEvidence::new(
            session,
            source,
            EvidenceAuthority::Lifecycle,
            EvidenceConfidence::Standard,
            observed_at,
            tie_break,
            patch,
        );
        Ok(CodexNormalization::Evidence(NormalizedCodexHook {
            evidence,
            context,
        }))
    }
}

fn required_string<'a>(
    object: &'a serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<&'a str, CodexHookError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or(CodexHookError::MissingField(field))
}

fn optional_string(object: &serde_json::Map<String, Value>, field: &str) -> Option<String> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
}

fn identity_digest(context: &CodexHookContext) -> String {
    let mut digest = Sha256::new();
    for value in [
        context.event.as_str(),
        context.session_id(),
        context.turn_id().unwrap_or("root-lifecycle"),
        context.agent_id().unwrap_or("root-agent"),
        context.agent_type().unwrap_or("root-agent"),
    ] {
        digest.update(value.len().to_le_bytes());
        digest.update(value.as_bytes());
    }
    format!("{:x}", digest.finalize())
}
