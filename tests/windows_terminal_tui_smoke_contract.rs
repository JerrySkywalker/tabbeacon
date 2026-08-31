const RUNNER: &str = include_str!("../scripts/run-windows-terminal-tui-smoke.ps1");
const CHILD: &str = include_str!("../scripts/invoke-windows-terminal-tui-smoke-child.ps1");
const CONTROL_CENTER: &str = include_str!("../src/control_center.rs");
const FIXTURE: &str = include_str!("../src/bin/tabbeacon-terminal-smoke-fixture.rs");

#[test]
fn terminal_runner_uses_a_bounded_durable_completion_contract() {
    for required in [
        "$deadline = [DateTimeOffset]::UtcNow.AddSeconds($TimeoutSeconds)",
        "-CompletionReceiptPath",
        "-CompletionToken",
        "-ExpectedHead",
        "-BinarySha256",
        "Get-FileHash -LiteralPath $resolvedBinary -Algorithm SHA256",
        "CHILD_COMPLETION_RECEIPT_OBSERVED",
        "DURABLE_COMPLETION_PROVEN",
        "PROCESS_QUERY_DEPENDENCY=none",
        "RESIDUAL_OWNED_PROCESS_OBSERVATION",
        "WaitForSingleObject",
        "VISUAL_OPERATION_DISPOSITION=$visualDisposition",
        "WINDOWS_TERMINAL_TUI_SMOKE=$overallDisposition",
        "TEMP_WT_CLEANUP=$($script:temporaryWtCleanupReceipt.temporary_wt_cleanup)",
        "OWNER_WINDOWS_CLOSED=$($script:temporaryWtCleanupReceipt.owner_windows_closed)",
        "BROAD_WINDOW_KILL_USED=$($script:temporaryWtCleanupReceipt.broad_window_kill_used.ToString().ToLowerInvariant())",
    ] {
        assert!(
            RUNNER.contains(required),
            "terminal runner is missing durable-completion safeguard: {required}"
        );
    }
    for forbidden in ["Get-CimInstance", "Test-ProcessAncestor", "PostMessage"] {
        assert!(
            !RUNNER.contains(forbidden),
            "terminal runner must not make the completion verdict depend on {forbidden}"
        );
    }
}

#[test]
fn terminal_child_atomically_binds_completion_to_the_candidate_and_fixture() {
    for required in [
        "[string]$CompletionReceiptPath",
        "[string]$CompletionToken",
        "[string]$ExpectedHead",
        "[string]$BinarySha256",
        "COMPLETION_SCHEMA=tabbeacon-wt-child-completion-v1",
        "COMPLETION_TOKEN=$CompletionToken",
        "EXPECTED_HEAD=$ExpectedHead",
        "BINARY_SHA256=$BinarySha256",
        "FIXTURE_RESULT_PRESENT=$($fixtureResultPresent.ToString().ToLowerInvariant())",
        "SENTINEL_WRITTEN=true",
        "COMPLETED=true",
        "[System.IO.File]::Move($temporaryPath, $Path)",
    ] {
        assert!(
            CHILD.contains(required),
            "terminal child is missing completion-receipt safeguard: {required}"
        );
    }
}

#[test]
fn terminal_fixture_proves_the_g61_title_explanation_surface() {
    for required in [
        "title_explanation_exercised",
        "fixture did not open Why this title",
    ] {
        assert!(
            CONTROL_CENTER.contains(required),
            "terminal fixture control path is missing G61 title-explanation evidence: {required}"
        );
    }
    assert!(
        FIXTURE.contains("TUI_TITLE_EXPLANATION={}"),
        "terminal fixture receipt is missing G61 title-explanation evidence"
    );
}

#[test]
fn terminal_fixture_proves_g62_integrations_and_badge_surfaces() {
    for required in [
        "integrations_visited",
        "provider_badge_staged",
        "fixture did not render the admitted provider capability projection",
    ] {
        assert!(
            CONTROL_CENTER.contains(required),
            "terminal fixture control path is missing G62 evidence: {required}"
        );
    }
    for required in [
        "TUI_INTEGRATIONS={}",
        "TUI_PROVIDER_CAPABILITY_MATRIX={}",
        "TUI_PROVIDER_BADGE_STAGED={}",
    ] {
        assert!(
            FIXTURE.contains(required),
            "terminal fixture receipt is missing G62 evidence: {required}"
        );
    }
    for required in [
        "TUI_INTEGRATIONS=$($integrations.ToString().ToLowerInvariant())",
        "TUI_PROVIDER_CAPABILITY_MATRIX=$($providerCapabilityMatrix.ToString().ToLowerInvariant())",
        "TUI_PROVIDER_BADGE_STAGED=$($providerBadgeStaged.ToString().ToLowerInvariant())",
    ] {
        assert!(
            RUNNER.contains(required),
            "terminal runner is missing G62 receipt verification: {required}"
        );
    }
}
