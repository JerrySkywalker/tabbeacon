#![cfg(windows)]

use std::{
    fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::os::windows::process::CommandExt;

fn run(binary: &Path, arguments: &[&str]) -> Value {
    let output = Command::new(binary).args(arguments).output().unwrap();
    assert!(
        output.status.success(),
        "args={arguments:?}, status={:?}, stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    serde_json::from_slice(&output.stdout).unwrap()
}

fn run_plain(binary: &Path, arguments: &[&str]) -> String {
    let output = Command::new(binary).args(arguments).output().unwrap();
    assert!(output.status.success(), "{:?}", output.status);
    assert!(output.stderr.is_empty());
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn public_cursor_management_preserves_foreign_project_hooks() {
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().to_str().unwrap();
    let directory = root.path().join(".cursor");
    fs::create_dir(&directory).unwrap();
    let path = directory.join("hooks.json");
    fs::write(
        &path,
        serde_json::to_vec(&json!({
            "version": 1,
            "unrelated": {"approval": "unchanged"},
            "hooks": {"stop": [{"command":"foreign-hook","timeout":4,"failClosed":true}]}
        }))
        .unwrap(),
    )
    .unwrap();
    let binary = Path::new(env!("CARGO_BIN_EXE_tabbeacon"));
    let check = ["cursor", "check", "--workspace", workspace, "--json"];
    assert_eq!(run(binary, &check)["integration"], "not_installed");
    let setup = ["setup", "--plain", "cursor", "--workspace", workspace];
    assert!(run_plain(binary, &setup).contains("CURSOR_INTEGRATION=installed"));
    assert_eq!(run(binary, &check)["integration"], "installed");
    let installed = fs::read(&path).unwrap();
    assert!(run_plain(binary, &setup).contains("CURSOR_INTEGRATION=installed"));
    assert_eq!(fs::read(&path).unwrap(), installed);
    let value: Value = serde_json::from_slice(&installed).unwrap();
    assert_eq!(value["hooks"]["stop"][0]["command"], "foreign-hook");
    assert!(
        !value["hooks"]["stop"][1]["command"]
            .as_str()
            .unwrap()
            .contains(r"\\?\")
    );
    assert_eq!(value["unrelated"]["approval"], "unchanged");
    let isolated_local_appdata = root.path().join("isolated-local-appdata");
    fs::create_dir(&isolated_local_appdata).unwrap();
    let config = Command::new(binary)
        .args([
            "config",
            "--plain",
            "provider",
            "cursor",
            "preview",
            "color-only",
        ])
        .current_dir(root.path())
        .env("LOCALAPPDATA", &isolated_local_appdata)
        .output()
        .unwrap();
    assert!(config.status.success());
    let config_text = String::from_utf8(config.stdout).unwrap();
    assert!(config_text.contains("LIVE_APPLICATION=UNPROVEN"));
    assert!(config_text.contains("EFFECTIVE_TAB_COLOR=native"));
    let data_root = root.path().join("isolated-cursor-data");
    fs::create_dir(&data_root).unwrap();
    let registered_command = value["hooks"]["stop"][1]["command"].as_str().unwrap();
    let expected_terminal = format!("{:x}", Sha256::digest(b"synthetic-wt-session"));
    let route_directory = data_root.join("tabbeacon/cursor-route-v1");
    let mut unbound = Command::new("cmd")
        .args(["/D", "/C"])
        .raw_arg(registered_command)
        .current_dir(root.path())
        .env("CURSOR_DATA_DIR", &data_root)
        .env("WT_SESSION", "synthetic-wt-session")
        .env_remove("TABBEACON_CURSOR_EXPECTED_WT_SHA256")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    unbound
        .stdin
        .take()
        .unwrap()
        .write_all(b"{\"hook_event_name\":\"sessionStart\",\"session_id\":\"synthetic-session\"}")
        .unwrap();
    let unbound_output = unbound.wait_with_output().unwrap();
    assert!(unbound_output.status.success());
    assert_eq!(unbound_output.stdout, b"{}\n");
    assert!(unbound_output.stderr.is_empty());
    assert!(!route_directory.exists());
    let mut hook = Command::new("cmd")
        .args(["/D", "/C"])
        .raw_arg(registered_command)
        .current_dir(root.path())
        .env("CURSOR_DATA_DIR", &data_root)
        .env("WT_SESSION", "synthetic-wt-session")
        .env("TABBEACON_CURSOR_EXPECTED_WT_SHA256", expected_terminal)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    hook.stdin
        .take()
        .unwrap()
        .write_all(b"\xef\xbb\xbf{\"hook_event_name\":\"sessionStart\",\"session_id\":\"synthetic-session\"}")
        .unwrap();
    let output = hook.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "status={:?}, stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"{}\n");
    assert!(output.stderr.is_empty());
    // A detached Hosted test process may have no owned console. In that case
    // dispatch refuses state creation; the public JSON protocol still holds.
    if tabbeacon::console_output::open_owned_console().is_ok() {
        assert!(route_directory.exists());
        eprintln!("PUBLIC_CURSOR_ROUTE=ADMITTED_IN_OWNED_TEST_CONSOLE");
    } else {
        assert!(!route_directory.exists());
        eprintln!("PUBLIC_CURSOR_ROUTE=CONSOLE_UNAVAILABLE_FAIL_OPEN");
    }
    let uninstall = ["cursor", "uninstall", "--workspace", workspace, "--json"];
    assert_eq!(run(binary, &uninstall)["integration"], "not_installed");
    assert_eq!(run(binary, &check)["integration"], "not_installed");
    let value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(value["hooks"]["stop"][0]["command"], "foreign-hook");
    assert_eq!(value["unrelated"]["approval"], "unchanged");
}
