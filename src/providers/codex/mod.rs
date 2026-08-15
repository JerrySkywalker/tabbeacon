//! Codex user-global command-hook provider.
//!
//! Raw Codex payloads stop at this module. Only provider-neutral evidence is
//! exposed to the core reconciler.

mod config;
mod runtime;

pub use config::{
    CodexDoctorReport, CodexIntegration, CodexIntegrationError, DoctorStatus, SetupOutcome,
    TitleOwnershipOutcome, UninstallOutcome,
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
    /// A compact start intentionally preserves the currently displayed state.
    PreserveCurrentState,
    /// A forward-compatible event that `TabBeacon` does not claim to understand.
    UnsupportedEvent,
}

/// Provider-neutral evidence plus the local cwd binding owned by the adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedCodexHook {
    evidence: AgentEvidence,
    cwd: PathBuf,
}

impl NormalizedCodexHook {
    /// Returns the provider-neutral evidence record.
    #[must_use]
    pub const fn evidence(&self) -> &AgentEvidence {
        &self.evidence
    }

    /// Returns the local cwd used for repository identity resolution.
    #[must_use]
    pub fn cwd(&self) -> &std::path::Path {
        &self.cwd
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

        let patch = match event_name {
            "SessionStart" => match required_string(object, "source")? {
                "startup" | "resume" | "clear" => StatePatch {
                    phase: FieldUpdate::set(Phase::Ready),
                    attention: FieldUpdate::clear(),
                    health: FieldUpdate::unchanged(),
                },
                "compact" => return Ok(CodexNormalization::PreserveCurrentState),
                _ => return Ok(CodexNormalization::UnsupportedEvent),
            },
            "UserPromptSubmit" | "PreToolUse" | "PostToolUse" => StatePatch {
                phase: FieldUpdate::set(Phase::Working),
                attention: FieldUpdate::clear(),
                health: FieldUpdate::unchanged(),
            },
            "PermissionRequest" => StatePatch {
                phase: FieldUpdate::set(Phase::WaitingUser),
                attention: FieldUpdate::set(Attention::Approval),
                health: FieldUpdate::unchanged(),
            },
            "Stop" => StatePatch {
                phase: FieldUpdate::set(Phase::WaitingUser),
                attention: FieldUpdate::set(Attention::ResultReady),
                health: FieldUpdate::unchanged(),
            },
            "SessionEnd" => StatePatch {
                phase: FieldUpdate::set(Phase::Ended),
                attention: FieldUpdate::clear(),
                health: FieldUpdate::unchanged(),
            },
            _ => return Ok(CodexNormalization::UnsupportedEvent),
        };

        let session_id = required_string(object, "session_id")?;
        let cwd = required_string(object, "cwd")?;
        let provider = AgentProvider::new(PROVIDER_ID)
            .map_err(|_| CodexHookError::InvalidIdentifier("provider"))?;
        let session = AgentSessionKey::new(provider, session_id)
            .map_err(|_| CodexHookError::InvalidIdentifier("session ID"))?;
        let source = EvidenceSource::new(BACKEND_ID, SOURCE_INSTANCE)
            .map_err(|_| CodexHookError::InvalidIdentifier("evidence source"))?;
        let tie_break =
            EvidenceTieBreak::new(format!("{event_name}:{}", canonical_json_digest(&value)))
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
            cwd: PathBuf::from(cwd),
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

fn canonical_json_digest(value: &Value) -> String {
    let canonical = canonical_json(value);
    let bytes = serde_json::to_vec(&canonical).expect("JSON values always serialize");
    format!("{:x}", Sha256::digest(bytes))
}

fn canonical_json(value: &Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut entries = object.iter().collect::<Vec<_>>();
            entries.sort_unstable_by(|left, right| left.0.cmp(right.0));
            Value::Object(
                entries
                    .into_iter()
                    .map(|(key, value)| (key.clone(), canonical_json(value)))
                    .collect(),
            )
        }
        Value::Array(values) => Value::Array(values.iter().map(canonical_json).collect()),
        other => other.clone(),
    }
}
