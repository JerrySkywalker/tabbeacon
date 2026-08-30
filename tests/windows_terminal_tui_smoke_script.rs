//! Static regression contract for the real Windows Terminal smoke watchdog.
//!
//! The interactive run is intentionally performed only by the admitted visual
//! gate. This test keeps the two safety properties which previously regressed
//! visible to ordinary CI: launch is asynchronous under a pre-existing outer
//! deadline, and a durable child receipt—not a process query or cleanup
//! observation—decides the real-terminal verdict.

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
fn smoke_completion_is_durable_bounded_and_owner_correlated() {
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
        script.contains("COMPLETION_RECEIPT_TOKEN_BOUND=")
            && script.contains("COMPLETION_RECEIPT_HEAD_BOUND=")
            && script.contains("COMPLETION_RECEIPT_BINARY_BOUND=")
            && script.contains("DURABLE_COMPLETION_PROVEN=")
            && script.contains("PROCESS_QUERY_DEPENDENCY=none"),
        "the child completion receipt must bind the owned run, candidate, and binary"
    );
    assert!(
        script.contains("__temporary-wt-register-v1")
            && script.contains("__temporary-wt-cleanup-v1")
            && script.contains("TEMP_WT_CLEANUP=")
            && script.contains("OWNER_WINDOWS_CLOSED=")
            && script.contains("BROAD_WINDOW_KILL_USED="),
        "the exact-owned temporary window must register and clean up on a separate receipt lane"
    );
    assert!(
        script.contains("WaitForSingleObject")
            && script.contains("RESIDUAL_OWNED_PROCESS_OBSERVATION=")
            && script.contains("if ($durableCompletionProven)"),
        "residual process observation must be bounded and strictly secondary"
    );
    assert!(
        script.contains("WATCHDOG_EXPIRED=")
            && script.contains("$passed = $durableCompletionProven -and")
            && script.contains("VISUAL_OPERATION_DISPOSITION=$visualDisposition"),
        "an expired watchdog or mismatched durable receipt cannot produce PASS"
    );
    for forbidden in [
        "Get-CimInstance",
        "function Invoke-BoundedProcessQuery",
        "function Stop-OwnedProcessTree",
        "PostMessage",
        "taskkill.exe",
    ] {
        assert!(
            !script.contains(forbidden),
            "completion must not depend on legacy process-query or termination logic: {forbidden}"
        );
    }
    let child = repository_source("scripts/invoke-windows-terminal-tui-smoke-child.ps1");
    for required in [
        "COMPLETION_SCHEMA=tabbeacon-wt-child-completion-v1",
        "COMPLETION_TOKEN=$CompletionToken",
        "EXPECTED_HEAD=$ExpectedHead",
        "BINARY_SHA256=$BinarySha256",
        "[System.IO.File]::Move($temporaryPath, $Path)",
    ] {
        assert!(
            child.contains(required),
            "child must atomically bind completion evidence: {required}"
        );
    }
}

#[test]
fn fixture_capability_probe_uses_the_same_local_contract_as_the_codex_adapter() {
    let fixture = repository_source("src/bin/tabbeacon-terminal-smoke-fixture.rs");
    let adapter = repository_source("src/providers/codex/config.rs");
    assert!(
        fixture.contains("FIXTURE_CODEX_FEATURES_ARGUMENTS")
            && fixture.contains("FIXTURE_CODEX_SCHEMA_ARGUMENTS"),
        "the fixture must emulate the local capability commands"
    );
    assert!(
        fixture.contains(".with_codex_program(executable)"),
        "the real adapter proof must use the owned fixture executable"
    );
    assert!(
        adapter.contains("probe_codex_capabilities") && adapter.contains("capability_probe"),
        "the checked Codex adapter must consume local capability evidence"
    );
}

#[test]
fn visual_worker_process_queries_share_the_bounded_helper_contract() {
    let runner = repository_source("src/visual/runner.rs");
    assert!(
        runner.contains("visual worker parent process query timed out")
            && runner.contains("visual worker parent identity query timed out")
            && runner.contains("bounded_powershell_output(&script, WORKER_PROCESS_QUERY_BUDGET)"),
        "worker parent and identity probes must not use synchronous unbounded PowerShell output"
    );
}
