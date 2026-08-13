# Visual CI Public-Runner Threat Model

## Decision for TB-G03

TabBeacon is a public repository. An interactive Windows visual runner can
observe a user's desktop and execute checked-out Rust code, so it is a
privileged environment. It must never execute arbitrary fork pull-request
code.

The initial visual lane is therefore a manually dispatched workflow whose
definition is sourced from trusted `main`. It accepts `expected_sha` and
`head_branch`, then verifies all of the following before it checks out or runs
candidate code:

1. the workflow was dispatched from `refs/heads/main`;
2. the actor is the repository owner (or a later, explicitly documented trusted
   actor); and
3. `refs/heads/<head_branch>` in `JerrySkywalker/tabbeacon` resolves exactly to
   `expected_sha`.

After checkout it asserts the checked-out Git `HEAD` is exactly
`expected_sha`. The visual harness independently refuses a visual PASS unless
its expected, checked-out, and visual evidence heads are equal.

The implementation is `.github/workflows/visual-ci.yml`. It has only a
`workflow_dispatch` trigger, requires the dispatch ref to be `main`, requires
the repository owner as actor, validates the same-repository
`refs/heads/<head_branch>` through `git ls-remote`, then performs an exact-SHA
checkout. It never has a `pull_request` or `pull_request_target` trigger.

## Rejected designs

- No self-hosted visual job runs on ordinary `pull_request` events.
- No `pull_request_target` workflow checks out or executes pull-request code.
- No fork ref, `refs/pull/*`, user-supplied repository URL, or unvalidated SHA
  is accepted as a visual-test target.
- No repository secret is required by the visual tests.
- A Session-0 or locked/noninteractive runner is not visual evidence, even if
  its job process starts successfully.

## Operational boundary

`TB-G03R` may provision exactly one dedicated marker-owned, user-session
runner for this repository through [`visual-runner.md`](visual-runner.md). It
uses the label `tabbeacon-visual` plus GitHub's standard Windows/x64 labels,
but the workflow also verifies its exact name and rejects Session 0 before
candidate checkout. It is not installed as a service or scheduled task, and it
does not change lock/security/power policy. Ambiguous existing listeners and
unmarked roots remain untouched.

Until the trusted dispatcher is present on `main` and that runner is online in
an interactive desktop session, the remote visual lane remains `BLOCKED`.
Hosted Windows code CI and local synthetic tests remain useful but cannot be
promoted to visual PASS.

The runner uploads only its dedicated TabBeacon evidence directory. That
directory contains captures of the positively owned test Terminal window, not
the user's existing windows, terminal text, secrets, environment variables, or
credentials.
