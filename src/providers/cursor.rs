//! Content-minimal Cursor Hook normalization. This module does not install a
//! Hook or claim a terminal route; runtime admission must bind both first.

use serde_json::{Map, Value};

use crate::core::{Attention, FieldUpdate, Health, Phase, StatePatch};

/// Upper bound for one structured Hook request before JSON parsing.
pub const MAX_CURSOR_HOOK_BYTES: usize = 64 * 1024;

/// The small lifecycle subset needed for presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorEvent {
    SessionStart,
    BeforeSubmitPrompt,
    Stop,
    SessionEnd,
}

/// A normalized event contains only opaque session/generation identities and
/// typed state. It never retains prompts, tool data, transcripts, or email.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CursorLifecycle {
    pub event: CursorEvent,
    pub session_id: String,
    pub generation_id: Option<String>,
    pub patch: StatePatch,
}

/// Content-free parsing disposition. Unsupported events remain fail-open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorParseError {
    Oversize,
    Malformed,
    MissingIdentity,
    UnsupportedOutcome,
}

/// Parses an allowlisted lifecycle event without retaining arbitrary payload.
///
/// A caller must still prove that the event belongs to the exact terminal and
/// current generation before applying its patch or writing color.
///
/// # Errors
///
/// Returns a content-free reason for malformed or insufficient evidence.
pub fn normalize_hook(raw: &[u8]) -> Result<Option<CursorLifecycle>, CursorParseError> {
    if raw.len() > MAX_CURSOR_HOOK_BYTES {
        return Err(CursorParseError::Oversize);
    }
    let value: Value = serde_json::from_slice(raw).map_err(|_| CursorParseError::Malformed)?;
    let object = value.as_object().ok_or(CursorParseError::Malformed)?;
    let Some(event) = object.get("hook_event_name").and_then(Value::as_str) else {
        return Err(CursorParseError::Malformed);
    };
    let event = match event {
        "sessionStart" => CursorEvent::SessionStart,
        "beforeSubmitPrompt" => CursorEvent::BeforeSubmitPrompt,
        "stop" => CursorEvent::Stop,
        "sessionEnd" => CursorEvent::SessionEnd,
        _ => return Ok(None),
    };
    let conversation_id = bounded_identity(object, "conversation_id");
    let hook_session_id = bounded_identity(object, "session_id");
    if conversation_id.is_some() && hook_session_id.is_some() && conversation_id != hook_session_id
    {
        return Err(CursorParseError::MissingIdentity);
    }
    let session_id = conversation_id
        .or(hook_session_id)
        .ok_or(CursorParseError::MissingIdentity)?;
    let generation_id = match event {
        CursorEvent::BeforeSubmitPrompt | CursorEvent::Stop => Some(
            bounded_identity(object, "generation_id").ok_or(CursorParseError::MissingIdentity)?,
        ),
        CursorEvent::SessionStart | CursorEvent::SessionEnd => {
            bounded_identity(object, "generation_id")
        }
    };
    let patch = match event {
        CursorEvent::SessionStart => StatePatch {
            phase: FieldUpdate::set(Phase::Ready),
            attention: FieldUpdate::clear(),
            health: FieldUpdate::clear(),
        },
        CursorEvent::BeforeSubmitPrompt => StatePatch {
            phase: FieldUpdate::set(Phase::Working),
            attention: FieldUpdate::clear(),
            health: FieldUpdate::clear(),
        },
        CursorEvent::Stop => {
            let status = object
                .get("status")
                .and_then(Value::as_str)
                .ok_or(CursorParseError::UnsupportedOutcome)?;
            let (attention, health) = match status {
                "completed" => (
                    FieldUpdate::set(Attention::ResultReady),
                    FieldUpdate::clear(),
                ),
                "aborted" => (FieldUpdate::clear(), FieldUpdate::set(Health::Interrupted)),
                "error" => (FieldUpdate::clear(), FieldUpdate::set(Health::Failed)),
                _ => return Err(CursorParseError::UnsupportedOutcome),
            };
            StatePatch {
                phase: FieldUpdate::set(Phase::WaitingUser),
                attention,
                health,
            }
        }
        CursorEvent::SessionEnd => StatePatch {
            phase: FieldUpdate::set(Phase::Ended),
            attention: FieldUpdate::clear(),
            health: FieldUpdate::unchanged(),
        },
    };
    Ok(Some(CursorLifecycle {
        event,
        session_id,
        generation_id,
        patch,
    }))
}

fn bounded_identity(object: &Map<String, Value>, name: &str) -> Option<String> {
    let value = object.get(name)?.as_str()?;
    (!value.is_empty() && value.len() <= 256 && !value.chars().any(char::is_control))
        .then(|| value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stop_status_is_typed_and_private_content_is_discarded() {
        for (status, health) in [
            ("completed", FieldUpdate::clear()),
            ("aborted", FieldUpdate::set(Health::Interrupted)),
            ("error", FieldUpdate::set(Health::Failed)),
        ] {
            let raw = format!(
                "{{\"hook_event_name\":\"stop\",\"conversation_id\":\"session-a\",\"generation_id\":\"turn-a\",\"status\":\"{status}\",\"user_email\":\"secret@example.org\",\"transcript_path\":\"private-transcript\",\"last_message\":\"private-prompt\"}}"
            );
            let normalized = normalize_hook(raw.as_bytes()).unwrap().unwrap();
            assert_eq!(normalized.patch.health, health);
            let debug = format!("{normalized:?}");
            assert!(!debug.contains("secret@example.org"));
            assert!(!debug.contains("private-transcript"));
            assert!(!debug.contains("private-prompt"));
        }
    }

    #[test]
    fn unknown_or_unauthenticated_event_cannot_claim_state() {
        assert_eq!(
            normalize_hook(br#"{"hook_event_name":"afterAgentThought","text":"private"}"#),
            Ok(None)
        );
        assert_eq!(
            normalize_hook(br#"{"hook_event_name":"stop","status":"error"}"#),
            Err(CursorParseError::MissingIdentity)
        );
        assert_eq!(
            normalize_hook(br#"{"hook_event_name":"stop","conversation_id":"s","generation_id":"g","status":"unknown"}"#),
            Err(CursorParseError::UnsupportedOutcome)
        );
        assert_eq!(
            normalize_hook(br#"{"hook_event_name":"stop","conversation_id":"s1","session_id":"s2","generation_id":"g","status":"completed"}"#),
            Err(CursorParseError::MissingIdentity)
        );
    }

    #[test]
    fn new_structured_prompt_clears_prior_health_without_retaining_prompt() {
        let event = normalize_hook(br#"{"hook_event_name":"beforeSubmitPrompt","conversation_id":"s","generation_id":"g2","prompt":"secret text"}"#)
            .unwrap()
            .unwrap();
        assert_eq!(event.event, CursorEvent::BeforeSubmitPrompt);
        assert_eq!(event.session_id, "s");
        assert_eq!(event.generation_id.as_deref(), Some("g2"));
        assert_eq!(event.patch.phase, FieldUpdate::set(Phase::Working));
        assert_eq!(event.patch.health, FieldUpdate::clear());
        assert!(!format!("{event:?}").contains("secret text"));
    }

    #[test]
    fn oversize_payload_is_rejected_before_json_parse() {
        let oversized = vec![b'x'; MAX_CURSOR_HOOK_BYTES + 1];
        assert_eq!(normalize_hook(&oversized), Err(CursorParseError::Oversize));
    }
}
