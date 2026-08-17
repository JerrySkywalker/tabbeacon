# TB v0.3 — Codex Presentation Reliability & Motion

## Status

COMPLETE. v0.3.0 is publicly released from
`ddb2a218b9fe7601a09caaa1c8c3a0c1d0af9419` under `v0.3.0`.
TB-G15 through TB-G19R are complete: G18 merged through same-SHA recovery PR #30
after its actual elevated-PowerShell proof, G19 merged as PR #31 after exact-head
hosted CI, and G19R completed the verified crates.io, GitHub Release, and Windows
x64 artifact publication.

Planning baseline when this track was created:

```text
public_v0.2_release_sha=0b1d5136833a05bf94b7d32c414a21da2f5ac78e
public_v0.2_tag=v0.2.0
post_release_main=9594c7055120cdb0731d2101f502503e4b30c3d4
v0.2_publication_and_closeout=complete
```

## Product objective

v0.3 remains Codex-first and Windows-Terminal-first. It makes the existing presentation
path visibly reliable in daily use:

1. if TabBeacon claims title ownership, the visible tab title converges or diagnostics truthfully classify why it cannot;
2. normal working state uses real title animation by default;
3. default motion is 100 ms/frame with a stable workspace alias on the right;
4. startup, resume, attention, result, end, terminal close, and worker failure converge deterministically;
5. Codex compatibility remains exact-profile explicit and maintainable.

The daily launch remains literally:

```powershell
codex
```

## Product invariants

```text
DAILY_COMMAND=codex
FAIL_OPEN=true
GLOBAL_DAEMON_BASELINE=false
HOOK_TRUST_BYPASS=false
PROVIDER_NEUTRAL_CORE=true
OFFLINE_WORKSPACE_IDENTITY=true
RAW_PROMPT_TOOL_MODEL_CONTENT_PERSISTED=false
```

## Scope boundaries

In scope:

- title-authority observation and conflict classification;
- regression coverage for titles reverting to PowerShell-like native text;
- default 100 ms braille working animation;
- bounded visible convergence;
- Windows Terminal application-title/profile policy diagnosis;
- ownership-safe remediation only where exact ownership is proven;
- normal and actual elevated PowerShell acceptance;
- exact Codex compatibility registry/maintenance tooling;
- v0.3 hardening and publication.

Deferred:

- Claude/OpenCode provider production work;
- Codex App Server production backend;
- global resident daemon;
- PTY/wrapper/PATH interception;
- remote dashboard;
- hook-health product work;
- cross-session `tabbeacon sessions` UI;
- package-manager/self-update work unless separately promoted.

## Target presentation defaults

```text
title       = tabbeacon
tab_color   = tabbeacon
activity    = title-spinner
spinner     = braille
theme       = muted-dark
frame_ms    = 100
```

Canonical working form:

```text
⠋ OWH
⠙ OWH
⠹ OWH
...
✓ OWH
```

Only the left status slot moves; the workspace alias stays stable.

## Goal dependency DAG

```text
v0.2 public release + post-release closeout
        ↓
TB-G15 — Title Authority Observatory            COMPLETE
        ↓
TB-G16 — Animation Engine v2 and Defaults       COMPLETE
        ↓
TB-G17 — Title Ownership / Conflict Remediation COMPLETE
        ↓
TB-G18 — Session Convergence and Recovery       COMPLETE
        ↓
TB-G19 — Codex Compatibility Registry v2        COMPLETE
        ↓
TB-G19R — v0.3 Hardening and Release            COMPLETE
```

The sequence remains sequential. Fast Lane v2 changes validation density, not dependency
order.

## Fast Lane v2 amendment — 2026-08-17

The initial G18 evidence-hardening experiment showed that requiring one independent
executor/artifact/proof binding for every traceability row creates more validation work
than product signal for this repository.

The accepted governance direction is therefore:

```text
traceability catalog != independent release gates
one material risk family -> one representative proof
evidence may be reused when its relevant risk surface is unchanged
one settled candidate -> one final code CI
one additional final gate per changed risk dimension
```

`dev_governance_files/QUALITY_GATES.md` is authoritative.

### G18 compression

The 32-row matrix remains for traceability, but normative acceptance is six families:

```text
lifecycle semantics
generation/isolation
workspace identity + one representative UIA smoke
normal visible convergence UIA pack
recovery
actual elevated PowerShell pack
```

Do not complete the retired 31-non-owner independent-executor architecture merely to
satisfy the earlier experimental verifier.

### G19 compression

G19 needs only:

```text
minimal typed compatibility registry
existing admitted 0.147.0 profile
supported / known-unadmitted / unknown diagnostics
lightweight relevant-source diff report
```

A new Codex profile admission is not a v0.3 requirement.

### G19R compression

G19R runs one release train. Release-specific code/package/publication gates are fresh;
accepted G15–G19 evidence is reused when its risk surface is unchanged, plus one small
representative integrated dogfood pack.

## Revised remaining effort

The original planning envelope of 22–40 effective engineering hours is retained as
historical planning context, but it is no longer the operational estimate after G15–G17
completion and the Fast Lane v2 amendment.

The release train completed without a remaining v0.3 delivery goal. Future work begins
as a separately admitted maintenance or v0.3.1 candidate goal.

## Execution contract

Each Goal:

1. starts from one accepted predecessor;
2. has one active writer per branch/worktree;
3. declares the changed-risk vector;
4. uses focused tests while iterating;
5. settles one candidate;
6. runs gates selected by `QUALITY_GATES.md` once at the final candidate;
7. reuses accepted unchanged-risk evidence explicitly;
8. records only decision-relevant durable evidence;
9. uses `PASS`, `FAIL`, `BLOCKED`, `UNPROVEN`, `REUSED`, or `N/A` truthfully;
10. merges only when its normative exit gate passes;
11. latches unchanged Owner/external blockers instead of repeatedly auditing them.

Do not manufacture progress by rerunning unchanged CI/audits or by building validation
infrastructure whose only purpose is satisfying a traceability row already covered by a
risk-family proof.

## v0.3 completion definition

```text
DEFAULT_TITLE_OWNER=tabbeacon
DEFAULT_ACTIVITY=title-spinner
DEFAULT_SPINNER=braille
TARGET_FRAME_INTERVAL_MS=100
VISIBLE_WORKING_FRAMES>=3 within 1 second
WORKSPACE_ALIAS_STABLE=true

NORMAL_POWERSHELL=PASS
ADMIN_POWERSHELL=PASS_ACTUAL_ELEVATED
TB_REG_TITLE_OWNERSHIP_001=CLOSED

TITLE_AUTHORITY=HEALTHY
or an intentionally unsupported/degraded channel is truthfully diagnosed

CODEX_PROFILE_POLICY=explicit
NO_GLOBAL_DAEMON=true
DAILY_COMMAND=codex

PACKAGE=PASS
PUBLICATION=PASS
```

A green tab color with a title visibly stuck at native PowerShell text is never a passing
presentation state.
