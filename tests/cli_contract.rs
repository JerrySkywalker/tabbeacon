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
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&json_en.stdout)
            .expect("English alias JSON parses"),
        serde_json::from_slice::<serde_json::Value>(&json_zh.stdout)
            .expect("Chinese alias JSON parses"),
        "machine alias JSON must not depend on Human locale"
    );
    let json_text = String::from_utf8(json_en.stdout).expect("alias JSON is UTF-8");
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

    let human = isolated_command(&root)
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
