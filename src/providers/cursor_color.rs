//! Cursor's exact-terminal color lease, called only while `CursorRouteStore`'s
//! cross-process route lock is held. No Hook protocol output is written here.

use std::{
    fs::{self, File},
    io::{self, Read, Write},
    path::Path,
};

use atomic_write_file::AtomicWriteFile;
use serde::{Deserialize, Serialize};

use crate::{
    core::{FieldUpdate, Health},
    presentation::{TabColor, owned_channel_release_bytes, strict_color_only_bytes},
    presentation_policy::ResolvedPresentation,
    settings::{ActivityMode, TabColorMode, TitleMode},
};

use super::cursor::{CursorEvent, CursorLifecycle};

const SCHEMA: &str = "tabbeacon-cursor-color-v1";
const MAX_LEASE_BYTES: u64 = 2_048;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum LeaseState {
    Unowned,
    // A Hook was interrupted between intent and a confirmed flush. Never
    // reset this ambiguous channel on behalf of the old session.
    Unknown,
    Owned,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ColorLease {
    schema: String,
    terminal_sha256: String,
    session_sha256: String,
    generation_sha256: Option<String>,
    color: Option<String>,
    state: LeaseState,
    // Aggregate only admitted output attempts for this session. Older v1
    // leases have no counters and remain readable with an unknown history.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    evidence: Option<ColorEvidence>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ColorEvidence {
    history_complete: bool,
    prompt_attempts: u16,
    stop_attempts: u16,
    color_attempts: u16,
    color_flushes: u16,
    release_attempts: u16,
    release_flushes: u16,
    session_end_seen: bool,
}

impl ColorLease {
    fn new(terminal: &str, session: &str) -> Self {
        Self {
            schema: SCHEMA.to_owned(),
            terminal_sha256: terminal.to_owned(),
            session_sha256: session.to_owned(),
            generation_sha256: None,
            color: None,
            state: LeaseState::Unowned,
            evidence: Some(ColorEvidence {
                history_complete: true,
                ..ColorEvidence::default()
            }),
        }
    }

    fn valid(&self, terminal: &str) -> bool {
        self.schema == SCHEMA
            && self.terminal_sha256 == terminal
            && is_sha256(&self.session_sha256)
            && self.generation_sha256.as_deref().is_none_or(is_sha256)
            && self.color.as_deref().is_none_or(|value| {
                matches!(value, "working" | "result_ready" | "interrupted" | "failed")
            })
            && self.evidence.as_ref().is_none_or(|evidence| {
                evidence.color_flushes <= evidence.color_attempts
                    && evidence.release_flushes <= evidence.release_attempts
                    && u32::from(evidence.prompt_attempts) + u32::from(evidence.stop_attempts)
                        >= u32::from(evidence.color_attempts)
            })
            && (self.state == LeaseState::Unowned
                || (self.generation_sha256.is_some() && self.color.is_some()))
    }
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn lease_path(state_root: &Path, terminal: &str) -> std::path::PathBuf {
    state_root
        .join("cursor-route-v1")
        .join(format!("color-{terminal}.json"))
}

fn load(path: &Path, terminal: &str) -> io::Result<Option<ColorLease>> {
    reject_link(path)?;
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let mut bytes = Vec::new();
    file.take(MAX_LEASE_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_LEASE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Cursor color lease oversized",
        ));
    }
    let lease: ColorLease = serde_json::from_slice(&bytes)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid Cursor color lease"))?;
    if !lease.valid(terminal) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Cursor color lease drift",
        ));
    }
    Ok(Some(lease))
}

fn save(path: &Path, lease: &ColorLease) -> io::Result<()> {
    reject_link(path)?;
    let bytes = serde_json::to_vec(lease).map_err(io::Error::other)?;
    let mut file = AtomicWriteFile::options().open(path)?;
    file.write_all(&bytes)?;
    file.flush()?;
    file.commit()
}

fn reject_link(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Cursor color lease cannot use a symbolic link",
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn strict_color_enabled(resolved: &ResolvedPresentation) -> bool {
    // The shared resolver can produce the same safe channel combination from
    // global defaults or a partial override. Output must agree with its
    // effective color, regardless of where that preference originated.
    matches!(
        resolved.effective.title(),
        TitleMode::Native | TitleMode::Off
    ) && resolved.effective.tab_color() == TabColorMode::TabBeacon
        && matches!(
            resolved.effective.activity(),
            ActivityMode::Off | ActivityMode::Native
        )
}

fn event_color(event: &CursorLifecycle) -> Option<(TabColor, &'static str)> {
    match event.event {
        CursorEvent::BeforeSubmitPrompt => Some((TabColor::Working, "working")),
        CursorEvent::Stop => match event.patch.health {
            FieldUpdate::Set(Health::Interrupted) => Some((TabColor::Interrupted, "interrupted")),
            FieldUpdate::Set(Health::Failed) => Some((TabColor::Failed, "failed")),
            _ => Some((TabColor::ResultReady, "result_ready")),
        },
        CursorEvent::SessionStart | CursorEvent::SessionEnd => None,
    }
}

/// Applies one admitted lifecycle transition to its exact terminal while the
/// route lock is held. The returned flag reports a successful output flush;
/// it is never a claim that Windows Terminal visibly rendered the color.
///
/// # Errors
///
/// Returns a bounded state or sink error; the Hook caller remains fail-open.
pub fn apply_admitted(
    state_root: &Path,
    terminal: &str,
    event: &CursorLifecycle,
    resolved: &ResolvedPresentation,
    sink: &mut impl Write,
) -> io::Result<bool> {
    let path = lease_path(state_root, terminal);
    let mut lease =
        load(&path, terminal)?.unwrap_or_else(|| ColorLease::new(terminal, &event.session_sha256));
    if event.event == CursorEvent::SessionStart {
        let old_owned =
            lease.session_sha256 != event.session_sha256 && lease.state == LeaseState::Owned;
        if old_owned {
            // Revoke release authority before touching the terminal. An old
            // sessionEnd can never reset a color acquired by this new session.
            lease.state = LeaseState::Unowned;
            save(&path, &lease)?;
            let bytes = owned_channel_release_bytes(true, false);
            sink.write_all(&bytes)?;
            sink.flush()?;
        }
        save(&path, &ColorLease::new(terminal, &event.session_sha256))?;
        return Ok(old_owned);
    }
    if lease.session_sha256 != event.session_sha256 {
        return Ok(false);
    }
    if event.event == CursorEvent::SessionEnd || !strict_color_enabled(resolved) {
        if event.event == CursorEvent::SessionEnd {
            lease.evidence.get_or_insert_default().session_end_seen = true;
        }
        if lease.state != LeaseState::Owned {
            // An accepted end is recorded even when this session had no color
            // to release. An unknown partial write grants no reset authority.
            if event.event == CursorEvent::SessionEnd {
                save(&path, &lease)?;
            }
            return Ok(false);
        }
        let evidence = lease.evidence.get_or_insert_default();
        evidence.release_attempts = evidence.release_attempts.saturating_add(1);
        lease.state = LeaseState::Unowned;
        lease.generation_sha256 = None;
        lease.color = None;
        save(&path, &lease)?;
        let bytes = owned_channel_release_bytes(true, false);
        sink.write_all(&bytes)?;
        sink.flush()?;
        let evidence = lease.evidence.get_or_insert_default();
        evidence.release_flushes = evidence.release_flushes.saturating_add(1);
        save(&path, &lease)?;
        return Ok(true);
    }
    let Some((color, name)) = event_color(event) else {
        return Ok(false);
    };
    if lease.state == LeaseState::Owned
        && lease.generation_sha256 == event.generation_sha256
        && lease.color.as_deref() == Some(name)
    {
        return Ok(false);
    }
    lease.state = LeaseState::Unknown;
    let evidence = lease.evidence.get_or_insert_default();
    evidence.color_attempts = evidence.color_attempts.saturating_add(1);
    match event.event {
        CursorEvent::BeforeSubmitPrompt => {
            evidence.prompt_attempts = evidence.prompt_attempts.saturating_add(1);
        }
        CursorEvent::Stop => {
            evidence.stop_attempts = evidence.stop_attempts.saturating_add(1);
        }
        CursorEvent::SessionStart | CursorEvent::SessionEnd => {}
    }
    lease.generation_sha256.clone_from(&event.generation_sha256);
    lease.color = Some(name.to_owned());
    save(&path, &lease)?;
    let bytes = strict_color_only_bytes(color, resolved.effective.theme());
    sink.write_all(&bytes)?;
    sink.flush()?;
    lease.state = LeaseState::Owned;
    let evidence = lease.evidence.get_or_insert_default();
    evidence.color_flushes = evidence.color_flushes.saturating_add(1);
    save(&path, &lease)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        presentation_policy::{
            ApplicationStatus, PresentationCapabilities, PresentationMode, PresentationOverride,
            resolve_presentation,
        },
        providers::cursor::normalize_hook,
        settings::PresentationSettings,
    };

    struct PartialSink(Vec<u8>);

    impl Write for PartialSink {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.extend_from_slice(&bytes[..bytes.len().min(3)]);
            Err(io::Error::other("injected partial terminal write"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn interrupted_color_write_never_grants_later_reset_authority() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("cursor-route-v1")).unwrap();
        let terminal = "a".repeat(64);
        let mode = |presentation| {
            resolve_presentation(
                PresentationSettings::default(),
                PresentationOverride::default().with_mode(presentation),
                PresentationCapabilities::CURSOR_COLOR_ONLY,
                ApplicationStatus::Unproven,
            )
        };
        let color = mode(PresentationMode::ColorOnly);
        let native = mode(PresentationMode::PreserveNative);
        let event = |name: &str, session: &str, generation: Option<&str>| {
            let mut input = serde_json::json!({"hook_event_name":name,"session_id":session});
            if let Some(generation) = generation {
                input["generation_id"] = generation.into();
            }
            normalize_hook(input.to_string().as_bytes())
                .unwrap()
                .unwrap()
        };
        let start = event("sessionStart", "one", None);
        let prompt = event("beforeSubmitPrompt", "one", Some("g1"));
        let end = event("sessionEnd", "one", None);
        let mut bytes = Vec::new();
        assert!(!apply_admitted(root.path(), &terminal, &start, &color, &mut bytes).unwrap());
        assert!(!apply_admitted(root.path(), &terminal, &prompt, &native, &mut bytes).unwrap());
        assert!(bytes.is_empty());
        let mut failing = PartialSink(Vec::new());
        assert!(apply_admitted(root.path(), &terminal, &prompt, &color, &mut failing).is_err());
        assert_eq!(
            load(&lease_path(root.path(), &terminal), &terminal)
                .unwrap()
                .unwrap()
                .state,
            LeaseState::Unknown
        );
        let failed = load(&lease_path(root.path(), &terminal), &terminal)
            .unwrap()
            .unwrap();
        let evidence = failed.evidence.unwrap();
        assert_eq!((evidence.color_attempts, evidence.color_flushes), (1, 0));
        assert!(!apply_admitted(root.path(), &terminal, &end, &color, &mut bytes).unwrap());
        assert!(
            bytes.is_empty(),
            "ambiguous partial write has no reset authority"
        );
        let ended = load(&lease_path(root.path(), &terminal), &terminal)
            .unwrap()
            .unwrap();
        assert!(ended.evidence.unwrap().session_end_seen);
        let next = event("sessionStart", "two", None);
        assert!(!apply_admitted(root.path(), &terminal, &next, &native, &mut bytes).unwrap());
        assert!(
            bytes.is_empty(),
            "new native session does not reset unknown output"
        );
    }

    #[test]
    fn inherited_effective_cursor_color_is_not_silently_suppressed() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("cursor-route-v1")).unwrap();
        let terminal = "b".repeat(64);
        let resolved = resolve_presentation(
            PresentationSettings::default(),
            PresentationOverride::default(),
            PresentationCapabilities::CURSOR_COLOR_ONLY,
            ApplicationStatus::Unproven,
        );
        assert_eq!(resolved.effective.tab_color(), TabColorMode::TabBeacon);
        let prompt = normalize_hook(
            br#"{"hook_event_name":"beforeSubmitPrompt","session_id":"inherited","generation_id":"g1"}"#
        ).unwrap().unwrap();
        let mut bytes = Vec::new();
        assert!(apply_admitted(root.path(), &terminal, &prompt, &resolved, &mut bytes).unwrap());
        assert!(bytes.starts_with(b"\x1b]4;264;rgb:"));
        assert!(
            !bytes
                .windows(4)
                .any(|part| part == b"]0;" || part == b"]9;4")
        );
    }

    #[test]
    fn flushed_color_and_exact_release_leave_content_minimal_session_counts() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("cursor-route-v1")).unwrap();
        let terminal = "c".repeat(64);
        let resolved = resolve_presentation(
            PresentationSettings::default(),
            PresentationOverride::default().with_mode(PresentationMode::ColorOnly),
            PresentationCapabilities::CURSOR_COLOR_ONLY,
            ApplicationStatus::Unproven,
        );
        let event = |name: &str, generation: Option<&str>| {
            let mut input = serde_json::json!({"hook_event_name":name,"session_id":"one"});
            if let Some(generation) = generation {
                input["generation_id"] = generation.into();
            }
            if name == "stop" {
                input["status"] = "completed".into();
            }
            normalize_hook(input.to_string().as_bytes())
                .unwrap()
                .unwrap()
        };
        let mut sink = Vec::new();
        assert!(
            !apply_admitted(
                root.path(),
                &terminal,
                &event("sessionStart", None),
                &resolved,
                &mut sink
            )
            .unwrap()
        );
        for generation in ["g1", "g2"] {
            assert!(
                apply_admitted(
                    root.path(),
                    &terminal,
                    &event("beforeSubmitPrompt", Some(generation)),
                    &resolved,
                    &mut sink
                )
                .unwrap()
            );
            assert!(
                apply_admitted(
                    root.path(),
                    &terminal,
                    &event("stop", Some(generation)),
                    &resolved,
                    &mut sink
                )
                .unwrap()
            );
        }
        assert!(
            apply_admitted(
                root.path(),
                &terminal,
                &event("sessionEnd", None),
                &resolved,
                &mut sink
            )
            .unwrap()
        );
        let lease = load(&lease_path(root.path(), &terminal), &terminal)
            .unwrap()
            .unwrap();
        assert_eq!(lease.state, LeaseState::Unowned);
        let evidence = lease.evidence.unwrap();
        assert!(evidence.history_complete);
        assert_eq!((evidence.color_attempts, evidence.color_flushes), (4, 4));
        assert_eq!((evidence.prompt_attempts, evidence.stop_attempts), (2, 2));
        assert_eq!(
            (evidence.release_attempts, evidence.release_flushes),
            (1, 1)
        );
        assert!(evidence.session_end_seen);
    }

    #[test]
    fn legacy_lease_history_remains_unknown_after_compatibility_read() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("cursor-route-v1")).unwrap();
        let terminal = "d".repeat(64);
        let path = lease_path(root.path(), &terminal);
        let mut legacy = serde_json::to_value(ColorLease::new(&terminal, &"e".repeat(64))).unwrap();
        legacy.as_object_mut().unwrap().remove("evidence");
        fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
        assert!(load(&path, &terminal).unwrap().unwrap().evidence.is_none());
        legacy["evidence"] = serde_json::json!({
            "history_complete": true,
            "prompt_attempts": 0,
            "stop_attempts": 0,
            "color_attempts": 0,
            "color_flushes": 1,
            "release_attempts": 0,
            "release_flushes": 0,
            "session_end_seen": false
        });
        fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
        assert!(load(&path, &terminal).is_err());
    }

    #[test]
    fn partial_release_records_attempt_without_claiming_flush_or_reset_authority() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("cursor-route-v1")).unwrap();
        let terminal = "f".repeat(64);
        let resolved = resolve_presentation(
            PresentationSettings::default(),
            PresentationOverride::default().with_mode(PresentationMode::ColorOnly),
            PresentationCapabilities::CURSOR_COLOR_ONLY,
            ApplicationStatus::Unproven,
        );
        let event = |name: &str, generation: Option<&str>| {
            let mut input = serde_json::json!({"hook_event_name":name,"session_id":"one"});
            if let Some(generation) = generation {
                input["generation_id"] = generation.into();
            }
            normalize_hook(input.to_string().as_bytes())
                .unwrap()
                .unwrap()
        };
        let mut sink = Vec::new();
        apply_admitted(
            root.path(),
            &terminal,
            &event("sessionStart", None),
            &resolved,
            &mut sink,
        )
        .unwrap();
        assert!(
            apply_admitted(
                root.path(),
                &terminal,
                &event("beforeSubmitPrompt", Some("g1")),
                &resolved,
                &mut sink
            )
            .unwrap()
        );
        let end = event("sessionEnd", None);
        assert!(
            apply_admitted(
                root.path(),
                &terminal,
                &end,
                &resolved,
                &mut PartialSink(Vec::new())
            )
            .is_err()
        );
        let lease = load(&lease_path(root.path(), &terminal), &terminal)
            .unwrap()
            .unwrap();
        assert_eq!(lease.state, LeaseState::Unowned);
        let evidence = lease.evidence.unwrap();
        assert_eq!((evidence.color_attempts, evidence.color_flushes), (1, 1));
        assert_eq!((evidence.prompt_attempts, evidence.stop_attempts), (1, 0));
        assert_eq!(
            (evidence.release_attempts, evidence.release_flushes),
            (1, 0)
        );
        assert!(evidence.session_end_seen);
        let bytes_before_retry = sink.len();
        assert!(!apply_admitted(root.path(), &terminal, &end, &resolved, &mut sink).unwrap());
        assert_eq!(sink.len(), bytes_before_retry);
    }
}
