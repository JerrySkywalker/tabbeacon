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

fn repository_source(path: &str) -> String {
    std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path))
        .expect("repository source reads")
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
    assert!(
        script.contains("function Get-TrackedProcessTreeState")
            && script.contains("function Get-ProcessIdentityState")
            && script.contains("State = 'unknown'"),
        "cleanup must distinguish unknown process state from proven completion"
    );
    assert!(
        script.contains("Get-CimInstance Win32_Process")
            && script.contains("return 'unknown'")
            && script.contains("TREE_ENUMERATION_PROVEN="),
        "descendant enumeration failures must be durable unproven evidence"
    );
    assert!(
        script.contains("WATCHDOG_EXPIRED=")
            && script.contains("$passed = -not $watchdogExpired -and $identityQueriesProven")
            && script.contains("$ownedTreeTracked -and $launcherCompleted"),
        "an expired watchdog, unknown identity, or unfinished launcher/tree cannot produce PASS"
    );
}

#[test]
fn fixture_profile_probe_uses_the_same_version_contract_as_the_codex_adapter() {
    let fixture = repository_source("src/bin/tabbeacon-terminal-smoke-fixture.rs");
    let adapter = repository_source("src/providers/codex/config.rs");
    assert!(
        fixture.contains("const FIXTURE_CODEX_VERSION_PROBE_ARGUMENT: &str = \"--version\""),
        "the fixture must recognize the adapter's version argument"
    );
    assert!(
        fixture.contains(".with_codex_program(executable)"),
        "the real adapter proof must use the owned fixture executable"
    );
    assert!(
        adapter.contains(".arg(\"--version\")"),
        "the checked Codex adapter contract must remain explicit"
    );
}
