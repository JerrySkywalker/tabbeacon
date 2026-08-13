# TB-G02 — Windows Terminal Presentation

## Goal

Implement the provider-neutral semantic presentation policy and typed Windows
Terminal virtual-terminal renderer. The presentation layer consumes only a
`SessionSnapshot` plus a caller-supplied title; it does not observe or name a
provider, repository, process, or terminal UI object.

## Starting point

- Repository: `JerrySkywalker/tabbeacon`
- STARTING_MAIN=`847ee6140326f73108be9532901c19e78ff79ca5`
- Previous governed goal: `TB-G01` (`PASS`)
- Feature branch: `tb-g02-windows-terminal-presentation`

## Authorized scope

- `src/presentation/**`;
- focused presentation tests under `tests/`;
- `docs/architecture.md`, only for the G02 model and encoding contract;
- this goal contract.

No core, provider, repository-identity, workflow, dependency, setup, or
external configuration change is authorized.

## Architecture contract

The implementation must preserve this one-way layering:

```text
SessionSnapshot + semantic title
        ↓
PresentationPolicy
        ↓
PresentationAction::{Apply(VisualState), Reset(...)}
        ↓
WindowsTerminalRenderer
        ↓
VT bytes
```

1. `src/core` must not depend on this presentation module or Windows Terminal.
2. The policy consumes only G01 semantic state axes and an opaque title string.
   It must not accept provider raw event types, provider names, process IDs,
   filesystem paths, repository identity, or terminal handles.
3. `VisualState` must explicitly model a safe title, tab-color semantic, and
   progress semantic. `PresentationAction::Reset` must carry the terminal
   presentation cleanup semantics for an ended session; `Ended` must not be
   flattened into `Ready`.
4. `WindowsTerminalRenderer` renders only typed presentation values. It has no
   dependency on `SessionSnapshot` or a provider input type.
5. Rendering returns deterministic bytes. It does not mutate settings, send
   input, create a PTY, launch a terminal, or perform UI Automation.

## Policy contract

The policy order is normative and must be fixed by direct tests:

1. `Health::Failed` → red + error progress.
2. `Health::Interrupted` → purple + clear progress.
3. `Health::Warning` → orange; `Working` uses indeterminate progress and every
   other phase uses warning progress.
4. `Attention::Approval` → yellow + warning progress.
5. `Attention::Question` → distinct question semantic, default yellow +
   warning progress.
6. `Attention::ResultReady` → blue + clear progress.
7. `Phase::Working` → green + indeterminate progress.
8. `Phase::Ready` → terminal default color + clear progress.
9. `Phase::Ended` → `PresentationAction::Reset`, never an apply/ready state.

The axes remain orthogonal. In particular, `Working + Warning` is orange plus
indeterminate progress; `WaitingUser + Approval` is yellow plus warning; and
`WaitingUser + ResultReady` is blue plus clear.

## Title safety contract

1. The policy converts every supplied title to a typed terminal-title value
   before rendering.
2. All Unicode control characters, including ESC, BEL, C1 controls, and title
   terminator components, are replaced deterministically before encoding.
3. The maximum rendered title length is a documented Unicode-scalar limit;
   overlong input is deterministically truncated with an ellipsis without
   splitting a scalar value.
4. The renderer emits one fixed title OSC envelope around only the typed,
   sanitized bytes. It uses an ST terminator, and no input can introduce an
   additional terminal control sequence.

## Windows Terminal VT encoding contract

The renderer targets Windows Terminal and uses only static control bytes plus
typed title/color/progress values:

- title: OSC `0`, ST-terminated;
- progress: Windows Terminal/ConEmu OSC `9;4`, ST-terminated, with clear,
  indeterminate, warning, and error states;
- dynamic frame/tab color: OSC `4` for the Windows Terminal frame-background
  color-table index `264`; reset with OSC `104` for that same index.

Frame color is an enhancement behind explicit renderer capabilities:

```text
frame color supported   → title + progress + frame color
frame color unsupported → title + progress
```

Unsupported color capability must never suppress title/progress bytes or make
the surrounding agent unusable. `Reset` must deterministically reapply the
safe title and clear progress; when frame color is supported, it also resets
the dynamic frame color.

## Fixture contract

Provide a deterministic, in-memory fixture that enumerates and replays at
least these named states without a provider, network call, terminal launch,
screen capture, or UI Automation:

```text
ready
working
result-ready
approval
question
warning-working
warning-idle
interrupted
failed
reset
```

The fixture is G03 input only. It does not claim a screenshot, UIA, animation,
or visual verdict.

## Acceptance criteria

1. The documented architecture boundary compiles with no new dependency.
2. The policy produces the specified deterministic precedence and preserves
   distinct `Question` semantics even if its default color matches approval.
3. The renderer produces exact, deterministic VT bytes for typed titles,
   progress, set/reset color, and presentation reset.
4. Hostile title input cannot inject an OSC/CSI/control sequence, and title
   length behavior is deterministic.
5. A disabled color capability preserves exact title/progress rendering while
   omitting only frame-color bytes.
6. The deterministic fixture covers every named state and has no external
   dependency.
7. Focused tests cover title, progress, color, reset, policy precedence,
   representative state chains, fixture coverage, and repeatability.
8. L0/L1 validation passes locally. The candidate is pushed in a draft PR and
   the hosted Windows exact-head code CI reports `CODE_HEAD == EXPECTED_HEAD`.
   Visual CI remains `N/A` until `TB-G03`.

## Required validation and evidence

Before PR creation, run:

```text
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build --locked --all-targets
pwsh -NoProfile -File .\\scripts\\ci\\run-local-ci.ps1
```

After committing the candidate, rerun the local wrapper with
`-ExpectedHead <candidate SHA>`. The existing `Windows / Hosted / Exact Head`
PR job is the required code-CI lane. A fixture replay is functional evidence,
not visual evidence; do not claim a visual result without the future G03 gate.

Completion evidence must state:

```text
GOAL_ID=TB-G02
STARTING_MAIN=847ee6140326f73108be9532901c19e78ff79ca5
EXPECTED_HEAD=<candidate SHA>
CODE_HEAD=<candidate SHA>
VISUAL_HEAD=N/A
LOCAL_VALIDATION=<PASS|FAIL|BLOCKED|UNPROVEN>
CI=<PASS|FAIL|BLOCKED|UNPROVEN>
VISUAL_CI=N/A
MANUAL_WT_FIXTURE=<PASS|FAIL|BLOCKED|UNPROVEN>
UNRELATED_DRIFT_TOUCHED=false
```

## Explicit non-goals

- Codex hooks, Codex App Server, Claude, OpenCode, or any provider backend;
- repository discovery, abbreviation, local repository identity, or history;
- UI Automation, screenshots, image analysis, visual CI, or a self-hosted
  visual runner;
- settings.json mutation, SendKeys, DLL injection, Terminal fork, terminal
  launch, or any Windows Terminal UI control outside typed VT bytes;
- setup, doctor, uninstall, daemon, PTY hosting, session management,
  orchestration, telemetry, network access, or external configuration writes;
- TB-G03 and every later roadmap goal.

## Completion rule

TB-G02 completes only when its candidate commit has required local PASS and a
successful hosted exact-head PR code-CI run. It may report fixture evidence,
but it must not treat fixture tests as visual-CI evidence or begin TB-G03.
