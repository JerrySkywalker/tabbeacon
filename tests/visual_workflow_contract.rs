const VISUAL_WORKFLOW: &str = include_str!("../.github/workflows/visual-ci.yml");

#[test]
fn visual_workflow_admits_only_trusted_exact_head_on_an_expected_runner() {
    for required in [
        "workflow_dispatch:",
        "github.ref == 'refs/heads/main'",
        "github.actor == github.repository_owner",
        "expected_sha:",
        "head_branch:",
        "expected_runner_name:",
        "refs/heads/$env:HEAD_BRANCH",
        "Exact-head mismatch after checkout",
        "persist-credentials: false",
        "Assert approved interactive runner identity",
        "RUNNER_IDENTITY=PASS",
        "tabbeacon-visual",
    ] {
        assert!(
            VISUAL_WORKFLOW.contains(required),
            "trusted visual workflow is missing required admission guard: {required}"
        );
    }
    assert!(
        !VISUAL_WORKFLOW.contains("pull_request_target:"),
        "untrusted pull_request_target execution is forbidden"
    );
    assert!(
        !VISUAL_WORKFLOW.contains("\n  pull_request:"),
        "the self-hosted visual workflow must not run generic pull requests"
    );
}
