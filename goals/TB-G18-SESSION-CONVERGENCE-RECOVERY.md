# TB-G18 — Session Convergence and Recovery

## Status

ACTIVE on Draft PR #29. TB-G15 through TB-G17 are complete. G18 has a 32-row
traceability catalog plus typed run-bound evidence verification, but the initial attempt
to make every row an independent executor/release gate was deliberately retired by the
Fast Lane v2 amendment below.

## Purpose

Prove that important Codex/session lifecycle paths converge to the correct visible
Windows Terminal presentation while keeping validation proportional to materially
different risks.

The goal is not to build a second product whose purpose is validating TabBeacon.

## Core convergence invariant

For a supported healthy title channel:

```text
CONVERGENCE_DEADLINE_MS<=1000
```

Working must show at least three distinct valid spinner frames within that bound and keep
the workspace alias stable.

## 32-row matrix role

`src/convergence.rs` remains a useful traceability/coverage catalog. It enumerates
lifecycle, recovery, workspace, isolation, UIA, and elevated obligations.

It is **not** normative that every row have:

- a dedicated executor;
- a separate durable artifact;
- an independent release gate;
- a fresh exact-head UIA run.

`src/convergence_evidence.rs` may remain as a strict diagnostic/evidence tool, but G18
acceptance is defined by the risk-family gates below. Do not continue building 32
independent executors merely to satisfy the earlier experimental verifier contract.

## Normative risk-family acceptance

### Family 1 — Lifecycle semantics

Covers the admitted lifecycle surface, including:

```text
SessionStart startup/resume/clear
UserPromptSubmit
PreCompact/PostCompact
SubagentStart/SubagentStop
Stop/result-ready
PermissionRequest/approval
SessionEnd
Question/Ctrl+C only to admitted Hook fidelity
```

Proof: existing deterministic/provider Hook fixtures plus focused tests.

Required result:

```text
LIFECYCLE_FAMILY=PASS
```

No separate durable receipt per event is required.

### Family 2 — Generation and isolation

Covers:

```text
new-turn supersession
stale-event rejection
root/subagent isolation
different-repository tabs
same-repository tabs
same-workspace parallel sessions
```

Proof: focused deterministic ownership/generation tests. Add real UIA only if this Goal
changes the presentation/session binding implementation itself.

Required result:

```text
GENERATION_ISOLATION=PASS
```

### Family 3 — Workspace identity

Deterministically cover:

```text
Git workspace
linked worktree
non-Git workspace
HOME
```

First inspect accepted UIA evidence for a real Git/worktree alias flowing through the
production presentation path, then compare its head with the candidate across only the
workspace/presentation paths that could invalidate that proof. If that relevant risk diff is
empty, the accepted representative UIA proof may be reused; otherwise run one representative
owned Windows Terminal/UIA workspace smoke.

Required result:

```text
WORKSPACE_MATRIX=PASS
REPRESENTATIVE_WORKSPACE_UIA=PASS|REUSED
```

Do not require four independent full UIA runs solely because the identity source differs.

### Family 4 — Normal visible convergence

Reuse accepted normal-PowerShell UIA evidence only when it proves every transition below and
the candidate has an empty presentation-risk diff. Otherwise run one owned normal-PowerShell
Windows Terminal/UIA pack on the settled candidate. One bounded pack proves the important
transitions:

```text
Working -> >=3 distinct braille frames within 1s
Stop -> stable result-ready title
PermissionRequest -> stable approval/attention title
workspace alias stable
title authority healthy
owned fixture cleanup
```

Required result:

```text
NORMAL_POWERSHELL_VISIBLE_CONVERGENCE=PASS|REUSED
VISIBLE_WORKING_FRAMES_GE_3=PASS
WORKSPACE_ALIAS_STABLE=true
```

This is the primary visible G18 proof. Do not split it into one UIA gate per lifecycle or
workspace row.

### Family 5 — Recovery

Covers:

```text
terminal close
worker crash
Codex disappearance
newer generation
binary relocation/upgrade
settings animated -> static/native/off
```

Proof: focused deterministic worker/ownership tests. Accepted prior terminal-close or
performance evidence may be reused when worker/cleanup/timing paths are unchanged.

Required result:

```text
RECOVERY_FAMILY=PASS
NO_STALE_WORKER=PASS
```

### Family 6 — Actual elevated PowerShell

One actual elevated Windows token is required. Synthetic `Administrator: PowerShell`
labels do not count.

Run one bounded elevated owned UIA pack proving:

```text
actual elevated token=true
working >=3 frames
stable alias
result-ready convergence
title authority healthy
cleanup pass
```

The Owner may perform this with one prepared pasteable elevated PowerShell command.
Do not bypass UAC or weaken system security.

Required result:

```text
ADMIN_POWERSHELL=PASS_ACTUAL_ELEVATED
```

## Primary regression closure

`TB-REG-TITLE-OWNERSHIP-001` closes when normal and actual elevated PowerShell each show
healthy visible TabBeacon authority, or when an intentionally unsupported/degraded
channel is truthfully classified without a false health claim.

The intended supported result is healthy authority in both contexts.

## Validation policy

G18 follows Fast Lane v2 in `dev_governance_files/QUALITY_GATES.md`.

During implementation:

- run focused affected tests;
- fix mechanical formatting directly;
- do not repeatedly run full CI/UIA;
- do not add a dedicated auditor unless a qualifying risk boundary changes.

After the candidate settles:

1. one final hosted code CI;
2. reuse accepted representative/normal UIA evidence when its exact proof and relevant risk
   diff permit it; otherwise one combined normal owned UIA convergence pack;
3. one actual elevated owned UIA pack;
4. reuse earlier G15–G17 / terminal-close / performance evidence when the relevant risk diff is empty.

The existing `LOCAL_INTERACTIVE_CAPTURE_PREFLIGHT_BLOCKED` pixel-capture limitation remains
latched unless the environment materially changes. Exact-tab UIA is sufficient for the
title/motion claims defined here.

## Explicitly retired work

Do not spend further engineering time on:

```text
31 non-owner independent scenario executors
31 separate durable scenario artifacts
mandatory 32/32 per-row proof-method binding for G18 release acceptance
repeated exact-head UIA for Git/worktree/non-Git/HOME variants
repeated full CI after documentation-only closeout
```

The existing matrix/verifier may remain for diagnostics and future deeper testing, but it
must not block G18 solely because every traceability row lacks a standalone artifact.

## Exit gate

```text
CONVERGENCE_DEADLINE_MS<=1000
LIFECYCLE_FAMILY=PASS
GENERATION_ISOLATION=PASS
WORKSPACE_MATRIX=PASS
REPRESENTATIVE_WORKSPACE_UIA=PASS|REUSED
NORMAL_POWERSHELL_VISIBLE_CONVERGENCE=PASS|REUSED
VISIBLE_WORKING_FRAMES_GE_3=PASS
WORKSPACE_ALIAS_STABLE=true
RECOVERY_FAMILY=PASS
NO_STALE_WORKER=PASS
ADMIN_POWERSHELL=PASS_ACTUAL_ELEVATED
TB_REG_TITLE_OWNERSHIP_001=CLOSED
CODE_CI=PASS
```

## Exit receipt

```text
GOAL_ID=TB-G18
DISPOSITION=<PASS|FAIL|BLOCKED|UNPROVEN>
STARTING_MAIN=<sha>
EXPECTED_HEAD=<sha>
CONVERGENCE_DEADLINE_MS=<n>
LIFECYCLE_FAMILY=<...>
GENERATION_ISOLATION=<...>
WORKSPACE_MATRIX=<...>
REPRESENTATIVE_WORKSPACE_UIA=<...>
NORMAL_POWERSHELL_VISIBLE_CONVERGENCE=<...>
VISIBLE_WORKING_FRAMES=<n>
WORKSPACE_ALIAS_STABLE=<...>
RECOVERY_FAMILY=<...>
ADMIN_POWERSHELL=<...>
TB_REG_TITLE_OWNERSHIP_001=<...>
CODE_CI=<...>
PIXEL_CAPTURE=<REUSED_BLOCKER|RECOVERED|N/A>
OWNER_ACTION=<none-or-specific>
```

Estimated remaining effective engineering effort after this amendment: **4–7 h**, assuming
the actual elevated run does not reveal a new platform defect.
