//! Session-scoped MCP transport for content-minimal Codex Hook delivery.
//!
//! Codex 0.149 keeps an MCP server connected for the lifetime of one Codex
//! session. This module deliberately accepts only the lifecycle identity fields
//! `TabBeacon` needs; prompt, assistant, tool-input, and tool-response content is
//! rejected before it can reach the product runtime or persistent state.

use std::{
    io::{self, BufRead, BufReader, BufWriter, Write},
    time::SystemTime,
};

use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::console_output::open_owned_console;

use super::{CodexHookEvent, CodexHookRuntime, HookDispatchOutcome};

pub const MCP_HOOK_SERVER_NAME: &str = "tabbeacon-hook";
pub const MCP_HOOK_TOOL_NAME: &str = "tabbeacon_hook_event";
const MAX_ID_BYTES: usize = 512;
const MAX_CWD_BYTES: usize = 32 * 1024;
const MAX_MESSAGE_BYTES: usize = 64 * 1024;

/// Returns the exact content-minimal template for one admitted MCP Hook event.
///
/// `SessionEnd` intentionally has no template: Codex 0.149 does not admit an
/// `mcp_tool` `SessionEnd` hook. The server releases its in-memory session binding
/// on stdio EOF and the existing bounded stale-state recovery remains in force.
#[must_use]
pub fn hook_input_template(event: CodexHookEvent) -> Option<Value> {
    let common = |mut input: Map<String, Value>| {
        input.insert(
            "hook_event_name".to_owned(),
            Value::String(event.as_str().to_owned()),
        );
        input.insert(
            "session_id".to_owned(),
            Value::String("${session_id}".to_owned()),
        );
        Value::Object(input)
    };

    match event {
        CodexHookEvent::SessionEnd => None,
        CodexHookEvent::SessionStart => Some(common(Map::from_iter([
            ("cwd".to_owned(), Value::String("${cwd}".to_owned())),
            ("source".to_owned(), Value::String("${source}".to_owned())),
        ]))),
        CodexHookEvent::PermissionRequest => Some(common(Map::from_iter([(
            "turn_id".to_owned(),
            Value::String("${turn_id}".to_owned()),
        )]))),
        CodexHookEvent::SubagentStart | CodexHookEvent::SubagentStop => {
            Some(common(Map::from_iter([
                ("cwd".to_owned(), Value::String("${cwd}".to_owned())),
                ("turn_id".to_owned(), Value::String("${turn_id}".to_owned())),
                (
                    "agent_id".to_owned(),
                    Value::String("${agent_id}".to_owned()),
                ),
                (
                    "agent_type".to_owned(),
                    Value::String("${agent_type}".to_owned()),
                ),
            ])))
        }
        _ => Some(common(Map::from_iter([
            ("cwd".to_owned(), Value::String("${cwd}".to_owned())),
            ("turn_id".to_owned(), Value::String("${turn_id}".to_owned())),
        ]))),
    }
}

/// A one-session MCP Hook receiver. It has no global listener or cross-session
/// state: Codex owns the stdio process and EOF is its lifetime boundary.
#[derive(Debug)]
pub struct McpHookSession {
    runtime: CodexHookRuntime,
    binding: Option<SessionBinding>,
}

#[derive(Debug, Clone)]
struct SessionBinding {
    session_id: String,
    cwd: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct McpHookInput {
    hook_event_name: String,
    session_id: String,
    #[serde(default)]
    turn_id: Option<String>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    agent_id: Option<String>,
    #[serde(default)]
    agent_type: Option<String>,
}

impl McpHookSession {
    #[must_use]
    pub fn new(runtime: CodexHookRuntime) -> Self {
        Self {
            runtime,
            binding: None,
        }
    }

    /// Dispatches one MCP `tools/call` argument object through the unchanged
    /// provider-neutral Hook reducer and the supplied terminal sink.
    #[must_use]
    pub fn dispatch_to(
        &mut self,
        arguments: &Value,
        observed_at: SystemTime,
        sink: &mut impl Write,
    ) -> HookDispatchOutcome {
        let Ok(input) = serde_json::from_value::<McpHookInput>(arguments.clone()) else {
            return HookDispatchOutcome::DegradedInput;
        };
        let Some(payload) = self.payload_for(input) else {
            return HookDispatchOutcome::DegradedInput;
        };
        self.runtime.dispatch_to(&payload, observed_at, sink)
    }

    /// Applies the internal equivalent of a `SessionEnd` only after the owning
    /// Codex MCP stdio connection reaches EOF. No `SessionEnd` `mcp_tool` hook is
    /// declared for Codex 0.149.
    #[must_use]
    pub fn cleanup_on_eof_to(
        &mut self,
        observed_at: SystemTime,
        sink: &mut impl Write,
    ) -> Option<HookDispatchOutcome> {
        let binding = self.binding.take()?;
        let payload = json!({
            "hook_event_name": "SessionEnd",
            "session_id": binding.session_id,
            "cwd": binding.cwd,
        });
        let raw = serde_json::to_vec(&payload).ok()?;
        Some(self.runtime.dispatch_to(&raw, observed_at, sink))
    }

    fn payload_for(&mut self, input: McpHookInput) -> Option<Vec<u8>> {
        let event = CodexHookEvent::parse(&input.hook_event_name)?;
        if event == CodexHookEvent::SessionEnd || !bounded_nonempty(&input.session_id, MAX_ID_BYTES)
        {
            return None;
        }
        if self
            .binding
            .as_ref()
            .is_some_and(|binding| binding.session_id != input.session_id)
        {
            // Codex owns this stdio process for one session/runtime. Never
            // permit a stray or delayed message to switch the in-memory root
            // binding; the normalizer still receives differing worktree CWDs
            // for the bound session so its durable root-anchor policy can
            // make that explicit authority decision.
            return None;
        }

        let cwd = match input.cwd.as_deref() {
            Some(cwd) if bounded_nonempty(cwd, MAX_CWD_BYTES) => cwd.to_owned(),
            None => self
                .binding
                .as_ref()
                .filter(|binding| binding.session_id == input.session_id)
                .map(|binding| binding.cwd.clone())?,
            Some(_) => return None,
        };

        if event == CodexHookEvent::SessionStart {
            if !matches!(
                input.source.as_deref(),
                Some("startup" | "resume" | "clear" | "compact")
            ) {
                return None;
            }
            self.binding = Some(SessionBinding {
                session_id: input.session_id.clone(),
                cwd: cwd.clone(),
            });
        }

        let mut payload = Map::from_iter([
            (
                "hook_event_name".to_owned(),
                Value::String(event.as_str().to_owned()),
            ),
            ("session_id".to_owned(), Value::String(input.session_id)),
            ("cwd".to_owned(), Value::String(cwd)),
        ]);
        if let Some(turn_id) = input
            .turn_id
            .filter(|value| bounded_nonempty(value, MAX_ID_BYTES))
        {
            payload.insert("turn_id".to_owned(), Value::String(turn_id));
        }
        if let Some(source) = input
            .source
            .filter(|value| bounded_nonempty(value, MAX_ID_BYTES))
        {
            payload.insert("source".to_owned(), Value::String(source));
        }
        if let Some(agent_id) = input
            .agent_id
            .filter(|value| bounded_nonempty(value, MAX_ID_BYTES))
        {
            payload.insert("agent_id".to_owned(), Value::String(agent_id));
        }
        if let Some(agent_type) = input
            .agent_type
            .filter(|value| bounded_nonempty(value, MAX_ID_BYTES))
        {
            payload.insert("agent_type".to_owned(), Value::String(agent_type));
        }
        serde_json::to_vec(&Value::Object(payload)).ok()
    }
}

fn bounded_nonempty(value: &str, limit: usize) -> bool {
    !value.trim().is_empty() && value.len() <= limit
}

/// Runs the internal stdio MCP server used by an owned Codex 0.149 transport.
/// It returns successfully on malformed traffic and never writes diagnostics to
/// stdout, which is reserved for the MCP protocol.
///
/// # Errors
///
/// Returns an I/O error only when stdin/stdout cannot be read, written, or
/// flushed; malformed Hook traffic is handled fail-open.
pub fn run_stdio_hook_server() -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let Some(runtime) = runtime_from_environment() else {
        return serve_stdio(
            BufReader::new(stdin.lock()),
            BufWriter::new(stdout.lock()),
            None,
        );
    };
    let console = open_owned_console().ok();
    serve_stdio_with_runtime(
        BufReader::new(stdin.lock()),
        BufWriter::new(stdout.lock()),
        runtime,
        console,
    )
}

fn runtime_from_environment() -> Option<CodexHookRuntime> {
    CodexHookRuntime::from_system_environment().ok()
}

fn serve_stdio_with_runtime<R: BufRead, W: Write>(
    reader: R,
    writer: W,
    runtime: CodexHookRuntime,
    console: Option<crate::console_output::OwnedConsole>,
) -> io::Result<()> {
    serve_stdio_inner(reader, writer, Some(McpHookSession::new(runtime)), console)
}

fn serve_stdio<R: BufRead, W: Write>(
    reader: R,
    writer: W,
    runtime: Option<CodexHookRuntime>,
) -> io::Result<()> {
    serve_stdio_inner(reader, writer, runtime.map(McpHookSession::new), None)
}

fn serve_stdio_inner<R: BufRead, W: Write>(
    mut reader: R,
    mut writer: W,
    mut session: Option<McpHookSession>,
    mut console: Option<crate::console_output::OwnedConsole>,
) -> io::Result<()> {
    let mut line = String::new();
    while reader.read_line(&mut line)? != 0 {
        if line.len() <= MAX_MESSAGE_BYTES
            && let Ok(request) = serde_json::from_str::<Value>(&line)
            && let Some(response) =
                response_for_request(&request, session.as_mut(), console.as_mut())
        {
            serde_json::to_writer(&mut writer, &response).map_err(io::Error::other)?;
            writer.write_all(b"\n")?;
            writer.flush()?;
        }
        line.clear();
    }
    if let Some(session) = session.as_mut() {
        if let Some(console) = console.as_mut() {
            let _ = session.cleanup_on_eof_to(SystemTime::now(), console);
        } else {
            let _ = session.cleanup_on_eof_to(SystemTime::now(), &mut io::sink());
        }
    }
    Ok(())
}

fn response_for_request(
    request: &Value,
    session: Option<&mut McpHookSession>,
    console: Option<&mut crate::console_output::OwnedConsole>,
) -> Option<Value> {
    let object = request.as_object()?;
    let id = object.get("id")?.clone();
    let method = object.get("method")?.as_str()?;
    let result = match method {
        "initialize" => json!({
            "protocolVersion": "2025-06-18",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": MCP_HOOK_SERVER_NAME, "version": env!("CARGO_PKG_VERSION") },
        }),
        "tools/list" => json!({ "tools": [hook_tool_definition()] }),
        "tools/call" => {
            let arguments = object
                .get("params")
                .and_then(Value::as_object)
                .filter(|params| {
                    params.get("name").and_then(Value::as_str) == Some(MCP_HOOK_TOOL_NAME)
                })
                .and_then(|params| params.get("arguments"))
                .cloned();
            if let (Some(session), Some(arguments)) = (session, arguments) {
                if let Some(console) = console {
                    let _ = session.dispatch_to(&arguments, SystemTime::now(), console);
                } else {
                    let _ = session.dispatch_to(&arguments, SystemTime::now(), &mut io::sink());
                }
            }
            json!({ "content": [], "isError": false })
        }
        _ => {
            return Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": "method not found" },
            }));
        }
    };
    Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

fn hook_tool_definition() -> Value {
    json!({
        "name": MCP_HOOK_TOOL_NAME,
        "description": "Internal content-minimal TabBeacon lifecycle delivery.",
        "inputSchema": {
            "type": "object",
            "additionalProperties": false,
            "required": ["hook_event_name", "session_id"],
            "properties": {
                "hook_event_name": { "type": "string" },
                "session_id": { "type": "string" },
                "turn_id": { "type": "string" },
                "cwd": { "type": "string" },
                "source": { "type": "string" },
                "agent_id": { "type": "string" },
                "agent_type": { "type": "string" },
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use std::{
        env, fs,
        path::{Path, PathBuf},
        process,
        sync::atomic::{AtomicU64, Ordering},
        time::UNIX_EPOCH,
    };

    use super::*;

    static NEXT_TEST_ROOT: AtomicU64 = AtomicU64::new(0);

    struct TestRoot(PathBuf);

    impl TestRoot {
        fn new() -> Self {
            let suffix = NEXT_TEST_ROOT.fetch_add(1, Ordering::Relaxed);
            let path =
                env::temp_dir().join(format!("tabbeacon-mcp-hook-{}-{suffix}", process::id()));
            fs::create_dir_all(&path).expect("temporary root creates");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn input(event: &str, root: &Path) -> Value {
        json!({
            "hook_event_name": event,
            "session_id": "mcp-session",
            "turn_id": "mcp-turn",
            "cwd": root,
        })
    }

    fn input_for_turn(event: &str, root: &Path, turn_id: &str) -> Value {
        let mut input = input(event, root);
        input["turn_id"] = Value::String(turn_id.to_owned());
        input
    }

    #[test]
    fn templates_are_content_minimal_and_omit_session_end() {
        for event in [
            CodexHookEvent::SessionStart,
            CodexHookEvent::UserPromptSubmit,
            CodexHookEvent::PermissionRequest,
            CodexHookEvent::PreToolUse,
            CodexHookEvent::PostToolUse,
            CodexHookEvent::PreCompact,
            CodexHookEvent::PostCompact,
            CodexHookEvent::SubagentStart,
            CodexHookEvent::SubagentStop,
            CodexHookEvent::Stop,
        ] {
            let template = hook_input_template(event).expect("admitted MCP event template");
            let text = template.to_string();
            assert!(!text.contains("prompt"));
            assert!(!text.contains("tool_input"));
            assert!(!text.contains("tool_response"));
        }
        assert!(hook_input_template(CodexHookEvent::SessionEnd).is_none());
    }

    #[test]
    fn permission_uses_the_bound_session_root_without_content() {
        let root = TestRoot::new();
        let runtime = CodexHookRuntime::new(root.path().join("state"), true);
        let mut session = McpHookSession::new(runtime);
        let mut output = Vec::new();
        let mut start = input("SessionStart", root.path());
        start["source"] = Value::String("startup".to_owned());
        start.as_object_mut().expect("object").remove("turn_id");
        assert_eq!(
            session.dispatch_to(&start, UNIX_EPOCH, &mut output),
            HookDispatchOutcome::Applied
        );

        let permission = json!({
            "hook_event_name": "PermissionRequest",
            "session_id": "mcp-session",
            "turn_id": "mcp-turn",
        });
        assert_eq!(
            session.dispatch_to(&permission, UNIX_EPOCH, &mut output),
            HookDispatchOutcome::Applied
        );
        assert!(String::from_utf8_lossy(&output).contains('!'));
    }

    #[test]
    fn bound_session_rejects_a_cross_session_event_without_rebinding_root() {
        let root = TestRoot::new();
        let runtime = CodexHookRuntime::new(root.path().join("state"), true);
        let mut session = McpHookSession::new(runtime);
        let mut output = Vec::new();
        let mut start = input("SessionStart", root.path());
        start["source"] = Value::String("startup".to_owned());
        start.as_object_mut().expect("object").remove("turn_id");
        assert_eq!(
            session.dispatch_to(&start, UNIX_EPOCH, &mut output),
            HookDispatchOutcome::Applied
        );
        let mut cross_session = input("UserPromptSubmit", root.path());
        cross_session["session_id"] = Value::String("other-session".to_owned());
        assert_eq!(
            session.dispatch_to(&cross_session, UNIX_EPOCH, &mut output),
            HookDispatchOutcome::DegradedInput
        );
        let permission = json!({
            "hook_event_name": "PermissionRequest",
            "session_id": "mcp-session",
            "turn_id": "mcp-turn",
        });
        assert_eq!(
            session.dispatch_to(&permission, UNIX_EPOCH, &mut output),
            HookDispatchOutcome::Applied
        );
    }

    #[test]
    fn unsupported_content_is_rejected_before_runtime_or_state_storage() {
        let root = TestRoot::new();
        let state = root.path().join("state");
        let runtime = CodexHookRuntime::new(&state, true);
        let mut session = McpHookSession::new(runtime);
        let marker = "never-store-this-prompt";
        let hostile = json!({
            "hook_event_name": "UserPromptSubmit",
            "session_id": "mcp-session",
            "turn_id": "mcp-turn",
            "cwd": root.path(),
            "prompt": marker,
        });
        assert_eq!(
            session.dispatch_to(&hostile, UNIX_EPOCH, &mut Vec::new()),
            HookDispatchOutcome::DegradedInput
        );
        let entries = fs::read_dir(&state).map_or_else(
            |_| Vec::new(),
            |items| {
                items
                    .filter_map(Result::ok)
                    .filter_map(|entry| fs::read(entry.path()).ok())
                    .collect::<Vec<_>>()
            },
        );
        assert!(
            entries
                .iter()
                .all(|bytes| !String::from_utf8_lossy(bytes).contains(marker))
        );
    }

    #[test]
    fn eof_cleanup_closes_the_bound_root_without_a_session_end_hook() {
        let root = TestRoot::new();
        let runtime = CodexHookRuntime::new(root.path().join("state"), true);
        let mut session = McpHookSession::new(runtime);
        let mut output = Vec::new();
        let mut start = input("SessionStart", root.path());
        start["source"] = Value::String("startup".to_owned());
        start.as_object_mut().expect("object").remove("turn_id");
        let _ = session.dispatch_to(&start, UNIX_EPOCH, &mut output);
        let _ = session.dispatch_to(
            &input("UserPromptSubmit", root.path()),
            UNIX_EPOCH,
            &mut output,
        );
        assert_eq!(
            session.cleanup_on_eof_to(UNIX_EPOCH, &mut output),
            Some(HookDispatchOutcome::Applied)
        );
        assert!(session.cleanup_on_eof_to(UNIX_EPOCH, &mut output).is_none());
    }

    #[test]
    fn lifecycle_transport_preserves_root_ordering_stop_and_subagent_isolation() {
        let root = TestRoot::new();
        let runtime = CodexHookRuntime::new(root.path().join("state"), true);
        let mut session = McpHookSession::new(runtime);
        let mut output = Vec::new();
        let mut start = input("SessionStart", root.path());
        start["source"] = Value::String("startup".to_owned());
        start.as_object_mut().expect("object").remove("turn_id");
        assert_eq!(
            session.dispatch_to(&start, UNIX_EPOCH, &mut output),
            HookDispatchOutcome::Applied
        );

        for event in ["UserPromptSubmit", "PreToolUse", "PostToolUse"] {
            assert_eq!(
                session.dispatch_to(
                    &input_for_turn(event, root.path(), "turn-1"),
                    UNIX_EPOCH,
                    &mut output,
                ),
                HookDispatchOutcome::Applied,
                "event={event}"
            );
        }
        let permission = json!({
            "hook_event_name": "PermissionRequest",
            "session_id": "mcp-session",
            "turn_id": "turn-1",
        });
        assert_eq!(
            session.dispatch_to(&permission, UNIX_EPOCH, &mut output),
            HookDispatchOutcome::Applied
        );
        for event in ["PreCompact", "PostCompact"] {
            assert_eq!(
                session.dispatch_to(
                    &input_for_turn(event, root.path(), "turn-1"),
                    UNIX_EPOCH,
                    &mut output,
                ),
                HookDispatchOutcome::PreservedCurrentState,
                "event={event}"
            );
        }
        let mut subagent = input_for_turn("SubagentStart", root.path(), "turn-1");
        subagent["agent_id"] = Value::String("thread-1".to_owned());
        subagent["agent_type"] = Value::String("thread".to_owned());
        assert_eq!(
            session.dispatch_to(&subagent, UNIX_EPOCH, &mut output),
            HookDispatchOutcome::IgnoredSubagent
        );
        assert_eq!(
            session.dispatch_to(
                &input_for_turn("Stop", root.path(), "turn-1"),
                UNIX_EPOCH,
                &mut output,
            ),
            HookDispatchOutcome::Applied
        );
        assert_eq!(
            session.dispatch_to(
                &input_for_turn("PostToolUse", root.path(), "turn-1"),
                UNIX_EPOCH,
                &mut output,
            ),
            HookDispatchOutcome::RejectedStaleGeneration
        );
        assert_eq!(
            session.dispatch_to(
                &input_for_turn("UserPromptSubmit", root.path(), "turn-2"),
                UNIX_EPOCH,
                &mut output,
            ),
            HookDispatchOutcome::Applied
        );
        assert_eq!(
            session.dispatch_to(
                &input_for_turn("PostToolUse", root.path(), "turn-1"),
                UNIX_EPOCH,
                &mut output,
            ),
            HookDispatchOutcome::RejectedStaleGeneration
        );
        assert_eq!(
            session.dispatch_to(
                &input_for_turn("Stop", root.path(), "turn-1"),
                UNIX_EPOCH,
                &mut output,
            ),
            HookDispatchOutcome::RejectedStaleGeneration
        );
        assert_eq!(
            session.dispatch_to(
                &input_for_turn("Stop", root.path(), "turn-2"),
                UNIX_EPOCH,
                &mut output,
            ),
            HookDispatchOutcome::Applied
        );
    }

    #[test]
    fn protocol_is_quiet_fail_open_and_advertises_one_hidden_hook_tool() {
        let request = concat!(
            "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}\n",
            "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\",\"params\":{}}\n",
            "{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"tools/call\",\"params\":{\"name\":\"tabbeacon_hook_event\",\"arguments\":{\"prompt\":\"blocked\"}}}\n"
        );
        let mut output = Vec::new();
        serve_stdio(BufReader::new(request.as_bytes()), &mut output, None).expect("stdio serves");
        let lines = String::from_utf8(output).expect("protocol is utf8");
        assert_eq!(lines.lines().count(), 3);
        assert!(lines.contains(MCP_HOOK_TOOL_NAME));
        assert!(!lines.contains("blocked"));
    }
}
