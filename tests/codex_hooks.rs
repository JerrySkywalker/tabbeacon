#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::{
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc, Barrier,
        atomic::{AtomicU64, Ordering},
    },
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tabbeacon::{
    core::{Attention, AuthoritySet, FieldUpdate, Health, Phase, StateAxis},
    providers::codex::{
        CodexHookError, CodexHookNormalizer, CodexHookRuntime, CodexIntegration,
        CodexIntegrationError, CodexNormalization, DoctorStatus, HookDispatchOutcome, SetupOutcome,
        UninstallOutcome,
    },
};
use toml_edit::{Array, DocumentMut, Item, Table, value};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

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
    let executable = root.child(if cfg!(windows) {
        "bin/tabbeacon.exe"
    } else {
        "bin/tabbeacon"
    });
    fs::create_dir_all(executable.parent().expect("binary parent"))
        .expect("binary parent is created");
    fs::write(&executable, b"test executable placeholder").expect("binary placeholder is written");
    let codex_probe = compile_codex_probe(root);
    CodexIntegration::new(root.child("codex-home"), root.child("state"), executable)
        .with_codex_program(codex_probe)
}

fn compile_codex_probe(root: &TestRoot) -> PathBuf {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("codex_version_probe.rs");
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

#[cfg(windows)]
fn run_codex_windows_hook(command_line: &str) -> std::process::Output {
    // Mirror Codex 0.147.0 command_runner::build_command: /C followed by one
    // raw, outer-quoted command line. Normal argument quoting is not equivalent.
    let shell = env::var_os("COMSPEC").unwrap_or_else(|| "cmd.exe".into());
    let mut command = Command::new(shell);
    command.arg("/C");
    command.raw_arg(format!(r#""{command_line}""#));
    command
        .output()
        .expect("Codex-compatible hook shell starts")
}

fn codex_event_key(event: &str) -> &'static str {
    match event {
        "SessionStart" => "session_start",
        "UserPromptSubmit" => "user_prompt_submit",
        "PreToolUse" => "pre_tool_use",
        "PermissionRequest" => "permission_request",
        "PostToolUse" => "post_tool_use",
        "Stop" => "stop",
        "SessionEnd" => "session_end",
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
    for event in [
        "SessionStart",
        "UserPromptSubmit",
        "PreToolUse",
        "PermissionRequest",
        "PostToolUse",
        "Stop",
        "SessionEnd",
    ] {
        let group = &hooks["hooks"][event][0];
        let key = format!("{}:{}:0:0", hooks_path.display(), codex_event_key(event));
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
    assert_eq!(
        CodexHookNormalizer
            .normalize(
                &serde_json::to_vec(&compact).expect("compact fixture serializes"),
                UNIX_EPOCH
            )
            .expect("compact is valid"),
        CodexNormalization::PreserveCurrentState
    );
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
    assert!(rendered.contains("\u{1b}]0;WM working\u{1b}\\"));
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
    assert!(rendered.contains("WM reset"));
    assert!(rendered.contains("\u{1b}]9;4;0;0\u{1b}\\"));
    assert!(rendered.contains("\u{1b}]104;264\u{1b}\\"));
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
    assert!(render(&ordinary, "one").contains("JPC working"));
    assert!(render(&linked, "two").contains("JPC working"));
    let newcomer = render(&colliding, "three");
    assert!(!newcomer.contains("]0;JPC working"));
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
fn runtime_failures_and_compact_remain_fail_open() {
    let root = TestRoot::new("fail-open");
    let runtime = CodexHookRuntime::new(root.child("state"), true);
    let invalid_repo = root.child("not-a-repository");
    fs::create_dir_all(&invalid_repo).expect("non-repository cwd exists");
    let prompt = hook_payload("UserPromptSubmit", "session-a", &invalid_repo);
    assert_eq!(
        runtime.dispatch_to(
            &serde_json::to_vec(&prompt).expect("payload serializes"),
            UNIX_EPOCH,
            &mut Vec::new()
        ),
        HookDispatchOutcome::DegradedRepositoryIdentity
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
    assert_eq!(report.overall(), DoctorStatus::Pass);
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
    config["hooks"]["state"][&trusted_keys[0]]["trusted_hash"] = value("sha256:modified");
    fs::write(&config_path, config.to_string()).expect("modified trust state writes");
    let report = integration.doctor();
    assert!(report.checks().iter().any(|check| {
        check.id() == "hooks.trust"
            && check.status() == DoctorStatus::Fail
            && check.summary().contains("modified/inactive")
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
    for event in [
        "SessionStart",
        "UserPromptSubmit",
        "PreToolUse",
        "PermissionRequest",
        "PostToolUse",
        "Stop",
        "SessionEnd",
    ] {
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
        Err(CodexIntegrationError::UnownedHookConflict)
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
    for event in [
        "SessionStart",
        "UserPromptSubmit",
        "PreToolUse",
        "PermissionRequest",
        "PostToolUse",
        "Stop",
        "SessionEnd",
    ] {
        assert_eq!(hooks["hooks"][event][0]["hooks"][0]["async"], false);
        assert_eq!(hooks["hooks"][event][0]["hooks"][0]["timeout"], 1);
        assert!(
            hooks["hooks"][event][0]["hooks"][0]["commandWindows"]
                .as_str()
                .expect("Windows command is a string")
                .ends_with(" || exit /b 0")
        );
    }
    #[cfg(windows)]
    {
        let command = hooks["hooks"]["UserPromptSubmit"][0]["hooks"][0]["commandWindows"]
            .as_str()
            .expect("Windows command is a string");
        let output = run_codex_windows_hook(command);
        assert!(
            output.status.success(),
            "missing managed binary must fail open"
        );
    }
}

#[cfg(windows)]
#[test]
fn explicit_blocking_like_hook_failure_is_neutralized_with_a_bounded_declaration() {
    let root = TestRoot::new("explicit-hook-failure");
    let executable = root.child("bin").join("tabbeacon-failure.cmd");
    let execution_marker = root.child("failure-probe-ran");
    fs::create_dir_all(executable.parent().expect("failure binary parent"))
        .expect("failure binary parent is created");
    fs::write(
        &executable,
        format!(
            "@echo off\r\necho ran> \"{}\"\r\nexit /b 2\r\n",
            execution_marker.display()
        ),
    )
    .expect("failure binary writes");
    let integration =
        CodexIntegration::new(root.child("codex-home"), root.child("state"), executable)
            .with_codex_program(compile_codex_probe(&root));
    integration.setup().expect("setup succeeds");
    let hooks: Value =
        serde_json::from_slice(&fs::read(root.child("codex-home/hooks.json")).expect("hooks read"))
            .expect("hooks parse");
    let handler = &hooks["hooks"]["PermissionRequest"][0]["hooks"][0];
    assert_eq!(handler["timeout"], 1);
    assert_eq!(handler["async"], false);
    let command = handler["commandWindows"]
        .as_str()
        .expect("Windows command is a string");
    let output = run_codex_windows_hook(command);
    assert!(
        output.status.success(),
        "exit-code-2 hook failure must be neutralized: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        execution_marker.is_file(),
        "failure probe must have executed; command={command:?}; stdout={}; stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
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
