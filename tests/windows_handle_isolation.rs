#![cfg(windows)]

use std::{
    env, fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde_json::{Value, json};
use tabbeacon::activity::{
    ACTIVITY_OBSERVER_PROBE_PROCESS_FILE, ACTIVITY_WORKER_PROBE_PROCESS_FILE,
    ACTIVITY_WORKER_PROBE_RECEIPT_ENV, ACTIVITY_WORKER_PROBE_RECEIPT_FILE,
    ACTIVITY_WORKER_PROBE_RESULT_READY_FILE, ACTIVITY_WORKER_PROBE_STARTED_FILE,
};
use tabbeacon::worker_runtime::WorkerRuntimeStore;
use windows::Win32::{
    Foundation::{
        CloseHandle, ERROR_BROKEN_PIPE, HANDLE, HANDLE_FLAG_INHERIT, HANDLE_FLAGS,
        SetHandleInformation,
    },
    Security::SECURITY_ATTRIBUTES,
    System::Pipes::{CreatePipe, PeekNamedPipe},
};
use windows::core::HRESULT;

// This is an OS-fixture watchdog, not the Codex Hook declaration. Keep it
// long enough to observe handle/EOF correctness even on a saturated Windows
// host where first execution of a newly built debug worker image can spend
// minutes in system process-start inspection. Release-mode Hook timing has a
// separate one-second acceptance gate; this watchdog must not impersonate it.
const STAGE_TIMEOUT: Duration = Duration::from_mins(5);
static TEST_ROOT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TestRoot(PathBuf);

impl TestRoot {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after Unix epoch")
            .as_nanos();
        let sequence = TEST_ROOT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path =
            env::temp_dir().join(format!("tb-pipe-{}-{nonce}-{sequence}", std::process::id()));
        fs::create_dir(&path).expect("isolated test root is created");
        Self(path)
    }

    fn child(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct InheritableSentinelPipe {
    read: HANDLE,
    write: Option<HANDLE>,
}

impl InheritableSentinelPipe {
    #[allow(unsafe_code)]
    fn new() -> Self {
        use std::{mem, ptr};

        let security = SECURITY_ATTRIBUTES {
            nLength: u32::try_from(mem::size_of::<SECURITY_ATTRIBUTES>()).unwrap_or(u32::MAX),
            lpSecurityDescriptor: ptr::null_mut(),
            bInheritHandle: true.into(),
        };
        let mut read = HANDLE::default();
        let mut write = HANDLE::default();
        unsafe { CreatePipe(&raw mut read, &raw mut write, Some(&raw const security), 0) }
            .expect("inheritable sentinel pipe is created");
        unsafe { SetHandleInformation(read, HANDLE_FLAG_INHERIT.0, HANDLE_FLAGS(0)) }
            .expect("sentinel read end is not inheritable");
        Self {
            read,
            write: Some(write),
        }
    }

    #[allow(unsafe_code)]
    fn close_writer(&mut self) {
        if let Some(write) = self.write.take() {
            unsafe { CloseHandle(write) }.expect("sentinel writer closes");
        }
    }

    #[allow(unsafe_code)]
    fn reaches_eof_within(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            match unsafe { PeekNamedPipe(self.read, None, 0, None, None, None) } {
                Ok(()) => {}
                Err(error) if error.code() == HRESULT::from_win32(ERROR_BROKEN_PIPE.0) => {
                    return true;
                }
                Err(error) => panic!("sentinel pipe inspection failed: {error}"),
            }
            if Instant::now() >= deadline {
                return false;
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for InheritableSentinelPipe {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        if let Some(write) = self.write.take() {
            let _ = unsafe { CloseHandle(write) };
        }
        let _ = unsafe { CloseHandle(self.read) };
    }
}

enum HookStream {
    Stdout(std::io::Result<Vec<u8>>),
    Stderr(std::io::Result<Vec<u8>>),
}

fn terminate_owned_tree(process_id: u32) {
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

fn wait_for_root(mut child: Child, payload: &[u8], stage: &str) {
    let process_id = child.id();
    let mut stdin = child.stdin.take().expect("Hook parent exposes stdin");
    stdin.write_all(payload).expect("Hook accepts payload");
    stdin.flush().expect("Hook payload flushes");
    drop(stdin);

    let mut stdout = child.stdout.take().expect("Hook parent exposes stdout");
    let mut stderr = child.stderr.take().expect("Hook parent exposes stderr");
    let (sender, receiver) = mpsc::channel();
    let stdout_sender = sender.clone();
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout.read_to_end(&mut bytes).map(|_| bytes);
        let _ = stdout_sender.send(HookStream::Stdout(result));
    });
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stderr.read_to_end(&mut bytes).map(|_| bytes);
        let _ = sender.send(HookStream::Stderr(result));
    });

    let deadline = Instant::now() + STAGE_TIMEOUT;
    let status = loop {
        match child.try_wait().expect("Hook root remains observable") {
            Some(status) => break status,
            None if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            None => {
                terminate_owned_tree(process_id);
                panic!("Hook root stage {stage} exceeded the bounded fixture timeout");
            }
        }
    };
    assert!(
        status.success(),
        "Hook root stage {stage} exits successfully"
    );

    let mut stdout_eof = false;
    let mut stderr_eof = false;
    let mut stdout_bytes = Vec::new();
    let mut stderr_bytes = Vec::new();
    while !(stdout_eof && stderr_eof) {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let stream = receiver.recv_timeout(remaining).unwrap_or_else(|_| {
            terminate_owned_tree(process_id);
            panic!("Hook stage {stage} left a redirected stream open after root exit")
        });
        match stream {
            HookStream::Stdout(Ok(bytes)) => {
                stdout_eof = true;
                stdout_bytes = bytes;
            }
            HookStream::Stderr(Ok(bytes)) => {
                stderr_eof = true;
                stderr_bytes = bytes;
            }
            HookStream::Stdout(Err(error)) | HookStream::Stderr(Err(error)) => {
                panic!("Hook stage {stage} stream collection failed: {error}")
            }
        }
    }
    assert!(
        stdout_bytes.is_empty(),
        "Hook stage {stage} keeps stdout silent"
    );
    assert!(
        stderr_bytes.is_empty(),
        "Hook stage {stage} keeps stderr silent"
    );
}

fn run_hook(
    executable: &Path,
    payload: &[u8],
    local_app_data: &Path,
    terminal_session: &str,
    probe_receipt: &Path,
    stage: &str,
) {
    let child = Command::new(executable)
        .args(["hook", "codex"])
        .env("LOCALAPPDATA", local_app_data)
        .env("WT_SESSION", terminal_session)
        .env(ACTIVITY_WORKER_PROBE_RECEIPT_ENV, probe_receipt)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Hook root starts");
    wait_for_root(child, payload, stage);
}

fn wait_for_file(path: &Path) {
    let deadline = Instant::now() + STAGE_TIMEOUT;
    while !path.is_file() {
        assert!(
            Instant::now() < deadline,
            "missing probe receipt: {}",
            path.display()
        );
        thread::sleep(Duration::from_millis(10));
    }
}

fn assert_stdio_classes(path: &Path, role: &str) {
    let deadline = Instant::now() + STAGE_TIMEOUT;
    let receipt: Value = loop {
        if let Ok(bytes) = fs::read(path)
            && let Ok(receipt) = serde_json::from_slice(&bytes)
        {
            break receipt;
        }
        assert!(
            Instant::now() < deadline,
            "probe receipt did not become complete JSON"
        );
        thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(receipt["role"], role);
    for stream in ["stdin_class", "stdout_class", "stderr_class"] {
        assert_eq!(receipt[stream], "CHAR", "{role} {stream} is NUL-backed");
    }
}

fn hook_payload(event: &str, session: &str, turn: Option<&str>, cwd: &Path) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "hook_event_name": event,
        "session_id": session,
        "turn_id": turn,
        "cwd": cwd,
        "model": "gpt-test",
        "permission_mode": "default",
        "transcript_path": null,
    }))
    .expect("Hook payload serializes")
}

fn prewarm_worker_runtime(local_app_data: &Path, executable: &Path) {
    WorkerRuntimeStore::new(local_app_data.join("TabBeacon").join("repository-identity"))
        .publish(executable)
        .expect("setup-prewarmed immutable worker image publishes");
}

#[test]
fn worker_and_observer_exclude_ambient_handles_and_release_runner_streams() {
    let root = TestRoot::new();
    let executable = PathBuf::from(env!("CARGO_BIN_EXE_tabbeacon"));
    let workspace = root.child("w");
    let local_app_data = root.child("l");
    fs::create_dir_all(&workspace).expect("workspace is created");
    assert!(
        Command::new("git")
            .args(["init", "--quiet"])
            .arg(&workspace)
            .status()
            .is_ok_and(|status| status.success()),
        "workspace Git fixture initializes"
    );
    let settings_root = local_app_data.join("TabBeacon");
    fs::create_dir_all(&settings_root).expect("settings root is created");
    fs::write(
        settings_root.join("config.toml"),
        concat!(
            "[presentation]\n",
            "title = \"tabbeacon\"\n",
            "tab_color = \"off\"\n",
            "activity = \"title-spinner\"\n",
            "spinner = \"braille\"\n",
            "theme = \"muted-dark\"\n",
            "provider_badge = \"always\"\n",
        ),
    )
    .expect("isolated settings are written");
    prewarm_worker_runtime(&local_app_data, &executable);
    let terminal_session = "00000000-0000-0000-0000-000000000063";
    let probe_receipt = local_app_data.join(ACTIVITY_WORKER_PROBE_RECEIPT_FILE);

    run_hook(
        &executable,
        &hook_payload("SessionStart", "pipe-session", None, &workspace),
        &local_app_data,
        terminal_session,
        &probe_receipt,
        "SessionStart",
    );

    let mut sentinels = [
        InheritableSentinelPipe::new(),
        InheritableSentinelPipe::new(),
        InheritableSentinelPipe::new(),
    ];
    run_hook(
        &executable,
        b"malformed",
        &local_app_data,
        terminal_session,
        &probe_receipt,
        "malformed control",
    );
    run_hook(
        &executable,
        &hook_payload(
            "UserPromptSubmit",
            "pipe-session",
            Some("pipe-turn"),
            &workspace,
        ),
        &local_app_data,
        terminal_session,
        &probe_receipt,
        "UserPromptSubmit",
    );

    let worker_receipt = local_app_data.join(ACTIVITY_WORKER_PROBE_PROCESS_FILE);
    let observer_receipt = local_app_data.join(ACTIVITY_OBSERVER_PROBE_PROCESS_FILE);
    wait_for_file(&worker_receipt);
    wait_for_file(&observer_receipt);
    assert_stdio_classes(&worker_receipt, "worker");
    assert_stdio_classes(&observer_receipt, "observer");

    for sentinel in &mut sentinels {
        sentinel.close_writer();
    }
    for (index, sentinel) in sentinels.iter().enumerate() {
        assert!(
            sentinel.reaches_eof_within(Duration::from_secs(2)),
            "ambient sentinel pipe {index} remains open"
        );
    }

    run_hook(
        &executable,
        &hook_payload("SessionEnd", "pipe-session", None, &workspace),
        &local_app_data,
        terminal_session,
        &probe_receipt,
        "SessionEnd",
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn root_stop_publishes_a_bounded_static_result_ready_worker() {
    let root = TestRoot::new();
    let executable = PathBuf::from(env!("CARGO_BIN_EXE_tabbeacon"));
    let workspace = root.child("w");
    let local_app_data = root.child("l");
    fs::create_dir_all(&workspace).expect("workspace is created");
    assert!(
        Command::new("git")
            .args(["init", "--quiet"])
            .arg(&workspace)
            .status()
            .is_ok_and(|status| status.success()),
        "workspace Git fixture initializes"
    );
    let settings_root = local_app_data.join("TabBeacon");
    fs::create_dir_all(&settings_root).expect("settings root is created");
    fs::write(
        settings_root.join("config.toml"),
        concat!(
            "[presentation]\n",
            "title = \"tabbeacon\"\n",
            "tab_color = \"off\"\n",
            "activity = \"title-spinner\"\n",
            "spinner = \"braille\"\n",
            "theme = \"muted-dark\"\n",
            "provider_badge = \"always\"\n",
        ),
    )
    .expect("isolated settings are written");
    prewarm_worker_runtime(&local_app_data, &executable);
    let terminal_session = "00000000-0000-0000-0000-000000000072";
    let probe_receipt = local_app_data.join(ACTIVITY_WORKER_PROBE_RECEIPT_FILE);

    run_hook(
        &executable,
        &hook_payload("SessionStart", "result-session", None, &workspace),
        &local_app_data,
        terminal_session,
        &probe_receipt,
        "SessionStart",
    );
    run_hook(
        &executable,
        &hook_payload(
            "UserPromptSubmit",
            "result-session",
            Some("result-turn"),
            &workspace,
        ),
        &local_app_data,
        terminal_session,
        &probe_receipt,
        "UserPromptSubmit",
    );
    wait_for_file(&local_app_data.join(ACTIVITY_WORKER_PROBE_PROCESS_FILE));
    wait_for_file(&local_app_data.join(ACTIVITY_OBSERVER_PROBE_PROCESS_FILE));
    wait_for_file(&local_app_data.join(ACTIVITY_WORKER_PROBE_STARTED_FILE));
    for marker in [
        ACTIVITY_WORKER_PROBE_PROCESS_FILE,
        ACTIVITY_OBSERVER_PROBE_PROCESS_FILE,
        ACTIVITY_WORKER_PROBE_STARTED_FILE,
        ACTIVITY_WORKER_PROBE_RECEIPT_FILE,
    ] {
        let path = local_app_data.join(marker);
        if path.exists() {
            fs::remove_file(path).expect("owned working-worker marker removes");
        }
    }
    run_hook(
        &executable,
        &hook_payload("Stop", "result-session", Some("result-turn"), &workspace),
        &local_app_data,
        terminal_session,
        &probe_receipt,
        "Stop",
    );

    wait_for_file(&local_app_data.join(ACTIVITY_WORKER_PROBE_RESULT_READY_FILE));
    for marker in [
        ACTIVITY_WORKER_PROBE_PROCESS_FILE,
        ACTIVITY_OBSERVER_PROBE_PROCESS_FILE,
        ACTIVITY_WORKER_PROBE_STARTED_FILE,
    ] {
        assert!(
            !local_app_data.join(marker).exists(),
            "ResultReady handoff must reuse the existing worker, marker={marker}"
        );
    }
    let sessions = Command::new(&executable)
        .args(["sessions", "--json"])
        .env("LOCALAPPDATA", &local_app_data)
        .output()
        .expect("isolated sessions inspection starts");
    assert!(sessions.status.success());
    let sessions: Value = serde_json::from_slice(&sessions.stdout).expect("sessions JSON parses");
    assert_eq!(sessions["active_sessions"], 1);
    let rows = sessions["sessions"].as_array().expect("session rows");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["semantic_state"], "result-ready");

    run_hook(
        &executable,
        &hook_payload("SessionEnd", "result-session", None, &workspace),
        &local_app_data,
        terminal_session,
        &probe_receipt,
        "SessionEnd",
    );
    let sessions = Command::new(executable)
        .args(["sessions", "--json"])
        .env("LOCALAPPDATA", &local_app_data)
        .output()
        .expect("post-cleanup sessions inspection starts");
    assert!(sessions.status.success());
    let sessions: Value =
        serde_json::from_slice(&sessions.stdout).expect("post-cleanup sessions JSON parses");
    assert_eq!(sessions["active_sessions"], 0);
}
