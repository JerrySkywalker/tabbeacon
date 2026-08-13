# TB-G03 — Visual CI Foundation

## Goal

Build deterministic, machine-verifiable visual testing for the TB-G02 Windows
Terminal presentation path. The test infrastructure must drive the existing
provider-neutral fixture and production presentation policy/renderer into a
dedicated Windows Terminal test window, then produce a trustworthy,
exact-head-bound evidence bundle from UI Automation and captured pixels.

## Starting point

- Repository: `JerrySkywalker/tabbeacon`
- STARTING_MAIN=`154014ea3738308ecd3949598993db646f1373a8`
- Previous governed goal: `TB-G02` (`PASS`)
- Feature branch: `tb-g03-visual-ci-foundation`

## Authorized scope

- `src/visual/**` for Windows-first visual-test infrastructure and pure visual
  oracle logic;
- `src/bin/tabbeacon-visual-fixture.rs`, if a dedicated fixture driver binary
  is required to render the production G02 fixture into the owned test window;
- `src/lib.rs` only for the required `visual` module export;
- `src/presentation/**` only for a narrow, provider-neutral fixture-title
  injection seam needed to uniquely correlate an owned G03 test tab. The
  policy, semantics, VT encoding, and palette remain the system under test and
  must not otherwise change;
- focused G03 tests under `tests/`;
- `scripts/visual/**` and the existing local CI wrapper only where a focused
  visual harness entry point is required;
- `.github/workflows/visual-ci.yml` for the trusted, manually-dispatched
  visual lane only;
- `Cargo.toml` and `Cargo.lock` only for the small, documented dependencies
  needed for safe Windows UI Automation/capture bindings, lossless PNG output,
  and deterministic JSON evidence;
- `docs/architecture.md`, `docs/visual-ci.md`,
  `docs/visual-ci-runner-threat-model.md`, and this goal contract.

No core reconciliation, provider, repository-identity, setup, doctor,
uninstall, daemon, PTY/session-management, or unrelated cleanup change is
authorized.

## Architecture contract

The implementation must preserve this one-way, provider-neutral test path:

```text
VisualTestCase
        ↓
FixtureDriver
        ↓
TerminalTestSession
        ↓
TargetLocator (UIA)
        ↓
CaptureBackend
        ↓
CapturedFrame
        ↓
VisualOracle
        ↓
AssertionResult
        ↓
EvidenceWriter
```

`src/core` must not reference UI Automation, HWNDs, image pixels, capture
backends, or evidence filesystem artifacts. The visual layer must not reference
provider raw event types or implement provider integration. The renderer must
remain a typed `VisualState -> VT bytes` implementation; UIA is verification
only and never product control.

`VisualTestCase`, ROI, frame, precondition, assertion, disposition, and
evidence-manifest concepts must use explicit Rust types. Capture is behind a
replaceable backend boundary, but G03 is deliberately Windows Terminal-first,
not a generic cross-platform screenshot framework.

## Required behavior and acceptance criteria

1. A machine-readable desktop preflight distinguishes a trustworthy interactive
   desktop `PASS` from `BLOCKED` conditions including Session 0/service
   execution, locked or inaccessible desktop, unavailable Windows Terminal,
   unavailable UIA, untrustworthy capture, and unsupported runtime. A
   preflight/capture problem is never reported as a presentation assertion
   failure.
2. The test session creates or targets only a positively owned Windows Terminal
   test window/tab using a unique run identifier and recorded dimensions. It
   does not manipulate, close, screenshot, or mutate settings for unrelated
   user windows. Normal cleanup attempts the G02 presentation reset first and
   removes only owned temporary resources.
3. UIA is the primary title oracle: it locates the owned Terminal window and
   tab, records the accessible tab name and bounding rectangle when available,
   and emits a compact diagnostic dump. OCR is not a title oracle.
4. A Rust Windows capture path produces an independently represented lossless
   full-window frame, target-tab crop, and required ROI crops for the owned
   session. Its evidence records the empirically selected capture method and
   visibility assumptions.
5. Color assertions use deterministic tab-background ROIs excluding text,
   controls, icons/progress, and obvious borders. They record sample count,
   aggregate/dispersion metrics, and tolerance-based classification for
   default, green, blue, yellow, orange, purple, and red; they do not use
   whole-image golden equality or one exact RGB pixel.
6. Working-state animation uses bounded progress/icon ROI frame sequences and
   documented deterministic delta metrics. It distinguishes
   `ANIMATION_PRESENT`, `ANIMATION_ABSENT`, `UNPROVEN_CAPTURE`, and
   `BLOCKED_ENVIRONMENT`; stationary and noisy synthetic cases must not falsely
   prove animation.
7. Every live run writes a deterministic, dedicated evidence directory
   containing `manifest.json`, `assertions.json`, `environment.json`, `uia.json`,
   lossless owned-window/tab/ROI images, and color/frame-delta metrics. The
   manifest records goal, expected/visual heads, run ID, timestamp, machine and
   Terminal versions, preflight, session/DPI/geometry, capture backend, fixture
   set, and assertion dispositions. It must reject a PASS when
   `EXPECTED_HEAD != CHECKED_OUT_HEAD != VISUAL_HEAD`, contain no secrets or
   unrelated terminal/window content, and fail safely if its output directory
   already exists.
8. Pure and synthetic tests cover ROI clipping/bounds, color aggregation and
   tolerance classification, frame deltas and animation thresholds, stationary
   and noisy frames, manifest serialization/validation, failure classification,
   exact-SHA checks, invalid dimensions/ROIs, inconsistent frames, target loss,
   color ambiguity, reset/cleanup after partial failure, and repeated
   deterministic input. These tests do not need an interactive desktop.
9. A narrowly scoped live empirical spike records Windows/Terminal versions,
   current session and desktop accessibility, DPI/display geometry, repository
   head, and Rust toolchain. It establishes, when preflight permits, dedicated
   session correlation, UIA title discovery, trustworthy owned-window pixels,
   working-frame temporal change, and measurable frame-color pixels. Its
   evidence remains classified honestly if any observation is blocked.
10. The visual workflow is manual/trusted only: its definition is sourced from
    `main`, admits a repository-owner or explicitly trusted actor, validates
    an in-repository `refs/heads/<head_branch>` equals `expected_sha`, checks
    out precisely that SHA, asserts it again before executing code, and uploads
    only the dedicated evidence directory. It must never execute fork PR code
    on a self-hosted runner, use `pull_request_target`, or add a generic
    self-hosted `pull_request` job. A noninteractive runner must publish a
    classified precondition result, not a fake visual assertion failure.
11. New dependencies are limited, recorded with their purpose and compatible
    license, and justified against the standard library/current dependency set.
12. L0/L1 gates pass locally and hosted code CI proves
    `EXPECTED_HEAD == CODE_HEAD`. If approved interactive visual infrastructure
    exists, the trusted visual workflow additionally proves
    `EXPECTED_HEAD == VISUAL_HEAD`. If it does not, implementation and code CI
    may finish as `BLOCKED_EXTERNAL`; visual evidence must not be faked and the
    goal must not merge as full PASS.

## Required validation and evidence

Run before candidate creation:

```text
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build --locked --all-targets
pwsh -NoProfile -File .\scripts\ci\run-local-ci.ps1
```

For the committed candidate, rerun the local wrapper with
`-ExpectedHead <candidate SHA>`, push the one feature branch, and open one
draft PR. Accept hosted code evidence only when its checkout SHA equals that
candidate. Retain live evidence only when preflight and its individual
observations actually execute; unit/synthetic tests are not visual PASS.

Completion records must include:

```text
GOAL_ID=TB-G03
STARTING_MAIN=154014ea3738308ecd3949598993db646f1373a8
EXPECTED_HEAD=<candidate-or-N/A>
CODE_HEAD=<candidate-or-N/A>
VISUAL_HEAD=<candidate-or-N/A>
LOCAL_VALIDATION=<PASS|FAIL|BLOCKED|UNPROVEN>
CI=<PASS|FAIL|BLOCKED|UNPROVEN>
LOCAL_VISUAL_PREFLIGHT=<PASS|BLOCKED|UNPROVEN>
LOCAL_UIA=<PASS|FAIL|BLOCKED|UNPROVEN>
LOCAL_CAPTURE=<PASS|FAIL|BLOCKED|UNPROVEN>
LOCAL_TITLE_ASSERTION=<PASS|FAIL|BLOCKED|UNPROVEN>
LOCAL_COLOR_ASSERTION=<PASS|FAIL|BLOCKED|UNPROVEN>
LOCAL_ANIMATION_ASSERTION=<PASS|FAIL|BLOCKED|UNPROVEN>
VISUAL_CI=<PASS|FAIL|BLOCKED|UNPROVEN>
VISUAL_REMOTE_INFRA=<READY|BLOCKED|N/A>
EVIDENCE_PATH=<path-or-N/A>
EVIDENCE_SHA=<sha-or-N/A>
SECURE_PUBLIC_RUNNER_THREAT_MODEL=<PASS|FAIL>
UNRELATED_DRIFT_TOUCHED=<true|false>
```

## Explicit non-goals

- Codex hooks, Codex App Server, Claude, OpenCode, or any other provider
  implementation;
- repository discovery, abbreviation, local identity history, or Git/worktree
  identity work;
- setup, doctor, uninstall, settings.json mutation, global configuration
  changes, runner registration, sleep/lock-screen/power/security-policy
  changes, or access to secrets/credential stores;
- UI Automation injection/control, OCR-based title verification, screenshot
  capture of unrelated windows, UI Automation-based product behavior, terminal
  forks, DLL injection, SendKeys, or PTY wrapping;
- a daemon, remote control, telemetry, terminal/session management, or a
  cross-platform screenshot framework;
- screenshot golden-image equality, visual image analysis, or visual work for
  TB-G04 and every later roadmap goal.

## Completion rule

TB-G03 is a full `PASS` only after the candidate has exact-head hosted code CI
and exact-head trusted interactive visual evidence. If the implementation and
code CI are complete but no approved interactive self-hosted visual runner is
available, finish `BLOCKED_EXTERNAL`, preserve the candidate/PR/evidence, do
not merge as PASS, and state the single runner-provisioning action needed to
resume.
