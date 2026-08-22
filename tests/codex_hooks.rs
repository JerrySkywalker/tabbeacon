#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::{
    env, fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
    sync::{
        Arc, Barrier,
        atomic::{AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tabbeacon::{
    core::{Attention, AuthoritySet, FieldUpdate, Health, Phase, StateAxis},
    hook_inventory::{HookCurrentness, HookInventoryAvailability, HookOwner, HookTrustState},
    providers::codex::{
        CodexCompatibilityRegistry, CodexCompatibilityState, CodexHookError, CodexHookEvent,
        CodexHookNormalizer, CodexHookProfile, CodexHookRuntime, CodexIntegration,
        CodexIntegrationError, CodexMutationAuthority, CodexNormalization, CodexRepairDisposition,
        CodexRuntimeContinuity, DoctorStatus, HookDispatchOutcome, SetupOutcome,
        TitleOwnershipOutcome, UninstallOutcome, UnknownEventPolicy,
    },
    repo::WorkspaceIdentityResolver,
    settings::{
        ActivityMode, PresentationSettings, PresentationTheme, ProviderBadgePolicy, SpinnerPreset,
        TabColorMode, TitleMode,
    },
};
use toml_edit::{Array, DocumentMut, Item, Table, value};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
const ADMITTED_HOOK_EVENTS: [&str; 11] = [
    "PreToolUse",
    "PermissionRequest",
    "PostToolUse",
    "PreCompact",
    "PostCompact",
    "SessionStart",
    "SessionEnd",
    "UserPromptSubmit",
    "SubagentStart",
    "SubagentStop",
    "Stop",
];
const LEGACY_HOOK_EVENTS: [&str; 7] = [
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PermissionRequest",
    "PostToolUse",
    "Stop",
    "SessionEnd",
];

#[cfg(windows)]
const WINDOWS_HOOK_STAGE_TIMEOUT: Duration = Duration::from_secs(10);
#[cfg(windows)]
const WINDOWS_PRODUCTION_HOOK_SLA_TIMEOUT: Duration = Duration::from_millis(900);
const WINDOWS_HOOK_COMMAND_SUFFIX: &str = " hook codex";
const WINDOWS_POWERSHELL_ENVELOPE_PREFIX: &str =
    "powershell.exe -NoProfile -NonInteractive -EncodedCommand ";

struct TestRoot {
    path: PathBuf,
}

impl TestRoot {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after Unix epoch")
            .as_nanos();
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = env::temp_dir().join(format!(
            "tabbeacon-g05-{label}-{}-{nonce}-{counter}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("isolated test root is created");
        Self { path }
    }

    fn child(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn hook_payload(event: &str, session: &str, cwd: &Path) -> Value {
    json!({
        "hook_event_name": event,
        "session_id": session,
        "cwd": cwd,
        "model": "gpt-test",
        "permission_mode": "default",
        "transcript_path": null,
        "turn_id": "turn-1"
    })
}

fn hook_payload_for_turn(event: &str, session: &str, turn: &str, cwd: &Path) -> Value {
    let mut payload = hook_payload(event, session, cwd);
    payload["turn_id"] = Value::String(turn.to_owned());
    payload
}

fn files_under(root: &Path) -> Vec<PathBuf> {
    let mut pending = vec![root.to_owned()];
    let mut files = Vec::new();
    while let Some(path) = pending.pop() {
        if path.is_dir() {
            pending.extend(
                fs::read_dir(path)
                    .expect("state directory reads")
                    .map(|entry| entry.expect("state entry reads").path()),
            );
        } else if path.is_file() {
            files.push(path);
        }
    }
    files
}

fn evidence(value: &Value) -> tabbeacon::core::AgentEvidence {
    let bytes = serde_json::to_vec(value).expect("hook fixture serializes");
    match CodexHookNormalizer
        .normalize(&bytes, UNIX_EPOCH)
        .expect("hook fixture normalizes")
    {
        CodexNormalization::Evidence(normalized) => normalized.evidence().clone(),
        other => panic!("expected evidence, got {other:?}"),
    }
}

fn git(cwd: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "Never")
        .output()
        .expect("local Git executable starts");
    assert!(
        output.status.success(),
        "local Git failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("Git output is UTF-8")
}

fn init_repo(path: &Path, remote: &str) {
    fs::create_dir_all(path).expect("repository directory is created");
    git(path, &["init", "--quiet"]);
    fs::write(path.join("README.md"), "Codex hook integration test\n")
        .expect("repository fixture is written");
    git(path, &["add", "README.md"]);
    git(
        path,
        &[
            "-c",
            "user.name=TabBeacon Test",
            "-c",
            "user.email=tabbeacon@example.invalid",
            "commit",
            "--quiet",
            "-m",
            "fixture",
        ],
    );
    git(path, &["remote", "add", "origin", remote]);
}

fn test_integration(root: &TestRoot) -> CodexIntegration {
    test_integration_with_codex_fixture(root, "codex_version_probe.rs")
}

fn test_integration_with_codex_fixture(root: &TestRoot, fixture: &str) -> CodexIntegration {
    let executable = root.child(if cfg!(windows) {
        "bin/tabbeacon.exe"
    } else {
        "bin/tabbeacon"
    });
    fs::create_dir_all(executable.parent().expect("binary parent"))
        .expect("binary parent is created");
    fs::write(&executable, b"test executable placeholder").expect("binary placeholder is written");
    let codex_probe = compile_codex_probe_fixture(root, fixture);
    CodexIntegration::new(root.child("codex-home"), root.child("state"), executable)
        .with_codex_program(codex_probe)
}

fn compile_codex_probe(root: &TestRoot) -> PathBuf {
    compile_codex_probe_fixture(root, "codex_version_probe.rs")
}

fn compile_codex_probe_fixture(root: &TestRoot, fixture: &str) -> PathBuf {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(fixture);
    let executable = root.child(if cfg!(windows) {
        "codex-version-probe.exe"
    } else {
        "codex-version-probe"
    });
    let compiler = env::var_os("RUSTC").map_or_else(|| "rustc".into(), PathBuf::from);
    let output = Command::new(compiler)
        .args(["--edition=2024"])
        .arg(source)
        .arg("-o")
        .arg(&executable)
        .output()
        .expect("Codex version probe compiler starts");
    assert!(
        output.status.success(),
        "Codex version probe failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    executable
}

fn write_hooks_fixture(codex_home: &Path, name: &str) {
    fs::create_dir_all(codex_home).expect("Codex fixture home is created");
    let fixture = hooks_fixture_path(name);
    fs::copy(fixture, codex_home.join("hooks.json")).expect("hook shape fixture is copied");
}

fn hooks_fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("codex-hook-shapes")
        .join(name)
}

fn hook_group_fixture(name: &str, event: &str) -> Value {
    let fixture: Value = serde_json::from_slice(
        &fs::read(hooks_fixture_path(name)).expect("hook group fixture reads"),
    )
    .expect("hook group fixture parses");
    fixture["hooks"][event][0].clone()
}

#[cfg(windows)]
#[derive(Clone, Copy)]
enum CodexWindowsHookShell {
    Pwsh7,
    WindowsPowerShell,
    Cmd,
    ComspecFallback,
}

#[cfg(windows)]
impl CodexWindowsHookShell {
    fn label(self) -> &'static str {
        match self {
            Self::Pwsh7 => "pwsh.exe/-NoProfile/-Command",
            Self::WindowsPowerShell => "powershell.exe/-NoProfile/-Command",
            Self::Cmd => "cmd.exe//c",
            Self::ComspecFallback => "empty-program/COMSPEC-fallback",
        }
    }
}

#[cfg(windows)]
enum HookStream {
    Stdout(io::Result<Vec<u8>>),
    Stderr(io::Result<Vec<u8>>),
}

#[cfg(windows)]
fn terminate_windows_hook_tree(process_id: u32) {
    let taskkill = env::var_os("SystemRoot").map_or_else(
        || PathBuf::from("taskkill.exe"),
        |root| PathBuf::from(root).join("System32").join("taskkill.exe"),
    );
    let _ = Command::new(taskkill)
        .args(["/PID", &process_id.to_string(), "/T", "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

#[cfg(windows)]
fn receive_hook_stream(
    receiver: &std::sync::mpsc::Receiver<HookStream>,
    deadline: Instant,
    process_id: u32,
    stage: &str,
    stdout: &mut Option<Vec<u8>>,
    stderr: &mut Option<Vec<u8>>,
) {
    let remaining = deadline.saturating_duration_since(Instant::now());
    let stream = receiver.recv_timeout(remaining).unwrap_or_else(|_| {
        terminate_windows_hook_tree(process_id);
        panic!("Windows hook stage {stage} left an output pipe open after its shell exited")
    });
    match stream {
        HookStream::Stdout(Ok(bytes)) => *stdout = Some(bytes),
        HookStream::Stderr(Ok(bytes)) => *stderr = Some(bytes),
        HookStream::Stdout(Err(error)) | HookStream::Stderr(Err(error)) => {
            terminate_windows_hook_tree(process_id);
            panic!("Windows hook stage {stage} could not collect shell output: {error}");
        }
    }
}

#[cfg(windows)]
fn write_and_wait_for_windows_hook(
    mut child: Child,
    input: &[u8],
    stage: &str,
    timeout: Duration,
) -> Output {
    let process_id = child.id();
    let mut stdin = child
        .stdin
        .take()
        .expect("Codex-compatible hook shell exposes stdin");
    stdin
        .write_all(input)
        .expect("Codex-compatible hook shell accepts stdin");
    stdin
        .flush()
        .expect("Codex-compatible hook shell flushes stdin");
    drop(stdin);

    let mut stdout_pipe = child
        .stdout
        .take()
        .expect("Codex-compatible hook shell exposes stdout");
    let mut stderr_pipe = child
        .stderr
        .take()
        .expect("Codex-compatible hook shell exposes stderr");
    let (sender, receiver) = std::sync::mpsc::channel();
    let stdout_sender = sender.clone();
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout_pipe.read_to_end(&mut bytes).map(|_| bytes);
        let _ = stdout_sender.send(HookStream::Stdout(result));
    });
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stderr_pipe.read_to_end(&mut bytes).map(|_| bytes);
        let _ = sender.send(HookStream::Stderr(result));
    });

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(25)),
            Ok(None) => {
                terminate_windows_hook_tree(process_id);
                panic!("Windows hook stage {stage} exceeded {timeout:?}");
            }
            Err(error) => {
                terminate_windows_hook_tree(process_id);
                panic!("Windows hook stage {stage} could not observe its shell: {error}");
            }
        }
    };

    let mut stdout = None;
    let mut stderr = None;
    while stdout.is_none() || stderr.is_none() {
        receive_hook_stream(
            &receiver,
            deadline,
            process_id,
            stage,
            &mut stdout,
            &mut stderr,
        );
    }
    Output {
        status,
        stdout: stdout.expect("stdout is collected"),
        stderr: stderr.expect("stderr is collected"),
    }
}

#[cfg(windows)]
fn run_codex_windows_hook_with_shell(
    shell: CodexWindowsHookShell,
    command_line: &str,
    input: &[u8],
    isolate_runtime_state: bool,
    local_app_data: Option<&Path>,
) -> std::process::Output {
    run_codex_windows_hook_with_shell_and_terminal(
        shell,
        command_line,
        input,
        isolate_runtime_state,
        local_app_data,
        None,
        WINDOWS_HOOK_STAGE_TIMEOUT,
    )
}

#[cfg(windows)]
fn run_codex_windows_hook_with_shell_and_terminal(
    shell: CodexWindowsHookShell,
    command_line: &str,
    input: &[u8],
    isolate_runtime_state: bool,
    local_app_data: Option<&Path>,
    terminal_session: Option<&str>,
    timeout: Duration,
) -> std::process::Output {
    // Mirror Codex 0.149 command_runner semantics exactly. A non-empty
    // TurnEnvironment shell receives normal arguments, except cmd.exe's /c
    // branch uses the runner's raw outer quotation. An empty program falls
    // back to COMSPEC.
    let mut command = match shell {
        CodexWindowsHookShell::Pwsh7 => {
            let mut command = Command::new("pwsh.exe");
            command.args(["-NoProfile", "-Command"]);
            command.arg(command_line);
            command
        }
        CodexWindowsHookShell::WindowsPowerShell => {
            let mut command = Command::new("powershell.exe");
            command.args(["-NoProfile", "-Command"]);
            command.arg(command_line);
            command
        }
        CodexWindowsHookShell::Cmd => {
            let mut command = Command::new("cmd.exe");
            command.arg("/c");
            command.raw_arg(format!(r#""{command_line}""#));
            command
        }
        CodexWindowsHookShell::ComspecFallback => {
            let program = env::var_os("COMSPEC").unwrap_or_else(|| "cmd.exe".into());
            let mut command = Command::new(program);
            command.arg("/C");
            command.raw_arg(format!(r#""{command_line}""#));
            command
        }
    };
    command.stdin(Stdio::piped());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    if let Some(local_app_data) = local_app_data {
        command.env("LOCALAPPDATA", local_app_data);
    } else if isolate_runtime_state {
        command.env_remove("LOCALAPPDATA");
    }
    if let Some(terminal_session) = terminal_session {
        command.env("WT_SESSION", terminal_session);
    } else {
        command.env_remove("WT_SESSION");
    }
    let child = command.spawn().expect("Codex-compatible hook shell starts");
    write_and_wait_for_windows_hook(child, input, shell.label(), timeout)
}

fn codex_event_key(event: &str) -> &'static str {
    match event {
        "PreToolUse" => "pre_tool_use",
        "PermissionRequest" => "permission_request",
        "PostToolUse" => "post_tool_use",
        "PreCompact" => "pre_compact",
        "PostCompact" => "post_compact",
        "SessionStart" => "session_start",
        "SessionEnd" => "session_end",
        "UserPromptSubmit" => "user_prompt_submit",
        "SubagentStart" => "subagent_start",
        "SubagentStop" => "subagent_stop",
        "Stop" => "stop",
        other => panic!("unsupported test event: {other}"),
    }
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

fn normalized_codex_hash(event: &str, group: &Value) -> String {
    let handler = &group["hooks"][0];
    let normalized = json!({
        "event_name": codex_event_key(event),
        "hooks": [{
            "type": "command",
            "command": handler["commandWindows"],
            "timeout": handler["timeout"],
            "async": handler["async"]
        }]
    });
    let bytes = serde_json::to_vec(&canonical_json(&normalized))
        .expect("normalized hook fixture serializes");
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn is_current_windows_hook_command(command: &str) -> bool {
    (command.ends_with(WINDOWS_HOOK_COMMAND_SUFFIX) && !command.contains(['\r', '\n']))
        || command.starts_with(WINDOWS_POWERSHELL_ENVELOPE_PREFIX)
}

fn install_current_codex_trust_state(codex_home: &Path) -> Vec<String> {
    let hooks_path = codex_home.join("hooks.json");
    let hooks: Value =
        serde_json::from_slice(&fs::read(&hooks_path).expect("hooks read")).expect("hooks parse");
    let config_path = codex_home.join("config.toml");
    let mut config = fs::read_to_string(&config_path)
        .expect("config reads")
        .parse::<DocumentMut>()
        .expect("config parses");
    if !config.as_table().contains_key("hooks") {
        config["hooks"] = Item::Table(Table::new());
    }
    if !config["hooks"]
        .as_table_like()
        .expect("hooks config is a table")
        .contains_key("state")
    {
        config["hooks"]["state"] = Item::Table(Table::new());
    }

    let mut keys = Vec::new();
    for event in ADMITTED_HOOK_EVENTS {
        let groups = hooks["hooks"][event]
            .as_array()
            .expect("hook event groups are an array");
        let group_index = groups
            .iter()
            .position(|group| {
                group["hooks"].as_array().is_some_and(|handlers| {
                    handlers.iter().any(|handler| {
                        handler["type"] == "command"
                            && handler["command"].as_str().is_some_and(|command| {
                                command.starts_with(WINDOWS_POWERSHELL_ENVELOPE_PREFIX)
                            })
                            && handler["commandWindows"]
                                .as_str()
                                .is_some_and(|command| is_current_windows_hook_command(command))
                    })
                })
            })
            .expect("owned command hook group is present exactly once");
        let group = &groups[group_index];
        let key = format!(
            "{}:{}:{group_index}:0",
            hooks_path.display(),
            codex_event_key(event)
        );
        let mut trusted = Table::new();
        trusted.insert("trusted_hash", value(normalized_codex_hash(event, group)));
        config["hooks"]["state"]
            .as_table_like_mut()
            .expect("hook state is a table")
            .insert(&key, Item::Table(trusted));
        keys.push(key);
    }
    fs::write(config_path, config.to_string()).expect("trusted config writes");
    keys
}

fn replace_manifest_owned_declarations_with_legacy(hooks: &mut Value, manifest: &mut Value) {
    let legacy_command = r#""C:\\legacy\\tabbeacon.exe" hook codex || exit /b 0"#;
    let current_owned = manifest["hooks"]
        .as_array()
        .expect("manifest owned hooks are an array")
        .clone();
    let mut legacy_owned = Vec::new();
    for mut owned in current_owned {
        let event = owned["event"]
            .as_str()
            .expect("owned event name")
            .to_owned();
        let prior_group = owned["group"].clone();
        if !LEGACY_HOOK_EVENTS.contains(&event.as_str()) {
            hooks["hooks"]
                .as_object_mut()
                .expect("hooks object")
                .remove(&event);
            continue;
        }
        let live_group = hooks["hooks"][&event]
            .as_array_mut()
            .expect("event groups are an array")
            .iter_mut()
            .find(|group| **group == prior_group)
            .expect("owned group is present in hooks");
        for group in [live_group, &mut owned["group"]] {
            group["hooks"][0]["command"] = json!(legacy_command);
            group["hooks"][0]["commandWindows"] = json!(legacy_command);
        }
        legacy_owned.push(owned);
    }
    manifest["hooks"] = Value::Array(legacy_owned);
}

fn remove_exact_manifest_owned_declarations(hooks: &mut Value, manifest: &Value) {
    for declaration in manifest["hooks"]
        .as_array()
        .expect("manifest owned hooks are an array")
    {
        let event = declaration["event"].as_str().expect("owned event name");
        let expected = &declaration["group"];
        let groups = hooks["hooks"][event]
            .as_array_mut()
            .expect("owned event groups are present");
        groups.retain(|group| group != expected);
        if groups.is_empty() {
            hooks["hooks"]
                .as_object_mut()
                .expect("hooks object")
                .remove(event);
        }
    }
}

fn owned_current_windows_handler_count(hooks: &Value) -> usize {
    hooks["hooks"]
        .as_object()
        .expect("hooks object")
        .values()
        .flat_map(|groups| groups.as_array().into_iter().flatten())
        .flat_map(|group| group["hooks"].as_array().into_iter().flatten())
        .filter(|handler| {
            handler["command"]
                .as_str()
                .is_some_and(|command| command.starts_with(WINDOWS_POWERSHELL_ENVELOPE_PREFIX))
                && handler["commandWindows"]
                    .as_str()
                    .is_some_and(|command| is_current_windows_hook_command(command))
        })
        .count()
}

#[cfg(windows)]
fn assert_real_hook_direct(executable: &Path, payload: &[u8]) {
    let child = Command::new(executable)
        .args(["hook", "codex"])
        .env_remove("LOCALAPPDATA")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("real TabBeacon starts");
    let output = write_and_wait_for_windows_hook(
        child,
        payload,
        "direct real hook",
        WINDOWS_HOOK_STAGE_TIMEOUT,
    );
    assert!(
        output.status.success(),
        "real direct hook failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[cfg(windows)]
fn assert_real_hook_with_local_state_within(
    executable: &Path,
    payload: &[u8],
    local_app_data: &Path,
    timeout: Duration,
    stage: &str,
) -> Duration {
    let child = Command::new(executable)
        .args(["hook", "codex"])
        .env("LOCALAPPDATA", local_app_data)
        .env_remove("WT_SESSION")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("real TabBeacon starts");
    let started = Instant::now();
    let output = write_and_wait_for_windows_hook(child, payload, stage, timeout);
    let elapsed = started.elapsed();
    assert!(
        output.status.success(),
        "real Hook failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty(), "real Hook wrote stdout");
    assert!(output.stderr.is_empty(), "real Hook wrote stderr");
    elapsed
}

#[cfg(windows)]
fn assert_real_hook_shell_matrix(command: &str, payload: &[u8]) {
    for shell in [
        CodexWindowsHookShell::Pwsh7,
        CodexWindowsHookShell::WindowsPowerShell,
        CodexWindowsHookShell::Cmd,
        CodexWindowsHookShell::ComspecFallback,
    ] {
        let output = run_codex_windows_hook_with_shell(shell, command, payload, true, None);
        assert!(
            output.status.success(),
            "{} real hook shell failed: {}",
            shell.label(),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            output.stdout.is_empty(),
            "{} wrote hook stdout",
            shell.label()
        );
        assert!(
            output.stderr.is_empty(),
            "{} wrote hook stderr",
            shell.label()
        );
    }
}

#[cfg(windows)]
fn assert_real_hook_ingress(root: &TestRoot, command: &str) {
    let local_app_data = root.child("isolated-local-app-data");
    let payload = serde_json::to_vec(&hook_payload(
        "UserPromptSubmit",
        "real-shell-ingress",
        Path::new(env!("CARGO_MANIFEST_DIR")),
    ))
    .expect("ingress payload serializes");
    let output = run_codex_windows_hook_with_shell(
        CodexWindowsHookShell::Pwsh7,
        command,
        &payload,
        false,
        Some(&local_app_data),
    );
    assert!(
        output.status.success(),
        "real hook ingress failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let registry_root = local_app_data.join("TabBeacon/repository-identity");
    assert!(
        fs::read_dir(registry_root)
            .expect("real hook ingress creates isolated registry state")
            .any(|entry| {
                entry.is_ok_and(|entry| {
                    entry.file_name().to_str().is_some_and(|name| {
                        name.starts_with("registry-v2-")
                            && Path::new(name)
                                .extension()
                                .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
                    })
                })
            }),
        "the real binary must receive Codex stdin and publish v2 repository identity state"
    );
}

#[cfg(windows)]
fn real_windows_hook_command(root: &TestRoot) -> (PathBuf, String) {
    let compiled_binary = PathBuf::from(env!("CARGO_BIN_EXE_tabbeacon"));
    assert!(
        compiled_binary.is_file(),
        "real TabBeacon binary is available"
    );
    let executable = root.child("real binary & quote'").join("tabbeacon.exe");
    fs::create_dir_all(executable.parent().expect("hostile binary parent"))
        .expect("hostile binary parent is created");
    fs::copy(&compiled_binary, &executable)
        .expect("real TabBeacon binary is copied to hostile path");
    let integration = CodexIntegration::new(
        root.child("codex-home"),
        root.child("integration-state"),
        &executable,
    )
    .with_codex_program(compile_codex_probe(root));
    assert_eq!(
        integration.setup().expect("setup succeeds"),
        SetupOutcome::InstalledTrustReviewRequired
    );
    let hooks: Value =
        serde_json::from_slice(&fs::read(root.child("codex-home/hooks.json")).expect("hooks read"))
            .expect("hooks parse");
    let handler = &hooks["hooks"]["UserPromptSubmit"][0]["hooks"][0];
    let command = handler["command"]
        .as_str()
        .expect("generated command is a string");
    let command_windows = handler["commandWindows"]
        .as_str()
        .expect("generated Windows command is a string");
    assert_eq!(command, command_windows);
    assert!(command.starts_with("powershell.exe -NoProfile -NonInteractive -EncodedCommand "));
    assert!(
        !command.contains(executable.to_str().expect("binary path is UTF-8")),
        "the hostile path must retain the encoded PowerShell safety envelope"
    );
    (executable, command.to_owned())
}

#[cfg(windows)]
fn fast_real_windows_hook_command(root: &TestRoot) -> (PathBuf, String, String) {
    let executable = PathBuf::from(env!("CARGO_BIN_EXE_tabbeacon"));
    let executable_text = executable.to_str().expect("compiled binary path is UTF-8");
    assert!(
        !executable_text.chars().any(|character| {
            matches!(
                character,
                '!' | '&'
                    | '|'
                    | ';'
                    | '$'
                    | '\''
                    | '`'
                    | '('
                    | ')'
                    | '<'
                    | '>'
                    | '@'
                    | '#'
                    | '{'
                    | '}'
            )
        }),
        "the isolated test binary path must select the shell-neutral fast declaration"
    );
    let integration = CodexIntegration::new(
        root.child("codex-home"),
        root.child("integration-state"),
        &executable,
    )
    .with_codex_program(compile_codex_probe(root));
    assert_eq!(
        integration.setup().expect("setup succeeds"),
        SetupOutcome::InstalledTrustReviewRequired
    );
    let hooks: Value =
        serde_json::from_slice(&fs::read(root.child("codex-home/hooks.json")).expect("hooks read"))
            .expect("hooks parse");
    let handler = &hooks["hooks"]["UserPromptSubmit"][0]["hooks"][0];
    let command = handler["commandWindows"]
        .as_str()
        .expect("generated Windows command is a string");
    let generic_command = handler["command"]
        .as_str()
        .expect("generated generic command is a string");
    assert_eq!(
        command,
        format!("{executable_text} hook codex"),
        "the fast declaration is a single native command accepted by every admitted shell"
    );
    assert!(
        generic_command.starts_with("powershell.exe -NoProfile -NonInteractive -EncodedCommand "),
        "the generic command retains compatibility for configured PowerShell shells"
    );
    (executable, generic_command.to_owned(), command.to_owned())
}

#[cfg(windows)]
fn trusted_real_windows_integration(root: &TestRoot) -> CodexIntegration {
    let executable = PathBuf::from(env!("CARGO_BIN_EXE_tabbeacon"));
    let integration = CodexIntegration::new(
        root.child("codex-home"),
        root.child("integration-state"),
        executable,
    )
    .with_codex_program(compile_codex_probe(root));
    assert_eq!(
        integration.setup().expect("setup succeeds"),
        SetupOutcome::InstalledTrustReviewRequired
    );
    install_current_codex_trust_state(&root.child("codex-home"));
    integration
}

#[cfg(windows)]
fn capture_windows_hook_stage(
    failures: &mut Vec<&'static str>,
    stage: &'static str,
    operation: impl FnOnce(),
) {
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation)).is_err() {
        failures.push(stage);
    }
}

#[test]
fn capabilities_are_lifecycle_only_and_provider_neutral() {
    let capabilities = CodexHookNormalizer::capabilities();
    assert!(capabilities.supports(
        StateAxis::Phase,
        tabbeacon::core::EvidenceAuthority::Lifecycle
    ));
    assert!(capabilities.supports(
        StateAxis::Attention,
        tabbeacon::core::EvidenceAuthority::Lifecycle
    ));
    assert_eq!(
        capabilities.authorities_for(StateAxis::Health),
        AuthoritySet::NONE
    );
}

#[test]
fn source_audited_profiles_are_explicit_and_future_versions_are_not_assumed() {
    let profile = CodexHookNormalizer::profile();
    assert_eq!(profile.id(), "codex-hooks-rust-v0.147.0");
    assert_eq!(profile.version(), (0, 147, 0));
    assert_eq!(profile.lifecycle_events().len(), 11);
    assert!(
        profile
            .lifecycle_events()
            .contains(&CodexHookEvent::PreCompact)
    );
    assert!(
        profile
            .lifecycle_events()
            .contains(&CodexHookEvent::PostCompact)
    );
    assert!(profile.turn_aware());
    assert!(profile.agent_aware());
    assert!(profile.compact_aware());
    assert!(profile.identity().session_id_required());
    assert!(
        profile
            .identity()
            .turn_id_required_outside_session_lifecycle()
    );
    assert!(
        profile
            .identity()
            .subagent_identity_required_for_subagent_lifecycle()
    );
    assert!(profile.timeout().synchronous_required());
    assert_eq!(profile.timeout().declaration_timeout_seconds(), 1);
    assert_eq!(profile.timeout().maximum_timeout_seconds(), 3);
    assert!(!profile.timeout().timeout_blocks_operation());
    assert!(profile.terminal_title_ownership().codex_owns_by_default());
    assert_eq!(
        profile
            .terminal_title_ownership()
            .tabbeacon_delegation_key(),
        "[tui].terminal_title = []"
    );
    assert_eq!(
        profile.unknown_event_policy(),
        UnknownEventPolicy::IgnoreFailOpen
    );
    assert_eq!(CodexHookProfile::for_version((0, 147, 0)), Some(profile));
    let profile_149 = CodexHookProfile::for_version((0, 149, 0))
        .expect("source-audited 0.149 fixture has an admitted profile");
    assert_eq!(profile_149.id(), "codex-hooks-rust-v0.149.0");
    assert_eq!(profile_149.version(), (0, 149, 0));
    assert_eq!(profile_149.lifecycle_events(), profile.lifecycle_events());
    assert_eq!(profile_149.identity(), profile.identity());
    assert_eq!(profile_149.timeout(), profile.timeout());
    assert_eq!(profile.wire_shape(), profile_149.wire_shape());
    assert_eq!(profile.wire_shape().id(), "codex-command-hooks-wire-v1");
    assert_eq!(profile.wire_shape().root_key(), "hooks");
    assert_eq!(profile.wire_shape().handler_type(), "command");
    assert_eq!(
        profile.wire_shape().command_fields(),
        ("command", "commandWindows")
    );
    assert_eq!(
        profile.wire_shape().execution_fields(),
        ("timeout", "async")
    );
    assert_eq!(profile.wire_shape().trust_state_field(), "trusted_hash");
    assert_eq!(
        profile_149.reconciliation_note(),
        "owned-command-hooks;external-mcp-tool-preserved"
    );
    assert_eq!(
        CodexCompatibilityRegistry::admitted_profiles(),
        &[profile, profile_149]
    );
    assert!(matches!(
        CodexCompatibilityRegistry::classify(Some((0, 147, 0))),
        CodexCompatibilityState::Supported(supported) if supported == profile
    ));
    assert!(matches!(
        CodexCompatibilityRegistry::classify(Some((0, 148, 0))),
        CodexCompatibilityState::Experimental(entry) if entry.version() == (0, 148, 0)
    ));
    assert_eq!(
        CodexCompatibilityRegistry::classify(Some((0, 149, 0))).label(),
        "supported"
    );
    assert_eq!(
        CodexCompatibilityRegistry::classify(Some((0, 150, 0))).label(),
        "unknown"
    );
}

#[test]
fn observed_codex_0149_shape_reconciles_only_exact_owned_declarations() {
    let root = TestRoot::new("codex-0149-observed-shape");
    let codex_home = root.child("codex-home");
    write_hooks_fixture(&codex_home, "0.149.0-observed.json");
    let integration = test_integration_with_codex_fixture(&root, "codex_version_probe_0149.rs");

    assert_eq!(
        integration
            .setup()
            .expect("0.149 setup is safe for an audited shape"),
        SetupOutcome::InstalledTrustReviewRequired
    );
    let trusted_keys = install_current_codex_trust_state(&codex_home);
    assert_eq!(trusted_keys.len(), ADMITTED_HOOK_EVENTS.len());

    let report = integration.doctor();
    assert_eq!(report.compatibility_state().label(), "supported");
    assert_eq!(
        report.hook_profile().map(CodexHookProfile::id),
        Some("codex-hooks-rust-v0.149.0")
    );
    assert!(report.checks().iter().any(|check| {
        check.id() == "codex.version"
            && check.status() == DoctorStatus::Pass
            && check.summary() == "Codex version is source-audited"
    }));
    assert!(report.checks().iter().any(|check| {
        check.id() == "hooks.declarations" && check.status() == DoctorStatus::Pass
    }));
    assert!(
        report
            .checks()
            .iter()
            .any(|check| { check.id() == "hooks.trust" && check.status() == DoctorStatus::Pass })
    );

    let hooks: Value = serde_json::from_slice(
        &fs::read(codex_home.join("hooks.json")).expect("reconciled hooks read"),
    )
    .expect("reconciled hooks parse");
    assert_eq!(hooks["hooks"]["Stop"].as_array().map(Vec::len), Some(2));
    assert_eq!(
        hooks["hooks"]["Stop"][0]["hooks"][0]["statusMessage"],
        "fixture external stop hook"
    );
}

#[test]
fn codex_0149_mcp_extension_is_preserved_during_owned_command_reconciliation() {
    let root = TestRoot::new("codex-0149-mcp-extension");
    let codex_home = root.child("codex-home");
    write_hooks_fixture(&codex_home, "0.149.0-mcp-extension.json");
    let integration = test_integration_with_codex_fixture(&root, "codex_version_probe_0149.rs");

    let hooks_path = codex_home.join("hooks.json");
    let before: Value = serde_json::from_slice(&fs::read(&hooks_path).expect("MCP fixture reads"))
        .expect("MCP fixture parses");
    let external_mcp_group = before["hooks"]["PostToolUse"][0].clone();

    assert_eq!(
        integration
            .setup()
            .expect("0.149 MCP extension does not block command reconciliation"),
        SetupOutcome::InstalledTrustReviewRequired
    );
    install_current_codex_trust_state(&codex_home);

    let after: Value =
        serde_json::from_slice(&fs::read(&hooks_path).expect("reconciled hooks read"))
            .expect("reconciled hooks parse");
    assert_eq!(after["hooks"]["PostToolUse"][0], external_mcp_group);
    assert_eq!(
        after["hooks"]["PostToolUse"][1]["hooks"][0]["type"],
        "command"
    );
    let report = integration.doctor();
    assert!(report.checks().iter().any(|check| {
        check.id() == "hooks.declarations" && check.status() == DoctorStatus::Pass
    }));
    assert!(
        report
            .checks()
            .iter()
            .any(|check| { check.id() == "hooks.trust" && check.status() == DoctorStatus::Pass })
    );
}

#[test]
#[allow(clippy::too_many_lines)] // Covers original third-party retention and full exact-repair idempotence.
fn repair_v2_preserves_original_baseline_third_party_groups_and_is_idempotent() {
    let root = TestRoot::new("orphaned-owned-hook-repair");
    let codex_home = root.child("codex-home");
    write_hooks_fixture(&codex_home, "orphaned-third-party-hooks.json");
    let integration = test_integration_with_codex_fixture(&root, "codex_version_probe_0149.rs");
    integration.setup().expect("0.149 setup succeeds");
    install_current_codex_trust_state(&codex_home);

    let hooks_path = codex_home.join("hooks.json");
    let config_path = codex_home.join("config.toml");
    let manifest_path = root.child("state/integration-v1.json");
    let mut hooks: Value =
        serde_json::from_slice(&fs::read(&hooks_path).expect("installed hooks read"))
            .expect("installed hooks parse");
    let manifest: Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("manifest reads"))
            .expect("manifest parses");
    let third_party_stop = hooks["hooks"]["Stop"][0].clone();
    let third_party_mcp = hooks["hooks"]["PostToolUse"][0].clone();
    let config_before = fs::read(&config_path).expect("config reads");
    let manifest_before = fs::read(&manifest_path).expect("manifest rereads");

    remove_exact_manifest_owned_declarations(&mut hooks, &manifest);
    fs::write(
        &hooks_path,
        serde_json::to_vec_pretty(&hooks).expect("orphaned hooks serialize"),
    )
    .expect("orphaned hooks write");
    let orphaned_bytes = fs::read(&hooks_path).expect("orphaned hooks read");

    let diagnosis = integration.doctor();
    assert!(diagnosis.checks().iter().any(|check| {
        check.id() == "ownership.manifest" && check.status() == DoctorStatus::Pass
    }));
    assert!(diagnosis.checks().iter().any(|check| {
        check.id() == "hooks.currentness" && check.status() == DoctorStatus::Pass
    }));
    assert!(diagnosis.checks().iter().any(|check| {
        check.id() == "hooks.declarations" && check.status() == DoctorStatus::Fail
    }));
    assert!(
        diagnosis
            .checks()
            .iter()
            .any(|check| { check.id() == "hooks.trust" && check.status() == DoctorStatus::Fail })
    );

    let preview = integration
        .repair(false, None)
        .expect("repair preview is safe");
    assert_eq!(preview.disposition, CodexRepairDisposition::ReadyToApply);
    assert_eq!(preview.missing_declarations, ADMITTED_HOOK_EVENTS.len());
    assert_eq!(preview.schema_version, 2);
    assert_eq!(preview.third_party_groups_preserved, 2);
    assert_eq!(preview.postinstall_third_party_groups_preserved, 0);
    assert!(preview.target_digest.starts_with("sha256:"));
    assert!(preview.manual_hook_trust_review_required);
    assert_eq!(
        fs::read(&hooks_path).expect("preview hooks read"),
        orphaned_bytes
    );
    assert_eq!(
        fs::read(&config_path).expect("preview config read"),
        config_before
    );
    assert_eq!(
        fs::read(&manifest_path).expect("preview manifest read"),
        manifest_before
    );

    let repaired = integration
        .repair(true, Some(&preview.target_digest))
        .expect("repair apply succeeds");
    assert_eq!(
        repaired.disposition,
        CodexRepairDisposition::RepairedTrustReviewRequired
    );
    assert_eq!(repaired.missing_declarations, ADMITTED_HOOK_EVENTS.len());
    assert_eq!(repaired.third_party_groups_preserved, 2);
    assert_eq!(repaired.postinstall_third_party_groups_preserved, 0);
    assert!(repaired.manual_hook_trust_review_required);
    let repaired_hooks: Value =
        serde_json::from_slice(&fs::read(&hooks_path).expect("repaired hooks read"))
            .expect("repaired hooks parse");
    assert_eq!(repaired_hooks["hooks"]["Stop"][0], third_party_stop);
    assert_eq!(repaired_hooks["hooks"]["PostToolUse"][0], third_party_mcp);
    assert_eq!(
        fs::read(&config_path).expect("repair config read"),
        config_before
    );
    assert_eq!(
        fs::read(&manifest_path).expect("repair manifest read"),
        manifest_before
    );

    let repaired_bytes = fs::read(&hooks_path).expect("idempotent hooks read");
    let idempotent = integration
        .repair(false, None)
        .expect("exact repair preflight succeeds");
    assert_eq!(idempotent.disposition, CodexRepairDisposition::AlreadyExact);
    assert_eq!(idempotent.missing_declarations, 0);
    assert!(idempotent.manual_hook_trust_review_required);
    assert_eq!(
        fs::read(&hooks_path).expect("idempotent hooks reread"),
        repaired_bytes
    );
    assert_eq!(
        integration
            .repair(true, Some(&idempotent.target_digest))
            .expect("exact repair apply is idempotent")
            .disposition,
        CodexRepairDisposition::AlreadyExact
    );
}

#[test]
fn repair_v2_preserves_a_postinstall_third_party_group_on_the_codex_0149_real_shape() {
    let root = TestRoot::new("repair-v2-postinstall-third-party");
    let codex_home = root.child("codex-home");
    write_hooks_fixture(&codex_home, "0.149.0-observed.json");
    let integration = test_integration_with_codex_fixture(&root, "codex_version_probe_0149.rs");
    integration.setup().expect("0.149 setup succeeds");

    let hooks_path = codex_home.join("hooks.json");
    let postinstall = hook_group_fixture("repair-v2-postinstall-third-party.json", "Stop");
    let postinstall_raw = r#"{
        "hooks" : [ { "timeout" : 1, "command" : "third-party post-install stop hook", "type" : "command" } ],
        "plugin" : "third.party.postinstall"
      }"#;
    fs::write(
        &hooks_path,
        format!(
            "{{\n  \"description\": \"Codex 0.149-shaped post-install fixture\",\n  \"hooks\": {{\n    \"Stop\": [\n      {postinstall_raw}\n    ]\n  }}\n}}\n"
        ),
    )
    .expect("noncanonical postinstall hooks write");
    let before = fs::read(&hooks_path).expect("postinstall hooks read");

    let preview = integration
        .repair(false, None)
        .expect("postinstall group is safely previewed");
    assert_eq!(preview.disposition, CodexRepairDisposition::ReadyToApply);
    assert_eq!(preview.postinstall_third_party_groups_preserved, 1);
    assert_eq!(
        fs::read(&hooks_path).expect("preview leaves postinstall group byte-stable"),
        before
    );

    integration
        .repair(true, Some(&preview.target_digest))
        .expect("postinstall group is safely preserved during apply");
    let after: Value = serde_json::from_slice(&fs::read(&hooks_path).expect("repaired hooks read"))
        .expect("repaired hooks parse");
    assert!(
        after["hooks"]["Stop"]
            .as_array()
            .expect("Stop groups remain an array")
            .contains(&postinstall)
    );
    let after_text = String::from_utf8(fs::read(&hooks_path).expect("repaired hooks reread"))
        .expect("repaired hooks are UTF-8");
    assert!(
        after_text.contains(postinstall_raw),
        "repair must retain the pre-existing third-party group bytes"
    );
}

#[test]
fn repair_v2_preserves_multiple_postinstall_third_party_groups() {
    let root = TestRoot::new("repair-v2-multiple-postinstall-third-party");
    let codex_home = root.child("codex-home");
    write_hooks_fixture(&codex_home, "0.149.0-observed.json");
    let integration = test_integration_with_codex_fixture(&root, "codex_version_probe_0149.rs");
    integration.setup().expect("0.149 setup succeeds");

    let hooks_path = codex_home.join("hooks.json");
    let manifest: Value = serde_json::from_slice(
        &fs::read(root.child("state/integration-v1.json")).expect("manifest reads"),
    )
    .expect("manifest parses");
    let mut hooks: Value =
        serde_json::from_slice(&fs::read(&hooks_path).expect("installed hooks read"))
            .expect("installed hooks parse");
    remove_exact_manifest_owned_declarations(&mut hooks, &manifest);
    let post_tool = hook_group_fixture(
        "repair-v2-multiple-postinstall-third-party.json",
        "PostToolUse",
    );
    let stop = hook_group_fixture("repair-v2-multiple-postinstall-third-party.json", "Stop");
    hooks["hooks"]["PostToolUse"]
        .as_array_mut()
        .expect("PostToolUse groups are an array")
        .push(post_tool.clone());
    hooks["hooks"]["Stop"]
        .as_array_mut()
        .expect("Stop groups are an array")
        .push(stop.clone());
    fs::write(
        &hooks_path,
        serde_json::to_vec_pretty(&hooks).expect("multiple postinstall hooks serialize"),
    )
    .expect("multiple postinstall hooks write");

    let preview = integration
        .repair(false, None)
        .expect("multiple postinstall groups are safely previewed");
    assert_eq!(preview.postinstall_third_party_groups_preserved, 2);
    integration
        .repair(true, Some(&preview.target_digest))
        .expect("multiple postinstall groups are safely preserved");
    let after: Value = serde_json::from_slice(&fs::read(&hooks_path).expect("repaired hooks read"))
        .expect("repaired hooks parse");
    assert!(
        after["hooks"]["PostToolUse"]
            .as_array()
            .expect("PostToolUse groups remain an array")
            .contains(&post_tool)
    );
    assert!(
        after["hooks"]["Stop"]
            .as_array()
            .expect("Stop groups remain an array")
            .contains(&stop)
    );
}

#[test]
fn repair_v2_preserves_a_postinstall_external_mcp_group() {
    let root = TestRoot::new("repair-v2-postinstall-mcp");
    let codex_home = root.child("codex-home");
    write_hooks_fixture(&codex_home, "0.149.0-observed.json");
    let integration = test_integration_with_codex_fixture(&root, "codex_version_probe_0149.rs");
    integration.setup().expect("0.149 setup succeeds");

    let hooks_path = codex_home.join("hooks.json");
    let manifest: Value = serde_json::from_slice(
        &fs::read(root.child("state/integration-v1.json")).expect("manifest reads"),
    )
    .expect("manifest parses");
    let mut hooks: Value =
        serde_json::from_slice(&fs::read(&hooks_path).expect("installed hooks read"))
            .expect("installed hooks parse");
    remove_exact_manifest_owned_declarations(&mut hooks, &manifest);
    let mcp = hook_group_fixture("repair-v2-postinstall-mcp.json", "PostToolUse");
    hooks["hooks"]["PostToolUse"]
        .as_array_mut()
        .expect("PostToolUse groups are an array")
        .push(mcp.clone());
    fs::write(
        &hooks_path,
        serde_json::to_vec_pretty(&hooks).expect("postinstall MCP hooks serialize"),
    )
    .expect("postinstall MCP hooks write");

    let preview = integration
        .repair(false, None)
        .expect("postinstall MCP group is safely previewed");
    assert_eq!(preview.postinstall_third_party_groups_preserved, 1);
    integration
        .repair(true, Some(&preview.target_digest))
        .expect("postinstall MCP group is safely preserved");
    let after: Value = serde_json::from_slice(&fs::read(&hooks_path).expect("repaired hooks read"))
        .expect("repaired hooks parse");
    assert!(
        after["hooks"]["PostToolUse"]
            .as_array()
            .expect("PostToolUse groups remain an array")
            .contains(&mcp)
    );
}

#[test]
fn repair_v2_refuses_concurrent_drift_after_a_valid_preview() {
    let root = TestRoot::new("repair-v2-concurrent-drift");
    let codex_home = root.child("codex-home");
    write_hooks_fixture(&codex_home, "0.149.0-observed.json");
    let integration = test_integration_with_codex_fixture(&root, "codex_version_probe_0149.rs");
    integration.setup().expect("0.149 setup succeeds");

    let hooks_path = codex_home.join("hooks.json");
    let manifest: Value = serde_json::from_slice(
        &fs::read(root.child("state/integration-v1.json")).expect("manifest reads"),
    )
    .expect("manifest parses");
    let mut hooks: Value =
        serde_json::from_slice(&fs::read(&hooks_path).expect("installed hooks read"))
            .expect("installed hooks parse");
    remove_exact_manifest_owned_declarations(&mut hooks, &manifest);
    fs::write(
        &hooks_path,
        serde_json::to_vec_pretty(&hooks).expect("orphaned hooks serialize"),
    )
    .expect("orphaned hooks write");
    let preview = integration
        .repair(false, None)
        .expect("repair preview succeeds before concurrent drift");
    assert!(matches!(
        integration.repair(true, None),
        Err(CodexIntegrationError::RepairPreviewDigestRequired)
    ));
    assert!(matches!(
        integration.repair(true, Some("not-a-sha256-digest")),
        Err(CodexIntegrationError::RepairPreviewDigestInvalid)
    ));
    assert_eq!(
        CodexIntegrationError::RepairPreviewDigestInvalid.repair_failure_class(),
        "PREVIEW_TARGET_DIGEST_INVALID"
    );

    hooks["hooks"]["Stop"]
        .as_array_mut()
        .expect("Stop groups are an array")
        .push(hook_group_fixture(
            "repair-v2-postinstall-third-party.json",
            "Stop",
        ));
    fs::write(
        &hooks_path,
        serde_json::to_vec_pretty(&hooks).expect("concurrent drift hooks serialize"),
    )
    .expect("concurrent drift hooks write");
    let drifted = fs::read(&hooks_path).expect("drifted hooks read");

    assert!(matches!(
        integration.repair(true, Some(&preview.target_digest)),
        Err(CodexIntegrationError::ConcurrentTargetDrift)
    ));
    assert_eq!(
        CodexIntegrationError::ConcurrentTargetDrift.repair_failure_class(),
        "CONCURRENT_DRIFT_REFUSAL"
    );
    assert_eq!(
        fs::read(&hooks_path).expect("drifted hooks reread"),
        drifted
    );
    fs::remove_file(&hooks_path).expect("target deletion simulates concurrent drift");
    assert!(matches!(
        integration.repair(true, Some(&preview.target_digest)),
        Err(CodexIntegrationError::ConcurrentTargetDrift)
    ));
}

#[test]
fn repair_v2_blocks_a_modified_partial_owned_group() {
    let root = TestRoot::new("repair-v2-modified-partial-owned");
    let codex_home = root.child("codex-home");
    write_hooks_fixture(&codex_home, "0.149.0-observed.json");
    let integration = test_integration_with_codex_fixture(&root, "codex_version_probe_0149.rs");
    integration.setup().expect("0.149 setup succeeds");

    let hooks_path = codex_home.join("hooks.json");
    let manifest_path = root.child("state/integration-v1.json");
    let mut hooks: Value =
        serde_json::from_slice(&fs::read(&hooks_path).expect("installed hooks read"))
            .expect("installed hooks parse");
    hooks["hooks"]["PreToolUse"][1]["hooks"][0]["async"] = json!(true);
    fs::write(
        &hooks_path,
        serde_json::to_vec_pretty(&hooks).expect("modified partial group serialize"),
    )
    .expect("modified partial group write");
    let before = fs::read(&hooks_path).expect("modified partial hooks read");
    let manifest_before = fs::read(&manifest_path).expect("manifest reads");

    assert!(matches!(
        integration.repair(false, None),
        Err(CodexIntegrationError::ModifiedOwnedHook)
    ));
    assert_eq!(
        fs::read(&hooks_path).expect("modified hooks reread"),
        before
    );
    assert_eq!(
        fs::read(&manifest_path).expect("manifest reread"),
        manifest_before
    );
}

#[test]
fn repair_v2_blocks_a_tampered_preinstall_baseline() {
    let root = TestRoot::new("repair-v2-baseline-drift");
    let codex_home = root.child("codex-home");
    write_hooks_fixture(&codex_home, "0.149.0-observed.json");
    let integration = test_integration_with_codex_fixture(&root, "codex_version_probe_0149.rs");
    integration.setup().expect("0.149 setup succeeds");

    let hooks_path = codex_home.join("hooks.json");
    let manifest: Value = serde_json::from_slice(
        &fs::read(root.child("state/integration-v1.json")).expect("manifest reads"),
    )
    .expect("manifest parses");
    let backup_path = PathBuf::from(
        manifest["hooks_backup"]["path"]
            .as_str()
            .expect("hooks backup path is recorded"),
    );
    fs::write(&backup_path, b"{\"hooks\":{}}").expect("baseline tamper writes");

    let mut hooks: Value =
        serde_json::from_slice(&fs::read(&hooks_path).expect("installed hooks read"))
            .expect("installed hooks parse");
    remove_exact_manifest_owned_declarations(&mut hooks, &manifest);
    fs::write(
        &hooks_path,
        serde_json::to_vec_pretty(&hooks).expect("orphaned hooks serialize"),
    )
    .expect("orphaned hooks write");
    let before = fs::read(&hooks_path).expect("orphaned hooks read");

    assert!(matches!(
        integration.repair(false, None),
        Err(CodexIntegrationError::BaselineDriftBlocked)
    ));
    assert_eq!(
        CodexIntegrationError::BaselineDriftBlocked.repair_failure_class(),
        "BASELINE_DRIFT_BLOCKED"
    );
    assert_eq!(fs::read(&hooks_path).expect("hooks reread"), before);
    fs::remove_file(&backup_path).expect("baseline deletion simulates unsafe drift");
    assert!(matches!(
        integration.repair(false, None),
        Err(CodexIntegrationError::BaselineDriftBlocked)
    ));
}

#[test]
fn repair_v2_blocks_a_malformed_or_unknown_hook_envelope() {
    let root = TestRoot::new("repair-v2-malformed-wire");
    let codex_home = root.child("codex-home");
    write_hooks_fixture(&codex_home, "0.149.0-observed.json");
    let integration = test_integration_with_codex_fixture(&root, "codex_version_probe_0149.rs");
    integration.setup().expect("0.149 setup succeeds");

    let hooks_path = codex_home.join("hooks.json");
    write_hooks_fixture(&codex_home, "repair-v2-malformed-wire.json");
    let before = fs::read(&hooks_path).expect("malformed hooks read");
    assert!(matches!(
        integration.repair(false, None),
        Err(CodexIntegrationError::HooksShape)
    ));
    assert_eq!(
        CodexIntegrationError::HooksShape.repair_failure_class(),
        "UNKNOWN_HOOK_WIRE_BLOCKED"
    );
    assert_eq!(
        fs::read(&hooks_path).expect("malformed hooks reread"),
        before
    );
}

#[test]
fn repair_v2_blocks_tabbeacon_like_unowned_replacements_without_writing_owner_state() {
    let root = TestRoot::new("ambiguous-owned-hook-repair");
    let codex_home = root.child("codex-home");
    write_hooks_fixture(&codex_home, "0.149.0-observed.json");
    let integration = test_integration_with_codex_fixture(&root, "codex_version_probe_0149.rs");
    integration.setup().expect("0.149 setup succeeds");

    let hooks_path = codex_home.join("hooks.json");
    let config_path = codex_home.join("config.toml");
    let manifest_path = root.child("state/integration-v1.json");
    write_hooks_fixture(&codex_home, "repair-v2-tabbeacon-like-unowned.json");
    let hooks_before = fs::read(&hooks_path).expect("ambiguous hooks read");
    let config_before = fs::read(&config_path).expect("config reads");
    let manifest_before = fs::read(&manifest_path).expect("manifest reads");

    assert!(matches!(
        integration.repair(false, None),
        Err(CodexIntegrationError::TabBeaconLikeAmbiguityBlocked)
    ));
    assert_eq!(
        CodexIntegrationError::TabBeaconLikeAmbiguityBlocked.repair_failure_class(),
        "TABBEACON_LIKE_AMBIGUITY_BLOCKED"
    );
    assert!(matches!(
        integration.repair(
            true,
            Some("sha256:0000000000000000000000000000000000000000000000000000000000000000")
        ),
        Err(CodexIntegrationError::TabBeaconLikeAmbiguityBlocked)
    ));
    assert_eq!(fs::read(&hooks_path).expect("hooks reread"), hooks_before);
    assert_eq!(
        fs::read(&config_path).expect("config reread"),
        config_before
    );
    assert_eq!(
        fs::read(&manifest_path).expect("manifest reread"),
        manifest_before
    );
}

#[test]
fn repair_v2_blocks_an_unproven_postinstall_command_replacement() {
    let root = TestRoot::new("unproven-owned-hook-replacement");
    let codex_home = root.child("codex-home");
    write_hooks_fixture(&codex_home, "orphaned-third-party-hooks.json");
    let integration = test_integration_with_codex_fixture(&root, "codex_version_probe_0149.rs");
    integration.setup().expect("0.149 setup succeeds");

    let hooks_path = codex_home.join("hooks.json");
    let config_path = codex_home.join("config.toml");
    let manifest_path = root.child("state/integration-v1.json");
    let manifest: Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("manifest reads"))
            .expect("manifest parses");
    let mut hooks: Value =
        serde_json::from_slice(&fs::read(&hooks_path).expect("installed hooks read"))
            .expect("installed hooks parse");
    remove_exact_manifest_owned_declarations(&mut hooks, &manifest);
    hooks["hooks"]["PreToolUse"] = json!([{
        "hooks": [{
            "type": "command",
            "command": "external replacement with no ownership baseline",
            "timeout": 1
        }]
    }]);
    fs::write(
        &hooks_path,
        serde_json::to_vec_pretty(&hooks).expect("replacement hooks serialize"),
    )
    .expect("replacement hooks write");
    let hooks_before = fs::read(&hooks_path).expect("replacement hooks read");
    let config_before = fs::read(&config_path).expect("config reads");
    let manifest_before = fs::read(&manifest_path).expect("manifest reads");

    assert!(matches!(
        integration.repair(false, None),
        Err(CodexIntegrationError::BaselineDriftBlocked)
    ));
    assert_eq!(fs::read(&hooks_path).expect("hooks reread"), hooks_before);
    assert_eq!(
        fs::read(&config_path).expect("config reread"),
        config_before
    );
    assert_eq!(
        fs::read(&manifest_path).expect("manifest reread"),
        manifest_before
    );
}

#[test]
fn repair_v2_never_synthesizes_a_missing_hooks_envelope() {
    let root = TestRoot::new("repair-v2-missing-hooks-envelope");
    let codex_home = root.child("codex-home");
    write_hooks_fixture(&codex_home, "0.149.0-observed.json");
    let integration = test_integration_with_codex_fixture(&root, "codex_version_probe_0149.rs");
    integration.setup().expect("0.149 setup succeeds");

    let hooks_path = codex_home.join("hooks.json");
    fs::write(
        &hooks_path,
        br#"{ "description": "unknown partial document" }"#,
    )
    .expect("partial hook document writes");
    let before = fs::read(&hooks_path).expect("partial hook document reads");

    assert!(matches!(
        integration.repair(false, None),
        Err(CodexIntegrationError::HooksShape)
    ));
    assert_eq!(
        fs::read(&hooks_path).expect("partial hook document rereads"),
        before
    );
}

#[test]
fn repair_and_runtime_continuity_reject_a_manifest_with_incoherent_executable() {
    let root = TestRoot::new("manifest-executable-coherence");
    let codex_home = root.child("codex-home");
    write_hooks_fixture(&codex_home, "0.149.0-observed.json");
    let integration = test_integration_with_codex_fixture(&root, "codex_version_probe_0149.rs");
    integration.setup().expect("0.149 setup succeeds");
    install_current_codex_trust_state(&codex_home);

    let manifest_path = root.child("state/integration-v1.json");
    let mut manifest: Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("manifest reads"))
            .expect("manifest parses");
    manifest["executable"] = json!(r"C:\\tabbeacon-fixture\\stale-tabbeacon.exe");
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).expect("manifest serialize"),
    )
    .expect("manifest writes");

    assert!(matches!(
        integration.repair(false, None),
        Err(CodexIntegrationError::StaleOwnedHook)
    ));
    assert_eq!(
        integration.doctor().runtime_continuity(),
        CodexRuntimeContinuity::Unproven
    );
}

#[test]
fn future_unknown_same_wire_preserves_runtime_but_blocks_all_mutation_authority() {
    let root = TestRoot::new("future-unknown-continuity");
    let codex_home = root.child("codex-home");
    write_hooks_fixture(&codex_home, "0.150.0-same-wire.json");
    let admitted = test_integration_with_codex_fixture(&root, "codex_version_probe_0149.rs");
    admitted.setup().expect("admitted setup succeeds");
    install_current_codex_trust_state(&codex_home);

    let hooks_path = codex_home.join("hooks.json");
    let config_path = codex_home.join("config.toml");
    let manifest_path = root.child("state/integration-v1.json");
    let hooks_before = fs::read(&hooks_path).expect("hooks read");
    let config_before = fs::read(&config_path).expect("config read");
    let manifest_before = fs::read(&manifest_path).expect("manifest read");
    let future = test_integration_with_codex_fixture(&root, "codex_version_probe_future.rs");

    let report = future.doctor();
    assert_eq!(report.compatibility_state().label(), "unknown");
    assert_eq!(report.mutation_authority(), CodexMutationAuthority::Blocked);
    assert_eq!(
        report.runtime_continuity(),
        CodexRuntimeContinuity::PreservedUnadmitted
    );
    assert!(report.checks().iter().any(|check| {
        check.id() == "hooks.currentness" && check.status() == DoctorStatus::Warning
    }));
    assert!(
        report
            .checks()
            .iter()
            .any(|check| { check.id() == "hooks.trust" && check.status() == DoctorStatus::Pass })
    );
    assert!(report.checks().iter().any(|check| {
        check.id() == "codex.runtime-continuity" && check.status() == DoctorStatus::Warning
    }));
    let inventory = future.hook_inventory();
    assert!(inventory.entries.iter().any(|entry| {
        entry.owner == HookOwner::TabBeacon
            && entry.trust_state == HookTrustState::Trusted
            && entry.currentness == HookCurrentness::InstalledExactUnadmitted
    }));
    assert!(matches!(
        future.setup(),
        Err(CodexIntegrationError::UnsupportedCodexVersion)
    ));
    assert!(matches!(
        future.repair(true, None),
        Err(CodexIntegrationError::UnsupportedCodexVersion)
    ));
    assert_eq!(
        CodexIntegrationError::UnsupportedCodexVersion.repair_failure_class(),
        "UNKNOWN_VERSION_MUTATION_BLOCKED"
    );
    assert!(matches!(
        future.reconcile_title_ownership(false),
        Err(CodexIntegrationError::UnsupportedCodexVersion)
    ));
    assert_eq!(fs::read(&hooks_path).expect("hooks reread"), hooks_before);
    assert_eq!(
        fs::read(&config_path).expect("config reread"),
        config_before
    );
    assert_eq!(
        fs::read(&manifest_path).expect("manifest reread"),
        manifest_before
    );
}

#[test]
fn future_unknown_declaration_drift_disables_runtime_continuity() {
    let root = TestRoot::new("future-unknown-continuity-drift");
    let codex_home = root.child("codex-home");
    write_hooks_fixture(&codex_home, "0.150.0-same-wire.json");
    let admitted = test_integration_with_codex_fixture(&root, "codex_version_probe_0149.rs");
    admitted.setup().expect("admitted setup succeeds");
    install_current_codex_trust_state(&codex_home);
    let future = test_integration_with_codex_fixture(&root, "codex_version_probe_future.rs");
    let hooks_path = codex_home.join("hooks.json");

    let mut hooks: Value =
        serde_json::from_slice(&fs::read(&hooks_path).expect("hooks read")).expect("hooks parse");
    hooks["hooks"]["PreToolUse"][0]["hooks"][0]["command"] = json!("declaration drift");
    hooks["hooks"]["PreToolUse"][0]["hooks"][0]["commandWindows"] = json!("declaration drift");
    fs::write(
        &hooks_path,
        serde_json::to_vec_pretty(&hooks).expect("drifted hooks serialize"),
    )
    .expect("drifted hooks write");
    assert_eq!(
        future.doctor().runtime_continuity(),
        CodexRuntimeContinuity::Unproven
    );
}

#[test]
fn future_unknown_trust_drift_disables_runtime_continuity() {
    let root = TestRoot::new("future-unknown-continuity-trust-drift");
    let codex_home = root.child("codex-home");
    write_hooks_fixture(&codex_home, "0.150.0-same-wire.json");
    let admitted = test_integration_with_codex_fixture(&root, "codex_version_probe_0149.rs");
    admitted.setup().expect("admitted setup succeeds");
    let trusted_keys = install_current_codex_trust_state(&codex_home);
    let future = test_integration_with_codex_fixture(&root, "codex_version_probe_future.rs");
    let config_path = codex_home.join("config.toml");

    let mut config = fs::read_to_string(&config_path)
        .expect("config reads")
        .parse::<DocumentMut>()
        .expect("config parses");
    config["hooks"]["state"][&trusted_keys[0]]["trusted_hash"] = value("sha256:drift");
    fs::write(&config_path, config.to_string()).expect("trust drift writes");
    assert_eq!(
        future.doctor().runtime_continuity(),
        CodexRuntimeContinuity::Unproven
    );
}

#[test]
fn future_unknown_schema_change_disables_runtime_continuity() {
    let root = TestRoot::new("future-unknown-continuity-schema-change");
    let codex_home = root.child("codex-home");
    write_hooks_fixture(&codex_home, "0.150.0-same-wire.json");
    let admitted = test_integration_with_codex_fixture(&root, "codex_version_probe_0149.rs");
    admitted.setup().expect("admitted setup succeeds");
    install_current_codex_trust_state(&codex_home);
    let future = test_integration_with_codex_fixture(&root, "codex_version_probe_future.rs");
    write_hooks_fixture(&codex_home, "0.150.0-schema-change.json");
    assert_eq!(
        future.doctor().runtime_continuity(),
        CodexRuntimeContinuity::Unproven
    );
    assert_eq!(
        future.hook_inventory().availability,
        HookInventoryAvailability::Unavailable
    );
}

#[test]
fn unknown_future_codex_versions_are_read_only_and_not_adopted() {
    let root = TestRoot::new("codex-unknown-future");
    let codex_home = root.child("codex-home");
    write_hooks_fixture(&codex_home, "0.149.0-observed.json");
    let hooks_before = fs::read(codex_home.join("hooks.json")).expect("fixture hooks read");
    let integration = test_integration_with_codex_fixture(&root, "codex_version_probe_future.rs");

    assert!(matches!(
        integration.setup(),
        Err(CodexIntegrationError::UnsupportedCodexVersion)
    ));
    assert_eq!(
        fs::read(codex_home.join("hooks.json")).expect("unknown-version hooks reread"),
        hooks_before
    );
    assert!(!root.child("state").exists());

    let report = integration.doctor();
    assert_eq!(report.compatibility_state().label(), "unknown");
    assert_eq!(report.hook_profile(), None);
    assert!(report.checks().iter().any(|check| {
        check.id() == "codex.version"
            && check.status() == DoctorStatus::Fail
            && check.summary()
                == "Detected: Codex 0.150.0; Registry: unknown; Hook profile: unclassified; Risk: manual review required"
    }));
    assert!(
        report
            .checks()
            .iter()
            .any(|check| { check.id() == "hooks.trust" && check.status() == DoctorStatus::Fail })
    );
    assert!(report.checks().iter().any(|check| {
        check.id() == "hooks.currentness" && check.status() == DoctorStatus::Fail
    }));
}

#[test]
fn observed_codex_0149_command_drift_is_never_reconciled_or_trusted() {
    let root = TestRoot::new("codex-0149-command-drift");
    let codex_home = root.child("codex-home");
    write_hooks_fixture(&codex_home, "0.147.0-known-good.json");
    let integration = test_integration_with_codex_fixture(&root, "codex_version_probe_0149.rs");
    integration
        .setup()
        .expect("0.149 setup succeeds before drift");
    install_current_codex_trust_state(&codex_home);

    let hooks_path = codex_home.join("hooks.json");
    let mut hooks: Value =
        serde_json::from_slice(&fs::read(&hooks_path).expect("owned hooks read"))
            .expect("owned hooks parse");
    hooks["hooks"]["PreToolUse"][1]["hooks"][0]["command"] = json!("external replacement");
    hooks["hooks"]["PreToolUse"][1]["hooks"][0]["commandWindows"] = json!("external replacement");
    fs::write(
        &hooks_path,
        serde_json::to_vec_pretty(&hooks).expect("drifted hooks serialize"),
    )
    .expect("drifted hooks write");
    let drifted_bytes = fs::read(&hooks_path).expect("drifted hooks reread");

    let report = integration.doctor();
    assert!(report.checks().iter().any(|check| {
        check.id() == "hooks.declarations" && check.status() == DoctorStatus::Fail
    }));
    assert!(
        report
            .checks()
            .iter()
            .any(|check| { check.id() == "hooks.trust" && check.status() == DoctorStatus::Fail })
    );
    assert!(matches!(
        integration.setup(),
        Err(CodexIntegrationError::ModifiedOwnedHook)
    ));
    assert_eq!(
        fs::read(&hooks_path).expect("drifted hooks remain untouched"),
        drifted_bytes
    );
}

#[test]
fn session_start_sources_distinguish_ready_from_compact_preservation() {
    let root = TestRoot::new("session-start");
    for source in ["startup", "resume", "clear"] {
        let mut payload = hook_payload("SessionStart", "session-a", &root.path);
        payload["source"] = Value::String(source.to_owned());
        let normalized = evidence(&payload);
        assert_eq!(normalized.patch.phase, FieldUpdate::set(Phase::Ready));
        assert_eq!(normalized.patch.attention, FieldUpdate::clear());
        assert_eq!(normalized.patch.health, FieldUpdate::unchanged());
    }

    let mut compact = hook_payload("SessionStart", "session-a", &root.path);
    compact["source"] = Value::String("compact".to_owned());
    assert!(matches!(
        CodexHookNormalizer
            .normalize(
                &serde_json::to_vec(&compact).expect("compact fixture serializes"),
                UNIX_EPOCH
            )
            .expect("compact is valid"),
        CodexNormalization::PreserveCurrentState(_)
    ));
}

#[test]
fn compact_and_subagent_lifecycle_are_classified_from_release_metadata() {
    let root = TestRoot::new("compact-subagent-classification");
    for event in ["PreCompact", "PostCompact"] {
        let payload = hook_payload(event, "session-a", &root.path);
        let normalized = CodexHookNormalizer
            .normalize(
                &serde_json::to_vec(&payload).expect("compact fixture serializes"),
                UNIX_EPOCH,
            )
            .expect("compact event is valid");
        let CodexNormalization::PreserveCurrentState(context) = normalized else {
            panic!("expected compact preservation, got {normalized:?}");
        };
        assert_eq!(context.turn_id(), Some("turn-1"));
        assert_eq!(context.agent_id(), None);
    }

    for event in ["SubagentStart", "SubagentStop"] {
        let mut payload = hook_payload(event, "session-a", &root.path);
        payload["agent_id"] = Value::String("agent-child".to_owned());
        payload["agent_type"] = Value::String("explorer".to_owned());
        let normalized = CodexHookNormalizer
            .normalize(
                &serde_json::to_vec(&payload).expect("subagent fixture serializes"),
                UNIX_EPOCH,
            )
            .expect("subagent event is valid");
        let CodexNormalization::IgnoreSubagent(context) = normalized else {
            panic!("expected subagent isolation, got {normalized:?}");
        };
        assert_eq!(context.agent_id(), Some("agent-child"));
        assert_eq!(context.agent_type(), Some("explorer"));
    }
}

#[test]
fn admitted_events_map_only_to_justified_phase_and_attention() {
    let root = TestRoot::new("mapping");
    let cases = [
        (
            "UserPromptSubmit",
            FieldUpdate::set(Phase::Working),
            FieldUpdate::clear(),
        ),
        (
            "PreToolUse",
            FieldUpdate::set(Phase::Working),
            FieldUpdate::clear(),
        ),
        (
            "PostToolUse",
            FieldUpdate::set(Phase::Working),
            FieldUpdate::clear(),
        ),
        (
            "PermissionRequest",
            FieldUpdate::set(Phase::WaitingUser),
            FieldUpdate::set(Attention::Approval),
        ),
        (
            "Stop",
            FieldUpdate::set(Phase::WaitingUser),
            FieldUpdate::set(Attention::ResultReady),
        ),
        (
            "SessionEnd",
            FieldUpdate::set(Phase::Ended),
            FieldUpdate::clear(),
        ),
    ];
    for (event, phase, attention) in cases {
        let mut payload = hook_payload(event, "session-a", &root.path);
        if event == "PostToolUse" {
            payload["tool_response"] = json!({"exit_code": 17, "status": "failed"});
        }
        let normalized = evidence(&payload);
        assert_eq!(normalized.patch.phase, phase, "event={event}");
        assert_eq!(normalized.patch.attention, attention, "event={event}");
        assert_eq!(
            normalized.patch.health,
            FieldUpdate::<Health>::unchanged(),
            "event={event}"
        );
    }
}

#[test]
fn malformed_missing_and_unknown_inputs_are_safe() {
    let root = TestRoot::new("invalid-input");
    assert_eq!(
        CodexHookNormalizer.normalize(b"not-json", UNIX_EPOCH),
        Err(CodexHookError::MalformedJson)
    );
    let missing = json!({"hook_event_name": "Stop", "cwd": root.path});
    assert_eq!(
        CodexHookNormalizer.normalize(
            &serde_json::to_vec(&missing).expect("missing fixture serializes"),
            UNIX_EPOCH
        ),
        Err(CodexHookError::MissingField("session_id"))
    );
    let unknown = hook_payload("FutureEvent", "session-a", &root.path);
    assert_eq!(
        CodexHookNormalizer
            .normalize(
                &serde_json::to_vec(&unknown).expect("unknown fixture serializes"),
                UNIX_EPOCH
            )
            .expect("unknown event is safely classified"),
        CodexNormalization::UnsupportedEvent
    );
}

#[test]
fn optional_agent_metadata_is_fail_open_and_sensitive_bodies_do_not_affect_evidence() {
    let root = TestRoot::new("content-minimization");
    let mut first = hook_payload("UserPromptSubmit", "session-a", &root.path);
    first["prompt"] = Value::String("sensitive prompt alpha".to_owned());
    let mut second = first.clone();
    second["prompt"] = Value::String("sensitive prompt beta".to_owned());
    assert_eq!(
        evidence(&first),
        evidence(&second),
        "prompt content must not affect normalized evidence"
    );

    let mut tool_a = hook_payload("PostToolUse", "session-a", &root.path);
    tool_a["tool_input"] = json!({"secret": "alpha"});
    tool_a["tool_response"] = json!({"secret": "beta"});
    let mut tool_b = tool_a.clone();
    tool_b["tool_input"] = json!({"secret": "changed"});
    tool_b["tool_response"] = json!({"secret": "changed"});
    assert_eq!(
        evidence(&tool_a),
        evidence(&tool_b),
        "tool bodies must not affect normalized evidence"
    );

    let mut subagent_prompt = first;
    subagent_prompt["agent_id"] = Value::String("agent-child".to_owned());
    assert!(matches!(
        CodexHookNormalizer
            .normalize(
                &serde_json::to_vec(&subagent_prompt).expect("subagent prompt serializes"),
                UNIX_EPOCH,
            )
            .expect("partial optional subagent metadata is contained"),
        CodexNormalization::IgnoreSubagent(_)
    ));
}

#[test]
fn duplicate_payloads_are_deterministic_and_sessions_remain_separate() {
    let root = TestRoot::new("idempotence");
    let first_payload = hook_payload("UserPromptSubmit", "session-a", &root.path);
    let mut reordered = serde_json::Map::new();
    for (key, value) in first_payload.as_object().expect("object").iter().rev() {
        reordered.insert(key.clone(), value.clone());
    }
    let first = evidence(&first_payload);
    let duplicate = evidence(&Value::Object(reordered));
    assert_eq!(first, duplicate);

    let second = evidence(&hook_payload("UserPromptSubmit", "session-b", &root.path));
    assert_ne!(first.session, second.session);
    assert_ne!(
        first.session.native_session_id(),
        second.session.native_session_id()
    );
}

#[test]
fn runtime_uses_repository_identity_reconciler_and_existing_renderer() {
    let root = TestRoot::new("runtime");
    let repo = root.child("workstation-manager");
    init_repo(
        &repo,
        "https://github.com/JerrySkywalker/workstation-manager.git",
    );
    let runtime = CodexHookRuntime::new(root.child("state"), true);
    let prompt = hook_payload("UserPromptSubmit", "session-a", &repo);
    let mut output = Vec::new();
    assert_eq!(
        runtime.dispatch_to(
            &serde_json::to_vec(&prompt).expect("prompt serializes"),
            UNIX_EPOCH,
            &mut output
        ),
        HookDispatchOutcome::Applied
    );
    let rendered = String::from_utf8_lossy(&output);
    assert!(rendered.contains("\u{1b}]0;○ WORKMANA\u{1b}\\"));
    assert!(!rendered.contains("working"));
    assert!(rendered.contains("\u{1b}]9;4;3;0\u{1b}\\"));
    assert!(rendered.contains("rgb:2e/cc/71"));

    let end = hook_payload("SessionEnd", "session-a", &repo);
    output.clear();
    assert_eq!(
        runtime.dispatch_to(
            &serde_json::to_vec(&end).expect("end serializes"),
            UNIX_EPOCH,
            &mut output
        ),
        HookDispatchOutcome::Applied
    );
    let rendered = String::from_utf8_lossy(&output);
    assert!(rendered.contains("○ WORKMANA"));
    assert!(!rendered.contains("reset"));
    assert!(rendered.contains("\u{1b}]9;4;0;0\u{1b}\\"));
    assert!(rendered.contains("\u{1b}]104;264\u{1b}\\"));
}

#[test]
fn runtime_renders_the_effective_workspace_alias_override() {
    let root = TestRoot::new("runtime-effective-alias");
    let repo = root.child("workstation-manager");
    let state = root.child("state");
    init_repo(
        &repo,
        "https://github.com/JerrySkywalker/workstation-manager.git",
    );
    WorkspaceIdentityResolver::new(&state)
        .set_alias_override(&repo, "LOCAL")
        .expect("local override is staged before the hook runtime");

    let runtime = CodexHookRuntime::new(&state, true);
    let prompt = hook_payload("UserPromptSubmit", "session-a", &repo);
    let mut output = Vec::new();
    assert_eq!(
        runtime.dispatch_to(
            &serde_json::to_vec(&prompt).expect("prompt serializes"),
            UNIX_EPOCH,
            &mut output,
        ),
        HookDispatchOutcome::Applied
    );
    let rendered = String::from_utf8_lossy(&output);
    assert!(rendered.contains("\u{1b}]0;○ LOCAL\u{1b}\\"));
    assert!(!rendered.contains("WORKMANA"));
}

#[test]
fn newer_turn_supersedes_and_rejects_stale_stop_working_and_revival() {
    let root = TestRoot::new("turn-supersession");
    let repo = root.child("repo");
    init_repo(&repo, "https://example.invalid/team/repo.git");
    let runtime = CodexHookRuntime::new(root.child("state"), true);

    for turn in ["turn-1", "turn-2"] {
        let payload = hook_payload_for_turn("UserPromptSubmit", "session-a", turn, &repo);
        assert_eq!(
            runtime.dispatch_to(
                &serde_json::to_vec(&payload).expect("prompt serializes"),
                UNIX_EPOCH,
                &mut Vec::new(),
            ),
            HookDispatchOutcome::Applied,
            "turn={turn}"
        );
    }

    for event in ["Stop", "PreToolUse", "PostToolUse", "PermissionRequest"] {
        let stale = hook_payload_for_turn(event, "session-a", "turn-1", &repo);
        let mut output = Vec::new();
        assert_eq!(
            runtime.dispatch_to(
                &serde_json::to_vec(&stale).expect("stale event serializes"),
                UNIX_EPOCH,
                &mut output,
            ),
            HookDispatchOutcome::RejectedStaleGeneration,
            "event={event}"
        );
        assert!(output.is_empty(), "stale event emitted terminal bytes");
    }

    let stale_prompt = hook_payload_for_turn("UserPromptSubmit", "session-a", "turn-1", &repo);
    assert_eq!(
        runtime.dispatch_to(
            &serde_json::to_vec(&stale_prompt).expect("stale prompt serializes"),
            UNIX_EPOCH,
            &mut Vec::new(),
        ),
        HookDispatchOutcome::RejectedStaleGeneration,
        "a retired turn must not revive activity"
    );

    let current_stop = hook_payload_for_turn("Stop", "session-a", "turn-2", &repo);
    let mut output = Vec::new();
    assert_eq!(
        runtime.dispatch_to(
            &serde_json::to_vec(&current_stop).expect("current stop serializes"),
            UNIX_EPOCH,
            &mut output,
        ),
        HookDispatchOutcome::Applied
    );
    assert!(String::from_utf8_lossy(&output).contains("\u{1b}]0;✓ "));
}

#[test]
fn subagent_start_stop_and_activity_cannot_replace_or_terminate_root_state() {
    let root = TestRoot::new("subagent-isolation");
    let repo = root.child("repo");
    init_repo(&repo, "https://example.invalid/team/repo.git");
    let runtime = CodexHookRuntime::new(root.child("state"), true);
    let root_prompt = hook_payload_for_turn("UserPromptSubmit", "session-a", "root-turn", &repo);
    assert_eq!(
        runtime.dispatch_to(
            &serde_json::to_vec(&root_prompt).expect("root prompt serializes"),
            UNIX_EPOCH,
            &mut Vec::new(),
        ),
        HookDispatchOutcome::Applied
    );

    for event in ["SubagentStart", "SubagentStop", "UserPromptSubmit"] {
        let mut payload = hook_payload_for_turn(event, "session-a", "child-turn", &repo);
        payload["agent_id"] = Value::String("agent-child".to_owned());
        payload["agent_type"] = Value::String("explorer".to_owned());
        let mut output = Vec::new();
        assert_eq!(
            runtime.dispatch_to(
                &serde_json::to_vec(&payload).expect("subagent event serializes"),
                UNIX_EPOCH,
                &mut output,
            ),
            HookDispatchOutcome::IgnoredSubagent,
            "event={event}"
        );
        assert!(output.is_empty(), "subagent event emitted terminal bytes");
    }

    let root_stop = hook_payload_for_turn("Stop", "session-a", "root-turn", &repo);
    assert_eq!(
        runtime.dispatch_to(
            &serde_json::to_vec(&root_stop).expect("root stop serializes"),
            UNIX_EPOCH,
            &mut Vec::new(),
        ),
        HookDispatchOutcome::Applied,
        "root turn must remain current after subagent lifecycle"
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn root_workspace_anchor_keeps_titles_stable_and_persists_only_safe_observations() {
    let root = TestRoot::new("root-workspace-anchor");
    let root_workspace = root.child("root-workspace");
    let alternate_workspace = root.child("temporary-worktree");
    let state = root.child("state");
    init_repo(
        &root_workspace,
        "https://example.invalid/team/root-workspace.git",
    );
    init_repo(
        &alternate_workspace,
        "https://example.invalid/team/temporary-worktree.git",
    );
    let resolver = WorkspaceIdentityResolver::new(&state);
    resolver
        .set_alias_override(&root_workspace, "ROOT")
        .expect("root alias is configured for deterministic title assertions");
    resolver
        .set_alias_override(&alternate_workspace, "ALT")
        .expect("alternate alias is configured without changing the root binding");

    let runtime = CodexHookRuntime::new(&state, true);
    let mut start = hook_payload("SessionStart", "session-anchor", &root_workspace);
    start["source"] = Value::String("startup".to_owned());
    let mut output = Vec::new();
    assert_eq!(
        runtime.dispatch_to(
            &serde_json::to_vec(&start).expect("start serializes"),
            UNIX_EPOCH,
            &mut output,
        ),
        HookDispatchOutcome::Applied
    );
    assert!(String::from_utf8_lossy(&output).contains("]0;○ ROOT"));

    let tool = hook_payload_for_turn(
        "PostToolUse",
        "session-anchor",
        "root-turn",
        &alternate_workspace,
    );
    output.clear();
    assert_eq!(
        runtime.dispatch_to(
            &serde_json::to_vec(&tool).expect("alternate tool serializes"),
            UNIX_EPOCH,
            &mut output,
        ),
        HookDispatchOutcome::Applied
    );
    let tool_title = String::from_utf8_lossy(&output);
    assert!(tool_title.contains("]0;○ ROOT"));
    assert!(!tool_title.contains("]0;○ ALT"));

    let mut subagent_start = hook_payload_for_turn(
        "SubagentStart",
        "session-anchor",
        "child-turn",
        &alternate_workspace,
    );
    subagent_start["agent_id"] = Value::String("agent-private-child".to_owned());
    subagent_start["agent_type"] = Value::String("explorer".to_owned());
    assert_eq!(
        runtime.dispatch_to(
            &serde_json::to_vec(&subagent_start).expect("subagent start serializes"),
            UNIX_EPOCH,
            &mut Vec::new(),
        ),
        HookDispatchOutcome::IgnoredSubagent
    );
    let anchor_state = files_under(&state)
        .into_iter()
        .find(|path| {
            path.to_string_lossy()
                .contains("codex-root-workspace-anchor-v1")
                && path
                    .extension()
                    .is_some_and(|extension| extension == "json")
        })
        .expect("explicit lifecycle observation writes one owned anchor state");
    let anchor_text = fs::read_to_string(&anchor_state).expect("anchor state reads");
    assert!(anchor_text.contains("\"active_subagents\": 1"));
    assert!(
        anchor_text.contains("\"workspace_mismatch_observed\": true"),
        "a distinct post-anchor worktree latches observability without replacing root authority"
    );
    let alternate_path = alternate_workspace.to_string_lossy().into_owned();
    for forbidden in [
        "agent-private-child",
        "temporary-worktree",
        alternate_path.as_str(),
        "session-anchor",
    ] {
        assert!(
            !anchor_text.contains(forbidden),
            "anchor state leaked {forbidden}"
        );
    }

    let mut subagent_stop = subagent_start.clone();
    subagent_stop["hook_event_name"] = Value::String("SubagentStop".to_owned());
    assert_eq!(
        runtime.dispatch_to(
            &serde_json::to_vec(&subagent_stop).expect("subagent stop serializes"),
            UNIX_EPOCH,
            &mut Vec::new(),
        ),
        HookDispatchOutcome::IgnoredSubagent
    );
    let anchor_text = fs::read_to_string(&anchor_state).expect("anchor state rereads");
    assert!(anchor_text.contains("\"active_subagents\": 0"));

    let mut rebind = hook_payload("SessionStart", "session-anchor", &alternate_workspace);
    rebind["source"] = Value::String("clear".to_owned());
    output.clear();
    assert_eq!(
        runtime.dispatch_to(
            &serde_json::to_vec(&rebind).expect("authorized rebind serializes"),
            UNIX_EPOCH,
            &mut output,
        ),
        HookDispatchOutcome::Applied
    );
    assert!(String::from_utf8_lossy(&output).contains("]0;○ ALT"));

    let mut resume = hook_payload("SessionStart", "session-anchor", &root_workspace);
    resume["source"] = Value::String("resume".to_owned());
    output.clear();
    assert_eq!(
        runtime.dispatch_to(
            &serde_json::to_vec(&resume).expect("authorized resume serializes"),
            UNIX_EPOCH,
            &mut output,
        ),
        HookDispatchOutcome::Applied
    );
    assert!(String::from_utf8_lossy(&output).contains("]0;○ ROOT"));

    let end = hook_payload("SessionEnd", "session-anchor", &root_workspace);
    assert_eq!(
        runtime.dispatch_to(
            &serde_json::to_vec(&end).expect("end serializes"),
            UNIX_EPOCH,
            &mut Vec::new(),
        ),
        HookDispatchOutcome::Applied
    );
    let retired_state = fs::read_to_string(&anchor_state).expect("retired state reads");
    assert!(retired_state.contains("\"anchor\": null"));
    assert!(!retired_state.contains("effective_alias"));
    assert!(!retired_state.contains("workspace_identity_sha256"));

    let reused_prompt = hook_payload_for_turn(
        "UserPromptSubmit",
        "session-anchor",
        "reused-turn",
        &alternate_workspace,
    );
    output.clear();
    assert_eq!(
        runtime.dispatch_to(
            &serde_json::to_vec(&reused_prompt).expect("reused prompt serializes"),
            UNIX_EPOCH + Duration::from_secs(24 * 60 * 60 + 1),
            &mut output,
        ),
        HookDispatchOutcome::Applied
    );
    let reused_title = String::from_utf8_lossy(&output);
    assert!(reused_title.contains(" ALT"));
    assert!(!reused_title.contains(" ROOT"));
}

#[test]
fn persisted_generation_state_contains_no_prompt_or_tool_bodies() {
    let root = TestRoot::new("persisted-content-minimization");
    let repo = root.child("repo");
    let state = root.child("state");
    init_repo(&repo, "https://example.invalid/team/repo.git");
    let runtime = CodexHookRuntime::new(&state, true);
    let marker = "TB-G10-SENSITIVE-CONTENT-MUST-NOT-PERSIST";
    let mut prompt =
        hook_payload_for_turn("UserPromptSubmit", "session-secret", "turn-secret", &repo);
    prompt["prompt"] = Value::String(marker.to_owned());
    assert_eq!(
        runtime.dispatch_to(
            &serde_json::to_vec(&prompt).expect("prompt serializes"),
            UNIX_EPOCH,
            &mut Vec::new(),
        ),
        HookDispatchOutcome::Applied
    );
    let mut tool = hook_payload_for_turn("PostToolUse", "session-secret", "turn-secret", &repo);
    tool["tool_input"] = json!({"marker": marker});
    tool["tool_response"] = json!({"marker": marker});
    assert_eq!(
        runtime.dispatch_to(
            &serde_json::to_vec(&tool).expect("tool serializes"),
            UNIX_EPOCH,
            &mut Vec::new(),
        ),
        HookDispatchOutcome::Applied
    );

    let files = files_under(&state);
    assert!(!files.is_empty(), "runtime state evidence must exist");
    for path in files {
        let bytes = fs::read(&path).expect("state file reads");
        let text = String::from_utf8_lossy(&bytes);
        assert!(
            !text.contains(marker),
            "sensitive body persisted in {path:?}"
        );
        assert!(
            !text.contains("tool_input"),
            "tool input key persisted in {path:?}"
        );
        assert!(
            !text.contains("tool_response"),
            "tool output key persisted in {path:?}"
        );
        assert!(!text.contains("prompt"), "prompt key persisted in {path:?}");
        assert!(
            !text.contains("session-secret"),
            "raw session ID persisted in {path:?}"
        );
        assert!(
            !text.contains("turn-secret"),
            "raw turn ID persisted in {path:?}"
        );
    }
}

#[test]
fn corrupt_generation_state_loses_decoration_without_emitting_terminal_bytes() {
    let root = TestRoot::new("corrupt-generation-state");
    let repo = root.child("repo");
    let state = root.child("state");
    init_repo(&repo, "https://example.invalid/team/repo.git");
    let runtime = CodexHookRuntime::new(&state, true);
    let prompt = hook_payload_for_turn("UserPromptSubmit", "session-a", "turn-1", &repo);
    assert_eq!(
        runtime.dispatch_to(
            &serde_json::to_vec(&prompt).expect("prompt serializes"),
            UNIX_EPOCH,
            &mut Vec::new(),
        ),
        HookDispatchOutcome::Applied
    );
    let generation_file = files_under(&state.join("codex-turn-state-v1"))
        .into_iter()
        .find(|path| path.extension().is_some_and(|value| value == "json"))
        .expect("generation state file exists");
    fs::write(generation_file, b"not-json").expect("generation state is corrupted for the test");

    let stop = hook_payload_for_turn("Stop", "session-a", "turn-1", &repo);
    let mut output = Vec::new();
    assert_eq!(
        runtime.dispatch_to(
            &serde_json::to_vec(&stop).expect("stop serializes"),
            UNIX_EPOCH,
            &mut output,
        ),
        HookDispatchOutcome::DegradedGenerationState
    );
    assert!(output.is_empty());
}

#[test]
fn production_hook_path_applies_each_required_v0_1_channel_combination() {
    let root = TestRoot::new("runtime-settings");
    let repo = root.child("workstation-manager");
    init_repo(
        &repo,
        "https://github.com/JerrySkywalker/workstation-manager.git",
    );
    let raw = serde_json::to_vec(&hook_payload("UserPromptSubmit", "session-a", &repo))
        .expect("prompt serializes");
    let cases = [
        (
            "full-muted-dark",
            PresentationSettings::new(
                TitleMode::TabBeacon,
                TabColorMode::TabBeacon,
                ActivityMode::TitleIndicator,
                SpinnerPreset::Codex,
                PresentationTheme::MutedDark,
            ),
            ["• WORKMANA", "rgb:1b/4e/3a", "]9;4;0;0"].as_slice(),
            ["9;4;3;0"].as_slice(),
        ),
        (
            "native",
            PresentationSettings::new(
                TitleMode::Native,
                TabColorMode::Native,
                ActivityMode::Native,
                SpinnerPreset::Codex,
                PresentationTheme::MutedDark,
            ),
            ["]9;4;0;0", "]104;264"].as_slice(),
            ["]0;", "rgb:"].as_slice(),
        ),
        (
            "minimal-title-only",
            PresentationSettings::new(
                TitleMode::TabBeacon,
                TabColorMode::Native,
                ActivityMode::TitleIndicator,
                SpinnerPreset::Codex,
                PresentationTheme::MutedDark,
            ),
            ["• WORKMANA", "]104;264"].as_slice(),
            ["rgb:", "9;4;3;0"].as_slice(),
        ),
        (
            "spinner-without-color",
            PresentationSettings::new(
                TitleMode::TabBeacon,
                TabColorMode::Off,
                ActivityMode::TitleSpinner,
                SpinnerPreset::Braille,
                PresentationTheme::MutedDark,
            ),
            ["⠋ WORKMANA", "]104;264"].as_slice(),
            ["rgb:", "9;4;3;0"].as_slice(),
        ),
        (
            "color-and-ring",
            PresentationSettings::new(
                TitleMode::TabBeacon,
                TabColorMode::TabBeacon,
                ActivityMode::WindowsTerminalRing,
                SpinnerPreset::Codex,
                PresentationTheme::MutedDark,
            ),
            ["○ WORKMANA", "rgb:1b/4e/3a", "]9;4;3;0"].as_slice(),
            ["• WORKMANA", "working"].as_slice(),
        ),
        (
            "activity-off",
            PresentationSettings::new(
                TitleMode::TabBeacon,
                TabColorMode::TabBeacon,
                ActivityMode::Off,
                SpinnerPreset::Codex,
                PresentationTheme::MutedDark,
            ),
            ["○ WORKMANA", "rgb:1b/4e/3a", "]9;4;0;0"].as_slice(),
            ["• WORKMANA", "working", "9;4;3;0"].as_slice(),
        ),
    ];
    for (name, settings, expected, absent) in cases {
        assert_runtime_settings_case(&root, &raw, name, settings, expected, absent);
    }
}

#[test]
fn runtime_withholds_always_badge_without_current_provider_admission() {
    let root = TestRoot::new("runtime-provider-admission");
    let repo = root.child("workstation-manager");
    init_repo(
        &repo,
        "https://github.com/JerrySkywalker/workstation-manager.git",
    );
    let raw = serde_json::to_vec(&hook_payload("UserPromptSubmit", "session-a", &repo))
        .expect("prompt serializes");
    let settings = PresentationSettings::default()
        .with_activity(ActivityMode::WindowsTerminalRing)
        .with_provider_badge(ProviderBadgePolicy::Always);
    let runtime = CodexHookRuntime::with_settings(root.child("state"), true, settings);
    let mut output = Vec::new();

    assert_eq!(
        runtime.dispatch_to(&raw, UNIX_EPOCH, &mut output),
        HookDispatchOutcome::Applied
    );
    let rendered = String::from_utf8(output).expect("terminal bytes are UTF-8");
    assert!(rendered.contains("○ WORKMANA"));
    assert!(
        !rendered.contains("·C"),
        "runtime must not infer current provider admission from stale setup evidence"
    );
}

fn assert_runtime_settings_case(
    root: &TestRoot,
    raw: &[u8],
    name: &str,
    settings: PresentationSettings,
    expected: &[&str],
    absent: &[&str],
) {
    let state = format!("state-{name}");
    let runtime = CodexHookRuntime::with_settings(root.child(&state), true, settings);
    let mut output = Vec::new();
    assert_eq!(
        runtime.dispatch_to(raw, UNIX_EPOCH, &mut output),
        HookDispatchOutcome::Applied,
        "{name} dispatches through the full hook path"
    );
    let rendered = String::from_utf8(output).expect("terminal bytes are UTF-8");
    for expected in expected {
        assert!(
            rendered.contains(expected),
            "{name} must include {expected:?}: {rendered}"
        );
    }
    for absent in absent {
        assert!(
            !rendered.contains(absent),
            "{name} must omit {absent:?}: {rendered}"
        );
    }
}

#[test]
fn runtime_preserves_linked_worktree_identity_and_collision_aliases() {
    let root = TestRoot::new("worktree-collision");
    let ordinary = root.child("ordinary");
    let linked = root.child("linked");
    init_repo(
        &ordinary,
        "https://example.invalid/team/jerry-proxy-control.git",
    );
    git(
        &ordinary,
        &[
            "worktree",
            "add",
            "--quiet",
            "--detach",
            linked.to_str().expect("test path is Unicode"),
        ],
    );
    let colliding = root.child("colliding");
    init_repo(
        &colliding,
        "https://example.invalid/team/java-platform-core.git",
    );
    let runtime = CodexHookRuntime::new(root.child("state"), false);
    let render = |cwd: &Path, session: &str| {
        let mut output = Vec::new();
        let payload = hook_payload("UserPromptSubmit", session, cwd);
        assert_eq!(
            runtime.dispatch_to(
                &serde_json::to_vec(&payload).expect("payload serializes"),
                UNIX_EPOCH,
                &mut output
            ),
            HookDispatchOutcome::Applied
        );
        String::from_utf8(output).expect("renderer output is UTF-8")
    };
    assert!(render(&ordinary, "one").contains("○ JPC"));
    assert!(render(&linked, "two").contains("○ JPC"));
    let newcomer = render(&colliding, "three");
    assert!(!newcomer.contains("]0;○ JPC"));
    assert!(!newcomer.contains("rgb:"));
}

struct FailingWriter;

impl Write for FailingWriter {
    fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::BrokenPipe, "test sink"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn non_git_runtime_and_remaining_failures_stay_fail_open() {
    let root = TestRoot::new("fail-open");
    let runtime = CodexHookRuntime::new(root.child("state"), true);
    let ordinary_workspace = root.child("not-a-repository");
    fs::create_dir_all(&ordinary_workspace).expect("non-repository cwd exists");
    let prompt = hook_payload("UserPromptSubmit", "session-a", &ordinary_workspace);
    let mut ordinary_output = Vec::new();
    assert_eq!(
        runtime.dispatch_to(
            &serde_json::to_vec(&prompt).expect("payload serializes"),
            UNIX_EPOCH,
            &mut ordinary_output
        ),
        HookDispatchOutcome::Applied
    );
    let ordinary_output = String::from_utf8(ordinary_output).expect("renderer output is UTF-8");
    assert!(ordinary_output.contains("]0;○ NAR"));
    assert!(!ordinary_output.contains(&ordinary_workspace.to_string_lossy().to_string()));
    assert_eq!(
        runtime.dispatch_to(
            &serde_json::to_vec(&hook_payload(
                "UserPromptSubmit",
                "missing-session",
                &root.child("missing-cwd")
            ))
            .expect("payload serializes"),
            UNIX_EPOCH,
            &mut Vec::new()
        ),
        HookDispatchOutcome::DegradedWorkspaceIdentity
    );
    assert_eq!(
        runtime.dispatch_to(b"broken", UNIX_EPOCH, &mut Vec::new()),
        HookDispatchOutcome::DegradedInput
    );

    let repo = root.child("repo");
    init_repo(&repo, "https://example.invalid/team/repo.git");
    assert_eq!(
        runtime.dispatch_to(
            &serde_json::to_vec(&hook_payload("Stop", "session-a", &repo))
                .expect("payload serializes"),
            UNIX_EPOCH,
            &mut FailingWriter
        ),
        HookDispatchOutcome::DegradedPresentationOutput
    );
    let mut compact = hook_payload("SessionStart", "session-a", &repo);
    compact["source"] = Value::String("compact".to_owned());
    assert_eq!(
        runtime.dispatch_to(
            &serde_json::to_vec(&compact).expect("compact serializes"),
            UNIX_EPOCH,
            &mut Vec::new()
        ),
        HookDispatchOutcome::PreservedCurrentState
    );
}

#[test]
fn setup_is_idempotent_preserves_unrelated_config_and_uninstalls_exactly_owned_parts() {
    let root = TestRoot::new("config-roundtrip");
    let codex_home = root.child("codex-home");
    fs::create_dir_all(&codex_home).expect("Codex home is created");
    let original_hooks = json!({
        "description": "owner hooks",
        "hooks": {
            "PreToolUse": [{
                "matcher": "Bash",
                "hooks": [{"type": "command", "command": "owner-hook"}]
            }]
        }
    });
    fs::write(
        codex_home.join("hooks.json"),
        serde_json::to_vec_pretty(&original_hooks).expect("hooks serialize"),
    )
    .expect("hooks fixture is written");
    let original_config = r#"# owner comment
model = "gpt-test"

[mcp_servers.owner]
command = "owner-mcp"

[tui]
terminal_title = ["activity", "project"]
animations = false
"#;
    fs::write(codex_home.join("config.toml"), original_config).expect("config fixture is written");
    let integration = test_integration(&root);
    assert_eq!(
        integration.setup().expect("setup succeeds"),
        SetupOutcome::InstalledTrustReviewRequired
    );
    let installed_hooks = fs::read(codex_home.join("hooks.json")).expect("installed hooks read");
    let installed_config = fs::read(codex_home.join("config.toml")).expect("installed config read");
    let manifest = fs::read(root.child("state/integration-v1.json")).expect("manifest reads");
    let parsed: Value = serde_json::from_slice(&installed_hooks).expect("installed hooks parse");
    assert_eq!(
        parsed["hooks"]["PreToolUse"].as_array().map(Vec::len),
        Some(2)
    );
    let config_text = String::from_utf8(installed_config.clone()).expect("config is UTF-8");
    assert!(config_text.contains("# owner comment"));
    assert!(config_text.contains("command = \"owner-mcp\""));
    assert!(config_text.contains("terminal_title = []"));

    assert_eq!(
        integration.setup().expect("second setup succeeds"),
        SetupOutcome::AlreadyInstalled
    );
    assert_eq!(
        fs::read(codex_home.join("hooks.json")).expect("hooks reread"),
        installed_hooks
    );
    assert_eq!(
        fs::read(codex_home.join("config.toml")).expect("config reread"),
        installed_config
    );
    assert_eq!(
        fs::read(root.child("state/integration-v1.json")).expect("manifest reread"),
        manifest
    );

    let doctor = integration.doctor();
    assert_eq!(doctor.overall(), DoctorStatus::Warning);
    assert!(doctor.checks().iter().any(|check| {
        check.id() == "hooks.trust"
            && check.status() == DoctorStatus::Warning
            && check.summary().contains("require review")
    }));

    assert_eq!(
        integration.uninstall().expect("uninstall succeeds"),
        UninstallOutcome::Removed
    );
    let restored_hooks: Value = serde_json::from_slice(
        &fs::read(codex_home.join("hooks.json")).expect("restored hooks read"),
    )
    .expect("restored hooks parse");
    assert_eq!(restored_hooks, original_hooks);
    let restored_config =
        fs::read_to_string(codex_home.join("config.toml")).expect("restored config reads");
    assert!(restored_config.contains("# owner comment"));
    assert!(
        restored_config.contains("[\"activity\", \"project\"]"),
        "restored config did not retain the prior title value: {restored_config}"
    );
    assert!(restored_config.contains("command = \"owner-mcp\""));
    assert_eq!(
        integration.uninstall().expect("repeat uninstall succeeds"),
        UninstallOutcome::NotInstalled
    );
}

#[test]
fn setup_upgrade_preserves_ntfy_plugins_multiple_stop_hooks_and_unknown_events() {
    let root = TestRoot::new("unrelated-hook-matrix");
    let codex_home = root.child("codex-home");
    fs::create_dir_all(&codex_home).expect("Codex home is created");
    let ntfy_stop = json!({
        "matcher": "",
        "hooks": [{"type": "command", "command": "codex-ntfy-notifier Stop"}]
    });
    let plugin_stop = json!({
        "hooks": [{"type": "command", "command": "owner-plugin stop"}],
        "plugin": "owner.plugin"
    });
    let user_stop = json!({
        "hooks": [{"type": "command", "command": "owner-user-hook"}]
    });
    let unknown_event = json!([{
        "hooks": [{"type": "command", "command": "future-owner-hook"}],
        "opaque_owner_field": true
    }]);
    let original_hooks = json!({
        "description": "owner hook matrix",
        "hooks": {
            "Stop": [ntfy_stop.clone(), plugin_stop.clone(), user_stop.clone()],
            "FutureLifecycleEvent": unknown_event.clone()
        }
    });
    fs::write(
        codex_home.join("hooks.json"),
        serde_json::to_vec_pretty(&original_hooks).expect("owner hooks serialize"),
    )
    .expect("owner hooks write");
    fs::write(codex_home.join("config.toml"), "model = \"gpt-test\"\n")
        .expect("owner config writes");

    let integration = test_integration(&root);
    assert_eq!(
        integration.setup().expect("setup succeeds"),
        SetupOutcome::InstalledTrustReviewRequired
    );
    let installed: Value = serde_json::from_slice(
        &fs::read(codex_home.join("hooks.json")).expect("installed hooks read"),
    )
    .expect("installed hooks parse");
    assert_eq!(
        &installed["hooks"]["Stop"].as_array().expect("Stop groups")[..3],
        &[ntfy_stop, plugin_stop, user_stop]
    );
    assert_eq!(installed["hooks"]["FutureLifecycleEvent"], unknown_event);
    assert_eq!(installed["hooks"]["Stop"].as_array().map(Vec::len), Some(4));

    assert_eq!(
        integration.uninstall().expect("uninstall succeeds"),
        UninstallOutcome::Removed
    );
    let restored: Value = serde_json::from_slice(
        &fs::read(codex_home.join("hooks.json")).expect("restored hooks read"),
    )
    .expect("restored hooks parse");
    assert_eq!(restored, original_hooks);
}

#[test]
fn title_preference_reconciliation_restores_native_codex_title_without_losing_baseline() {
    let root = TestRoot::new("title-preference");
    let codex_home = root.child("codex-home");
    fs::create_dir_all(&codex_home).expect("Codex home is created");
    let original = "[tui]\nterminal_title = [\"activity\", \"project\"]\n";
    fs::write(codex_home.join("config.toml"), original).expect("owner config writes");
    let integration = test_integration(&root);
    integration.setup().expect("setup owns the title");

    assert_eq!(
        integration
            .reconcile_title_ownership(false)
            .expect("native preference restores title"),
        TitleOwnershipOutcome::Updated
    );
    assert!(
        fs::read_to_string(codex_home.join("config.toml"))
            .expect("native config reads")
            .contains("[\"activity\", \"project\"]")
    );
    let report = integration.doctor();
    assert!(report.checks().iter().any(|check| {
        check.id() == "terminal.title"
            && check.status() == DoctorStatus::Pass
            && check.summary().contains("native")
    }));

    assert_eq!(
        integration
            .reconcile_title_ownership(true)
            .expect("TabBeacon reacquires title"),
        TitleOwnershipOutcome::Updated
    );
    assert!(
        fs::read_to_string(codex_home.join("config.toml"))
            .expect("owned config reads")
            .contains("terminal_title = []")
    );
    integration
        .uninstall()
        .expect("uninstall restores original baseline");
    assert_eq!(
        fs::read_to_string(codex_home.join("config.toml")).expect("restored config reads"),
        original
    );
}

#[test]
fn title_disabled_setup_preserves_an_absent_codex_config_through_lifecycle() {
    let root = TestRoot::new("absent-title-config");
    let integration = test_integration(&root);
    let codex_home = root.child("codex-home");

    assert_eq!(
        integration
            .setup_with_title_ownership(false)
            .expect("native title setup succeeds"),
        SetupOutcome::InstalledTrustReviewRequired
    );
    assert!(codex_home.join("hooks.json").is_file());
    assert!(
        !codex_home.join("config.toml").exists(),
        "native title setup must preserve an originally absent config"
    );
    assert_eq!(
        integration
            .setup_with_title_ownership(false)
            .expect("repeat native title setup succeeds"),
        SetupOutcome::AlreadyInstalled
    );
    let report = integration.doctor();
    assert!(report.checks().iter().any(|check| {
        check.id() == "terminal.title"
            && check.status() == DoctorStatus::Pass
            && check.summary().contains("native")
    }));

    assert_eq!(
        integration
            .reconcile_title_ownership(true)
            .expect("TabBeacon title acquisition succeeds"),
        TitleOwnershipOutcome::Updated
    );
    assert!(codex_home.join("config.toml").is_file());
    assert_eq!(
        integration
            .reconcile_title_ownership(false)
            .expect("native title restoration succeeds"),
        TitleOwnershipOutcome::Updated
    );
    assert!(
        !codex_home.join("config.toml").exists(),
        "restoring the original empty config must remove it"
    );

    assert_eq!(
        integration
            .uninstall()
            .expect("native title uninstall succeeds"),
        UninstallOutcome::Removed
    );
    assert!(!root.child("state/integration-v1.json").exists());
    assert!(!codex_home.join("config.toml").exists());
}

#[cfg(windows)]
#[test]
fn title_reconciliation_refuses_a_dangling_codex_config_symlink() {
    let root = TestRoot::new("dangling-title-config-link");
    let integration = test_integration(&root);
    let codex_home = root.child("codex-home");
    integration
        .setup_with_title_ownership(false)
        .expect("native title setup succeeds");
    let config_path = codex_home.join("config.toml");
    let missing_target = root.child("missing-config-target.toml");
    match std::os::windows::fs::symlink_file(&missing_target, &config_path) {
        Ok(()) => {}
        // Windows permits unprivileged symbolic links only when Developer Mode
        // is enabled. Ordinary integration coverage may skip this capability-
        // conditioned fixture, but the dedicated release workflow must prove
        // it actually executed on a capable runner.
        Err(error) if error.raw_os_error() == Some(1314) => {
            assert!(
                std::env::var_os("TABBEACON_REQUIRE_DANGLING_SYMLINK").is_none(),
                "dangling symbolic-link release fixture requires Windows symbolic-link capability: {error}"
            );
            eprintln!("skipping symbolic-link fixture: Windows privilege unavailable");
            return;
        }
        Err(error) => panic!("dangling config link creates in the isolated fixture: {error}"),
    }

    assert!(matches!(
        integration.reconcile_title_ownership(true),
        Err(CodexIntegrationError::SymbolicLinkTarget)
    ));
    assert!(
        fs::symlink_metadata(&config_path)
            .expect("config link remains inspectable")
            .file_type()
            .is_symlink(),
        "title reconciliation must not replace the unowned link"
    );
    println!("DANGLING_SYMLINK_FIXTURE_EXECUTED=true");
    println!("DANGLING_SYMLINK_POLICY=PASS");
}

#[test]
fn repeated_setup_ten_times_keeps_exactly_eleven_owned_hook_definitions() {
    let root = TestRoot::new("setup-ten-times");
    let integration = test_integration(&root);
    assert_eq!(
        integration.setup().expect("initial setup succeeds"),
        SetupOutcome::InstalledTrustReviewRequired
    );
    for _ in 0..10 {
        assert_eq!(
            integration.setup().expect("repeated setup succeeds"),
            SetupOutcome::AlreadyInstalled
        );
    }
    let hooks: Value = serde_json::from_slice(
        &fs::read(root.child("codex-home/hooks.json")).expect("installed hooks read"),
    )
    .expect("installed hooks parse");
    assert_eq!(owned_current_windows_handler_count(&hooks), 11);
    for event in ADMITTED_HOOK_EVENTS {
        assert_eq!(hooks["hooks"][event].as_array().map(Vec::len), Some(1));
    }
}

#[test]
fn relocated_executable_migrates_exact_owned_hooks_without_duplicates() {
    let root = TestRoot::new("relocated-executable");
    let original = test_integration(&root);
    assert_eq!(
        original.setup().expect("initial setup succeeds"),
        SetupOutcome::InstalledTrustReviewRequired
    );

    let relocated_executable = root.child(if cfg!(windows) {
        "relocated/tabbeacon.exe"
    } else {
        "relocated/tabbeacon"
    });
    fs::create_dir_all(relocated_executable.parent().expect("relocated parent"))
        .expect("relocated parent is created");
    fs::write(&relocated_executable, b"relocated test executable")
        .expect("relocated executable is written");
    let relocated = CodexIntegration::new(
        root.child("codex-home"),
        root.child("state"),
        &relocated_executable,
    )
    .with_codex_program(root.child(if cfg!(windows) {
        "codex-version-probe.exe"
    } else {
        "codex-version-probe"
    }));

    let before_migration = relocated.doctor();
    assert!(before_migration.checks().iter().any(|check| {
        check.id() == "hooks.currentness" && check.status() == DoctorStatus::Fail
    }));
    assert_eq!(
        relocated.setup().expect("exact owned relocation succeeds"),
        SetupOutcome::Upgraded
    );

    let manifest: Value = serde_json::from_slice(
        &fs::read(root.child("state/integration-v1.json")).expect("migrated manifest reads"),
    )
    .expect("migrated manifest parses");
    assert_eq!(
        manifest["executable"],
        json!(relocated_executable.to_string_lossy().to_string())
    );
    let hooks: Value = serde_json::from_slice(
        &fs::read(root.child("codex-home/hooks.json")).expect("migrated hooks read"),
    )
    .expect("migrated hooks parse");
    assert_eq!(owned_current_windows_handler_count(&hooks), 11);
    let after_migration = relocated.doctor();
    assert!(after_migration.checks().iter().any(|check| {
        check.id() == "hooks.currentness" && check.status() == DoctorStatus::Pass
    }));
    assert_eq!(
        relocated
            .uninstall()
            .expect("migrated integration uninstalls"),
        UninstallOutcome::Removed
    );
}

#[test]
fn relocated_executable_refuses_an_unsafe_recorded_manifest_target() {
    let root = TestRoot::new("unsafe-relocation");
    let original = test_integration(&root);
    original.setup().expect("initial setup succeeds");
    let manifest_path = root.child("state/integration-v1.json");
    let mut manifest: Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("manifest reads before mutation"))
            .expect("manifest parses before mutation");
    manifest["executable"] = json!("relative-tabbeacon.exe");
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).expect("unsafe manifest serializes"),
    )
    .expect("unsafe manifest writes");
    let hooks_path = root.child("codex-home/hooks.json");
    let hooks_before = fs::read(&hooks_path).expect("hooks read before unsafe relocation");

    let relocated_executable = root.child(if cfg!(windows) {
        "relocated/tabbeacon.exe"
    } else {
        "relocated/tabbeacon"
    });
    fs::create_dir_all(relocated_executable.parent().expect("relocated parent"))
        .expect("relocated parent is created");
    fs::write(&relocated_executable, b"relocated test executable")
        .expect("relocated executable is written");
    let relocated = CodexIntegration::new(
        root.child("codex-home"),
        root.child("state"),
        &relocated_executable,
    )
    .with_codex_program(root.child(if cfg!(windows) {
        "codex-version-probe.exe"
    } else {
        "codex-version-probe"
    }));

    assert!(matches!(
        relocated.setup(),
        Err(CodexIntegrationError::OwnershipManifest)
    ));
    assert_eq!(
        fs::read(&hooks_path).expect("hooks read after unsafe relocation"),
        hooks_before
    );
}

#[test]
fn setup_upgrades_exact_owned_declarations_without_duplicates_or_baseline_loss() {
    let root = TestRoot::new("owned-upgrade");
    let codex_home = root.child("codex-home");
    fs::create_dir_all(&codex_home).expect("Codex home is created");
    let unrelated_stop = json!({
        "hooks": [{"type": "command", "command": "owner-stop-hook"}]
    });
    fs::write(
        codex_home.join("hooks.json"),
        serde_json::to_vec_pretty(&json!({
            "description": "owner hooks",
            "hooks": {"Stop": [unrelated_stop.clone()]}
        }))
        .expect("owner hooks serialize"),
    )
    .expect("owner hooks are written");
    fs::write(codex_home.join("config.toml"), "model = \"gpt-test\"\n")
        .expect("owner config is written");

    let integration = test_integration(&root);
    assert_eq!(
        integration.setup().expect("initial setup succeeds"),
        SetupOutcome::InstalledTrustReviewRequired
    );
    let manifest_path = root.child("state/integration-v1.json");
    let mut manifest: Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("manifest reads"))
            .expect("manifest parses");
    let original_hooks_backup = manifest["hooks_backup"].clone();
    let original_config_backup = manifest["config_backup"].clone();
    let mut hooks: Value =
        serde_json::from_slice(&fs::read(codex_home.join("hooks.json")).expect("hooks read"))
            .expect("hooks parse");

    replace_manifest_owned_declarations_with_legacy(&mut hooks, &mut manifest);
    fs::write(
        codex_home.join("hooks.json"),
        serde_json::to_vec_pretty(&hooks).expect("legacy hooks serialize"),
    )
    .expect("legacy hooks are written");
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).expect("legacy manifest serialize"),
    )
    .expect("legacy manifest is written");

    let before_upgrade = integration.doctor();
    assert_eq!(before_upgrade.overall(), DoctorStatus::Fail);
    assert!(before_upgrade.checks().iter().any(|check| {
        check.id() == "hooks.currentness"
            && check.status() == DoctorStatus::Fail
            && check.summary().contains("require a TabBeacon upgrade")
    }));

    assert_eq!(
        integration.setup().expect("owned upgrade succeeds"),
        SetupOutcome::Upgraded
    );
    let upgraded_manifest: Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("upgraded manifest reads"))
            .expect("upgraded manifest parses");
    assert_eq!(upgraded_manifest["hooks_backup"], original_hooks_backup);
    assert_eq!(upgraded_manifest["config_backup"], original_config_backup);
    let upgraded_hooks: Value = serde_json::from_slice(
        &fs::read(codex_home.join("hooks.json")).expect("upgraded hooks read"),
    )
    .expect("upgraded hooks parse");
    let owned_handler_count = owned_current_windows_handler_count(&upgraded_hooks);
    assert_eq!(
        owned_handler_count, 11,
        "upgrade must never append another eleven hooks"
    );
    assert_eq!(upgraded_hooks["hooks"]["Stop"][0], unrelated_stop);
    let after_upgrade = integration.doctor();
    assert!(after_upgrade.checks().iter().any(|check| {
        check.id() == "hooks.currentness" && check.status() == DoctorStatus::Pass
    }));

    assert_eq!(
        integration
            .uninstall()
            .expect("upgraded integration uninstalls"),
        UninstallOutcome::Removed
    );
    let uninstalled_hooks: Value = serde_json::from_slice(
        &fs::read(codex_home.join("hooks.json")).expect("uninstalled hooks read"),
    )
    .expect("uninstalled hooks parse");
    assert_eq!(uninstalled_hooks["hooks"]["Stop"], json!([unrelated_stop]));
}

#[test]
fn setup_and_uninstall_are_safe_for_absent_files_and_modified_ownership() {
    let root = TestRoot::new("ownership");
    let integration = test_integration(&root);
    assert_eq!(
        integration.setup().expect("empty setup succeeds"),
        SetupOutcome::InstalledTrustReviewRequired
    );
    let codex_home = root.child("codex-home");
    assert!(codex_home.join("hooks.json").is_file());
    assert!(codex_home.join("config.toml").is_file());

    let mut hooks: Value =
        serde_json::from_slice(&fs::read(codex_home.join("hooks.json")).expect("hooks read"))
            .expect("hooks parse");
    hooks["hooks"]["Stop"][0]["hooks"][0]["timeout"] = json!(2);
    fs::write(
        codex_home.join("hooks.json"),
        serde_json::to_vec_pretty(&hooks).expect("mutated hooks serialize"),
    )
    .expect("mutated hook is written");
    let report = integration.doctor();
    assert!(report.checks().iter().any(|check| {
        check.id() == "hooks.declarations" && check.status() == DoctorStatus::Fail
    }));
    assert!(matches!(
        integration.uninstall(),
        Err(CodexIntegrationError::ModifiedOwnedHook)
    ));
    assert!(codex_home.join("config.toml").is_file());
}

#[test]
fn doctor_supports_current_codex_trust_shape_and_detects_inactive_or_conflicting_state() {
    let root = TestRoot::new("doctor-current-shape");
    let integration = test_integration(&root);
    integration.setup().expect("setup succeeds");
    let codex_home = root.child("codex-home");
    let trusted_keys = install_current_codex_trust_state(&codex_home);

    let report = integration.doctor();
    assert_eq!(report.overall(), DoctorStatus::Warning);
    assert!(report.checks().iter().any(|check| {
        check.id() == "hooks.runtime-probe" && check.status() == DoctorStatus::Warning
    }));
    assert!(report.checks().iter().any(|check| {
        check.id() == "hooks.trust"
            && check.status() == DoctorStatus::Pass
            && check.summary().contains("trusted and active")
    }));

    let config_path = codex_home.join("config.toml");
    let mut config = fs::read_to_string(&config_path)
        .expect("trusted config reads")
        .parse::<DocumentMut>()
        .expect("trusted config parses");
    config["hooks"]["state"][&trusted_keys[0]]["enabled"] = value(false);
    fs::write(&config_path, config.to_string()).expect("disabled hook state writes");
    let report = integration.doctor();
    assert!(report.checks().iter().any(|check| {
        check.id() == "hooks.trust"
            && check.status() == DoctorStatus::Fail
            && check.summary().contains("disabled")
    }));

    install_current_codex_trust_state(&codex_home);
    let mut config = fs::read_to_string(&config_path)
        .expect("trusted config reads")
        .parse::<DocumentMut>()
        .expect("trusted config parses");
    config["hooks"]["state"][&trusted_keys[0]]["trusted_hash"] = value("sha256:modified");
    fs::write(&config_path, config.to_string()).expect("modified trust state writes");
    let report = integration.doctor();
    assert!(report.checks().iter().any(|check| {
        check.id() == "hooks.trust"
            && check.status() == DoctorStatus::Fail
            && check.summary().contains("TRUST_HASH_STALE_OR_CHANGED")
    }));
    assert!(report.checks().iter().any(|check| {
        check.id() == "hooks.declarations"
            && check.status() == DoctorStatus::Pass
            && check.summary().contains("DECLARATION_EXACT")
    }));
    assert!(report.checks().iter().any(|check| {
        check.id() == "hooks.currentness"
            && check.status() == DoctorStatus::Pass
            && check.summary().contains("CURRENTNESS_CURRENT")
    }));

    install_current_codex_trust_state(&codex_home);
    let mut config = fs::read_to_string(&config_path)
        .expect("restored trusted config reads")
        .parse::<DocumentMut>()
        .expect("restored trusted config parses");
    let mut title = Array::new();
    title.push("project");
    config["tui"]["terminal_title"] = value(title);
    fs::write(config_path, config.to_string()).expect("conflicting title writes");
    let report = integration.doctor();
    assert!(report.checks().iter().any(|check| {
        check.id() == "terminal.title"
            && check.status() == DoctorStatus::Fail
            && check.summary().contains("conflicts")
    }));
}

#[test]
#[allow(clippy::too_many_lines)] // One progression proves the distinct inventory state transitions.
fn hook_inventory_is_exact_redacted_and_distinguishes_trust_from_declaration_drift() {
    let root = TestRoot::new("hook-inventory");
    let integration = test_integration(&root);
    integration.setup().expect("setup succeeds");
    let codex_home = root.child("codex-home");
    let trusted_keys = install_current_codex_trust_state(&codex_home);

    let before = files_under(&root.path)
        .into_iter()
        .map(|path| {
            let relative = path
                .strip_prefix(&root.path)
                .expect("owned state remains below the fixture root")
                .to_owned();
            (relative, fs::read(path).expect("fixture state reads"))
        })
        .collect::<Vec<_>>();
    let inventory = integration.hook_inventory();
    let after = files_under(&root.path)
        .into_iter()
        .map(|path| {
            let relative = path
                .strip_prefix(&root.path)
                .expect("owned state remains below the fixture root")
                .to_owned();
            (relative, fs::read(path).expect("fixture state rereads"))
        })
        .collect::<Vec<_>>();
    assert_eq!(before, after, "inventory refresh is read-only");
    assert_eq!(inventory.availability, HookInventoryAvailability::Available);
    assert_eq!(
        inventory
            .entries
            .iter()
            .filter(|entry| entry.owner == HookOwner::TabBeacon)
            .count(),
        11,
        "all admitted Codex lifecycle declarations appear once"
    );
    assert!(inventory.entries.iter().all(|entry| {
        entry.owner != HookOwner::TabBeacon
            || (entry.trust_state == HookTrustState::Trusted
                && entry.currentness == HookCurrentness::Current)
    }));

    let config_path = codex_home.join("config.toml");
    let mut config = fs::read_to_string(&config_path)
        .expect("trusted config reads")
        .parse::<DocumentMut>()
        .expect("trusted config parses");
    config["hooks"]["state"][&trusted_keys[0]]["enabled"] = value(false);
    fs::write(&config_path, config.to_string()).expect("disabled state writes");
    let disabled = integration.hook_inventory();
    assert!(disabled.entries.iter().any(
        |entry| entry.event == "pre_tool_use" && entry.trust_state == HookTrustState::Disabled
    ));

    install_current_codex_trust_state(&codex_home);
    let mut config = fs::read_to_string(&config_path)
        .expect("restored config reads")
        .parse::<DocumentMut>()
        .expect("restored config parses");
    config["hooks"]["state"][&trusted_keys[0]]["trusted_hash"] = value("sha256:stale");
    fs::write(&config_path, config.to_string()).expect("stale hash writes");
    let stale_hash = integration.hook_inventory();
    assert!(stale_hash.entries.iter().any(|entry| {
        entry.event == "pre_tool_use"
            && entry.trust_state == HookTrustState::HashStaleOrChanged
            && entry.currentness == HookCurrentness::Current
    }));

    let hooks_path = codex_home.join("hooks.json");
    let mut hooks: Value =
        serde_json::from_slice(&fs::read(&hooks_path).expect("hooks read")).expect("hooks parse");
    hooks["hooks"]["Stop"]
        .as_array_mut()
        .expect("Stop groups are an array")
        .push(json!({
            "hooks": [{"type": "command", "command": "secret-token=do-not-leak"}]
        }));
    fs::write(
        &hooks_path,
        serde_json::to_vec_pretty(&hooks).expect("third-party hooks serialize"),
    )
    .expect("third-party hooks write");
    let mixed = integration.hook_inventory();
    assert!(
        mixed
            .entries
            .iter()
            .any(|entry| entry.owner == HookOwner::ThirdParty)
    );
    let json = serde_json::to_string(&mixed).expect("inventory serializes");
    assert!(!json.contains("secret-token"));
    assert!(!mixed.plain_lines().join("\n").contains("secret-token"));
    assert!(
        !mixed
            .human_table(tabbeacon::human_presentation::ResolvedLocale::EnUs)
            .contains("secret-token")
    );
    assert!(
        !mixed
            .human_table(tabbeacon::human_presentation::ResolvedLocale::ZhCn)
            .contains("secret-token")
    );

    hooks["hooks"]["PreToolUse"][0]["hooks"][0]["timeout"] = json!(2);
    fs::write(
        &hooks_path,
        serde_json::to_vec_pretty(&hooks).expect("modified hooks serialize"),
    )
    .expect("modified owned hook writes");
    let declaration_modified = integration.hook_inventory();
    assert!(declaration_modified.entries.iter().any(|entry| {
        entry.event == "pre_tool_use"
            && entry.currentness == HookCurrentness::DeclarationModifiedOrMissing
            && entry.owner == HookOwner::UnownedOrAmbiguous
    }));

    fs::write(&hooks_path, b"[]").expect("malformed hooks write");
    assert_eq!(
        integration.hook_inventory().availability,
        HookInventoryAvailability::Unavailable,
        "malformed provider state fails closed without a parse panic"
    );
}

#[test]
fn concurrent_setup_serializes_first_assignment_without_duplicate_hooks() {
    let root = TestRoot::new("concurrent-setup");
    let integration = test_integration(&root);
    let worker_count = 6_usize;
    let barrier = Arc::new(Barrier::new(worker_count));
    let workers = (0..worker_count)
        .map(|_| {
            let integration = integration.clone();
            let barrier = barrier.clone();
            thread::spawn(move || {
                barrier.wait();
                integration.setup().expect("concurrent setup succeeds")
            })
        })
        .collect::<Vec<_>>();
    let outcomes = workers
        .into_iter()
        .map(|worker| worker.join().expect("setup worker joins"))
        .collect::<Vec<_>>();
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| **outcome == SetupOutcome::InstalledTrustReviewRequired)
            .count(),
        1
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| **outcome == SetupOutcome::AlreadyInstalled)
            .count(),
        worker_count - 1
    );
    let hooks: Value =
        serde_json::from_slice(&fs::read(root.child("codex-home/hooks.json")).expect("hooks read"))
            .expect("hooks parse");
    for event in ADMITTED_HOOK_EVENTS {
        assert_eq!(hooks["hooks"][event].as_array().map(Vec::len), Some(1));
    }
}

#[test]
fn title_drift_and_unowned_matching_hooks_are_refused_without_overwrite() {
    let root = TestRoot::new("config-refusal");
    let integration = test_integration(&root);
    integration.setup().expect("setup succeeds");
    let config_path = root.child("codex-home/config.toml");
    fs::write(&config_path, "[tui]\nterminal_title = [\"project\"]\n")
        .expect("owned title is modified");
    let before = fs::read(&config_path).expect("modified config reads");
    assert!(matches!(
        integration.uninstall(),
        Err(CodexIntegrationError::ModifiedOwnedTitle)
    ));
    assert_eq!(fs::read(&config_path).expect("config rereads"), before);

    let other = TestRoot::new("unowned-hook");
    let codex_home = other.child("codex-home");
    fs::create_dir_all(&codex_home).expect("second Codex home is created");
    fs::write(
        codex_home.join("hooks.json"),
        br#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"tabbeacon hook codex"}]}]}}"#,
    )
    .expect("unowned matching hook is written");
    let other_integration = test_integration(&other);
    assert!(matches!(
        other_integration.setup(),
        Err(CodexIntegrationError::TabBeaconLikeAmbiguityBlocked)
    ));
}

#[test]
fn missing_managed_binary_is_diagnosed_and_command_shell_remains_fail_open() {
    let root = TestRoot::new("missing-binary");
    let integration = test_integration(&root);
    integration.setup().expect("setup succeeds");
    let manifest: Value = serde_json::from_slice(
        &fs::read(root.child("state/integration-v1.json")).expect("manifest reads"),
    )
    .expect("manifest parses");
    let executable = PathBuf::from(
        manifest["executable"]
            .as_str()
            .expect("manifest executable is a string"),
    );
    fs::remove_file(executable).expect("managed binary is removed for the test");
    let report = integration.doctor();
    assert!(report.checks().iter().any(|check| {
        check.id() == "tabbeacon.executable" && check.status() == DoctorStatus::Fail
    }));
    let hooks: Value =
        serde_json::from_slice(&fs::read(root.child("codex-home/hooks.json")).expect("hooks read"))
            .expect("hooks parse");
    for event in ADMITTED_HOOK_EVENTS {
        assert_eq!(hooks["hooks"][event][0]["hooks"][0]["async"], false);
        assert_eq!(hooks["hooks"][event][0]["hooks"][0]["timeout"], 1);
        let command = hooks["hooks"][event][0]["hooks"][0]["commandWindows"]
            .as_str()
            .expect("Windows command is a string");
        assert!(is_current_windows_hook_command(command));
        assert!(
            !command.contains(['\r', '\n']),
            "the direct fast declaration is exactly one command line"
        );
    }
}

#[cfg(windows)]
#[test]
fn generated_windows_command_runs_the_real_binary_in_the_codex_0149_shell_model() {
    // Keep a fixture component between `windows` and the generated process id.
    // The bare `...-windows-<pid>` shape can leave the nested PowerShell
    // command host open on this Windows runner, obscuring the command-shape
    // behavior this test is intended to prove.
    let root = TestRoot::new("real-command-windows-shell");
    let (executable, command) = real_windows_hook_command(&root);

    let payload = serde_json::to_vec(&hook_payload("UserPromptSubmit", "real-shell", &root.path))
        .expect("Codex-shaped payload serializes");
    assert_real_hook_direct(&executable, &payload);
    assert_real_hook_shell_matrix(&command, &payload);
    assert_real_hook_ingress(&root, &command);
}

#[cfg(windows)]
#[test]
fn shell_neutral_windows_declaration_succeeds_in_every_codex_0149_shell_mode() {
    let root = TestRoot::new("safe-command-windows-shell");
    let (executable, generic_command, windows_command) = fast_real_windows_hook_command(&root);
    let payload = serde_json::to_vec(&hook_payload("UserPromptSubmit", "safe-shell", &root.path))
        .expect("Codex-shaped payload serializes");

    // The generic declaration remains the established compatibility fallback.
    // Production `commandWindows` must use the same shell-neutral form in a
    // configured pwsh/PowerShell/cmd shell and in the empty-shell COMSPEC path.
    assert_real_hook_direct(&executable, &payload);
    assert_real_hook_shell_matrix(&generic_command, &payload);
    assert_real_hook_shell_matrix(&windows_command, &payload);
    assert_real_hook_ingress(&root, &windows_command);
}

#[cfg(windows)]
#[test]
fn legacy_cmd_fail_open_declaration_reproduces_the_codex_0149_turn_environment_failure() {
    let root = TestRoot::new("legacy-cmd-turn-environment");
    let (executable, _generic_command, _windows_command) = fast_real_windows_hook_command(&root);
    let legacy_command = format!("\"{}\" hook codex || exit /b 0", executable.display());
    let payload = serde_json::to_vec(&hook_payload(
        "UserPromptSubmit",
        "legacy-shell",
        &root.path,
    ))
    .expect("Codex-shaped payload serializes");

    for shell in [
        CodexWindowsHookShell::Pwsh7,
        CodexWindowsHookShell::WindowsPowerShell,
    ] {
        let output =
            run_codex_windows_hook_with_shell(shell, &legacy_command, &payload, true, None);
        assert!(
            !output.status.success(),
            "legacy cmd declaration unexpectedly succeeded in {}",
            shell.label()
        );
    }
}

#[cfg(windows)]
#[test]
fn static_doctor_requires_a_bounded_explicit_hook_runtime_probe() {
    let root = TestRoot::new("doctor-runtime-probe");
    let integration = trusted_real_windows_integration(&root);
    let hooks_path = root.child("codex-home/hooks.json");
    let config_path = root.child("codex-home/config.toml");
    let manifest_path = root.child("integration-state/integration-v1.json");
    let before = [
        fs::read(&hooks_path).expect("hooks read"),
        fs::read(&config_path).expect("config read"),
        fs::read(&manifest_path).expect("manifest read"),
    ];

    let static_report = integration.doctor();
    assert_eq!(
        static_report.check_status("hooks.runtime-probe"),
        Some(DoctorStatus::Warning)
    );
    assert_eq!(static_report.overall(), DoctorStatus::Warning);

    let probed_report = integration.doctor_with_runtime_probe();
    assert_eq!(
        probed_report.check_status("hooks.runtime-probe"),
        Some(DoctorStatus::Pass)
    );
    assert_eq!(probed_report.overall(), DoctorStatus::Pass);
    assert_eq!(
        before,
        [
            fs::read(&hooks_path).expect("hooks reread"),
            fs::read(&config_path).expect("config reread"),
            fs::read(&manifest_path).expect("manifest reread"),
        ],
        "runtime probing must not mutate Codex configuration, trust, or the ownership manifest"
    );
}

#[cfg(windows)]
#[test]
fn real_windows_hook_shell_stages_are_independently_bounded() {
    let root = TestRoot::new("real-command-stage-diagnostics");
    let (executable, command) = real_windows_hook_command(&root);
    let payload = serde_json::to_vec(&hook_payload("UserPromptSubmit", "real-shell", &root.path))
        .expect("Codex-shaped payload serializes");
    let mut failures = Vec::new();
    capture_windows_hook_stage(&mut failures, "direct", || {
        assert_real_hook_direct(&executable, &payload);
    });
    for (stage, shell) in [
        ("Pwsh7", CodexWindowsHookShell::Pwsh7),
        (
            "WindowsPowerShell",
            CodexWindowsHookShell::WindowsPowerShell,
        ),
        ("Cmd", CodexWindowsHookShell::Cmd),
        ("COMSPEC", CodexWindowsHookShell::ComspecFallback),
    ] {
        capture_windows_hook_stage(&mut failures, stage, || {
            let output = run_codex_windows_hook_with_shell(shell, &command, &payload, true, None);
            assert!(
                output.status.success(),
                "{} real hook shell failed: {}",
                shell.label(),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(
                output.stdout.is_empty(),
                "{} wrote hook stdout",
                shell.label()
            );
            assert!(
                output.stderr.is_empty(),
                "{} wrote hook stderr",
                shell.label()
            );
        });
    }
    capture_windows_hook_stage(&mut failures, "Ingress", || {
        assert_real_hook_ingress(&root, &command);
    });
    assert!(
        failures.is_empty(),
        "bounded real Windows hook stages failed: {}",
        failures.join(", ")
    );
}

#[cfg(windows)]
#[test]
fn anchored_production_hook_is_bounded_below_the_one_second_declaration() {
    let root = TestRoot::new("anchored-production-hook-sla");
    let executable = PathBuf::from(env!("CARGO_BIN_EXE_tabbeacon"));
    let workspace = root.child("workspace");
    let local_app_data = root.child("isolated-local-app-data");
    init_repo(
        &workspace,
        "https://example.invalid/team/performance-anchor.git",
    );

    let mut start = hook_payload("SessionStart", "performance-session", &workspace);
    start["source"] = Value::String("startup".to_owned());
    let start = serde_json::to_vec(&start).expect("start payload serializes");
    let _ = assert_real_hook_with_local_state_within(
        &executable,
        &start,
        &local_app_data,
        WINDOWS_HOOK_STAGE_TIMEOUT,
        "SessionStart anchor",
    );

    let prompt = serde_json::to_vec(&hook_payload_for_turn(
        "UserPromptSubmit",
        "performance-session",
        "performance-turn",
        &workspace,
    ))
    .expect("prompt payload serializes");
    let _ = assert_real_hook_with_local_state_within(
        &executable,
        &prompt,
        &local_app_data,
        WINDOWS_PRODUCTION_HOOK_SLA_TIMEOUT,
        "anchored UserPromptSubmit",
    );

    let post_tool = serde_json::to_vec(&hook_payload_for_turn(
        "PostToolUse",
        "performance-session",
        "performance-turn",
        &workspace,
    ))
    .expect("post-tool payload serializes");
    let elapsed = assert_real_hook_with_local_state_within(
        &executable,
        &post_tool,
        &local_app_data,
        WINDOWS_PRODUCTION_HOOK_SLA_TIMEOUT,
        "anchored PostToolUse",
    );
    assert!(
        elapsed < WINDOWS_PRODUCTION_HOOK_SLA_TIMEOUT,
        "ordinary production Hook must remain below the one-second declaration, elapsed={elapsed:?}"
    );
}

#[cfg(windows)]
#[test]
fn anchored_default_windows_hook_declaration_finishes_within_the_one_second_sla() {
    // The system coordinator publishes a content-bound worker image. Debug
    // test binaries are deliberately much larger than the distributed release
    // CLI, so the enforceable production SLA belongs to the explicit release
    // CI stage below rather than a misleading debug-artifact threshold.
    if cfg!(debug_assertions) {
        eprintln!("release-only Codex Hook SLA gate");
        return;
    }
    let root = TestRoot::new("anchored-default-shell-hook-declaration-sla");
    let (_executable, _generic_command, command) = fast_real_windows_hook_command(&root);
    let workspace = root.child("workspace");
    let local_app_data = root.child("isolated-local-app-data");
    init_repo(
        &workspace,
        "https://example.invalid/team/performance-shell-anchor.git",
    );
    let terminal_session = "00000000-0000-0000-0000-000000000052";

    let mut start = hook_payload("SessionStart", "performance-shell-session", &workspace);
    start["source"] = Value::String("startup".to_owned());
    let start = serde_json::to_vec(&start).expect("start payload serializes");
    let start_output = run_codex_windows_hook_with_shell_and_terminal(
        CodexWindowsHookShell::ComspecFallback,
        &command,
        &start,
        false,
        Some(&local_app_data),
        Some(terminal_session),
        WINDOWS_HOOK_STAGE_TIMEOUT,
    );
    assert!(
        start_output.status.success(),
        "SessionStart declaration failed: {}",
        String::from_utf8_lossy(&start_output.stderr)
    );

    let post_tool = serde_json::to_vec(&hook_payload_for_turn(
        "PostToolUse",
        "performance-shell-session",
        "performance-shell-turn",
        &workspace,
    ))
    .expect("post-tool payload serializes");
    let output = run_codex_windows_hook_with_shell_and_terminal(
        CodexWindowsHookShell::ComspecFallback,
        &command,
        &post_tool,
        false,
        Some(&local_app_data),
        Some(terminal_session),
        WINDOWS_PRODUCTION_HOOK_SLA_TIMEOUT,
    );
    assert!(
        output.status.success(),
        "anchored default Windows declaration failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(windows)]
#[test]
fn malformed_hook_runtime_is_fail_open_in_every_admitted_shell() {
    let root = TestRoot::new("malformed-hook-runtime-fail-open");
    let (_executable, _generic_command, command) = fast_real_windows_hook_command(&root);
    for shell in [
        CodexWindowsHookShell::Pwsh7,
        CodexWindowsHookShell::WindowsPowerShell,
        CodexWindowsHookShell::Cmd,
        CodexWindowsHookShell::ComspecFallback,
    ] {
        let output = run_codex_windows_hook_with_shell(shell, &command, b"malformed", false, None);
        assert!(
            output.status.success(),
            "malformed hook runtime input must fail open in {}: {}",
            shell.label(),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            output.stdout.is_empty() && output.stderr.is_empty(),
            "malformed runtime handling must remain silent in {}",
            shell.label()
        );
    }
}

#[test]
fn internal_hook_cli_is_silent_fail_open_and_no_launcher_surface_exists() {
    let binary = env!("CARGO_BIN_EXE_tabbeacon");
    let mut child = Command::new(binary)
        .args(["hook", "codex"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("hook CLI starts");
    child
        .stdin
        .take()
        .expect("hook stdin is piped")
        .write_all(b"malformed")
        .expect("malformed hook input writes");
    let output = child.wait_with_output().expect("hook CLI exits");
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());

    let help = Command::new(binary)
        .arg("--help")
        .output()
        .expect("help command starts");
    let help = String::from_utf8(help.stdout).expect("help is UTF-8");
    assert!(help.contains("tabbeacon setup codex"));
    assert!(!help.lines().any(|line| line.trim() == "tabbeacon codex"));
}
