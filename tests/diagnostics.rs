#![cfg(windows)]

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::{
        OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::Value;

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
            "tabbeacon-g13-{label}-{}-{nonce}-{counter}",
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
        format!("@echo off\r\necho codex-cli {version}\r\n"),
    )
    .expect("fake Codex probe writes");
    directory
}

fn run_cli(
    root: &TestRoot,
    arguments: &[&str],
    codex_directory: Option<&Path>,
    comspec: Option<&Path>,
) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_tabbeacon"));
    command
        .args(arguments)
        .env("CODEX_HOME", root.child("codex-home"))
        .env("LOCALAPPDATA", root.child("local-appdata"))
        .env("USERPROFILE", root.child("user-profile"))
        .env("XDG_STATE_HOME", root.child("xdg-state"))
        .env_remove("WT_SESSION")
        .env_remove("WT_PROFILE_ID");
    if let Some(codex_directory) = codex_directory {
        let path = env::join_paths([codex_directory, inherited_path()])
            .expect("fake Codex path joins safely");
        command.env("PATH", path);
    } else {
        command.env("PATH", inherited_path());
    }
    if let Some(comspec) = comspec {
        command.env("COMSPEC", comspec);
    }
    command.output().expect("TabBeacon diagnostics CLI starts")
}

fn run_terminal_cli(root: &TestRoot, arguments: &[&str], codex_directory: Option<&Path>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_tabbeacon"));
    command
        .args(arguments)
        .env("CODEX_HOME", root.child("codex-home"))
        .env("LOCALAPPDATA", root.child("local-appdata"))
        .env("USERPROFILE", root.child("user-profile"))
        .env("XDG_STATE_HOME", root.child("xdg-state"))
        .env("WT_SESSION", "11111111-1111-1111-1111-111111111111")
        .env("WT_PROFILE_ID", "{22222222-2222-2222-2222-222222222222}");
    if let Some(codex_directory) = codex_directory {
        let path = env::join_paths([codex_directory, inherited_path()])
            .expect("fake Codex path joins safely");
        command.env("PATH", path);
    } else {
        command.env("PATH", inherited_path());
    }
    command.output().expect("TabBeacon title-policy CLI starts")
}

fn json_output(output: &Output) -> Value {
    assert!(
        output.stderr.is_empty(),
        "JSON diagnostics must not write stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout.clone()).expect("diagnostics stdout is UTF-8");
    serde_json::from_str(&stdout).expect("diagnostics stdout is valid JSON only")
}

#[test]
fn status_json_is_structured_private_and_read_only_for_isolated_state() {
    let root = TestRoot::new("status");
    let fake_codex = fake_codex_directory(&root, "0.147.0");
    let activity_directory = root
        .child("local-appdata")
        .join("TabBeacon")
        .join("repository-identity")
        .join("activity-worker-v1");
    fs::create_dir_all(&activity_directory).expect("isolated activity directory creates");
    fs::write(
        activity_directory.join(format!("lease-{}.json", "a".repeat(64))),
        br#"{"session_id":"session-secret","turn_id":"turn-secret","prompt":"prompt-secret"}"#,
    )
    .expect("corrupt activity fixture writes");

    let output = run_cli(&root, &["status", "--json"], Some(&fake_codex), None);

    assert!(output.status.success(), "status is observational");
    let report = json_output(&output);
    let serialized = serde_json::to_string(&report).expect("report reserializes");
    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["codex"]["version"], "0.147.0");
    assert_eq!(report["codex"]["profile_supported"], true);
    assert_eq!(report["codex"]["profile_state"], "supported");
    assert_eq!(report["presentation"]["source"], "default");
    assert_eq!(report["title"]["desired_owner"], "tabbeacon");
    assert_eq!(report["title"]["codex_writer_state"], "unavailable");
    assert_eq!(report["title"]["application_title_policy"], "unavailable");
    assert_eq!(report["title"]["visible_probe"], "not_run");
    assert_eq!(report["title"]["probe_boundary"], "not_run");
    assert_eq!(report["title"]["authority"], "unverified");
    assert_eq!(report["doctor"]["title"], report["title"]);
    assert_eq!(report["activity"]["observation"], "lease_based");
    assert_eq!(report["activity"]["active_leases"], 0);
    assert_eq!(report["activity"]["stale_leases"], 0);
    assert_eq!(report["activity"]["invalid_leases"], 1);
    assert_eq!(report["workspace"]["alias_registry_count"], 0);
    for sensitive in [
        "session-secret",
        "turn-secret",
        "prompt-secret",
        "codex-home",
        "local-appdata",
        "repository-identity",
        "PowerShell",
    ] {
        assert!(
            !serialized.contains(sensitive),
            "diagnostic JSON leaked {sensitive}"
        );
    }
    assert!(
        !root.child("local-appdata/TabBeacon/config.toml").exists(),
        "status must not create presentation settings"
    );
    assert!(
        !root.child("local-appdata/TabBeacon/config.lock").exists(),
        "status must not create a settings lock"
    );
    assert!(
        !root
            .child("local-appdata/TabBeacon/repository-identity/registry.lock")
            .exists(),
        "status must not create an alias-registry lock"
    );
    assert!(
        !root
            .child("local-appdata/TabBeacon/windows-terminal-title-policy-v1.json")
            .exists(),
        "passive diagnostics must not create a title-policy ownership receipt"
    );
}

#[test]
fn explicit_title_policy_repair_is_fixture_scoped_and_passive_inspection_is_read_only() {
    let root = TestRoot::new("title-policy");
    let settings = root
        .child("local-appdata")
        .join("Packages")
        .join("Microsoft.WindowsTerminal_8wekyb3d8bbwe")
        .join("LocalState")
        .join("settings.json");
    fs::create_dir_all(settings.parent().expect("fixture parent"))
        .expect("fixture settings parent creates");
    let original = r#"{
  // retained comment
  "profiles": {
    "defaults": {},
    "list": [
      { "guid": "{22222222-2222-2222-2222-222222222222}", "name": "PowerShell", "suppressApplicationTitle": true },
    ],
  },
  "unknown": { "kept": true },
}
"#;
    fs::write(&settings, original).expect("fixture settings writes");

    let inspect = run_terminal_cli(&root, &["title-policy", "inspect", "--json"], None);
    assert!(inspect.status.success());
    let inspect = json_output(&inspect);
    assert_eq!(inspect["settings_source"], "stable");
    assert_eq!(inspect["active_profile_resolution"], "resolved_guid");
    assert_eq!(inspect["application_title_policy"], "suppressed_by_profile");
    assert_eq!(inspect["remediation"], "available");
    assert_eq!(
        fs::read_to_string(&settings).expect("inspection bytes"),
        original
    );
    assert!(
        !root
            .child("local-appdata/TabBeacon/windows-terminal-title-policy-v1.json")
            .exists(),
        "inspection must not create an ownership receipt"
    );

    let repair = run_terminal_cli(&root, &["title-policy", "repair", "--json"], None);
    assert!(repair.status.success());
    let repair = json_output(&repair);
    assert_eq!(repair["result"]["document_modified"], true);
    assert_eq!(repair["result"]["user_config_preserved"], true);
    let repaired = fs::read_to_string(&settings).expect("repaired bytes");
    assert!(repaired.contains("// retained comment"));
    assert!(repaired.contains("\"unknown\": { \"kept\": true }"));
    assert!(repaired.contains("\"suppressApplicationTitle\": false"));

    let second = run_terminal_cli(&root, &["title-policy", "repair", "--json"], None);
    let second = json_output(&second);
    assert_eq!(second["result"]["state"], "already_owned");
    assert_eq!(second["result"]["document_modified"], false);
    assert_eq!(
        fs::read_to_string(&settings).expect("second bytes"),
        repaired
    );

    let restore = run_terminal_cli(&root, &["title-policy", "restore", "--json"], None);
    assert!(restore.status.success());
    let restore = json_output(&restore);
    assert_eq!(restore["result"]["document_modified"], true);
    assert_eq!(
        fs::read_to_string(&settings).expect("restored bytes"),
        original
    );
}

#[test]
fn status_distinguishes_missing_and_unsupported_codex_without_failing() {
    let missing_root = TestRoot::new("missing-codex");
    let missing_shell = missing_root.child("missing-cmd.exe");
    let missing = run_cli(
        &missing_root,
        &["status", "--json"],
        None,
        Some(&missing_shell),
    );
    assert!(missing.status.success(), "status remains observational");
    let missing = json_output(&missing);
    assert!(missing["codex"]["version"].is_null());
    assert_eq!(missing["codex"]["profile_supported"], false);
    assert_eq!(missing["codex"]["profile_state"], "unknown_or_unavailable");

    let unsupported_root = TestRoot::new("unsupported-codex");
    let fake_codex = fake_codex_directory(&unsupported_root, "0.148.0");
    let unsupported = run_cli(
        &unsupported_root,
        &["status", "--json"],
        Some(&fake_codex),
        None,
    );
    assert!(unsupported.status.success(), "status remains observational");
    let unsupported = json_output(&unsupported);
    assert_eq!(unsupported["codex"]["version"], "0.148.0");
    assert!(unsupported["codex"]["hook_profile"].is_null());
    assert_eq!(unsupported["codex"]["profile_supported"], false);
    assert_eq!(unsupported["codex"]["profile_state"], "known_unadmitted");
}

#[test]
fn status_reports_an_isolated_owned_hook_installation_without_trusting_it() {
    let root = TestRoot::new("installed");
    let fake_codex = fake_codex_directory(&root, "0.147.0");

    let setup = run_cli(&root, &["setup", "codex"], Some(&fake_codex), None);
    assert!(
        setup.status.success(),
        "isolated setup must succeed: {}",
        String::from_utf8_lossy(&setup.stderr)
    );
    let settings = run_cli(
        &root,
        &["config", "set", "spinner", "braille"],
        Some(&fake_codex),
        None,
    );
    assert!(
        settings.status.success(),
        "isolated settings change succeeds"
    );
    let status = run_cli(&root, &["status", "--json"], Some(&fake_codex), None);
    assert!(status.status.success());
    let report = json_output(&status);

    assert_eq!(report["integration"]["installed"], true);
    assert_eq!(report["integration"]["owned_hook_count"], 11);
    assert_eq!(report["integration"]["declaration_status"], "pass");
    assert_eq!(report["integration"]["hook_trust"], "review_required");
    assert_eq!(report["integration"]["title_ownership"], "tabbeacon");
    assert_eq!(report["presentation"]["source"], "configured");
    assert_eq!(report["presentation"]["spinner_preset"], "braille");
    assert_eq!(report["doctor"]["overall"], "warning");
}

#[test]
fn status_handles_malformed_settings_without_creating_a_lock() {
    let root = TestRoot::new("malformed-settings");
    let fake_codex = fake_codex_directory(&root, "0.147.0");
    let settings_directory = root.child("local-appdata/TabBeacon");
    fs::create_dir_all(&settings_directory).expect("settings fixture directory creates");
    fs::write(
        settings_directory.join("config.toml"),
        "[presentation\ntitle = ",
    )
    .expect("malformed settings write");

    let output = run_cli(&root, &["status", "--json"], Some(&fake_codex), None);

    assert!(output.status.success());
    let report = json_output(&output);
    assert_eq!(report["presentation"]["source"], "invalid");
    assert!(report["presentation"]["title_mode"].is_null());
    assert!(
        !settings_directory.join("config.lock").exists(),
        "status must not lock malformed settings"
    );
}

#[test]
fn doctor_json_and_human_output_share_failure_exit_semantics() {
    let root = TestRoot::new("doctor");
    let fake_codex = fake_codex_directory(&root, "0.147.0");

    let json = run_cli(&root, &["doctor", "--json"], Some(&fake_codex), None);
    assert!(
        !json.status.success(),
        "an uninstalled integration remains a doctor failure"
    );
    let report = json_output(&json);
    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["overall"], "fail");
    assert!(
        report["failures"]
            .as_array()
            .is_some_and(|failures| !failures.is_empty()),
        "doctor failures must be represented structurally"
    );

    let human = run_cli(&root, &["doctor"], Some(&fake_codex), None);
    assert!(!human.status.success());
    let human = String::from_utf8(human.stdout).expect("human doctor output is UTF-8");
    assert!(human.contains("DOCTOR=FAIL"));

    let status = run_cli(&root, &["status"], Some(&fake_codex), None);
    assert!(status.status.success());
    let status = String::from_utf8(status.stdout).expect("human status output is UTF-8");
    assert!(status.contains("STATUS_SCHEMA_VERSION=1"));
    assert!(status.contains("DOCTOR=FAIL"));
}

#[test]
fn convergence_matrix_is_typed_bounded_and_read_only() {
    let root = TestRoot::new("convergence-matrix");

    let output = run_cli(&root, &["convergence", "matrix", "--json"], None, None);

    assert!(
        output.status.success(),
        "matrix is an observational contract"
    );
    let matrix = json_output(&output);
    let rows = matrix.as_array().expect("matrix is a JSON array");
    assert!(rows.len() >= 32, "all G18 contexts are represented");
    for row in rows {
        assert!(row["scenario_id"].is_string());
        assert!(row["event_sequence"].is_array());
        assert!(row["expected_semantic_state"].is_string());
        assert!(row["expected_visible_state"].is_string());
        assert_eq!(row["maximum_convergence_deadline_ms"], 1_000);
        assert!(row["proof_method"].is_string());
        assert!(row["cleanup_requirement"].is_string());
        assert_eq!(row["result"], "pending_evidence");
    }
    for required in [
        "normal_powershell_visible",
        "actual_elevated_powershell_visible",
        "same_workspace_parallel_sessions",
        "linked_worktree",
        "terminal_close",
    ] {
        assert!(
            rows.iter().any(|row| row["scenario_id"] == required),
            "matrix is missing {required}"
        );
    }
    assert!(
        !root.child("local-appdata/TabBeacon/config.toml").exists(),
        "matrix inspection must not create settings"
    );
    assert!(
        !root
            .child("local-appdata/TabBeacon/windows-terminal-title-policy-v1.json")
            .exists(),
        "matrix inspection must not create a policy receipt"
    );
}
