# TB-G15 — Title Authority Observatory

## Status

PLANNED. First implementation Goal in the v0.3 Codex Presentation Reliability & Motion track. Start only after v0.2 release closure reaches a terminal accepted baseline.

## Purpose

Turn title reliability from an inferred property into an observable one. Reproduce and classify the dogfood failure where tab color follows TabBeacon state but the visible title remains or reverts to `PowerShell` / `Administrator: PowerShell`.

## Starting invariant

The implementation must preserve:

```text
DAILY_COMMAND=codex
NO_GLOBAL_DAEMON=true
FAIL_OPEN=true
OWNER_SHELL_PROFILE_MUTATION=false
```

## Deliverables

### Typed title-authority model

Introduce a bounded operational classification such as:

```text
healthy
suppressed
contended
unavailable
unverified
```

The exact type names may differ, but meanings must remain distinct.

### Visible-title probe

Build a trusted Windows Terminal observation seam that can:

1. write a unique safe test title through the production title path;
2. observe the visible tab title using the existing trusted Windows/UIA infrastructure;
3. sample the visible title over a bounded timeline (nominally around 50/150/300/750 ms or another evidence-backed schedule);
4. distinguish never-visible suppression from visible-then-overwritten contention;
5. clean up the test title and any owned test process/window.

Do not claim arbitrary shell-writer identity unless evidence supports it.

### Diagnostics integration

Expose the classification in `status`, `status --json`, `doctor`, and `doctor --json` without leaking unrelated Windows Terminal settings or shell-profile contents.

Example machine-oriented fields may include:

```text
title.desired_owner
title.codex_writer
title.application_title_policy
title.visible_probe
title.authority
```

### Regression reproduction

Add `TB-REG-TITLE-OWNERSHIP-001` covering at least:

```text
normal PowerShell
Administrator: PowerShell
Git workspace
ordinary non-Git workspace
```

A passing diagnostic must not coexist with a visible title stuck at native PowerShell text after TabBeacon has declared title ownership.

## Non-goals

- changing default animation cadence;
- editing Windows Terminal settings;
- editing PowerShell profile;
- fixing every identified contention source in this Goal;
- Claude/OpenCode/App Server work.

## Validation

Use risk-based Fast Lane:

- focused diagnostics/probe tests;
- one trusted Windows Terminal integration proof;
- exact-head Visual only for any changed visible presentation/oracle behavior;
- one final hosted code CI;
- no repeated auditor cycles after unchanged evidence.

## Exit gate

```text
TITLE_AUTHORITY_MODEL=PASS
VISIBLE_TITLE_PROBE=PASS
SUPPRESSED_VS_CONTENDED_DISTINCTION=PASS
NORMAL_POWERSHELL_REPRO=PASS
ADMIN_POWERSHELL_REPRO=PASS
DIAGNOSTICS_PRIVACY=PASS
TB_REG_TITLE_OWNERSHIP_001=REPRODUCED_AND_CLASSIFIED
```

A product fix is not required yet; a truthful reproducible diagnosis is.

## Exit receipt

```text
GOAL_ID=TB-G15
DISPOSITION=<PASS|FAIL|BLOCKED|UNPROVEN>
STARTING_MAIN=<sha>
EXPECTED_HEAD=<sha>
TITLE_AUTHORITY_MODEL=<...>
VISIBLE_TITLE_PROBE=<...>
NORMAL_POWERSHELL=<...>
ADMIN_POWERSHELL=<...>
ROOT_CAUSE_CLASS=<value>
CI=<...>
VISUAL=<...|N/A>
OWNER_ACTION=<none-or-specific>
```
