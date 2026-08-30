#![cfg(windows)]

use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
static ORIGINAL_PATH: OnceLock<PathBuf> = OnceLock::new();

struct TestRoot {
    path: PathBuf,
}

impl TestRoot {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test clock is after epoch")
            .as_nanos();
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = env::temp_dir().join(format!(
            "tabbeacon-g40-{label}-{}-{nonce}-{counter}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("isolated test root creates");
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

fn inherited_path() -> &'static Path {
    ORIGINAL_PATH.get_or_init(|| {
        PathBuf::from(
            env::var_os("PATH").expect("test process has a PATH for cmd.exe and Cargo binaries"),
        )
    })
}

fn fake_codex_directory(root: &TestRoot, version: &str) -> PathBuf {
    let directory = root.child("fake-codex");
    fs::create_dir_all(&directory).expect("fake Codex directory creates");
    fs::write(
        directory.join("codex.cmd"),
        format!(
            "@echo off\r\nif \"%1\"==\"--version\" (echo codex-cli {version} & exit /b 0)\r\nif \"%1\"==\"features\" if \"%2\"==\"list\" (echo hooks stable true & exit /b 0)\r\nif \"%1\"==\"app-server\" if \"%2\"==\"generate-json-schema\" (mkdir \"%4\" 2>nul & echo {{\"hooks\":\"command\"}}>\"%4\\schema.json\" & exit /b 0)\r\nexit /b 2\r\n"
        ),
    )
    .expect("fake Codex version probe writes");
    directory
}

fn fake_agy_directory(root: &TestRoot) -> PathBuf {
    let directory = root.child("fake-agy");
    fs::create_dir_all(&directory).expect("fake Agy directory creates");
    fs::copy(env!("CARGO_BIN_EXE_tabbeacon"), directory.join("agy.exe"))
        .expect("fake literal Agy executable copies");
    directory
}

fn isolated_command(root: &TestRoot) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_tabbeacon"));
    command
        .env("CODEX_HOME", root.child("codex-home"))
        .env("LOCALAPPDATA", root.child("local-appdata"))
        .env("USERPROFILE", root.child("user-profile"))
        .env("XDG_STATE_HOME", root.child("xdg-state"))
        .env_remove("WT_SESSION")
        .env_remove("WT_PROFILE_ID");
    command
}

fn isolated_command_with_codex(root: &TestRoot, codex_directory: &Path) -> Command {
    let mut command = isolated_command(root);
    let path = env::join_paths([codex_directory, inherited_path()])
        .expect("isolated Codex version-probe PATH joins safely");
    command.env("PATH", path);
    command
}

fn git(cwd: &Path, args: &[&str]) {
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
}

fn init_repository(path: &Path) {
    fs::create_dir_all(path).expect("repository directory is created");
    git(path, &["init", "--quiet"]);
    fs::write(path.join("README.md"), "portable alias test repository\n")
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
}

fn isolated_command_with_agy(root: &TestRoot, agy_directory: &Path) -> Command {
    let mut command = isolated_command(root);
    let path = env::join_paths([agy_directory, inherited_path()])
        .expect("isolated Agy version-probe PATH joins safely");
    command.env("PATH", path);
    command
}

fn command_with_stdin(mut command: Command, input: &[u8]) -> std::process::Output {
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("qualification command starts");
    child
        .stdin
        .take()
        .expect("qualification stdin is available")
        .write_all(input)
        .expect("qualification input writes");
    child
        .wait_with_output()
        .expect("qualification command exits")
}

#[test]
fn help_version_and_powershell_completion_are_available_without_hidden_workers() {
    let root = TestRoot::new("help");

    let version = isolated_command(&root)
        .arg("--version")
        .output()
        .expect("version command starts");
    assert!(version.status.success());
    assert_eq!(
        String::from_utf8(version.stdout)
            .expect("version is UTF-8")
            .trim(),
        format!("tabbeacon {}", env!("CARGO_PKG_VERSION"))
    );

    let help = isolated_command(&root)
        .arg("--help")
        .output()
        .expect("help command starts");
    assert!(help.status.success());
    let help = String::from_utf8(help.stdout).expect("help is UTF-8");
    assert!(help.contains("tabbeacon setup codex"));
    assert!(!help.contains("__activity-worker-v1"));

    let completions = isolated_command(&root)
        .args(["completions", "powershell"])
        .output()
        .expect("completion command starts");
    assert!(completions.status.success());
    let completions = String::from_utf8(completions.stdout).expect("completion script is UTF-8");
    assert!(completions.contains("Register-ArgumentCompleter"));
    assert!(!completions.contains("PROFILE"));
}

#[test]
fn agy_preadmission_cli_is_content_minimal_and_cannot_enable_a_provider() {
    let root = TestRoot::new("agy-preadmission");

    let plan = isolated_command(&root)
        .args(["agy", "plan", "--json"])
        .output()
        .expect("Agy plan starts");
    assert!(plan.status.success());
    let plan: serde_json::Value = serde_json::from_slice(&plan.stdout).expect("plan is JSON");
    assert_eq!(plan["admission"], "unadmitted");
    assert_eq!(plan["provider_enablement"], "disabled");
    assert_eq!(plan["configuration_mutation"], "none");
    assert_eq!(plan["direct_command"]["executable"], "agy");
    assert_eq!(plan["direct_command"]["launch_boundary"], "direct_only");

    let version = isolated_command(&root)
        .args([
            "agy",
            "version",
            "--observed",
            "1.1.17",
            "--documented",
            "1.1.14",
            "--json",
        ])
        .output()
        .expect("Agy version diagnostic starts");
    assert!(version.status.success());
    let version: serde_json::Value =
        serde_json::from_slice(&version.stdout).expect("version diagnostic is JSON");
    assert_eq!(version["admission"], "unadmitted");
    assert_eq!(version["drift"], "documentation_older");
}

#[test]
fn agy_preadmission_payload_recorders_drop_content_and_fail_open() {
    let root = TestRoot::new("agy-preadmission-payload");

    let title_state = command_with_stdin(
        {
            let mut command = isolated_command(&root);
            command.args(["agy", "title-state", "--json"]);
            command
        },
        br#"{
          "conversation_id":"private-conversation",
          "workspace":{"current_dir":"C:/private/worktree","project_dir":"C:/private/project"},
          "agent_state":"working",
          "task_count":2,
          "transcript_path":"C:/private/transcript.jsonl",
          "email":"owner@example.test",
          "model":{"id":"private-model"}
        }"#,
    );
    assert!(title_state.status.success());
    let title_text = String::from_utf8(title_state.stdout).expect("title state is UTF-8");
    let title_value: serde_json::Value =
        serde_json::from_str(&title_text).expect("title state is JSON");
    assert_eq!(title_value["disposition"], "observed");
    assert_eq!(title_value["observation"]["admission"], "unadmitted");
    for forbidden in [
        "private-conversation",
        "C:/private",
        "transcript_path",
        "owner@example.test",
        "private-model",
    ] {
        assert!(
            !title_text.contains(forbidden),
            "title output leaked {forbidden}"
        );
    }

    let hook_state = command_with_stdin(
        {
            let mut command = isolated_command(&root);
            command.args(["agy", "hook-state", "post-tool-use", "--json"]);
            command
        },
        br#"{
          "conversationId":"private-hook-id",
          "workspacePaths":["C:/private/one"],
          "transcriptPath":"C:/private/transcript.jsonl",
          "toolCall":{"args":{"CommandLine":"private command"}}
        }"#,
    );
    assert!(hook_state.status.success());
    let hook_text = String::from_utf8(hook_state.stdout).expect("Hook state is UTF-8");
    let hook_value: serde_json::Value =
        serde_json::from_str(&hook_text).expect("Hook state is JSON");
    assert_eq!(hook_value["disposition"], "observed");
    assert!(hook_text.contains("content_fields_dropped"));
    for forbidden in ["private-hook-id", "C:/private", "private command"] {
        assert!(
            !hook_text.contains(forbidden),
            "Hook output leaked {forbidden}"
        );
    }

    let callback = command_with_stdin(
        {
            let mut command = isolated_command(&root);
            command.args(["agy", "__title-callback-v1"]);
            command
        },
        br#"{"agent_state":"malformed""#,
    );
    assert!(callback.status.success());
    assert_eq!(
        String::from_utf8(callback.stdout)
            .expect("callback output is UTF-8")
            .trim(),
        "Agy"
    );
    assert!(callback.stderr.is_empty());
}

#[test]
#[allow(clippy::too_many_lines)]
fn agy_qualification_workflow_persists_only_minimized_candidates_and_cleans_safely() {
    let root = TestRoot::new("agy-qualification-workflow");
    let qualification = root.child("tabbeacon-agy-qualification-workflow");

    let init = isolated_command(&root)
        .args(["agy", "qualification", "init", "--root"])
        .arg(&qualification)
        .arg("--json")
        .output()
        .expect("qualification init starts");
    assert!(init.status.success());
    let init: serde_json::Value = serde_json::from_slice(&init.stdout).expect("init JSON");
    assert_eq!(init["initialized"], true);
    assert_eq!(init["production_enabled"], false);

    let title = command_with_stdin(
        {
            let mut command = isolated_command(&root);
            command
                .args(["agy", "qualification", "record-title", "--root"])
                .arg(&qualification)
                .arg("--json");
            command
        },
        br#"{
          "conversation_id":"private-conversation",
          "agent_state":"working",
          "workspace":{"current_dir":"C:/private/root","project_dir":"C:/private/root"},
          "task_count":3,
          "tool_confirmation_pending":true,
          "prompt":"private prompt",
          "assistant":"private assistant",
          "transcript_path":"C:/private/transcript"
        }"#,
    );
    assert!(title.status.success());
    let title: serde_json::Value = serde_json::from_slice(&title.stdout).expect("title JSON");
    assert_eq!(title["disposition"], "observed");

    let hook = command_with_stdin(
        {
            let mut command = isolated_command(&root);
            command
                .args([
                    "agy",
                    "qualification",
                    "record-hook",
                    "post-tool-use",
                    "--root",
                ])
                .arg(&qualification)
                .arg("--json");
            command
        },
        br#"{
          "conversationId":"private-hook",
          "workspacePaths":["C:/private/root"],
          "toolCall":{"args":{"prompt":"private tool argument"}},
          "error":"private error"
        }"#,
    );
    assert!(hook.status.success());

    let unknown_hook = command_with_stdin(
        {
            let mut command = isolated_command(&root);
            command
                .args([
                    "agy",
                    "qualification",
                    "__hook-callback-v1",
                    "FuturePrivateEvent",
                    "--root",
                ])
                .arg(&qualification);
            command
        },
        br#"{"prompt":"unknown private prompt","tool_output":"unknown private output"}"#,
    );
    assert!(unknown_hook.status.success());
    assert!(unknown_hook.stdout.is_empty());
    assert!(unknown_hook.stderr.is_empty());

    for operation in ["inspect", "profile", "review", "status"] {
        let output = isolated_command(&root)
            .args(["agy", "qualification", operation, "--root"])
            .arg(&qualification)
            .arg("--json")
            .output()
            .expect("qualification operation starts");
        assert!(output.status.success(), "{operation} succeeds");
        let value: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("qualification JSON");
        assert_eq!(value["provider_enabled"], false, "{operation}");
    }

    let durable = fs::read_dir(&qualification)
        .expect("qualification workspace reads")
        .flat_map(|entry| fs::read(entry.expect("artifact entry").path()).expect("artifact reads"))
        .collect::<Vec<_>>();
    let durable = String::from_utf8_lossy(&durable);
    for forbidden in [
        "private-conversation",
        "private-hook",
        "C:/private",
        "private prompt",
        "private assistant",
        "private tool argument",
        "private error",
        "unknown private prompt",
        "unknown private output",
        "transcript_path",
        "toolCall",
    ] {
        assert!(!durable.contains(forbidden), "artifact leaked {forbidden}");
    }

    let clean = isolated_command(&root)
        .args(["agy", "qualification", "clean", "--confirm", "--root"])
        .arg(&qualification)
        .arg("--json")
        .output()
        .expect("qualification clean starts");
    assert!(clean.status.success());
    assert!(!qualification.exists());
}

#[test]
fn agy_direct_probe_invokes_literal_path_search_without_admitting_provider() {
    let root = TestRoot::new("agy-direct-probe");
    let qualification = root.child("tabbeacon-agy-qualification-probe");
    let agy = fake_agy_directory(&root);
    let init = isolated_command(&root)
        .args(["agy", "qualification", "init", "--root"])
        .arg(&qualification)
        .output()
        .expect("qualification init starts");
    assert!(init.status.success());

    let probe = isolated_command_with_agy(&root, &agy)
        .args(["agy", "qualification", "probe", "--root"])
        .arg(&qualification)
        .arg("--json")
        .output()
        .expect("direct probe starts");
    assert!(probe.status.success());
    let text = String::from_utf8(probe.stdout).expect("probe is UTF-8");
    let probe: serde_json::Value = serde_json::from_str(&text).expect("probe JSON");
    assert_eq!(probe["installed"], true);
    assert_eq!(probe["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(probe["executable_resolution_class"], "literal_path_search");
    assert_eq!(probe["qualification_admission_state"], "unadmitted");
    assert_eq!(probe["provider_enabled"], false);
    assert!(!text.contains(&agy.to_string_lossy().to_string()));
}

#[test]
fn agy_adversarial_payloads_cannot_reach_cli_output_or_title_protocol() {
    let root = TestRoot::new("agy-adversarial");
    let duplicate_state = br#"{
      "agent_state":"future-private-state",
      "agent_state":"idle",
      "cwd":"C:/private/\u001b[31mworkspace",
      "conversation_id":"private-id-\u2603",
      "assistant_content":"private prompt and assistant content"
    }"#;
    let state = command_with_stdin(
        {
            let mut command = isolated_command(&root);
            command.args(["agy", "title-state", "--json"]);
            command
        },
        duplicate_state,
    );
    assert!(state.status.success());
    let state_text = String::from_utf8(state.stdout).expect("state output is UTF-8");
    let state_value: serde_json::Value = serde_json::from_str(&state_text).expect("state JSON");
    assert_eq!(state_value["disposition"], "malformed");

    let duplicate_hook = br#"{
      "workspacePaths":["C:/private/first"],
      "workspacePaths":["C:/private/second"],
      "toolCall":{"args":{"prompt":"private tool arguments"}},
      "error":"private failure"
    }"#;
    let hook = command_with_stdin(
        {
            let mut command = isolated_command(&root);
            command.args(["agy", "hook-state", "post-tool-use", "--json"]);
            command
        },
        duplicate_hook,
    );
    assert!(hook.status.success());
    let hook_text = String::from_utf8(hook.stdout).expect("Hook output is UTF-8");
    let hook_value: serde_json::Value = serde_json::from_str(&hook_text).expect("Hook JSON");
    assert_eq!(hook_value["disposition"], "malformed");

    let oversized = vec![b'x'; 64 * 1024 + 1];
    let oversized_state = command_with_stdin(
        {
            let mut command = isolated_command(&root);
            command.args(["agy", "title-state", "--plain"]);
            command
        },
        &oversized,
    );
    assert!(oversized_state.status.success());
    assert_eq!(
        String::from_utf8(oversized_state.stdout)
            .expect("plain output is UTF-8")
            .lines()
            .nth(1),
        Some("AGY_TITLE_STATE=oversized")
    );

    let callback = command_with_stdin(
        {
            let mut command = isolated_command(&root);
            command.args(["agy", "__title-callback-v1"]);
            command
        },
        duplicate_state,
    );
    assert!(callback.status.success());
    assert_eq!(callback.stdout, b"Agy\n");
    assert!(callback.stderr.is_empty());

    for forbidden in [
        "future-private-state",
        "C:/private",
        "private-id",
        "private prompt",
        "private tool arguments",
        "private failure",
    ] {
        assert!(!state_text.contains(forbidden), "state leaked {forbidden}");
        assert!(!hook_text.contains(forbidden), "Hook leaked {forbidden}");
    }
}

#[test]
fn ordinary_runtime_reports_truthful_agy_probe_and_setup_state() {
    let root = TestRoot::new("agy-isolation");

    let setup_help = isolated_command(&root)
        .args(["setup", "--help"])
        .output()
        .expect("setup help starts");
    assert!(setup_help.status.success());
    let setup_help = String::from_utf8(setup_help.stdout).expect("setup help is UTF-8");
    assert!(setup_help.contains("supported provider integration"));
    assert!(setup_help.contains("codex"));
    assert!(setup_help.contains("Agy"));

    let sessions = isolated_command(&root)
        .args(["sessions", "--json"])
        .output()
        .expect("sessions starts");
    assert!(sessions.status.success());
    let sessions = String::from_utf8(sessions.stdout).expect("sessions output is UTF-8");
    assert!(!sessions.to_ascii_lowercase().contains("agy"));

    let status = isolated_command(&root)
        .args(["status", "--json"])
        .output()
        .expect("status starts");
    assert!(status.status.success());
    let status: serde_json::Value = serde_json::from_slice(&status.stdout).expect("status is JSON");
    assert!(status["codex"].is_object());
    assert!(status.get("agy").is_none());
    assert!(status["providers"]["providers"].is_array());
    let agy = status["providers"]["providers"]
        .as_array()
        .expect("provider rows")
        .iter()
        .find(|provider| provider["id"] == "agy")
        .expect("Agy registry row");
    let configuration_state = agy["configuration_state"]
        .as_str()
        .expect("Agy configuration state");
    assert!(matches!(
        configuration_state,
        "known_unadmitted" | "supported_not_configured" | "unsupported_version"
    ));
    assert_eq!(agy["readiness"]["production_enabled"], false);

    let unsupported_setup = isolated_command(&root)
        .args(["setup", "agy"])
        .output()
        .expect("unsupported setup invocation starts");
    if configuration_state == "supported_not_configured" {
        assert!(unsupported_setup.status.success());
        assert!(
            root.child("user-profile/.gemini/antigravity-cli/settings.json")
                .exists()
        );
    } else {
        assert!(
            !unsupported_setup.status.success(),
            "Agy setup requires an exact admitted local probe"
        );
    }
}

#[test]
fn output_modes_preserve_machine_json_and_admit_legacy_plain_output() {
    let root = TestRoot::new("output-modes");

    let json = isolated_command(&root)
        .args(["status", "--json"])
        .output()
        .expect("JSON status starts");
    assert!(json.status.success());
    assert!(json.stderr.is_empty(), "JSON mode has no human stderr");
    let value: serde_json::Value =
        serde_json::from_slice(&json.stdout).expect("JSON status remains a document");
    assert_eq!(value["schema_version"], 1);
    let capability_state = value["codex"]["profile_state"]
        .as_str()
        .expect("Codex capability state is a string");
    assert!(
        matches!(
            capability_state,
            "full" | "degraded" | "incompatible" | "unproven"
        ),
        "Codex compatibility is a capability state, not an exact-version profile: {capability_state}"
    );

    let plain = isolated_command(&root)
        .args(["status", "--plain"])
        .output()
        .expect("plain status starts");
    assert!(plain.status.success());
    let plain = String::from_utf8(plain.stdout).expect("plain status is UTF-8");
    assert!(plain.contains("STATUS_SCHEMA_VERSION=1"));
    assert!(plain.contains("DOCTOR="));

    let doctor_plain = isolated_command(&root)
        .args(["doctor", "--plain"])
        .output()
        .expect("plain doctor starts");
    assert!(
        !doctor_plain.status.success(),
        "uninstalled doctor remains a failure"
    );
    let doctor_plain = String::from_utf8(doctor_plain.stdout).expect("plain doctor is UTF-8");
    assert!(doctor_plain.contains("CHECK="));
    assert!(doctor_plain.contains("DOCTOR=FAIL"));
}

#[test]
fn hooks_cli_is_machine_stable_localized_for_humans_and_never_reveals_commands() {
    let root = TestRoot::new("hooks-cli");

    let json_en = isolated_command(&root)
        .args(["hooks", "--json", "--lang", "en-US"])
        .output()
        .expect("English JSON Hook inventory starts");
    let json_zh = isolated_command(&root)
        .args(["hooks", "--json", "--lang", "zh-CN"])
        .output()
        .expect("Chinese JSON Hook inventory starts");
    assert!(json_en.status.success());
    assert!(json_zh.status.success());
    let json: serde_json::Value =
        serde_json::from_slice(&json_en.stdout).expect("Hook inventory JSON parses");
    assert_eq!(json["schema_version"], 1);
    assert_eq!(json["read_only"], true);
    assert_eq!(json["availability"], "unavailable");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&json_en.stdout)
            .expect("English Hook JSON parses"),
        serde_json::from_slice::<serde_json::Value>(&json_zh.stdout)
            .expect("Chinese Hook JSON parses"),
        "machine Hook inventory is locale independent"
    );

    let plain = isolated_command(&root)
        .args(["hooks", "--plain"])
        .output()
        .expect("plain Hook inventory starts");
    assert!(plain.status.success());
    let plain = String::from_utf8(plain.stdout).expect("plain Hook inventory is UTF-8");
    assert!(plain.contains("HOOKS_SCHEMA_VERSION=1"));
    assert!(plain.contains("HOOKS_READ_ONLY=true"));
    assert!(plain.contains("ARBITRARY_COMMANDS_REDACTED=true"));
    assert!(plain.contains("AUTO_HOOK_TRUST=false"));
    assert!(!plain.contains("powershell.exe"));
    assert!(!plain.contains("commandWindows"));

    let human = isolated_command(&root)
        .args(["hooks", "--lang", "zh-CN"])
        .output()
        .expect("Chinese Human Hook inventory starts");
    assert!(human.status.success());
    let human = String::from_utf8(human.stdout).expect("Human Hook inventory is UTF-8");
    assert!(human.contains("钩子"));
    assert!(human.contains("未更改任何配置"));
}

#[test]
fn machine_transports_are_locale_independent() {
    let root = TestRoot::new("localized-human-output");

    let status_json_en = isolated_command(&root)
        .args(["status", "--json", "--lang", "en-US"])
        .output()
        .expect("English JSON status starts");
    let status_json_zh = isolated_command(&root)
        .args(["status", "--json", "--lang", "zh-CN"])
        .output()
        .expect("Chinese JSON status starts");
    assert!(status_json_en.status.success());
    assert!(status_json_zh.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&status_json_en.stdout)
            .expect("English JSON parses"),
        serde_json::from_slice::<serde_json::Value>(&status_json_zh.stdout)
            .expect("Chinese JSON parses"),
        "machine JSON must not depend on Human locale"
    );

    let status_plain_en = isolated_command(&root)
        .args(["status", "--plain", "--lang", "en-US"])
        .output()
        .expect("English plain status starts");
    let status_plain_zh = isolated_command(&root)
        .args(["status", "--plain", "--lang", "zh-CN"])
        .output()
        .expect("Chinese plain status starts");
    assert_eq!(
        status_plain_en.stdout, status_plain_zh.stdout,
        "legacy key/value output must not depend on Human locale"
    );

    let doctor_plain_en = isolated_command(&root)
        .args(["doctor", "--plain", "--lang", "en-US"])
        .output()
        .expect("English plain doctor starts");
    let doctor_plain_zh = isolated_command(&root)
        .args(["doctor", "--plain", "--lang", "zh-CN"])
        .output()
        .expect("Chinese plain doctor starts");
    assert_eq!(
        doctor_plain_en.stdout, doctor_plain_zh.stdout,
        "plain doctor receipts must not depend on Human locale"
    );

    let doctor_json_en = isolated_command(&root)
        .args(["doctor", "--json", "--lang", "en-US"])
        .output()
        .expect("English JSON doctor starts");
    let doctor_json_zh = isolated_command(&root)
        .args(["doctor", "--json", "--lang", "zh-CN"])
        .output()
        .expect("Chinese JSON doctor starts");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&doctor_json_en.stdout)
            .expect("English doctor JSON parses"),
        serde_json::from_slice::<serde_json::Value>(&doctor_json_zh.stdout)
            .expect("Chinese doctor JSON parses"),
        "doctor JSON must not depend on Human locale"
    );

    let sessions_json_en = isolated_command(&root)
        .args(["sessions", "--json", "--lang", "en-US"])
        .output()
        .expect("English JSON sessions starts");
    let sessions_json_zh = isolated_command(&root)
        .args(["sessions", "--json", "--lang", "zh-CN"])
        .output()
        .expect("Chinese JSON sessions starts");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&sessions_json_en.stdout)
            .expect("English sessions JSON parses"),
        serde_json::from_slice::<serde_json::Value>(&sessions_json_zh.stdout)
            .expect("Chinese sessions JSON parses"),
        "sessions JSON must not depend on Human locale"
    );

    let sessions_plain_en = isolated_command(&root)
        .args(["sessions", "--plain", "--lang", "en-US"])
        .output()
        .expect("English plain sessions starts");
    let sessions_plain_zh = isolated_command(&root)
        .args(["sessions", "--plain", "--lang", "zh-CN"])
        .output()
        .expect("Chinese plain sessions starts");
    assert_eq!(
        sessions_plain_en.stdout, sessions_plain_zh.stdout,
        "sessions plain receipts must not depend on Human locale"
    );

    let config_plain_en = isolated_command(&root)
        .args(["config", "show", "--plain", "--lang", "en-US"])
        .output()
        .expect("English plain config starts");
    let config_plain_zh = isolated_command(&root)
        .args(["config", "show", "--plain", "--lang", "zh-CN"])
        .output()
        .expect("Chinese plain config starts");
    assert_eq!(
        config_plain_en.stdout, config_plain_zh.stdout,
        "config plain receipts must not depend on Human locale"
    );
}

#[test]
fn alias_read_only_surfaces_are_locale_safe_private_and_non_mutating() {
    let root = TestRoot::new("alias-read-only");
    let registry_root = root.child("local-appdata/TabBeacon/repository-identity");
    let preference_root = root.child("local-appdata/TabBeacon/workspace-preferences");

    let json_en = isolated_command(&root)
        .args(["alias", "show", "--json", "--lang", "en-US"])
        .output()
        .expect("English alias JSON starts");
    let json_zh = isolated_command(&root)
        .args(["alias", "show", "--json", "--lang", "zh-CN"])
        .output()
        .expect("Chinese alias JSON starts");
    assert!(json_en.status.success());
    assert!(json_zh.status.success());
    assert!(json_en.stderr.is_empty());
    assert!(json_zh.stderr.is_empty());
    let json_en = serde_json::from_slice::<serde_json::Value>(&json_en.stdout)
        .expect("English alias JSON parses");
    assert_eq!(
        json_en,
        serde_json::from_slice::<serde_json::Value>(&json_zh.stdout)
            .expect("Chinese alias JSON parses"),
        "machine alias JSON must not depend on Human locale"
    );
    assert_eq!(json_en["schema"], "tabbeacon-alias-v2");
    let selected = &json_en["selected_candidate"];
    assert_eq!(
        selected["score_components"]["total"], selected["score"],
        "selected total must be the engine total"
    );
    let json_text = serde_json::to_string(&json_en).expect("alias JSON reserializes");
    assert!(!json_text.contains("remote:"));
    assert!(!json_text.contains("dir-v1:"));
    assert!(!json_text.contains("repository-identity"));
    assert!(
        !registry_root.exists(),
        "show must not create a registry root or lock"
    );
    assert!(
        !preference_root.exists(),
        "show must not create a preference root or lock"
    );

    let chinese = isolated_command(&root)
        .args(["alias", "explain", "--lang", "zh-CN"])
        .output()
        .expect("Chinese alias explain starts");
    assert!(chinese.status.success());
    let chinese = String::from_utf8(chinese.stdout).expect("Chinese alias Human output is UTF-8");
    assert!(chinese.contains("自适应别名说明"));
    assert!(!chinese.contains("ALIAS_SCHEMA_VERSION="));
    assert!(!chinese.contains("remote:"));
    assert!(!chinese.contains("repository-identity"));
    assert!(!registry_root.exists(), "explain remains read only");
    assert!(!preference_root.exists(), "explain remains read only");

    let title_json_en = isolated_command(&root)
        .args(["explain", "title", "--json", "--lang", "en-US"])
        .output()
        .expect("English title explanation starts");
    let title_json_zh = isolated_command(&root)
        .args(["explain", "title", "--json", "--lang", "zh-CN"])
        .output()
        .expect("Chinese title explanation starts");
    assert!(title_json_en.status.success());
    assert!(title_json_zh.status.success());
    let title_json_en = serde_json::from_slice::<serde_json::Value>(&title_json_en.stdout)
        .expect("English title explanation JSON parses");
    assert_eq!(
        title_json_en,
        serde_json::from_slice::<serde_json::Value>(&title_json_zh.stdout)
            .expect("Chinese title explanation JSON parses"),
        "machine title explanation must not depend on Human locale"
    );
    assert_eq!(title_json_en["schema"], "tabbeacon-title-explanation-v1");
    assert_eq!(
        title_json_en["provider"], "not_session_correlated",
        "multi-provider title explanation must not invent a provider without workspace evidence"
    );
    assert_eq!(
        title_json_en["workspace"]["root_binding_status"], "not_session_correlated",
        "read-only CLI must not claim a native-session correlation"
    );
    let title_text = serde_json::to_string(&title_json_en).expect("title explanation reserializes");
    assert!(!title_text.contains(root.path.to_string_lossy().as_ref()));
    assert!(!title_text.contains("repository-identity"));

    let title_human = isolated_command(&root)
        .args(["explain", "title", "--lang", "zh-CN"])
        .output()
        .expect("Chinese title explanation starts");
    assert!(title_human.status.success());
    let title_human = String::from_utf8(title_human.stdout).expect("title explanation is UTF-8");
    assert!(title_human.contains("为何使用此标题"));
    assert!(!title_human.contains("TITLE_EXPLANATION_SCHEMA_VERSION="));
    assert!(!title_human.contains("repository-identity"));
    assert!(
        !registry_root.exists(),
        "title explanation remains read only"
    );
    assert!(
        !preference_root.exists(),
        "title explanation remains read only"
    );
}

#[test]
fn alias_set_reset_and_collision_remain_device_local_and_generic() {
    let root = TestRoot::new("alias-mutation");
    let first = root.child("first-workspace");
    let second = root.child("second-workspace");
    fs::create_dir_all(&first).expect("first workspace creates");
    fs::create_dir_all(&second).expect("second workspace creates");

    let first_set = isolated_command(&root)
        .current_dir(&first)
        .args(["alias", "set", "CUSTOM", "--plain"])
        .output()
        .expect("first custom alias starts");
    assert!(first_set.status.success());
    let first_set = String::from_utf8(first_set.stdout).expect("first receipt is UTF-8");
    assert!(first_set.contains("ALIAS_OPERATION=SET"));
    assert!(first_set.contains("CUSTOM_ALIAS=CUSTOM"));

    let collision = isolated_command(&root)
        .current_dir(&second)
        .args(["alias", "set", "CUSTOM", "--plain"])
        .output()
        .expect("colliding custom alias starts");
    assert_eq!(collision.status.code(), Some(2));
    let collision = String::from_utf8(collision.stderr).expect("collision is UTF-8");
    assert!(collision.contains("ALIAS=FAIL"));
    assert!(collision.contains("REASON=alias_conflict"));
    assert!(!collision.contains(first.to_string_lossy().as_ref()));
    assert!(!collision.contains(second.to_string_lossy().as_ref()));

    let reset = isolated_command(&root)
        .current_dir(&first)
        .args(["alias", "reset", "--plain"])
        .output()
        .expect("alias reset starts");
    assert!(reset.status.success());
    let reset = String::from_utf8(reset.stdout).expect("reset receipt is UTF-8");
    assert!(reset.contains("ALIAS_OPERATION=RESET"));
    assert!(reset.contains("CUSTOM_ALIAS=NONE"));
    for workspace in [&first, &second] {
        assert!(!workspace.join(".tabbeacon").exists());
        assert!(!workspace.join(".tabbeacon.toml").exists());
        assert!(!workspace.join("tabbeacon.toml").exists());
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn export_import_is_preview_first_portable_and_non_tty_apply_is_explicit() {
    let source = TestRoot::new("export-source");
    let target = TestRoot::new("import-target");
    let repository = source.child("repository");
    init_repository(&repository);
    let export_path = source.child("portable-settings.json");

    let config = isolated_command(&source)
        .current_dir(&repository)
        .args(["config", "set", "spinner", "pulse", "--plain"])
        .output()
        .expect("source presentation setting starts");
    assert!(config.status.success());
    let interface = isolated_command(&source)
        .current_dir(&repository)
        .args(["interface", "set", "language", "zh-CN", "--plain"])
        .output()
        .expect("source Interface setting starts");
    assert!(interface.status.success());
    let alias = isolated_command(&source)
        .current_dir(&repository)
        .args(["alias", "set", "TB", "--plain"])
        .output()
        .expect("source alias setting starts");
    assert!(alias.status.success());

    let exported = isolated_command(&source)
        .args([
            "export",
            "--output",
            export_path.to_str().expect("test path is UTF-8"),
            "--plain",
        ])
        .output()
        .expect("export starts");
    assert!(exported.status.success());
    let exported = String::from_utf8(exported.stdout).expect("export receipt is UTF-8");
    assert!(exported.contains("EXPORT=PASS"));
    let export_document = fs::read_to_string(&export_path).expect("export document reads");
    assert!(export_document.contains("tabbeacon-export-v1"));
    assert!(!export_document.contains("remote:"));
    assert!(!export_document.contains("dir-v1:"));
    assert!(!export_document.contains("session_id"));
    assert!(!export_document.contains("LOCALAPPDATA"));

    let no_overwrite = isolated_command(&source)
        .args([
            "export",
            "--output",
            export_path.to_str().expect("test path is UTF-8"),
            "--plain",
        ])
        .output()
        .expect("second export starts");
    assert!(!no_overwrite.status.success());
    assert!(
        String::from_utf8(no_overwrite.stderr)
            .expect("overwrite failure is UTF-8")
            .contains("EXPORT=FAIL")
    );
    assert_eq!(
        fs::read_to_string(&export_path).expect("original export remains readable"),
        export_document,
        "a refused overwrite must leave the complete original document intact"
    );
    let forced_overwrite = isolated_command(&source)
        .args([
            "export",
            "--output",
            export_path.to_str().expect("test path is UTF-8"),
            "--force",
            "--plain",
        ])
        .output()
        .expect("forced export starts");
    assert!(forced_overwrite.status.success());
    assert_eq!(
        fs::read_to_string(&export_path).expect("forced export remains readable"),
        export_document,
        "forced replacement must install one complete canonical document"
    );
    let human_export = isolated_command(&source)
        .args([
            "export",
            "--output",
            export_path.to_str().expect("test path is UTF-8"),
            "--force",
            "--lang",
            "zh-CN",
        ])
        .output()
        .expect("Chinese Human export starts");
    assert!(human_export.status.success());
    let human_export = String::from_utf8(human_export.stdout).expect("Human export is UTF-8");
    assert!(human_export.contains("TabBeacon 导出"));
    assert!(!human_export.contains("EXPORT="));

    let target_alias = isolated_command(&target)
        .current_dir(&repository)
        .args(["alias", "set", "OTHER", "--plain"])
        .output()
        .expect("target identity bootstrap starts");
    assert!(target_alias.status.success());
    let preview = isolated_command(&target)
        .args([
            "import",
            export_path.to_str().expect("test path is UTF-8"),
            "--plain",
        ])
        .output()
        .expect("non-TTY preview starts");
    assert!(preview.status.success());
    let preview = String::from_utf8(preview.stdout).expect("preview receipt is UTF-8");
    assert!(preview.contains("IMPORT=PREVIEW"));
    assert!(preview.contains("NON_TTY_MUTATION_REQUIRES_APPLY=true"));
    assert!(
        !target.child("local-appdata/TabBeacon/config.toml").exists(),
        "preview did not write Presentation state"
    );
    let human_preview = isolated_command(&target)
        .args([
            "import",
            export_path.to_str().expect("test path is UTF-8"),
            "--lang",
            "zh-CN",
        ])
        .output()
        .expect("Chinese Human preview starts");
    assert!(human_preview.status.success());
    let human_preview = String::from_utf8(human_preview.stdout).expect("Human preview is UTF-8");
    assert!(human_preview.contains("TabBeacon 导入"));
    assert!(human_preview.contains("导入预览"));
    assert!(!human_preview.contains("IMPORT="));
    let oversize_path = target.child("oversize.json");
    fs::write(&oversize_path, vec![b' '; 1024 * 1024 + 1]).expect("oversize fixture writes");
    let oversize = isolated_command(&target)
        .args([
            "import",
            oversize_path.to_str().expect("test path is UTF-8"),
            "--plain",
        ])
        .output()
        .expect("oversize import starts");
    assert!(!oversize.status.success());
    assert!(
        String::from_utf8(oversize.stderr)
            .expect("oversize failure is UTF-8")
            .contains("IMPORT=FAIL")
    );

    let applied = isolated_command(&target)
        .args([
            "import",
            export_path.to_str().expect("test path is UTF-8"),
            "--apply",
            "--plain",
        ])
        .output()
        .expect("explicit import starts");
    assert!(applied.status.success());
    let applied = String::from_utf8(applied.stdout).expect("apply receipt is UTF-8");
    assert!(applied.contains("IMPORT=applied"));

    let settings = isolated_command(&target)
        .args(["config", "show", "--plain"])
        .output()
        .expect("target settings read");
    assert!(settings.status.success());
    assert!(
        String::from_utf8(settings.stdout)
            .expect("settings receipt is UTF-8")
            .contains("SPINNER_PRESET=pulse")
    );
    let interface = isolated_command(&target)
        .args(["interface", "show", "--plain"])
        .output()
        .expect("target Interface read");
    assert!(interface.status.success());
    assert!(
        String::from_utf8(interface.stdout)
            .expect("Interface receipt is UTF-8")
            .contains("INTERFACE_LANGUAGE=zh-CN")
    );
    let alias = isolated_command(&target)
        .current_dir(&repository)
        .args(["alias", "show", "--plain"])
        .output()
        .expect("target alias read");
    assert!(alias.status.success());
    assert!(
        String::from_utf8(alias.stdout)
            .expect("alias receipt is UTF-8")
            .contains("CUSTOM_ALIAS=TB")
    );
}

#[test]
fn human_locale_and_interface_state_stay_user_local() {
    let root = TestRoot::new("localized-human-interface");

    let interface_path = root
        .child("local-appdata")
        .join("TabBeacon")
        .join("interface.toml");
    assert!(
        !interface_path.exists(),
        "read-only Human locale resolution must not create Interface state"
    );

    let auto_human = isolated_command(&root)
        .args(["status", "--lang", "zh-CN"])
        .output()
        .expect("redirected automatic-color Human status starts");
    assert!(auto_human.status.success());
    let auto_human = String::from_utf8(auto_human.stdout).expect("automatic Human output is UTF-8");
    assert!(auto_human.contains("TabBeacon 状态"));
    assert!(
        !auto_human.contains('\u{1b}'),
        "redirected color=auto must not emit ANSI"
    );
    assert!(
        !interface_path.exists(),
        "passive Human rendering must still leave Interface state absent"
    );

    let stored_language = isolated_command(&root)
        .args(["interface", "set", "language", "zh-CN", "--plain"])
        .output()
        .expect("Interface language set starts");
    assert!(stored_language.status.success());
    assert!(
        interface_path.exists(),
        "explicit preference write creates state"
    );

    let stored_color = isolated_command(&root)
        .args(["interface", "set", "color", "never", "--plain"])
        .output()
        .expect("Interface color set starts");
    assert!(stored_color.status.success());

    let human = isolated_command(&root)
        .arg("status")
        .output()
        .expect("stored Chinese Human status starts");
    assert!(human.status.success());
    let human = String::from_utf8(human.stdout).expect("Human output is UTF-8");
    assert!(human.contains("TabBeacon 状态"));
    assert!(
        !human.contains('\u{1b}'),
        "color=never must keep redirected Human output monochrome"
    );
}

#[test]
fn sessions_human_output_localizes_without_exposing_machine_receipts() {
    let root = TestRoot::new("localized-sessions-output");
    let chinese = isolated_command(&root)
        .args(["sessions", "--lang", "zh-CN"])
        .output()
        .expect("Chinese sessions starts");
    assert!(chinese.status.success());
    let chinese = String::from_utf8(chinese.stdout).expect("Chinese sessions output is UTF-8");
    assert!(chinese.contains("会话"));
    assert!(chinese.contains("仅基于租约进行观察"));
    assert!(!chinese.contains("SESSIONS_SCHEMA_VERSION="));
}

#[test]
fn doctor_human_output_honors_explicit_chinese_locale() {
    let root = TestRoot::new("localized-doctor-output");
    let chinese = isolated_command(&root)
        .args(["doctor", "--lang", "zh-CN"])
        .output()
        .expect("Chinese doctor starts");
    let chinese = String::from_utf8(chinese.stdout).expect("Chinese doctor output is UTF-8");
    assert!(chinese.contains("TabBeacon 诊断"));
    assert!(!chinese.contains("CHECK="));
}

#[test]
fn config_defaults_to_human_monochrome_output_and_plain_retains_receipts() {
    let root = TestRoot::new("human-config-output");

    let human = isolated_command(&root)
        .args(["config", "show", "--lang", "en-US"])
        .output()
        .expect("human config show starts");
    assert!(human.status.success());
    let human = String::from_utf8(human.stdout).expect("human config output is UTF-8");
    assert!(human.contains("Presentation settings"));
    assert!(!human.contains("CONFIG_PATH="));
    assert!(!human.contains("TITLE_MODE="));
    assert!(!human.contains("TITLE_SPINNER_FEASIBILITY="));
    assert!(
        !human.contains('\u{1b}'),
        "redirected human output must not contain ANSI color"
    );

    let plain = isolated_command(&root)
        .args(["config", "show", "--plain"])
        .output()
        .expect("plain config show starts");
    assert!(plain.status.success());
    let plain = String::from_utf8(plain.stdout).expect("plain config output is UTF-8");
    assert!(plain.contains("CONFIG_PATH="));
    assert!(plain.contains("TITLE_MODE="));
    assert!(plain.contains("TITLE_SPINNER_FEASIBILITY=PRODUCTION"));
    assert!(!plain.contains('\u{1b}'));
}

#[test]
fn config_human_output_localizes_without_changing_plain_receipts() {
    let root = TestRoot::new("localized-config-output");
    let chinese = isolated_command(&root)
        .args(["config", "show", "--lang", "zh-CN"])
        .output()
        .expect("Chinese config show starts");
    assert!(chinese.status.success());
    let chinese = String::from_utf8(chinese.stdout).expect("Chinese config output is UTF-8");
    assert!(chinese.contains("外观呈现设置"));
    assert!(chinese.contains("标签颜色"));
    assert!(chinese.contains("低调深色"));
    assert!(!chinese.contains("CONFIG_PATH="));
}

#[test]
fn human_common_errors_keep_machine_receipts_out_and_localize_stable_wording() {
    let root = TestRoot::new("localized-human-errors");

    let config = isolated_command(&root)
        .args(["config", "set", "theme", "unsupported", "--lang", "zh-CN"])
        .output()
        .expect("Chinese config validation starts");
    assert_eq!(config.status.code(), Some(2));
    let config_stderr = String::from_utf8(config.stderr).expect("config error is UTF-8");
    assert!(config_stderr.contains("无法更新配置"));
    assert!(config_stderr.contains("请运行 tabbeacon config show"));
    assert!(!config_stderr.contains("CONFIG=FAIL"));

    let interface = isolated_command(&root)
        .args([
            "interface",
            "set",
            "language",
            "unsupported",
            "--lang",
            "zh-CN",
        ])
        .output()
        .expect("Chinese Interface validation starts");
    assert_eq!(interface.status.code(), Some(2));
    let interface_stderr = String::from_utf8(interface.stderr).expect("Interface error is UTF-8");
    assert!(interface_stderr.contains("不支持的界面偏好值"));
    assert!(!interface_stderr.contains("INTERFACE=FAIL"));
}

#[test]
fn uninstall_defaults_to_human_output_and_preserves_plain_receipts() {
    let root = TestRoot::new("human-uninstall-output");

    let human = isolated_command(&root)
        .args(["uninstall", "codex"])
        .output()
        .expect("human uninstall starts");
    assert!(human.status.success());
    let human = String::from_utf8(human.stdout).expect("human uninstall output is UTF-8");
    assert!(human.contains("No owned Codex integration is installed."));
    assert!(!human.contains("UNINSTALL_SAFETY="));
    assert!(!human.contains("OWNER_ACTION="));

    let plain = isolated_command(&root)
        .args(["uninstall", "codex", "--plain"])
        .output()
        .expect("plain uninstall starts");
    assert!(plain.status.success());
    let plain = String::from_utf8(plain.stdout).expect("plain uninstall output is UTF-8");
    assert!(plain.contains("UNINSTALL_SAFETY=PASS"));
    assert!(plain.contains("CODEX_INTEGRATION=NOT_INSTALLED"));
    assert!(plain.contains("OWNER_ACTION=none"));
}

#[test]
fn setup_codex_defaults_to_human_output_and_plain_retains_receipts() {
    let root = TestRoot::new("human-setup-output");
    let codex = fake_codex_directory(&root, "0.149.0");

    let human = isolated_command_with_codex(&root, &codex)
        .args(["setup", "codex", "--lang", "en-US"])
        .output()
        .expect("human setup codex starts");
    assert!(human.status.success());
    let human = String::from_utf8(human.stdout).expect("human setup output is UTF-8");
    assert!(human.contains("Codex integration installed."));
    assert!(!human.contains("SETUP_IDEMPOTENCE="));
    assert!(!human.contains("CODEX_INTEGRATION="));
    assert!(!human.contains("OWNER_ACTION="));
    assert!(
        !human.contains('\u{1b}'),
        "redirected human output must not contain ANSI color"
    );

    let plain = isolated_command_with_codex(&root, &codex)
        .args(["setup", "codex", "--plain"])
        .output()
        .expect("plain setup codex starts");
    assert!(plain.status.success());
    let plain = String::from_utf8(plain.stdout).expect("plain setup output is UTF-8");
    assert!(plain.contains("SETUP_IDEMPOTENCE=PASS"));
    assert!(plain.contains("CODEX_INTEGRATION=ALREADY_INSTALLED"));
    assert!(plain.contains("OWNER_ACTION=run tabbeacon doctor"));
    assert!(!plain.contains('\u{1b}'));
}

#[test]
fn codex_repair_v2_plain_and_json_diagnostics_bind_apply_to_preview_digest() {
    let root = TestRoot::new("codex-repair-v2-output");
    let codex = fake_codex_directory(&root, "0.149.0");
    let setup = isolated_command_with_codex(&root, &codex)
        .args(["setup", "codex", "--plain"])
        .output()
        .expect("setup codex starts");
    assert!(setup.status.success());

    let hooks_path = root.child("codex-home/hooks.json");
    let manifest_path = root.child("local-appdata/TabBeacon/codex-integration/integration-v1.json");
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("repair manifest reads"))
            .expect("repair manifest parses");
    let mut hooks: serde_json::Value =
        serde_json::from_slice(&fs::read(&hooks_path).expect("installed hooks read"))
            .expect("installed hooks parse");
    for declaration in manifest["hooks"]
        .as_array()
        .expect("manifest hooks are an array")
    {
        let event = declaration["event"].as_str().expect("manifest Hook event");
        let group = &declaration["group"];
        let groups = hooks["hooks"][event]
            .as_array_mut()
            .expect("installed event groups are arrays");
        groups.retain(|candidate| candidate != group);
        if groups.is_empty() {
            hooks["hooks"]
                .as_object_mut()
                .expect("hooks root is an object")
                .remove(event);
        }
    }
    fs::write(
        &hooks_path,
        serde_json::to_vec_pretty(&hooks).expect("orphaned hooks serialize"),
    )
    .expect("orphaned hooks write");

    let preview = isolated_command_with_codex(&root, &codex)
        .args(["repair", "codex", "--plain"])
        .output()
        .expect("repair preview starts");
    assert!(preview.status.success());
    let preview = String::from_utf8(preview.stdout).expect("repair preview is UTF-8");
    assert!(preview.contains("REPAIR_SCHEMA_VERSION=2"));
    assert!(preview.contains("REPAIR_DISPOSITION=READY_TO_APPLY"));
    assert!(preview.contains("POSTINSTALL_THIRD_PARTY_PRESERVED=false"));
    let digest = preview
        .lines()
        .find_map(|line| line.strip_prefix("TARGET_DIGEST="))
        .expect("preview target digest is emitted")
        .to_owned();
    assert!(digest.starts_with("sha256:"));

    let missing_digest = isolated_command_with_codex(&root, &codex)
        .args(["repair", "codex", "--apply", "--plain"])
        .output()
        .expect("repair apply without digest starts");
    assert!(!missing_digest.status.success());
    let missing_digest = String::from_utf8(missing_digest.stderr).expect("plain error is UTF-8");
    assert!(missing_digest.contains("REPAIR_FAILURE_CLASS=PREVIEW_TARGET_DIGEST_REQUIRED"));
    assert!(missing_digest.contains("MANUAL_TRUST_BOUNDARY=/hooks"));

    let missing_digest_json = isolated_command_with_codex(&root, &codex)
        .args(["repair", "codex", "--apply", "--json"])
        .output()
        .expect("repair JSON error starts");
    assert!(!missing_digest_json.status.success());
    let error_json: serde_json::Value = serde_json::from_slice(&missing_digest_json.stderr)
        .expect("repair JSON error is machine-readable");
    assert_eq!(error_json["repair_disposition"], "blocked");
    assert_eq!(
        error_json["failure_class"],
        "PREVIEW_TARGET_DIGEST_REQUIRED"
    );
    assert_eq!(error_json["auto_hook_trust"], false);

    let applied = isolated_command_with_codex(&root, &codex)
        .args([
            "repair",
            "codex",
            "--apply",
            "--expected-target-digest",
            &digest,
            "--plain",
        ])
        .output()
        .expect("digest-bound repair apply starts");
    assert!(applied.status.success());
    let applied = String::from_utf8(applied.stdout).expect("repair apply is UTF-8");
    assert!(applied.contains("REPAIR_DISPOSITION=REPAIRED_TRUST_REVIEW_REQUIRED"));
    assert!(applied.contains("AUTO_HOOK_TRUST=false"));
    assert!(applied.contains("OWNER_ACTION=launch codex, review TabBeacon hooks in /hooks"));
}

#[test]
fn human_setup_and_config_failures_do_not_emit_machine_receipts() {
    let root = TestRoot::new("human-setup-failure-output");

    let setup = isolated_command(&root)
        .args(["setup", "--lang", "en-US"])
        .output()
        .expect("human guided setup starts");
    assert!(!setup.status.success());
    let setup_error = String::from_utf8(setup.stderr).expect("human setup error is UTF-8");
    assert!(setup_error.contains("Setup needs an interactive terminal."));
    assert!(!setup_error.contains("SETUP="));
    assert!(!setup_error.contains("NEXT_ACTION="));
    assert!(!setup_error.contains('\u{1b}'));

    let setup_plain = isolated_command(&root)
        .args(["setup", "--plain"])
        .output()
        .expect("plain guided setup starts");
    assert!(!setup_plain.status.success());
    let setup_plain_error =
        String::from_utf8(setup_plain.stderr).expect("plain setup error is UTF-8");
    assert!(setup_plain_error.contains("SETUP=BLOCKED"));
    assert!(setup_plain_error.contains("NEXT_ACTION="));

    let config = isolated_command(&root)
        .args(["config", "wizard", "--lang", "en-US"])
        .output()
        .expect("human config wizard starts");
    assert!(!config.status.success());
    let config_error = String::from_utf8(config.stderr).expect("human config error is UTF-8");
    assert!(config_error.contains("Configuration needs an interactive terminal."));
    assert!(!config_error.contains("CONFIG="));
    assert!(!config_error.contains("NEXT_ACTION="));

    let direct_setup = isolated_command(&root)
        .env_remove("LOCALAPPDATA")
        .args(["setup", "codex", "--lang", "en-US"])
        .output()
        .expect("human direct setup starts");
    assert!(!direct_setup.status.success());
    let direct_setup_error =
        String::from_utf8(direct_setup.stderr).expect("human direct setup error is UTF-8");
    assert!(direct_setup_error.contains("Setup could not be completed:"));
    assert!(!direct_setup_error.contains("SETUP="));
    assert!(!direct_setup_error.contains("REASON="));

    let direct_setup_plain = isolated_command(&root)
        .env_remove("LOCALAPPDATA")
        .args(["setup", "codex", "--plain"])
        .output()
        .expect("plain direct setup starts");
    assert!(!direct_setup_plain.status.success());
    let direct_setup_plain_error =
        String::from_utf8(direct_setup_plain.stderr).expect("plain direct setup error is UTF-8");
    assert!(direct_setup_plain_error.contains("SETUP=FAIL"));
    assert!(direct_setup_plain_error.contains("REASON="));

    let config = isolated_command(&root)
        .env_remove("LOCALAPPDATA")
        .args(["config", "show", "--lang", "en-US"])
        .output()
        .expect("human config show starts");
    assert!(!config.status.success());
    let config_error = String::from_utf8(config.stderr).expect("human config failure is UTF-8");
    assert!(config_error.contains("Configuration could not be completed:"));
    assert!(!config_error.contains("CONFIG="));
    assert!(!config_error.contains("REASON="));

    let uninstall = isolated_command(&root)
        .env_remove("LOCALAPPDATA")
        .args(["uninstall", "codex"])
        .output()
        .expect("human uninstall starts");
    assert!(!uninstall.status.success());
    let uninstall_error =
        String::from_utf8(uninstall.stderr).expect("human uninstall failure is UTF-8");
    assert!(uninstall_error.contains("Uninstall could not be completed:"));
    assert!(!uninstall_error.contains("UNINSTALL="));
    assert!(!uninstall_error.contains("REASON="));
}

#[test]
fn invalid_arguments_keep_usage_exit_code_and_interactive_commands_refuse_pipes() {
    let root = TestRoot::new("non-tty");

    let invalid = isolated_command(&root)
        .args(["status", "--json", "--plain"])
        .output()
        .expect("invalid status starts");
    assert_eq!(invalid.status.code(), Some(2));

    let setup = isolated_command(&root)
        .args(["setup", "--lang", "en-US"])
        .output()
        .expect("piped setup starts");
    assert_eq!(setup.status.code(), Some(2));
    let setup_stderr = String::from_utf8(setup.stderr).expect("setup error is UTF-8");
    assert!(setup_stderr.contains("Setup needs an interactive terminal."));
    assert!(!setup_stderr.contains("SETUP="));
    assert!(!setup_stderr.contains("SETTINGS_UNCHANGED="));
    assert!(!root.child("local-appdata/TabBeacon/config.toml").exists());

    let wizard = isolated_command(&root)
        .args(["config", "wizard", "--lang", "en-US"])
        .output()
        .expect("piped config wizard starts");
    assert_eq!(wizard.status.code(), Some(2));
    let wizard_stderr = String::from_utf8(wizard.stderr).expect("wizard error is UTF-8");
    assert!(wizard_stderr.contains("Configuration needs an interactive terminal."));
    assert!(!wizard_stderr.contains("CONFIG="));
    assert!(!root.child("local-appdata/TabBeacon/config.toml").exists());

    let ui = isolated_command(&root)
        .arg("ui")
        .output()
        .expect("piped UI admission starts");
    assert!(ui.status.success());
    let ui = String::from_utf8(ui.stdout).expect("UI admission is UTF-8");
    assert!(ui.contains("TABBEACON_UI=NON_INTERACTIVE"));
    assert!(!ui.contains('\u{1b}'));
}

#[test]
fn g105_timing_uses_setup_prewarmed_exact_worker_images() {
    let script = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("scripts")
            .join("measure-codex-hook-runtime.ps1"),
    )
    .expect("G105 timing script reads");
    for required in [
        "function Initialize-G105QualificationState",
        "$info.EnvironmentVariables['LOCALAPPDATA'] = $State",
        "$info.EnvironmentVariables['CODEX_HOME'] = $codexHome",
        "$info.ArgumentList.Add('setup')",
        "$info.ArgumentList.Add('codex')",
        "Initialize-G105QualificationState -State $State",
        "$publishedHash -ne $binarySha256",
    ] {
        assert!(
            script.contains(required),
            "G105 timing must preserve the setup-prewarmed exact-image contract: {required}"
        );
    }
}
