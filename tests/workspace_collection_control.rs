//! Production Workspace cancellation/deadline regression, with no ambient Git.
//! The parent never changes PATH; only its owned child uses a temporary Git fixture.
use std::{
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, Instant, SystemTime},
};
use tabbeacon::repo::WorkspaceIdentityResolver;
use terminal_ui_runtime::{
    refresh::{
        CollectError, CollectionControl, ReadOnlyCollector, RefreshController, RefreshPolicy,
        RefreshPoll, RefreshRequest, RequestKind,
    },
    snapshot::{Collected, Freshness, SnapshotMetadata},
};

struct WorkspaceCollector(PathBuf);
impl ReadOnlyCollector<(), u64> for WorkspaceCollector {
    fn collect(
        &mut self,
        _: &RefreshRequest<()>,
        control: &CollectionControl,
    ) -> Result<Collected<u64>, CollectError> {
        let resolver = WorkspaceIdentityResolver::with_home_directory(self.0.join("state"), None);
        if std::env::var("TB_FIXTURE_MODE").as_deref() == Ok("before-stage") {
            std::fs::write(self.0.join("collector-entered"), b"fixture").unwrap();
            // Explicitly held gate: real clock and dispatch deadline continue running.
            while !self.0.join("release-collector").exists() {
                if control.check().is_err() {
                    break;
                }
                thread::yield_now();
            }
        }
        let result = resolver.inspect_alias_controlled(&self.0, Some(control));
        println!(
            "DISCOVERY_RETURNED_OK={} IDENTITY_CLASS={}",
            result.is_ok(),
            result
                .as_ref()
                .map_or("unavailable", |inspection| inspection
                    .identity_class()
                    .as_str())
        );
        control.check()?;
        result.map_err(|_| CollectError::Failed("fixture workspace unavailable".into()))?;
        Ok(Collected {
            value: 1,
            empty: false,
            metadata: SnapshotMetadata {
                source: "synthetic Git".into(),
                scope: "owned fixture".into(),
                sampled_at: SystemTime::UNIX_EPOCH,
                freshness: Freshness::Unknown,
            },
        })
    }
}

fn synthetic_git(root: &Path, operation: &str) {
    assert!(root.join("OWNED-WORKSPACE-CONTROL-FIXTURE").is_file());
    std::fs::write(
        root.join(format!("child-{operation}")),
        std::process::id().to_string(),
    )
    .unwrap();
    if std::env::var("TB_FIXTURE_MODE").as_deref() == Ok("marker-failure") {
        std::process::exit(42);
    }
    std::fs::write(root.join(format!("started-{operation}")), b"fixture").unwrap();
    let stalled = std::env::var("TB_FIXTURE_GIT_STAGE").unwrap();
    if operation == stalled && std::env::var("TB_FIXTURE_MODE").as_deref() == Ok("in-stage") {
        // A controlled stage gate, separately reported from the original 1600ms stall.
        while !root.join("release-stage").exists() {
            thread::yield_now();
        }
    }
    thread::sleep(Duration::from_millis(if operation == stalled {
        1600
    } else {
        5
    }));
    std::fs::write(root.join(format!("completed-{operation}")), b"fixture").unwrap();
    match operation {
        "rev-parse" => println!(
            "{}\n{}\n{}",
            root.display(),
            root.join(".git").display(),
            root.join(".git").display()
        ),
        "config" => std::process::exit(1),
        "rev-list" => println!("{}", "a".repeat(40)),
        _ => panic!("unadmitted synthetic Git operation"),
    }
}

#[cfg(windows)]
#[allow(unsafe_code)] // Observe only the PID supplied by this owned fixture, never signal it.
fn child_alive(pid: u32) -> bool {
    use windows::Win32::{
        Foundation::CloseHandle,
        System::Threading::{GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
    };
    unsafe {
        let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return false;
        };
        let mut code = 0;
        let alive = GetExitCodeProcess(handle, &raw mut code).is_ok() && code == 259;
        let _ = CloseHandle(handle);
        alive
    }
}

#[cfg(not(windows))]
fn child_alive(pid: u32) -> bool {
    Path::new("/proc").join(pid.to_string()).exists()
}

fn assert_early_result(
    root: &Path,
    mode: &str,
    result: &RefreshPoll,
    observed_child: Option<(u32, bool)>,
) {
    let classification = match (result, observed_child) {
        (RefreshPoll::Accepted { .. }, Some(_)) if mode == "marker-failure" => {
            "FALLBACK_ACCEPTED_AFTER_MARKER_FAILURE"
        }
        (RefreshPoll::Accepted { .. }, None) if mode == "start-failure" => {
            "FALLBACK_ACCEPTED_AFTER_PROCESS_START_FAILURE"
        }
        (RefreshPoll::Failed(CollectError::Timeout), None) => "BEFORE_STAGE_TIMEOUT",
        (RefreshPoll::Failed(CollectError::Timeout), Some(_)) => {
            "PROCESS_ENTERED_WITHOUT_MARKER_TIMEOUT"
        }
        (RefreshPoll::Failed(_), Some(_)) => "MARKER_FAILURE",
        (RefreshPoll::Failed(_), None) => "PROCESS_START_OR_DISCOVERY_FAILURE",
        _ => "UNEXPECTED_SUCCESS_OR_TASK_RESULT",
    };
    println!("DIAGNOSTIC={classification} MODE={mode}");
    match mode {
        "before-stage" => {
            assert_eq!(*result, RefreshPoll::Failed(CollectError::Timeout));
            assert!(root.join("collector-entered").exists());
            assert!(observed_child.is_none());
        }
        "marker-failure" => {
            assert!(observed_child.is_some());
            assert!(!observed_child.unwrap().1);
            println!(
                "MARKER_FAILURE=PROVEN CHILD_EXIT_REQUESTED=42 WORKSPACE_DIRECTORY_FALLBACK={}",
                matches!(result, RefreshPoll::Accepted { .. })
            );
        }
        "start-failure" => {
            assert!(observed_child.is_none());
            assert!(!root.join("bin/git.exe").exists() && !root.join("bin/git").exists());
            println!(
                "PROCESS_START_FAILURE=CONTROLLED_MISSING_EXECUTABLE WORKSPACE_DIRECTORY_FALLBACK={}",
                matches!(result, RefreshPoll::Accepted { .. })
            );
        }
        _ => {
            panic!("Original in-stage qualification did not reach its target: {classification}")
        }
    }
}

fn owned_workspace_check(root: &Path, stage: &str, deadline: bool) {
    assert!(root.join("OWNED-WORKSPACE-CONTROL-FIXTURE").is_file());
    let mode = std::env::var("TB_FIXTURE_MODE").unwrap_or_else(|_| "original".into());
    let task_ms = if !deadline {
        12000
    } else if mode == "in-stage" {
        2000
    } else {
        700
    };
    println!(
        "MODE={mode} TASK_BUDGET_MS={task_ms} ORIGINAL_EQUIVALENCE={}",
        mode == "original"
    );
    let mut runtime = RefreshController::spawn(
        WorkspaceCollector(root.to_path_buf()),
        RefreshPolicy {
            task_timeout: Duration::from_millis(task_ms),
            poll_interval: Duration::from_secs(1),
            stale_after: Duration::from_secs(15),
        },
    )
    .unwrap();
    runtime.request((), RequestKind::Manual).unwrap();
    let start = Instant::now();
    let mut early_result = None;
    let mut stage_result = None;
    let mut observed_child = None;
    while !root.join(format!("started-{stage}")).exists() {
        if let Ok(text) = std::fs::read_to_string(root.join(format!("child-{stage}"))) {
            let pid = text.parse().unwrap();
            observed_child = Some((pid, child_alive(pid)));
        }
        match runtime.poll() {
            RefreshPoll::Pending => {}
            result => {
                // The child may enter between the observation and terminal poll.
                // A completed task causally follows its child cleanup; observe again.
                if let Ok(text) = std::fs::read_to_string(root.join(format!("child-{stage}"))) {
                    let pid = text.parse().unwrap();
                    observed_child = Some((pid, child_alive(pid)));
                }
                let marker_after_result = root.join(format!("started-{stage}")).exists();
                println!(
                    "MARKER_WAIT_RESULT={result:?} OWNED_CHILD_FINAL={observed_child:?} TARGET_MARKER_AFTER_RESULT={marker_after_result} ELAPSED_MS={}",
                    start.elapsed().as_millis()
                );
                if marker_after_result {
                    stage_result = Some(result);
                } else {
                    early_result = Some(result);
                }
                break;
            }
        }
        assert!(
            start.elapsed() < Duration::from_secs(3),
            "MARKER_WAIT_EXPIRED: owned_child={observed_child:?}; runtime still pending"
        );
        thread::sleep(Duration::from_millis(2));
    }
    if let Some(result) = early_result {
        assert_early_result(root, &mode, &result, observed_child);
    } else if deadline {
        println!("TARGET_STAGE_STARTED=true MODE={mode}");
        loop {
            match stage_result.take().unwrap_or_else(|| runtime.poll()) {
                RefreshPoll::Pending => {
                    assert!(start.elapsed() < Duration::from_secs(3));
                    thread::sleep(Duration::from_millis(2));
                }
                RefreshPoll::Failed(error) => {
                    assert_eq!(error, CollectError::Timeout);
                    break;
                }
                other => panic!("unexpected task result: {other:?}"),
            }
        }
    }
    runtime.shutdown(Duration::from_secs(1)).unwrap();
    assert!(!runtime.worker_owned());
    drop(runtime);
    for operation in ["rev-parse", "config", "rev-list"] {
        if let Ok(text) = std::fs::read_to_string(root.join(format!("child-{operation}"))) {
            assert!(
                !child_alive(text.parse().unwrap()),
                "owned Git child remains alive"
            );
        }
    }
    assert!(!root.join(format!("completed-{stage}")).exists());
    let later = match stage {
        "rev-parse" => &["config", "rev-list"][..],
        "config" => &["rev-list"][..],
        _ => &[][..],
    };
    for next in later {
        assert!(!root.join(format!("started-{next}")).exists());
    }
    assert!(!root.join("state").exists());
    println!(
        "STAGE={stage} DEADLINE={deadline} GIT_COMMANDS_AFTER_CANCEL=false COOPERATIVE_WORKER_JOIN=PASS"
    );
}

fn regression(stage: &str, deadline: bool, mode: &str) {
    let mut builder = tempfile::Builder::new();
    builder.prefix("tabbeacon-workspace-control-");
    let root = if let Some(owned) = std::env::var_os("TB_ENGINEERING_ARTIFACT_ROOT") {
        let stores = PathBuf::from(owned).join("deadline-stores");
        std::fs::create_dir_all(&stores).unwrap();
        builder.tempdir_in(stores).unwrap()
    } else {
        builder.tempdir().unwrap()
    };
    std::fs::write(
        root.path().join("OWNED-WORKSPACE-CONTROL-FIXTURE"),
        b"fixture",
    )
    .unwrap();
    std::fs::create_dir(root.path().join(".git")).unwrap();
    std::fs::write(
        root.path().join(".git/config"),
        b"[include]\npath=unused-fixture\n",
    )
    .unwrap();
    let bin = root.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let executable = std::env::current_exe().unwrap();
    if mode != "start-failure" {
        std::fs::copy(
            &executable,
            bin.join(if cfg!(windows) { "git.exe" } else { "git" }),
        )
        .unwrap();
    }
    let mut paths = vec![bin];
    if mode != "start-failure" {
        paths.extend(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ));
    }
    let mut child = Command::new(&executable)
        .arg("--owned-workspace-check")
        .arg(root.path())
        .arg(stage)
        .arg(deadline.to_string())
        .env("PATH", std::env::join_paths(paths).unwrap())
        .env("TB_FIXTURE_GIT_STAGE", stage)
        .env("TB_FIXTURE_MODE", mode)
        .env("TEMP", root.path())
        .env("TMP", root.path())
        .spawn()
        .unwrap();
    let parent_start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if parent_start.elapsed() > Duration::from_secs(15) {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("owned workspace harness timed out; child reaped");
        }
        thread::sleep(Duration::from_millis(2));
    };
    assert!(status.success(), "{stage}/deadline={deadline}");
}

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "-C") {
        synthetic_git(Path::new(&args[1]), &args[2]);
    } else if args
        .first()
        .is_some_and(|arg| arg == "--owned-workspace-check")
    {
        owned_workspace_check(Path::new(&args[1]), &args[2], args[3].parse().unwrap());
    } else if args.iter().any(|arg| arg == "--list") {
        println!("workspace_cancellation_and_deadline: test");
    } else {
        let diagnostic_only = args.iter().any(|arg| arg == "--diagnostic-only");
        let original_only = args.iter().any(|arg| arg == "--original-only");
        assert!(!(diagnostic_only && original_only));
        if !original_only {
            regression("rev-parse", true, "before-stage");
            regression("rev-parse", true, "in-stage");
            regression("rev-parse", false, "in-stage");
            regression("rev-parse", false, "marker-failure");
            regression("rev-parse", false, "start-failure");
            println!(
                "DIAGNOSTIC_FAMILY=PASS CASES=5 CONTROLLED_IN_STAGE_TASK_MS=2000 NOT_ORIGINAL_700MS_QUALIFICATION=true"
            );
        }
        if !diagnostic_only {
            for deadline in [false, true] {
                for stage in ["rev-parse", "config", "rev-list"] {
                    regression(stage, deadline, "original");
                }
            }
            println!("ORIGINAL_BUDGET_FAMILY=PASS CASES=6");
        }
        println!(
            "test result: ok. {} owned production-chain cases passed",
            usize::from(!diagnostic_only) * 6 + usize::from(!original_only) * 5
        );
    }
}
