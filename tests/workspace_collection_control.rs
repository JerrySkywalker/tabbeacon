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
        let result = resolver.inspect_alias_controlled(&self.0, Some(control));
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
    std::fs::write(root.join(format!("started-{operation}")), b"fixture").unwrap();
    let stalled = std::env::var("TB_FIXTURE_GIT_STAGE").unwrap();
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

fn owned_workspace_check(root: &Path, stage: &str, deadline: bool) {
    assert!(root.join("OWNED-WORKSPACE-CONTROL-FIXTURE").is_file());
    let mut runtime = RefreshController::spawn(
        WorkspaceCollector(root.to_path_buf()),
        RefreshPolicy {
            task_timeout: Duration::from_millis(if deadline { 700 } else { 12000 }),
            poll_interval: Duration::from_secs(1),
            stale_after: Duration::from_secs(15),
        },
    )
    .unwrap();
    runtime.request((), RequestKind::Manual).unwrap();
    let start = Instant::now();
    while !root.join(format!("started-{stage}")).exists() {
        assert!(
            start.elapsed() < Duration::from_secs(3),
            "Git stage never started"
        );
        thread::sleep(Duration::from_millis(2));
    }
    if deadline {
        loop {
            match runtime.poll() {
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

fn regression(stage: &str, deadline: bool) {
    let root = tempfile::Builder::new()
        .prefix("tabbeacon-workspace-control-")
        .tempdir()
        .unwrap();
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
    std::fs::copy(
        &executable,
        bin.join(if cfg!(windows) { "git.exe" } else { "git" }),
    )
    .unwrap();
    let mut paths = vec![bin];
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    let status = Command::new(&executable)
        .arg("--owned-workspace-check")
        .arg(root.path())
        .arg(stage)
        .arg(deadline.to_string())
        .env("PATH", std::env::join_paths(paths).unwrap())
        .env("TB_FIXTURE_GIT_STAGE", stage)
        .status()
        .unwrap();
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
        for deadline in [false, true] {
            for stage in ["rev-parse", "config", "rev-list"] {
                regression(stage, deadline);
            }
        }
        println!("test result: ok. 6 owned production-chain cases passed");
    }
}
