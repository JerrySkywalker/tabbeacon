//! Static regression contract for the real Windows Terminal smoke watchdog.
//!
//! The interactive run is intentionally performed only by the admitted visual
//! gate. This test keeps the two safety properties which previously regressed
//! visible to ordinary CI: launch is asynchronous under a pre-existing outer
//! deadline, and forceful cleanup is limited to the verified owned child tree.

use std::path::PathBuf;

fn smoke_script() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("scripts")
        .join("run-windows-terminal-tui-smoke.ps1");
    std::fs::read_to_string(path).expect("real Windows Terminal smoke script reads")
}

#[test]
fn smoke_launch_and_cleanup_are_watchdog_bounded_and_owner_correlated() {
    let script = smoke_script();
    let deadline = script
        .find("$deadline = [DateTimeOffset]::UtcNow.AddSeconds($TimeoutSeconds)")
        .expect("watchdog deadline exists");
    let asynchronous_launch = script
        .find("$wtLauncher = Start-Process -FilePath $wtCommand.Source")
        .expect("Windows Terminal launch is asynchronous");
    assert!(
        deadline < asynchronous_launch,
        "the outer watchdog must begin before Windows Terminal launch"
    );
    assert!(
        !script.contains("& $wtCommand.Source @arguments"),
        "the harness must not synchronously wait for wt.exe"
    );
    assert!(
        script.contains("function Stop-OwnedProcessTree"),
        "a bounded owned-tree termination helper is required"
    );
    assert!(
        script.contains("$windowChildLineageBound -and $null -ne $childProcessId"),
        "forceful cleanup must require a verified owned terminal-child lineage"
    );
    assert!(
        script.contains("OWNED_CHILD_TREE_TERMINATION_SUCCEEDED="),
        "cleanup disposition must be recorded in durable evidence"
    );
}
