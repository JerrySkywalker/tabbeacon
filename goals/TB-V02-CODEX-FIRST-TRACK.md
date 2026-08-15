# TB v0.2 — Codex-first product track

## Status

Planning document only. This file does not authorize implementation work by itself.

Starting production baseline when this plan was admitted:

```text
v0.1.1 release head = 2259ab5aa5dd2f42a3c13b072dd61814b713af7a
```

The v0.1 mandatory path is complete. Near-term work is intentionally limited to improving the direct Codex CLI + Codex Hooks + Windows Terminal workflow.

## Product objective

Make TabBeacon effectively feature-complete for daily multi-tab Codex work without changing the user's agent launch habit.

The daily command remains:

```powershell
codex
```

The production observation backend remains Codex Hooks for this track.

## Frozen scope boundaries

### In scope

- compact status-first Windows Terminal tab titles;
- real working-state title animation if a safe ephemeral-worker model is proven;
- Codex Hook compatibility/profile detection;
- turn/generation awareness where the admitted Hook contract exposes reliable metadata;
- root/subagent isolation where the admitted Hook contract exposes reliable metadata;
- compact lifecycle handling where supported;
- guided setup/configuration UX;
- live preview;
- human-readable and JSON operational diagnostics;
- exact-head functional, visual, and real-Codex hardening;
- public v0.2 release closure.

### Explicitly deferred

- Codex App Server backend (`TB-G06X`);
- Claude provider (`TB-G20`);
- OpenCode provider (`TB-G30`);
- a global always-running TabBeacon daemon as the default animation architecture;
- fake `codex.exe`, PATH shadow, PowerShell `codex` function, PTY wrapper, or launcher wrapper;
- TUI text scraping as an authoritative state source;
- arbitrary user-defined executable hooks or arbitrary animation frame strings;
- full-screen configuration application unless lightweight guided CLI proves insufficient;
- package-manager/self-update work;
- ARM64 release until validated independently.

## Product invariants

Every G09–G14 implementation must preserve:

```text
DAILY_COMMAND=codex
ZERO_WORKFLOW_CHANGE=true
FAIL_OPEN=true
OFFLINE_REPOSITORY_IDENTITY=true
PROVIDER_NEUTRAL_CORE=true
HOOK_TRUST_BYPASS=false
GLOBAL_DAEMON_BASELINE=false
```

Codex Hooks currently have no authority to invent confirmed health states. Unless a future admitted Hook contract proves otherwise:

```text
HOOK_HEALTH_AUTHORITY=NONE
```

A shell failure, tool failure, timeout, missing event, model message, or TUI text must not silently become `Warning`, `Interrupted`, or `Failed`.

# 1. Title grammar v2

## Canonical grammar

The default production title is:

```text
<status-slot> <repository-alias>
```

The left side is semantic and may change. The right side is identity and must remain stable during one activity sequence.

Examples:

```text
○ OWH     ready/neutral
⠋ OWH     working frame 1
⠙ OWH     working frame 2
⠹ OWH     working frame 3
✓ OWH     result ready
! OWH     approval required
? OWH     question/answer required
```

Square brackets are not part of the default grammar; `OWH` above represents the existing stable repository alias system.

## Grammar requirements

- status slot is always first;
- exactly one ordinary separator between status slot and repository alias;
- repository alias is never animated;
- animation changes only the status slot;
- default titles do not redundantly append `working`, `result-ready`, `approval`, `question`, or similar lifecycle prose;
- typed sanitization and maximum title length still apply after layout composition;
- title-off/native modes retain their existing ownership semantics;
- dynamic tab color and activity channel choices remain independent.

The concrete glyph table may be tuned by G09 fixtures, but left-status/right-identity positioning is frozen by this plan.

# 2. Goal dependency DAG

```text
TB-G09 — Status-First Title Grammar v2
   ↓
TB-G10 — Codex Hook Compatibility and Turn/Agent Awareness
   ↓
TB-G11 — Session-Scoped Ephemeral Activity Animator
   ↓
TB-G12 — Guided Setup and Configuration Wizard v2
   ↓
TB-G13 — Operational Status and Machine-Readable Diagnostics
   ↓
TB-G14 — Codex-Only v0.2 Hardening and Release
```

Default execution is sequential. A later Architect may admit narrowly independent documentation/tests in parallel, but no implementation goal may assume an interface from an unfinished predecessor.

# 3. TB-G09 — Status-First Title Grammar v2

## Intended code boundary

Primary areas are expected to include:

- presentation title composition;
- Codex runtime's current repository/status title handoff;
- deterministic presentation fixtures;
- visual oracle expectations;
- user documentation describing title semantics.

The provider normalizer/reconciler should not learn terminal glyphs.

## Acceptance

- repository alias and semantic status are separate inputs before final title layout;
- default title grammar is status-first;
- working marker is left of repository alias;
- result/attention markers are left of repository alias;
- no default lifecycle prose suffix remains;
- existing `title=native` and `title=off` behavior is unchanged;
- L0/L1/L2 pass;
- exact-head L3 visual evidence proves title position and normal reset behavior.

Estimated effort: 2–4 h.

# 4. TB-G10 — Codex Hook compatibility and turn/agent awareness

## Design intent

The v0.1.1 normalizer intentionally supports a conservative fixed Hook set and ignores unknown events. v0.2 should remain conservative while becoming more explicit about the installed Codex Hook profile.

The goal is not "support every event from upstream main". The goal is to understand the actually admitted installed release and reject stale/cross-agent updates that matter to animation lifecycle.

## Candidate metadata

Where proven by the admitted Codex release, G10 may normalize non-sensitive identity/order fields such as:

```text
session_id
turn_id
agent_id
agent_type
hook event/source
generation/order evidence
cwd
```

Prompt text, model output, tool arguments/results, credentials, and arbitrary Hook payload bodies remain out of persistent state.

## Required behaviors

- deterministic Hook capability/profile classification;
- version/profile surfaced internally for diagnostics;
- stale previous-turn lifecycle events cannot override the admitted newer turn;
- subagent events cannot accidentally end or replace root-agent activity;
- compact lifecycle is explicitly classified when reliable Hook events are available;
- unsupported events remain fail-open and forward-compatible;
- ownership-safe setup still preserves unrelated user Hooks/config;
- `/hooks` remains the sole user trust path.

Estimated effort: 4–8 h.

# 5. TB-G11 — Session-scoped ephemeral activity animator

## Feasibility first

No production worker architecture is admitted until a focused spike proves that a worker can safely outlive one synchronous Hook invocation while remaining bound to the correct Windows Terminal presentation target.

The spike must prove:

1. Hook return/timeout does not incorrectly terminate or block the worker.
2. Worker output continues to affect only the originating terminal context.
3. Two parallel Codex tabs remain isolated.
4. A tab/session close yields bounded cleanup.
5. Worker creation failure degrades only decoration and never Codex usability.

If any mandatory proof is unavailable, disposition is `BLOCKED` or `UNPROVEN`. Do not quietly introduce a global service to manufacture a PASS.

## Worker lifecycle model

Conceptual minimum:

```text
WorkerKey
  session identity
  turn/generation identity
  terminal binding

WorkerPresentation
  repository alias
  semantic active state
  spinner preset

WorkerLease
  generation
  last update
  expiry/cleanup data
```

The implementation may choose process-safe atomic state, named pipes, or another small local IPC mechanism after the feasibility spike. The roadmap does not pre-select IPC prematurely.

## Supersession rules

- newer turn/generation supersedes older worker generation;
- reliable result-ready/attention/end evidence stops active animation and writes a static final state;
- stale events cannot revive an older worker;
- stale/abandoned workers self-expire;
- upgrades cannot leave an obsolete executable continuously overwriting the current release's presentation.

## Visual acceptance

At least two distinct working frames must be observed at the exact candidate head while the repository alias remains unchanged and positionally stable.

Estimated effort: 8–16 h after feasibility admission.

# 6. TB-G12 — Guided setup and configuration wizard v2

## Existing baseline to preserve

The product already exposes:

```text
config show
config wizard
config set
config preset
config reset
preview
setup codex
doctor
uninstall codex
```

G12 does not create a competing settings layer.

## User flow target

A new guided entry point:

```powershell
tabbeacon setup
```

should coordinate discovery, configuration, preview, provider setup, trust handoff, and final doctor guidance while retaining `tabbeacon setup codex` for scripts/advanced use.

Suggested interaction:

```text
TabBeacon Setup

Windows Terminal   detected
Codex              detected / compatible profile
TabBeacon          current version
Hooks              absent / current / upgrade required

Presentation
  Theme       > muted-dark
  Activity    > animated spinner
  Spinner     > codex
  Tab color   > dynamic
  Title       > TabBeacon

Preview
  ⠋ OWH

[Apply] [Cancel]
```

Requirements:

- lightweight keyboard selection is sufficient;
- preview is live or near-live without persisting until Apply;
- Cancel restores the pre-wizard presentation/config state;
- settings remain closed typed values;
- trust is never bypassed;
- final guidance points to Codex `/hooks`, then `tabbeacon doctor`.

Estimated effort: 4–8 h.

# 7. TB-G13 — Operational status and structured diagnostics

## Human interface

Add a concise operational view such as:

```text
TABBEACON_VERSION=...
BINARY_PATH=...
CODEX_VERSION=...
HOOK_PROFILE=...
HOOK_TRUST=...
TITLE_MODE=...
ACTIVITY_MODE=...
SPINNER_PRESET=...
THEME=...
ACTIVE_WORKERS=...
STALE_WORKERS=...
DOCTOR=...
```

Exact labels may evolve under a typed output model.

## Machine interface

Provide stable JSON for automation, preferably through:

```text
tabbeacon status --json
tabbeacon doctor --json
```

Requirements:

- no prompt/tool/model content;
- no credentials;
- no unrelated user configuration dump;
- warnings and hard failures remain distinct;
- nonzero exit status remains meaningful for doctor failure;
- schema is documented enough for a Codex maintenance task to consume without scraping human prose.

Estimated effort: 2–4 h.

# 8. TB-G14 — Codex-only v0.2 hardening and release

No new product feature is admitted in this goal.

## Required scenarios

- multiple parallel Codex tabs;
- same-repository multiple sessions;
- linked worktrees and alias collisions;
- stale prior-turn events;
- root/subagent isolation where supported;
- compact lifecycle where supported;
- normal prompt → working → result-ready path;
- approval transition;
- session end/reset;
- Ctrl+C / abnormal termination only to the evidence fidelity Hooks can actually prove;
- Codex process disappearance and stale worker cleanup;
- terminal tab close;
- worker crash;
- missing TabBeacon executable;
- upgrade/relocation;
- settings changes;
- fail-open behavior.

## Release evidence

All applicable lanes must bind to the same exact candidate SHA:

```text
EXPECTED_HEAD == CODE_HEAD == VISUAL_HEAD
```

L4 real-Codex evidence must prove direct `codex` launch remains unchanged.

The visual run must prove:

- changing left animation frame;
- stable right repository identity;
- correct semantic tab color;
- correct static state after reliable completion/attention evidence;
- cleanup/reset behavior.

Publish GitHub Release and crates.io intentionally from the accepted source version.

Estimated effort: 3–6 h.

# 9. Verification matrix

| Goal | L0 | L1 | L2 | L3 visual | L4 real Codex |
| --- | --- | --- | --- | --- | --- |
| G09 | required | required | required | required | focused if needed |
| G10 | required | required | required | N/A unless presentation changes | required compatibility smoke |
| G11 | required | required | required | required | required |
| G12 | required | required | required | required for preview/presentation changes | required setup/trust handoff smoke |
| G13 | required | required | required | N/A unless presentation changes | focused doctor/status smoke |
| G14 | required | required | required | required | required |

A predecessor's evidence is not automatically reusable after a successor changes the relevant behavior. Release closure requires evidence at the release candidate head.

# 10. Risk register

## R1 — worker cannot safely bind to the originating terminal after Hook exit

Impact: high.

Response: feasibility gate; retain static v0.1 fallback if unproven. Do not introduce a global daemon by default.

## R2 — stale turn/subagent Hook events terminate the wrong animation

Impact: high.

Response: G10 precedes G11; generation/agent filtering is part of admission.

## R3 — Codex Hook schema changes across releases

Impact: medium/high.

Response: capability/profile detection and fail-open unknown-event handling; freeze behavior against admitted releases, not upstream `main` alone.

## R4 — presentation becomes too verbose for many tabs

Impact: medium.

Response: status-first compact grammar; repository alias is the only default textual identity, lifecycle prose removed.

## R5 — wizard becomes a second terminal application

Impact: medium.

Response: lightweight guided CLI/TUI-like interaction only; no full-screen framework requirement without evidence.

## R6 — diagnostics leak Hook payload content

Impact: high.

Response: structured summaries expose only typed operational metadata and counts, never prompt/tool/model bodies.

# 11. Effort envelope

```text
TB-G09   2–4 h
TB-G10   4–8 h
TB-G11   8–16 h
TB-G12   4–8 h
TB-G13   2–4 h
TB-G14   3–6 h
-----------------
Total    23–46 h
```

`TB-G11` is the dominant uncertainty. If its feasibility gate fails, the correct outcome is to retain the safe static indicator and reassess architecture rather than claim the planned v0.2 animation complete.

# 12. Completion definition

After G09–G14 are complete and post-release dogfood has no open P0/P1 defects, the Codex Hooks/Windows Terminal product path may be considered functionally near-closed.

Future App Server, Claude, and OpenCode work remains optional extension work and does not reduce this Codex-only completion status.
