# TabBeacon Interactive Visual Runner

## Purpose and boundary

`TB-G03R` supplies a source-controlled bootstrap for one dedicated GitHub
Actions runner for `JerrySkywalker/tabbeacon`. It exists solely to run the
trusted Windows Terminal visual lane. It is not a general CI worker and must
never execute public fork pull requests.

The runner is user-scoped and generated outside the checkout at:

```text
%LOCALAPPDATA%\TabBeacon\visual-runner
```

Its default name is `tabbeacon-visual-<machine>`. GitHub adds the standard
`self-hosted`, `Windows`, and `X64` labels; bootstrap adds
`tabbeacon-visual`. The workflow additionally requires the exact runner name
provided by the trusted dispatcher before it checks out candidate code.

## Lifecycle

Run all commands as the intended logged-on user in a nonzero interactive
session. The scripts refuse Session 0, services, unmarked roots, and attempts
to adopt another runner's state.

```powershell
pwsh -NoProfile -File .\scripts\visual-runner\bootstrap.ps1
pwsh -NoProfile -File .\scripts\visual-runner\start.ps1
pwsh -NoProfile -File .\scripts\visual-runner\doctor.ps1
pwsh -NoProfile -File .\scripts\visual-runner\stop.ps1
pwsh -NoProfile -File .\scripts\visual-runner\uninstall.ps1       # dry run
pwsh -NoProfile -File .\scripts\visual-runner\uninstall.ps1 -Execute
```

Bootstrap downloads the official current `actions/runner` Windows x64 archive,
verifies it against that release's `hashes.txt`, obtains a short-lived
repository registration token with `gh api`, configures the runner, then
forgets the token. The token is never echoed, logged, committed, or stored in
TabBeacon metadata. The GitHub runner's own scoped listener credential remains
inside the owned runner root as required by GitHub Actions; no PAT or broad
repository write credential is persisted by TabBeacon.

`start.ps1` launches an owned hidden user-session host, which starts only the
runner listener under the marker-proven root. It temporarily calls
`SetThreadExecutionState(ES_CONTINUOUS|ES_SYSTEM_REQUIRED|ES_DISPLAY_REQUIRED)`
while that host is alive, then resets the request on exit. This does not change
power plans, lock policy, autologon, security controls, or system settings.
`stop.ps1` asks only that owned host to stop and waits boundedly. It never
stops ambiguous listener processes. `uninstall.ps1` is dry-run by default and
requires both the ownership marker and an offline host before it requests an
ephemeral remove token and deletes the exact owned root.

## Security and dispatch

The visual workflow is defined on trusted `main`, has only a manual dispatch,
accepts only the repository owner, admits the in-repository branch reference
before checkout, rechecks the exact SHA after checkout, and asserts the exact
registered runner name and a nonzero job session before candidate code runs.
It never uses `pull_request`, `pull_request_target`, a fork repository URL, or
persisted checkout credentials.

For the initial bootstrap, `main` must first contain the reviewed dispatcher;
do not weaken its `main` source condition by dispatching the candidate branch.
If the dispatcher is not yet on `main`, retain the visual lane as blocked until
the owner authorizes that trusted-main bootstrap under repository governance.

## Evidence hygiene

Only `artifacts/visual/<run-id>` from the fixture harness may be uploaded. The
window-only capture backend uses the exact UIA-correlated title to resolve one
HWND; it never desktop-copies pixels behind a transparent Terminal window.
Evidence must be discarded from review when a capture precondition fails or a
prior backend has sampled unrelated desktop content.
