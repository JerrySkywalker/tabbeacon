# TB-G18 — Session Convergence and Recovery

## Status

PLANNED. Depends on accepted G15–G17 behavior from the reconciled post-release
v0.3 sequence.

## Purpose

Prove that every important Codex/session lifecycle path converges to the correct *visible* Windows Terminal presentation, not merely correct internal bytes or state.

## Core convergence invariant

For a supported healthy title channel, expected visible presentation must converge within a bounded window.

Nominal v0.3 requirement:

```text
CONVERGENCE_DEADLINE_MS=1000
```

The exact implementation may use a tighter bound, but release evidence must not relax beyond one second without an explicit design change.

## Required lifecycle scenarios

At minimum:

```text
fresh Codex launch
SessionStart startup
SessionStart resume
SessionStart clear
UserPromptSubmit
working animation
new turn supersession
PreCompact/PostCompact
subagent start/stop isolation
Stop/result-ready
PermissionRequest/approval
question where reliable evidence exists
SessionEnd
Ctrl+C / interruption only to the fidelity Hooks actually prove
Codex abnormal disappearance cleanup
worker crash
terminal close
```

## Required shell/terminal contexts

```text
normal PowerShell
Administrator: PowerShell
Git workspace
linked worktree
non-Git workspace
HOME
multiple tabs
same-workspace parallel sessions
```

## Visible animation requirement

For Working on a healthy title channel:

```text
within 1 second:
  observe >=3 distinct valid spinner frames
  workspace alias remains stable
```

For final static states:

```text
within 1 second:
  result-ready -> stable ✓ <alias>
  approval     -> stable ! <alias>
  ready        -> stable ○ <alias> where applicable
```

A temporary write followed by a lasting `PowerShell`-style title is not convergence.

## Primary regression closure

`TB-REG-TITLE-OWNERSHIP-001` is CLOSED only if normal and elevated PowerShell runs both satisfy one of:

1. `TITLE_AUTHORITY=healthy` and the visible title converges to TabBeacon; or
2. a genuinely degraded/suppressed/contended channel is explicitly detected and truthfully reported without claiming presentation health.

The preferred release outcome is healthy authority for both normal and elevated PowerShell.

## Recovery and cleanup

Prove bounded cleanup/supersession for:

- terminal close;
- worker crash;
- Codex disappearance;
- newer turn/generation;
- binary relocation/upgrade;
- settings changes from animated to static/native/off.

No stale worker may keep overwriting a newer accepted state.

## Validation

Use a scenario matrix rather than repeated generic audits. Reuse deterministic evidence where it genuinely proves an invariant; use trusted Windows Terminal observation for visible convergence.

Presentation-visible changes require one exact-head Visual acceptance run at the final candidate head.

## Exit gate

```text
CONVERGENCE_DEADLINE_MS<=1000
VISIBLE_WORKING_FRAMES_GE_3=PASS
NORMAL_POWERSHELL=PASS
ADMIN_POWERSHELL=PASS
GIT_WORKSPACE=PASS
NON_GIT_WORKSPACE=PASS
MULTI_TAB_ISOLATION=PASS
NEWER_GENERATION_SUPERSESSION=PASS
TERMINAL_CLOSE_CLEANUP=PASS
WORKER_CRASH_FAIL_OPEN=PASS
TB_REG_TITLE_OWNERSHIP_001=CLOSED
```

## Exit receipt

```text
GOAL_ID=TB-G18
DISPOSITION=<PASS|FAIL|BLOCKED|UNPROVEN>
STARTING_MAIN=<sha>
EXPECTED_HEAD=<sha>
CONVERGENCE_DEADLINE_MS=<n>
NORMAL_POWERSHELL=<...>
ADMIN_POWERSHELL=<...>
VISIBLE_WORKING_FRAMES=<n>
MULTI_TAB=<...>
TERMINAL_CLOSE=<...>
TB_REG_TITLE_OWNERSHIP_001=<...>
CI=<...>
VISUAL=<...>
OWNER_ACTION=<none-or-specific>
```
