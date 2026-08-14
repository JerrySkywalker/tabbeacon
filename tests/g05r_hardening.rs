use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::UNIX_EPOCH;

use serde_json::{Value, json};
use tabbeacon::providers::codex::{CodexHookRuntime, HookDispatchOutcome};

static NEXT_ROOT: AtomicU64 = AtomicU64::new(0);

struct LabRoot {
    path: PathBuf,
}

impl LabRoot {
    fn new(name: &str) -> Self {
        let sequence = NEXT_ROOT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "tabbeacon-g05r-{name}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("G05R test root is created");
        Self { path }
    }

    fn child(&self, relative: &str) -> PathBuf {
        self.path.join(relative)
    }
}

impl Drop for LabRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn git(cwd: &Path, arguments: &[&str]) {
    let output = Command::new("git")
        .current_dir(cwd)
        .args(arguments)
        .output()
        .expect("Git starts for G05R fixture");
    assert!(
        output.status.success(),
        "git {arguments:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn init_repo(path: &Path, remote: &str) {
    fs::create_dir_all(path).expect("repository directory is created");
    git(path, &["init", "--quiet"]);
    git(path, &["remote", "add", "origin", remote]);
}

fn payload(event: &str, session: &str, cwd: &Path) -> Value {
    let mut value = json!({
        "hook_event_name": event,
        "session_id": session,
        "cwd": cwd,
        "transcript_path": "ignored-control-\u{1b}]0;not-a-title\u{7}",
    });
    if event == "SessionStart" {
        value["source"] = Value::String("startup".to_string());
    }
    value
}

fn dispatch(runtime: &CodexHookRuntime, value: &Value) -> (HookDispatchOutcome, String) {
    let mut output = Vec::new();
    let outcome = runtime.dispatch_to(
        &serde_json::to_vec(value).expect("payload serializes"),
        UNIX_EPOCH,
        &mut output,
    );
    (
        outcome,
        String::from_utf8(output).expect("presentation bytes are UTF-8"),
    )
}

fn title_prefix(rendered: &str) -> String {
    let start = rendered.find("\u{1b}]0;").expect("title OSC is present") + 4;
    let end = rendered[start..]
        .find("\u{1b}\\")
        .map(|offset| start + offset)
        .expect("title OSC terminator is present");
    rendered[start..end]
        .split_once(' ')
        .expect("title contains alias and state")
        .0
        .to_string()
}

#[test]
fn concurrent_sessions_and_collision_repositories_remain_isolated() {
    let root = LabRoot::new("multi-session");
    let established = root.child("jerry-proxy-control");
    let newcomer = root.child("java-platform-core");
    init_repo(
        &established,
        "https://example.invalid/team/jerry-proxy-control.git",
    );
    init_repo(&newcomer, "git@example.invalid:team/java-platform-core.git");

    let runtime = CodexHookRuntime::new(root.child("state"), true);
    let (outcome, established_render) = dispatch(
        &runtime,
        &payload("UserPromptSubmit", "established", &established),
    );
    assert_eq!(outcome, HookDispatchOutcome::Applied);
    let established_alias = title_prefix(&established_render);
    assert_eq!(established_alias, "JPC");

    let (outcome, newcomer_render) = dispatch(
        &runtime,
        &payload("UserPromptSubmit", "newcomer", &newcomer),
    );
    assert_eq!(outcome, HookDispatchOutcome::Applied);
    let newcomer_alias = title_prefix(&newcomer_render);
    assert_ne!(newcomer_alias, established_alias);

    let barrier = Arc::new(Barrier::new(17));
    let mut workers = Vec::new();
    for index in 0..16 {
        let runtime = runtime.clone();
        let barrier = Arc::clone(&barrier);
        let repository = if index % 2 == 0 {
            established.clone()
        } else {
            newcomer.clone()
        };
        let expected_alias = if index % 2 == 0 {
            established_alias.clone()
        } else {
            newcomer_alias.clone()
        };
        workers.push(thread::spawn(move || {
            let (event, suffix) = match index % 4 {
                0 => ("UserPromptSubmit", "working"),
                1 => ("PermissionRequest", "approval"),
                2 => ("Stop", "result-ready"),
                _ => ("SessionEnd", "reset"),
            };
            barrier.wait();
            let session = format!("session-{index}");
            let (outcome, rendered) = dispatch(&runtime, &payload(event, &session, &repository));
            (outcome, rendered, expected_alias, suffix, session)
        }));
    }
    barrier.wait();

    for worker in workers {
        let (outcome, rendered, alias, suffix, session) =
            worker.join().expect("G05R worker does not panic");
        assert_eq!(outcome, HookDispatchOutcome::Applied);
        assert!(rendered.contains(&format!("]0;{alias} {suffix}")));
        assert!(
            !rendered.contains(&session),
            "provider session identifiers must not leak into presentation"
        );
        assert!(rendered.contains("\u{1b}]9;4;"));
    }
}

#[test]
fn rapid_state_chains_do_not_cross_contaminate_sessions() {
    let root = LabRoot::new("rapid-chain");
    let repository = root.child("workstation-manager");
    init_repo(
        &repository,
        "https://example.invalid/team/workstation-manager.git",
    );
    let runtime = CodexHookRuntime::new(root.child("state"), true);

    let cases = [
        ("UserPromptSubmit", "working"),
        ("PermissionRequest", "approval"),
        ("PreToolUse", "working"),
        ("Stop", "result-ready"),
        ("SessionEnd", "reset"),
    ];
    for (event, suffix) in cases {
        let (outcome, rendered) = dispatch(&runtime, &payload(event, "session-a", &repository));
        assert_eq!(outcome, HookDispatchOutcome::Applied);
        assert!(rendered.contains(&format!("]0;WM {suffix}")));
    }

    let (outcome, rendered) = dispatch(
        &runtime,
        &payload("UserPromptSubmit", "session-b", &repository),
    );
    assert_eq!(outcome, HookDispatchOutcome::Applied);
    assert!(rendered.contains("]0;WM working"));
    assert!(!rendered.contains("result-ready"));
}

#[test]
fn unavailable_or_corrupt_registry_state_is_contained() {
    let root = LabRoot::new("state-failure");
    let repository = root.child("repo");
    init_repo(
        &repository,
        "https://example.invalid/team/workstation-manager.git",
    );

    let state_root_file = root.child("state-as-file");
    fs::write(&state_root_file, b"not a directory").expect("state-root conflict is written");
    let runtime = CodexHookRuntime::new(&state_root_file, true);
    let (outcome, rendered) = dispatch(
        &runtime,
        &payload("UserPromptSubmit", "session-a", &repository),
    );
    assert_eq!(outcome, HookDispatchOutcome::DegradedRepositoryIdentity);
    assert!(rendered.is_empty());

    let corrupt_state = root.child("corrupt-state");
    fs::create_dir_all(&corrupt_state).expect("corrupt state root is created");
    fs::write(corrupt_state.join("registry-v1.json"), b"{partial")
        .expect("corrupt registry fixture is written");
    let runtime = CodexHookRuntime::new(corrupt_state, true);
    let (outcome, _rendered) = dispatch(
        &runtime,
        &payload("UserPromptSubmit", "session-b", &repository),
    );
    assert!(matches!(
        outcome,
        HookDispatchOutcome::Applied | HookDispatchOutcome::DegradedRepositoryIdentity
    ));
}
