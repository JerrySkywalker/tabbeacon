#![cfg(windows)]

//! Real Windows regression for the package-binary mapping boundary.
//!
//! The fixture owns an isolated local-appdata root and two direct child
//! processes. It never touches the Owner Cargo installation, Terminal
//! settings, or a live `TabBeacon` lease.

use std::{
    env,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::{Child, Command},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use sha2::{Digest, Sha256};
use tabbeacon::worker_runtime::WorkerRuntimeStore;

use std::os::windows::process::CommandExt;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;

struct TestRoot(PathBuf);

impl TestRoot {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test clock is after epoch")
            .as_nanos();
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = env::temp_dir().join(format!(
            "tabbeacon-g63-worker-runtime-{}-{nonce}-{counter}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("isolated test root creates");
        Self(path)
    }

    fn local_appdata(&self) -> PathBuf {
        self.0.join("local-appdata")
    }

    fn state_root(&self) -> PathBuf {
        self.local_appdata()
            .join("TabBeacon")
            .join("repository-identity")
    }

    fn installed_binary(&self) -> PathBuf {
        self.0.join("cargo-bin").join("tabbeacon.exe")
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn sha256(path: &Path) -> String {
    let mut file = fs::File::open(path).expect("binary opens for content identity");
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher).expect("binary hashes");
    format!("{:x}", hasher.finalize())
}

fn framed_digest(values: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for value in values {
        hasher.update(value.len().to_le_bytes());
        hasher.update(value.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn now_ms() -> u64 {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test clock is after epoch")
            .as_millis(),
    )
    .expect("test timestamp fits u64")
}

fn write_active_lease(
    state_root: &Path,
    session: &str,
    wt_session: &str,
    generation: u64,
    owner_sha256: &str,
    runtime_image_sha256: &str,
) -> String {
    let terminal_binding_sha256 = framed_digest(&[wt_session]);
    let key_sha256 = framed_digest(&[session, &terminal_binding_sha256]);
    let now = now_ms();
    let lease = serde_json::json!({
        "schema": "tabbeacon-activity-worker-lease-v1",
        "key_sha256": key_sha256,
        "session_sha256": framed_digest(&[session]),
        "turn_sha256": framed_digest(&["turn"]),
        "terminal_binding_sha256": terminal_binding_sha256,
        "generation": generation,
        "event_sequence": 1,
        "revision": 1,
        "owner_sha256": owner_sha256,
        "runtime_image_sha256": runtime_image_sha256,
        "active": true,
        "presentation": {
            "workspace_alias": "TB",
            "provider": "codex",
            "semantic_state": "working",
            "spinner_preset": "braille"
        },
        "updated_unix_ms": now,
        "expires_unix_ms": now.saturating_add(60_000)
    });
    let directory = state_root.join("activity-worker-v1");
    fs::create_dir_all(&directory).expect("isolated lease directory creates");
    fs::write(
        directory.join(format!("lease-{key_sha256}.json")),
        serde_json::to_vec_pretty(&lease).expect("lease serializes"),
    )
    .expect("isolated lease writes");
    key_sha256
}

fn spawn_runtime_worker(
    executable: &Path,
    local_appdata: &Path,
    wt_session: &str,
    key_sha256: &str,
    generation: u64,
) -> Child {
    Command::new(executable)
        .args([
            "__activity-worker-v1",
            key_sha256,
            &generation.to_string(),
            "1",
        ])
        .env("LOCALAPPDATA", local_appdata)
        .env("WT_SESSION", wt_session)
        // A real Windows console is required because the production worker
        // owns CONOUT$ rather than redirected Hook stdout.
        .creation_flags(CREATE_NEW_CONSOLE)
        .spawn()
        .expect("runtime worker starts")
}

fn assert_alive(child: &mut Child, label: &str) {
    for _ in 0..20 {
        if child.try_wait().expect("worker state reads").is_none() {
            return;
        }
        thread::sleep(Duration::from_millis(25));
    }
    panic!("{label} worker exited before the owned lease could be observed");
}

fn stop_owned_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn active_runtime_workers_leave_the_installed_cli_replaceable() {
    let root = TestRoot::new();
    let source = PathBuf::from(env!("CARGO_BIN_EXE_tabbeacon"));
    let installed = root.installed_binary();
    fs::create_dir_all(installed.parent().expect("cargo-bin parent exists"))
        .expect("isolated cargo-bin creates");
    fs::copy(&source, &installed).expect("candidate package CLI copies");

    let runtime_store = WorkerRuntimeStore::new(root.state_root());
    let first_image = runtime_store
        .publish(&installed)
        .expect("first immutable runtime image publishes");
    assert_ne!(
        fs::canonicalize(&first_image.executable).expect("runtime image canonicalizes"),
        fs::canonicalize(&installed).expect("installed binary canonicalizes"),
        "long-lived worker must not map the package-installed CLI"
    );
    let wt_session = "g63-isolated-windows-console";
    let first_key = write_active_lease(
        &root.state_root(),
        "first-session",
        wt_session,
        1,
        &sha256(&first_image.executable),
        &first_image.content_sha256,
    );
    let mut first_worker = spawn_runtime_worker(
        &first_image.executable,
        &root.local_appdata(),
        wt_session,
        &first_key,
        1,
    );
    assert_alive(&mut first_worker, "first runtime-image");

    // This is a real Windows replace operation on a distinct installed CLI
    // path while a real TabBeacon activity worker remains mapped from image 1.
    let replacement = root.0.join("next-tabbeacon.exe");
    fs::copy(&source, &replacement).expect("next package candidate copies");
    OpenOptions::new()
        .append(true)
        .open(&replacement)
        .expect("next candidate opens")
        .write_all(b"tabbeacon-g63-runtime-image-fixture")
        .expect("next candidate receives an inert PE overlay");
    fs::rename(&replacement, &installed)
        .expect("Windows replaces the package CLI while image worker is active");
    assert_alive(
        &mut first_worker,
        "old runtime-image after package replacement",
    );

    let second_image = runtime_store
        .publish(&installed)
        .expect("new package content publishes a second immutable image");
    assert_ne!(
        first_image.content_sha256, second_image.content_sha256,
        "new package content must receive a distinct runtime image"
    );
    let second_key = write_active_lease(
        &root.state_root(),
        "second-session",
        wt_session,
        2,
        &sha256(&second_image.executable),
        &second_image.content_sha256,
    );
    let mut second_worker = spawn_runtime_worker(
        &second_image.executable,
        &root.local_appdata(),
        wt_session,
        &second_key,
        2,
    );
    assert_alive(&mut second_worker, "new runtime-image");
    assert_alive(&mut first_worker, "old runtime-image alongside new image");

    stop_owned_child(&mut second_worker);
    stop_owned_child(&mut first_worker);
}
