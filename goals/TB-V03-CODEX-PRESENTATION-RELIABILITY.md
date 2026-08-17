# TB v0.3 — Codex Presentation Reliability & Motion

## Status

ACCEPTED PLANNING BASELINE. v0.2.0 is publicly released and its release
closeout is merged. This document authorizes the frozen sequential v0.3
planning order only; implementation still begins with TB-G15.

Planning baseline when this document was created:

```text
public_v0.2_release_sha=0b1d5136833a05bf94b7d32c414a21da2f5ac78e
public_v0.2_tag=v0.2.0
post_release_main=9594c7055120cdb0731d2101f502503e4b30c3d4
v0.2_publication_and_closeout=complete
```

The planning branch was reconciled from the frozen pre-release baseline onto
the post-release `main`. The public release SHA remains the release identity;
the newer post-release `main` is the implementation predecessor. The canonical
`dev_governance_files/ROADMAP.md` records the active sequence.

## Reconciled predecessor capabilities

Post-release recovery added terminal-close worker cleanup and an owned live-tab
UIA seam (`OwnedTabTitleReader`, `observe_frames`, and
`locate_and_activate_any_with_title_reader`). TB-G15 must generalize this
trusted, owned-target capability where necessary rather than build a second
independent Windows UIA stack. The predecessor worker/session changes remain
the safety baseline for TB-G16.

The diagnostics contract is also fixed before implementation:

```text
PASSIVE_DIAGNOSTICS=READ_ONLY
ACTIVE_TITLE_PROBE=EXPLICIT_OPT_IN
COMMON_TYPED_MODEL=true
```

Normal `status` and `doctor` invocations, including their JSON forms, never
temporarily write the active terminal title. An explicitly requested active
probe may write only to an owned fixture through the production presentation
path and reports its result through the same typed model.

## Product objective

v0.3 remains Codex-first and Windows-Terminal-first. The objective is not another provider. It is to make the already-implemented presentation path visibly reliable in everyday Codex use:

1. if TabBeacon claims title ownership, the visible tab title must converge to the expected TabBeacon title or diagnostics must explicitly classify why it cannot;
2. normal working state uses real title animation by default;
3. the default animation target is 100 ms per frame (10 FPS) with the stable workspace alias fixed on the right;
4. startup, resume, attention, result, end, terminal close, and worker failure converge to deterministic visible states;
5. Codex compatibility is version/profile explicit and maintainable as Codex evolves.

The daily launch remains literally:

```powershell
codex
```

## Frozen scope boundaries

### In scope

- title-authority observation and conflict classification;
- regression coverage for titles reverting to `PowerShell` / `Administrator: PowerShell`;
- default animated working state;
- 100 ms activity frames driven by a monotonic schedule;
- bounded startup/static-state convergence;
- Windows Terminal application-title/profile policy diagnosis;
- ownership-safe remediation only where exact ownership can be proven;
- normal and elevated PowerShell scenarios;
- Codex release/profile compatibility tooling;
- v0.3 hardening and release.

### Deferred

- Claude provider;
- OpenCode provider;
- Codex App Server production backend;
- global resident daemon;
- PTY / wrapper / PATH interception;
- remote dashboard;
- hook-health product work;
- cross-session `tabbeacon sessions` UI (candidate for v0.3.1);
- package-manager/self-update work unless separately promoted.

## Current v0.2 findings that motivate v0.3

The following source facts are compatibility inputs for the v0.3 track:

- production activity worker exists and is session/turn/terminal scoped;
- at planning admission, the worker advanced title frames every 180 ms;
- at planning admission, `PresentationSettings::default()` selected static `title-indicator` rather than the production worker;
- the built-in `codex` spinner preset is the reduced `•` / `◦` pair, while `braille` provides a ten-frame spinner;
- Codex title suppression is managed through `[tui].terminal_title = []`, but TabBeacon does not currently model Windows Terminal profile title policy or shell-side title writers;
- successful OSC output is therefore not equivalent to proof that the user-visible tab title remained correct.

## Primary regression

Track this as a first-class v0.3 regression:

```text
TB-REG-TITLE-OWNERSHIP-001

start from PowerShell or Administrator: PowerShell
launch codex
TabBeacon color changes correctly
TabBeacon title may appear briefly
visible title then remains/reverts to PowerShell-like native text
```

v0.3 may not declare title health `PASS` while this condition is present.

## Target v0.3 presentation defaults

Recommended new-install/default profile:

```text
title       = tabbeacon
tab_color   = tabbeacon
activity    = title-spinner
spinner     = braille
theme       = muted-dark
frame_ms    = 100 (normative internal default)
```

Canonical working example:

```text
○ OWH
  ↓
⠋ OWH
⠙ OWH
⠹ OWH
⠸ OWH
⠼ OWH
...
  ↓
✓ OWH
```

Only the left status slot moves. The workspace alias on the right remains stable.

### Presets v2

```text
native   -> native title, native color, native activity
minimal  -> TabBeacon title, native color, static indicator
balanced -> TabBeacon title, TabBeacon color, 100 ms braille title-spinner
full     -> TabBeacon title, TabBeacon color, 100 ms braille spinner + WT ring
```

Existing configured users are not silently rewritten. `tabbeacon setup` may detect a legacy v0.2 static profile and offer the v0.3 balanced profile, but persistence still requires Apply.

## Goal dependency DAG

```text
v0.2 public release + post-release closeout
        ↓
TB-G15 — Title Authority Observatory
        ↓
TB-G16 — Animation Engine v2 and Defaults
        ↓
TB-G17 — Title Ownership / Conflict Remediation
        ↓
TB-G18 — Session Convergence and Recovery
        ↓
TB-G19 — Codex Compatibility Registry v2
        ↓
TB-G19R — v0.3 Hardening and Release
```

Default execution is sequential because each goal changes the evidence expected by the next goal.

## Goal summaries and nominal effort

| Goal | Scope | Effective effort |
| --- | --- | ---: |
| G15 | visible title-authority probe, diagnostics, regression harness | 3–5 h |
| G16 | 100 ms animation, new defaults/presets, bounded settle, performance | 5–8 h |
| G17 | WT/shell contention diagnosis and ownership-safe remediation | 4–8 h |
| G18 | startup/resume/admin/session convergence matrix | 4–7 h |
| G19 | versioned Codex compatibility registry and source-diff tooling | 3–6 h |
| G19R | final dogfood, exact-head closure, v0.3 release | 3–6 h |
| **Total** | | **22–40 h** |

Expected center after v0.2 is roughly 28–32 effective engineering hours if G17 does not reveal an unusually complex shell-title conflict.

## vm-cell-style execution contract

Each implementation Goal uses the same evidence-oriented execution shape:

1. start from one authoritative accepted predecessor main;
2. one active writer per branch/worktree;
3. define the changed-risk surface before mutation;
4. implement in a focused branch/worktree;
5. use focused tests while iterating;
6. settle one candidate head;
7. run only the gates required by the changed risk, exact-head where applicable;
8. record durable evidence outside the repository under `V:\build\tabbeacon\<RUN_ID>`;
9. classify the result `PASS`, `FAIL`, `BLOCKED`, or `UNPROVEN`;
10. merge only an accepted candidate;
11. stop on an unchanged Owner/external blocker after one blocker fingerprint (`BLOCKER_LATCHED=true`).

Do not manufacture progress by repeating audits or CI against an unchanged blocker.

## Product invariants retained

```text
DAILY_COMMAND=codex
FAIL_OPEN=true
GLOBAL_DAEMON_BASELINE=false
HOOK_TRUST_BYPASS=false
PROVIDER_NEUTRAL_CORE=true
OFFLINE_WORKSPACE_IDENTITY=true
RAW_PROMPT_TOOL_MODEL_CONTENT_PERSISTED=false
```

## v0.3 completion definition

v0.3 is complete only when all are true:

```text
DEFAULT_TITLE_OWNER=tabbeacon
DEFAULT_ACTIVITY=title-spinner
DEFAULT_SPINNER=braille
TARGET_FRAME_INTERVAL_MS=100

VISIBLE_WORKING_FRAMES>=3 within 1 second
WORKSPACE_ALIAS_STABLE=true

NORMAL_POWERSHELL=PASS
ADMIN_POWERSHELL=PASS
GIT_WORKSPACE=PASS
NON_GIT_WORKSPACE=PASS
MULTI_TAB_ISOLATION=PASS

TITLE_AUTHORITY=HEALTHY
or a degraded channel is explicitly and truthfully diagnosed

NO_GLOBAL_DAEMON=true
DAILY_COMMAND=codex
CODEX_PROFILE=explicitly admitted
```

A green tab color with a title visibly stuck at `PowerShell` or `Administrator: PowerShell` is not a passing presentation state.
