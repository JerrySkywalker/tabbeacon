#![cfg(windows)]

use std::{
    env, fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

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
fn config_defaults_to_human_monochrome_output_and_plain_retains_receipts() {
    let root = TestRoot::new("human-config-output");

    let human = isolated_command(&root)
        .args(["config", "show"])
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

    let human = isolated_command(&root)
        .args(["setup", "codex"])
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

    let plain = isolated_command(&root)
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
fn human_setup_and_config_failures_do_not_emit_machine_receipts() {
    let root = TestRoot::new("human-setup-failure-output");

    let setup = isolated_command(&root)
        .arg("setup")
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
        .args(["config", "wizard"])
        .output()
        .expect("human config wizard starts");
    assert!(!config.status.success());
    let config_error = String::from_utf8(config.stderr).expect("human config error is UTF-8");
    assert!(config_error.contains("Configuration needs an interactive terminal."));
    assert!(!config_error.contains("CONFIG="));
    assert!(!config_error.contains("NEXT_ACTION="));

    let direct_setup = isolated_command(&root)
        .env_remove("LOCALAPPDATA")
        .args(["setup", "codex"])
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
        .args(["config", "show"])
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
        .arg("setup")
        .output()
        .expect("piped setup starts");
    assert_eq!(setup.status.code(), Some(2));
    let setup_stderr = String::from_utf8(setup.stderr).expect("setup error is UTF-8");
    assert!(setup_stderr.contains("Setup needs an interactive terminal."));
    assert!(!setup_stderr.contains("SETUP="));
    assert!(!setup_stderr.contains("SETTINGS_UNCHANGED="));
    assert!(!root.child("local-appdata/TabBeacon/config.toml").exists());

    let wizard = isolated_command(&root)
        .args(["config", "wizard"])
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
