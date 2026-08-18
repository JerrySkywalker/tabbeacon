#![cfg(windows)]

use std::{
    env, fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TestRoot(PathBuf);

impl TestRoot {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test clock is after epoch")
            .as_nanos();
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = env::temp_dir().join(format!(
            "tabbeacon-g45x-sessions-{}-{nonce}-{counter}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("isolated root creates");
        Self(path)
    }

    fn child(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    fn activity_directory(&self) -> PathBuf {
        self.child("local-appdata")
            .join("TabBeacon")
            .join("repository-identity")
            .join("activity-worker-v1")
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn digest(character: char) -> String {
    std::iter::repeat_n(character, 64).collect()
}

#[derive(Clone, Copy)]
struct LeaseFixture<'a> {
    key: char,
    session: char,
    terminal: char,
    workspace_alias: &'a str,
    state: &'a str,
    updated_unix_ms: u64,
    expires_unix_ms: u64,
}

fn write_lease(directory: &std::path::Path, fixture: LeaseFixture<'_>) {
    let key = digest(fixture.key);
    let lease = serde_json::json!({
        "schema": "tabbeacon-activity-worker-lease-v1",
        "key_sha256": key,
        "session_sha256": digest(fixture.session),
        "turn_sha256": digest('7'),
        "terminal_binding_sha256": digest(fixture.terminal),
        "generation": 1,
        "event_sequence": 1,
        "revision": 1,
        "owner_sha256": digest('8'),
        "active": true,
        "presentation": {
            "workspace_alias": fixture.workspace_alias,
            "semantic_state": fixture.state,
            "spinner_preset": "braille"
        },
        "updated_unix_ms": fixture.updated_unix_ms,
        "expires_unix_ms": fixture.expires_unix_ms
    });
    fs::write(
        directory.join(format!("lease-{key}.json")),
        serde_json::to_vec_pretty(&lease).expect("lease serializes"),
    )
    .expect("lease fixture writes");
}

fn write_test_leases(directory: &std::path::Path, now: u64) {
    for fixture in [
        LeaseFixture {
            key: 'a',
            session: 'b',
            terminal: 'c',
            workspace_alias: "OWH",
            state: "working",
            updated_unix_ms: now.saturating_sub(5_000),
            expires_unix_ms: now.saturating_add(60_000),
        },
        LeaseFixture {
            key: 'd',
            session: 'e',
            terminal: 'f',
            workspace_alias: "OWH",
            state: "approval",
            updated_unix_ms: now.saturating_sub(600_000),
            expires_unix_ms: now.saturating_sub(1),
        },
        LeaseFixture {
            key: '0',
            session: '1',
            terminal: '2',
            workspace_alias: r"C:\private\repo",
            state: "working",
            updated_unix_ms: now.saturating_sub(1_000),
            expires_unix_ms: now.saturating_add(60_000),
        },
        LeaseFixture {
            key: '3',
            session: '4',
            terminal: '5',
            workspace_alias: "SAFE",
            state: "working",
            updated_unix_ms: now.saturating_add(60_000),
            expires_unix_ms: now.saturating_add(120_000),
        },
        LeaseFixture {
            key: '6',
            session: '7',
            terminal: '8',
            workspace_alias: "SAFE",
            state: "working",
            updated_unix_ms: now,
            expires_unix_ms: now.saturating_sub(1),
        },
    ] {
        write_lease(directory, fixture);
    }
    fs::write(
        directory.join(format!("lease-{}.json", digest('9'))),
        br#"{"session_id":"session-secret","turn_id":"turn-secret","prompt":"prompt-secret","assistant":"assistant-secret","tool_output":"tool-secret","credential":"credential-secret","canonical_workspace":"C:\\private\\repo"}"#,
    )
    .expect("invalid private lease writes");
}

fn command(root: &TestRoot) -> Command {
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

fn assert_private_sessions_report(report: &serde_json::Value) {
    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["observation"], "ephemeral_lease_snapshot");
    assert_eq!(report["health"], "warning");
    assert_eq!(report["active_sessions"], 1);
    assert_eq!(report["stale_sessions"], 1);
    assert_eq!(report["invalid_leases"], 4);
    assert_eq!(
        report["sessions"].as_array().expect("session rows").len(),
        2
    );
    assert_eq!(report["read_only"], true);
    assert_eq!(report["boundaries"]["raw_native_session_ids"], false);
    assert_eq!(report["boundaries"]["prompt_content"], false);
    assert_eq!(report["boundaries"]["remote_control"], false);
    let states = report["sessions"]
        .as_array()
        .expect("session rows")
        .iter()
        .map(|session| session["semantic_state"].as_str().expect("semantic state"))
        .collect::<Vec<_>>();
    assert!(states.contains(&"working"));
    assert!(states.contains(&"approval"));

    let serialized = serde_json::to_string(report).expect("report reserializes");
    for forbidden in [
        "session-secret",
        "turn-secret",
        "prompt-secret",
        "assistant-secret",
        "tool-secret",
        "credential-secret",
        "canonical_workspace",
        "C:\\\\private\\\\repo",
        "SAFE",
        &digest('a'),
        &digest('b'),
        &digest('c'),
        &digest('d'),
        &digest('e'),
        &digest('f'),
    ] {
        assert!(
            !serialized.contains(forbidden),
            "sessions output leaked {forbidden}"
        );
    }
}

#[test]
fn sessions_cli_is_read_only_private_and_truthful_for_concurrent_leases() {
    let root = TestRoot::new();
    let directory = root.activity_directory();
    fs::create_dir_all(&directory).expect("activity directory creates");
    let now = u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test clock is after epoch")
            .as_millis(),
    )
    .expect("test time fits milliseconds");
    write_test_leases(&directory, now);
    let before = fs::read_dir(&directory)
        .expect("directory reads")
        .map(|entry| {
            let path = entry.expect("entry reads").path();
            let bytes = fs::read(&path).expect("fixture reads");
            (path, bytes)
        })
        .collect::<Vec<_>>();

    let output = command(&root)
        .args(["sessions", "--json"])
        .output()
        .expect("sessions JSON starts");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("sessions JSON parses");
    assert_private_sessions_report(&report);
    for (path, bytes) in before {
        assert_eq!(fs::read(path).expect("fixture rereads"), bytes);
    }
    assert!(!directory.join("activity-worker.lock").exists());

    let plain = command(&root)
        .args(["sessions", "--plain"])
        .output()
        .expect("sessions plain starts");
    assert!(plain.status.success());
    let plain = String::from_utf8(plain.stdout).expect("plain output is UTF-8");
    for marker in [
        "SESSIONS_VIEW=PASS",
        "READ_ONLY=true",
        "RAW_NATIVE_SESSION_IDS=false",
        "PROMPT_CONTENT=false",
        "REMOTE_CONTROL=false",
    ] {
        assert!(plain.contains(marker), "missing {marker}");
    }
}
