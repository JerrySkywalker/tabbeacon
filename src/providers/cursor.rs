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

/// A bounded, in-memory route for one positively supplied terminal/session
/// binding. The caller must independently prove the console binding before
/// constructing or using this guard; this type does not grant Hook trust.
#[derive(Debug, Clone)]
pub struct CursorSessionRoute {
    terminal_binding_sha256: String,
    session_id: String,
    active_generation_id: Option<String>,
    seen_generations: Vec<String>,
    ended: bool,
}

impl CursorSessionRoute {
    /// Begins one route only from a sessionStart event on the expected console.
    #[must_use]
    pub fn from_session_start(
        event: &CursorLifecycle,
        expected_terminal_binding_sha256: &str,
        observed_terminal_binding_sha256: &str,
        console_openable: bool,
    ) -> Option<Self> {
        (event.event == CursorEvent::SessionStart
            && console_openable
            && is_sha256(expected_terminal_binding_sha256)
            && expected_terminal_binding_sha256 == observed_terminal_binding_sha256)
            .then(|| Self {
                terminal_binding_sha256: expected_terminal_binding_sha256.to_owned(),
                session_id: event.session_id.clone(),
                active_generation_id: None,
                seen_generations: Vec::new(),
                ended: false,
            })
    }

    /// Admits only events from the exact bound terminal and current generation.
    /// A seen superseded generation cannot reopen a newer one.
    pub fn admit(
        &mut self,
        event: &CursorLifecycle,
        observed_terminal_binding_sha256: &str,
        console_openable: bool,
    ) -> bool {
        if self.ended
            || !console_openable
            || observed_terminal_binding_sha256 != self.terminal_binding_sha256
            || event.session_id != self.session_id
        {
            return false;
        }
        match event.event {
            CursorEvent::SessionStart => false,
            CursorEvent::BeforeSubmitPrompt => {
                let Some(generation) = event.generation_id.as_ref() else {
                    return false;
                };
                if self.active_generation_id.as_ref() == Some(generation) {
                    return true;
                }
                if self.seen_generations.contains(generation) {
                    return false;
                }
                if self.seen_generations.len() == 32 {
                    // A bounded guard cannot safely forget an older generation
                    // and then treat its late event as new.
                    return false;
                }
                self.seen_generations.push(generation.clone());
                self.active_generation_id = Some(generation.clone());
                true
            }
            CursorEvent::Stop => {
                if self.active_generation_id.as_ref() != event.generation_id.as_ref() {
                    return false;
                }
                self.active_generation_id = None;
                true
            }
            CursorEvent::SessionEnd => {
                self.active_generation_id = None;
                self.ended = true;
                true
            }
        }
    }
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
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

    #[test]
    fn route_rejects_foreign_terminal_session_and_superseded_generation() {
        fn event(name: &str, session: &str, generation: Option<&str>) -> CursorLifecycle {
            let mut input = serde_json::json!({
                "hook_event_name": name,
                "conversation_id": session,
            });
            if let Some(generation) = generation {
                input["generation_id"] = generation.into();
            }
            if name == "stop" {
                input["status"] = "completed".into();
            }
            normalize_hook(input.to_string().as_bytes())
                .unwrap()
                .unwrap()
        }
        let terminal = "a".repeat(64);
        let foreign_terminal = "b".repeat(64);
        let start = event("sessionStart", "session-a", None);
        assert!(
            CursorSessionRoute::from_session_start(&start, &terminal, &foreign_terminal, true)
                .is_none()
        );
        assert!(
            CursorSessionRoute::from_session_start(&start, &terminal, &terminal, false).is_none()
        );
        let mut route =
            CursorSessionRoute::from_session_start(&start, &terminal, &terminal, true).unwrap();
        let first = event("beforeSubmitPrompt", "session-a", Some("g1"));
        let second = event("beforeSubmitPrompt", "session-a", Some("g2"));
        assert!(route.admit(&first, &terminal, true));
        assert!(!route.admit(&second, &foreign_terminal, true));
        assert!(!route.admit(&event("stop", "session-b", Some("g1")), &terminal, true));
        assert!(route.admit(&second, &terminal, true));
        assert!(!route.admit(&event("stop", "session-a", Some("g1")), &terminal, true));
        assert!(!route.admit(&first, &terminal, true));
        assert!(route.admit(&event("stop", "session-a", Some("g2")), &terminal, true));
        assert!(!route.admit(&second, &terminal, true));
        assert!(route.admit(&event("sessionEnd", "session-a", None), &terminal, true));
        assert!(!route.admit(
            &event("beforeSubmitPrompt", "session-a", Some("g3")),
            &terminal,
            true
        ));
    }
}
