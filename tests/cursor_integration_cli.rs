#![cfg(windows)]

use std::{
    cell::RefCell,
    fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
    time::Duration,
};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::os::windows::process::CommandExt;
use tabbeacon::{
    providers::{
        cursor::CursorRouteStore,
        cursor_integration::CursorHookIntegration,
        cursor_runtime::{CursorDispatchOutcome, dispatch_with_settings_bounded},
    },
    settings::PresentationSettingsStore,
};

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
#[allow(clippy::too_many_lines)] // One isolated public workflow covers install, Hook dispatch and exact removal.
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
    assert!(config_text.contains("EFFECTIVE_TAB_COLOR=tabbeacon"));
    let registered_command = value["hooks"]["stop"][1]["command"].as_str().unwrap();
    let route_directory = isolated_local_appdata.join("TabBeacon/cursor-route-v1");
    let mut unbound = Command::new("cmd")
        .args(["/D", "/C"])
        .raw_arg(registered_command)
        .current_dir(root.path())
        .env("LOCALAPPDATA", &isolated_local_appdata)
        .env_remove("WT_SESSION")
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
        .env("LOCALAPPDATA", &isolated_local_appdata)
        .env("WT_SESSION", "synthetic-wt-session")
        .env_remove("TABBEACON_CURSOR_EXPECTED_WT_SHA256")
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
    if route_directory.exists() {
        assert!(route_directory.exists());
        eprintln!("PUBLIC_CURSOR_ROUTE=ADMITTED_WITH_PARENT_CONSOLE_BINDING");
    } else {
        eprintln!("PUBLIC_CURSOR_ROUTE=PARENT_CONSOLE_BINDING_UNAVAILABLE_FAIL_OPEN");
    }
    let uninstall = ["cursor", "uninstall", "--workspace", workspace, "--json"];
    assert_eq!(run(binary, &uninstall)["integration"], "not_installed");
    assert_eq!(run(binary, &check)["integration"], "not_installed");
    let value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(value["hooks"]["stop"][0]["command"], "foreign-hook");
    assert_eq!(value["unrelated"]["approval"], "unchanged");
}

#[test]
#[allow(clippy::too_many_lines)] // Public Apply and the next admitted Hook form one transition proof.
fn public_cursor_preference_apply_reports_deferred_visible_settlement() {
    let root = tempfile::tempdir().unwrap();
    let isolated_local_appdata = root.path().join("local-appdata");
    fs::create_dir(&isolated_local_appdata).unwrap();
    let binary = Path::new(env!("CARGO_BIN_EXE_tabbeacon"));
    let run_config = |arguments: &[&str]| {
        let output = Command::new(binary)
            .args(["config", "--plain", "provider", "cursor"])
            .args(arguments)
            .current_dir(root.path())
            .env("LOCALAPPDATA", &isolated_local_appdata)
            .output()
            .unwrap();
        assert!(output.status.success(), "status={:?}", output.status);
        assert!(output.stderr.is_empty());
        String::from_utf8(output.stdout).unwrap()
    };
    let settings_path = isolated_local_appdata.join("TabBeacon/config.toml");
    let route_path = isolated_local_appdata.join("TabBeacon/cursor-route-v1");

    let preview = run_config(&["preview", "color-only"]);
    assert!(preview.contains("CHANGE_APPLIED=false"));
    assert!(!settings_path.exists());
    let color = run_config(&["apply", "color-only"]);
    assert!(color.contains("CHANGE_APPLIED=true"));
    assert!(color.contains("REQUESTED_TAB_COLOR=tabbeacon"));
    assert!(color.contains("VISIBLE_OUTPUT_APPLY_BOUNDARY=NEXT_OWNED_EVENT_OR_OLD_TAB_CLOSE"));
    assert!(settings_path.exists());
    assert!(route_path.exists(), "Apply participates in the route lock");
    assert_eq!(
        fs::read_dir(&route_path).unwrap().count(),
        1,
        "Apply creates only the bounded lock, not a session or color lease"
    );

    CursorHookIntegration::new(root.path(), binary)
        .unwrap()
        .install()
        .unwrap();
    let store = PresentationSettingsStore::new(&settings_path);
    let terminal = "synthetic-owned-tab";
    let digest = format!("{:x}", Sha256::digest(terminal.as_bytes()));
    let sink = RefCell::new(Vec::new());
    let dispatch = |event: &[u8]| {
        let mut output = sink.borrow_mut();
        dispatch_with_settings_bounded(
            event,
            root.path(),
            binary,
            settings_path.parent().unwrap(),
            &digest,
            terminal,
            true,
            &store,
            &mut *output,
        )
        .unwrap()
    };
    assert_eq!(
        dispatch(br#"{"hook_event_name":"sessionStart","session_id":"owned"}"#),
        CursorDispatchOutcome::RouteAdmitted
    );
    assert_eq!(
        dispatch(br#"{"hook_event_name":"beforeSubmitPrompt","session_id":"owned","generation_id":"g1"}"#),
        CursorDispatchOutcome::OutputFlushed
    );
    let before_native = sink.borrow().len();
    assert!(before_native > 0);

    let native = run_config(&["apply", "preserve-native"]);
    assert!(native.contains("CHANGE_APPLIED=true"));
    assert!(native.contains("REQUESTED_TAB_COLOR=native"));
    assert!(native.contains("REQUESTED_TITLE=native"));
    assert!(native.contains("EFFECTIVE_ACTIVITY=native"));
    assert!(native.contains("LIVE_APPLICATION=UNPROVEN"));
    assert!(native.contains("VISIBLE_OUTPUT_APPLY_BOUNDARY=NEXT_OWNED_EVENT_OR_OLD_TAB_CLOSE"));
    assert_eq!(
        sink.borrow().len(),
        before_native,
        "Apply does not emit terminal output"
    );
    assert_eq!(
        dispatch(br#"{"hook_event_name":"stop","session_id":"owned","generation_id":"g1","status":"completed"}"#),
        CursorDispatchOutcome::OutputFlushed
    );
    assert_eq!(&sink.borrow()[before_native..], b"\x1b]104;264\x1b\\");
    let after_native = sink.borrow().len();
    assert_eq!(
        dispatch(br#"{"hook_event_name":"stop","session_id":"owned","generation_id":"g1","status":"completed"}"#),
        CursorDispatchOutcome::Ignored
    );
    assert_eq!(sink.borrow().len(), after_native);

    let inherited = run_config(&["inherit", "--apply"]);
    assert!(inherited.contains("CHANGE_APPLIED=true"));
    assert!(inherited.contains("VISIBLE_OUTPUT_APPLY_BOUNDARY=NEXT_OWNED_EVENT_OR_OLD_TAB_CLOSE"));
    assert_eq!(
        dispatch(br#"{"hook_event_name":"beforeSubmitPrompt","session_id":"owned","generation_id":"g2"}"#),
        CursorDispatchOutcome::OutputFlushed
    );
    assert!(sink.borrow()[after_native..].starts_with(b"\x1b]4;264;rgb:"));
    assert_eq!(
        dispatch(br#"{"hook_event_name":"stop","session_id":"owned","generation_id":"g2","status":"completed"}"#),
        CursorDispatchOutcome::OutputFlushed
    );
    let result_ready_bytes = sink.borrow().len();
    let native_after_result = run_config(&["apply", "preserve-native"]);
    assert!(native_after_result.contains("CHANGE_APPLIED=true"));
    assert_eq!(sink.borrow().len(), result_ready_bytes);
    assert_eq!(
        dispatch(br#"{"hook_event_name":"sessionEnd","session_id":"owned"}"#),
        CursorDispatchOutcome::OutputFlushed
    );
    assert_eq!(&sink.borrow()[result_ready_bytes..], b"\x1b]104;264\x1b\\");
}

#[test]
fn public_cursor_apply_waits_for_the_existing_route_output_lock() {
    let root = tempfile::tempdir().unwrap();
    let local_appdata = root.path().join("local-appdata");
    fs::create_dir(&local_appdata).unwrap();
    let binary = Path::new(env!("CARGO_BIN_EXE_tabbeacon"));
    let settings_path = local_appdata.join("TabBeacon/config.toml");
    let initial = Command::new(binary)
        .args([
            "config",
            "--plain",
            "provider",
            "cursor",
            "apply",
            "color-only",
        ])
        .current_dir(root.path())
        .env("LOCALAPPDATA", &local_appdata)
        .output()
        .unwrap();
    assert!(initial.status.success());
    let before = fs::read(&settings_path).unwrap();
    let state_root = settings_path.parent().unwrap();
    let mut child = CursorRouteStore::new(state_root)
        .with_route_lock(|| {
            let mut child = Command::new(binary)
                .args([
                    "config",
                    "--plain",
                    "provider",
                    "cursor",
                    "apply",
                    "preserve-native",
                ])
                .current_dir(root.path())
                .env("LOCALAPPDATA", &local_appdata)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?;
            std::thread::sleep(Duration::from_millis(40));
            assert!(child.try_wait()?.is_none());
            assert_eq!(fs::read(&settings_path)?, before);
            Ok(child)
        })
        .unwrap();
    assert!(child.wait().unwrap().success());
    assert_ne!(fs::read(settings_path).unwrap(), before);
}
