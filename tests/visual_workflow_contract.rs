const VISUAL_WORKFLOW: &str = include_str!("../.github/workflows/visual-ci.yml");
const RELEASE_DANGLING_SYMLINK_WORKFLOW: &str =
    include_str!("../.github/workflows/release-dangling-symlink.yml");

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
        "$env:RUNNER_NAME",
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
    assert!(
        !VISUAL_WORKFLOW.contains("${{ runner.name }}"),
        "runner context is unavailable at job-level environment admission"
    );
}

#[test]
fn release_dangling_symlink_workflow_requires_capable_exact_head_execution() {
    for required in [
        "workflow_dispatch:",
        "github.ref == 'refs/heads/main'",
        "github.actor == github.repository_owner",
        "runs-on: windows-latest",
        "expected_sha:",
        "head_branch:",
        "refs/heads/$env:HEAD_BRANCH",
        "persist-credentials: false",
        "TABBEACON_REQUIRE_DANGLING_SYMLINK: '1'",
        "--exact --nocapture",
        "DANGLING_SYMLINK_FIXTURE_EXECUTED=true",
        "DANGLING_SYMLINK_POLICY=PASS",
    ] {
        assert!(
            RELEASE_DANGLING_SYMLINK_WORKFLOW.contains(required),
            "release dangling-symlink workflow is missing required guard: {required}"
        );
    }
    assert!(
        !RELEASE_DANGLING_SYMLINK_WORKFLOW.contains("pull_request_target:"),
        "untrusted pull_request_target execution is forbidden"
    );
    assert!(
        !RELEASE_DANGLING_SYMLINK_WORKFLOW.contains("\n  pull_request:"),
        "the release fixture must not run generic pull requests"
    );
}
