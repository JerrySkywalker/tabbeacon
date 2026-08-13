# TB-G03R — Visual Infrastructure Recovery and Closure

## Purpose and authority

This governed recovery closes the remaining `TB-G03` interactive-visual
infrastructure gap without starting `TB-G04`. It is authorized only for
`JerrySkywalker/tabbeacon` at `V:\src\tabbeacon`.

- `STARTING_MAIN=154014ea3738308ecd3949598993db646f1373a8`
- active G03 branch: `tb-g03-visual-ci-foundation`
- starting G03 candidate:
  `a90f16d80b8c9bfa4d72fb360944d95c3745212f`
- existing PR: `https://github.com/JerrySkywalker/tabbeacon/pull/3`

The owner has specifically authorized a dedicated repository-scoped GitHub
Actions runner on this Windows machine, operated only in the current
interactive user session for G03. This recovery contract supersedes the G03
contract's runner-registration non-goal only for the owned runner lifecycle
defined here; all other G03 boundaries remain in force.

## Authorized scope

- `scripts/visual-runner/**` for an idempotent, user-scoped runner bootstrap,
  start, doctor, stop, and ownership-safe uninstall path;
- `docs/visual-ci.md`, `docs/visual-ci-runner-threat-model.md`, and a focused
  runner operations document for the security, lifecycle, capture, and
  recovery contract;
- `src/visual/**`, `src/bin/tabbeacon-visual-fixture.rs`, and focused
  `tests/visual_ci.rs` changes needed to repair an empirically demonstrated
  owned-window activation/capture defect, evidence integrity, or test
  resilience;
- existing G03 workflow/script only for focused trusted-manual-dispatch and
  exact-head hardening;
- this contract and the existing G03 goal document where the recovery boundary
  must be cross-referenced.

The generated runner directory is outside the checkout at the current user's
`LOCALAPPDATA\TabBeacon\visual-runner`; it is never committed. It may contain
GitHub Actions runner credentials created by `config.cmd`, but no user PAT,
repository write token, or registration/remove token may be persisted or
printed.

## Acceptance contract

1. Re-admission proves the active branch, remote branch, PR head, hosted code
   CI, runner inventory, user session, input desktop, Windows Terminal, and
   GitHub authentication. Ambiguous pre-existing runner resources are retained.
2. The source-controlled lifecycle scripts are idempotent and user-scoped.
   They use a positively owned marker, a dedicated root, bounded process state,
   `gh api` short-lived registration/remove tokens kept in memory, redacted
   logs, and safe dry-run uninstall by default. They never install a service or
   scheduled task.
3. The runner is registered only for `JerrySkywalker/tabbeacon`, has the
   standard self-hosted Windows/x64 labels plus `tabbeacon-visual`, and is
   proved online by the repository API while running in the active nonzero user
   session. No unrelated listener is stopped, reconfigured, or removed.
4. The trusted visual workflow remains `workflow_dispatch` only, sourced from
   `main`, restricted to the owner/trusted actor, admits only an in-repository
   branch whose exact ref equals the lowercase expected SHA before checkout,
   rechecks the checkout, persists no checkout credentials, and never executes
   fork or `pull_request_target` code. Static admission tests cover this policy
   where practical.
5. A captured owned Terminal window is activated only after exact UIA run-token
   and title correlation. Activation uses the maintained UIA binding's
   `SetForegroundWindow` wrapper plus UIA focus; it uses no SendKeys, no
   injection, no settings mutation, and never targets another window. Capture
   must still fail closed when foreground/focus/geometry cannot be proven.
6. Exact-head local visual evidence covers G02 fixture title, ready/default,
   working/green, result-ready/blue, approval/yellow, warning/orange,
   interrupted/purple, failed/red, reset/default, working animation, and
   stationary false-positive resistance. PASS evidence has lossless owned
   pixels, UIA data, assertion and metric files, and a deterministic evidence
   tree hash after sanitation checks.
7. The trusted workflow runs only after its source and candidate are admitted.
   A remote visual PASS is accepted only when its trusted workflow, runner,
   exact checkout, evidence head, uploaded owned artifact, title, color, and
   animation all prove the same final candidate SHA.
8. At least three independent exact-head visual runs must pass on the same
   eligible runner. A doctor -> online -> stop -> offline -> start -> online
   lifecycle is proved; uninstall is exercised only as an ownership-safe dry
   run unless an owned removal is necessary for recovery.
9. Any repository correction creates a new candidate and receives fresh L0/L1
   local validation, exact-head hosted code CI, and exact-head visual evidence.
   PR #3 remains the sole G03 PR and is merged only after every required lane
   passes for one SHA.

## Validation and evidence

For the final candidate run:

```text
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build --locked --all-targets
pwsh -NoProfile -File .\scripts\ci\run-local-ci.ps1 -ExpectedHead <final-head>
```

Expected evidence is the runner doctor/lifecycle result, exact workflow run
IDs, only the owned visual evidence directories/artifacts, SHA-256 evidence
tree integrity, runner name/labels/session/mode/version, and explicit failure
classification. A locked desktop, unavailable UIA, untrusted pixels, absent
workflow bootstrap, runner/API failure, or evidence mismatch is `BLOCKED` or
`UNPROVEN`, never a visual PASS.

## Explicit non-goals

- any TB-G04 or later roadmap implementation, including repository identity or
  abbreviation;
- Codex hooks/App Server, Claude/OpenCode, or any provider implementation;
- daemon, PTY/session manager, remote control, telemetry, or generic desktop
  automation framework;
- Windows lock/security/autologon, Defender/firewall, proxy, display/theme, or
  persistent power-plan changes; a process-lifetime sleep request is permitted
  only while the owned runner host is alive and must be documented;
- Session-0/service runner installation, scheduled tasks, public/fork
  `pull_request` execution, `pull_request_target`, secrets in candidate code,
  or arbitrary user-supplied repositories/refs;
- SendKeys, DLL injection, UI Automation product behavior, settings.json
  mutation, unrelated terminal/window capture, or modification/cleanup of
  ambiguous existing runners;
- a second G03 PR or unrelated cleanup.

## Completion rule

`TB-G03` can close as `PASS` only when the final candidate satisfies
`EXPECTED_HEAD == CODE_HEAD == VISUAL_HEAD`, local and remote visual assertions
are all `PASS`, the approved runner is online and repeatable, and PR #3 is
merged under the repository governance. Otherwise retain the most precise
`BLOCKED`, `FAIL`, or `UNPROVEN` disposition and do not start TB-G04.
