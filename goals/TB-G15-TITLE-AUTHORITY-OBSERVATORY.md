# TB-G15 — Title Authority Observatory

## Status

ACCEPTED PENDING MERGE. The focused implementation candidate at
`0847edb6829b1a1fc859af346071a50bf5568404` has passed its required local,
owned-fixture, exact-head UIA/title, and hosted code validation. PR #25 remains
the only admitted merge path. TB-G16 stays unavailable until that PR's merge is
verified on `main`; the public v0.2 release SHA remains a distinct historical
release identity.

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

Generalize the existing trusted owned-tab UIA seam rather than introduce a
second independent UIA stack. The active probe must be able to:

1. write a unique safe test title through the production title path;
2. observe the visible tab title using the existing trusted Windows/UIA infrastructure;
3. sample the visible title over a bounded timeline (nominally around 50/150/300/750 ms or another evidence-backed schedule);
4. distinguish never-visible suppression from visible-then-overwritten contention;
5. clean up the test title and any owned test process/window.

Do not claim arbitrary shell-writer identity unless evidence supports it.

### Passive diagnostics and explicit active probe

The following principle is canonical:

```text
PASSIVE_DIAGNOSTICS=READ_ONLY
ACTIVE_TITLE_PROBE=EXPLICIT_OPT_IN
COMMON_TYPED_MODEL=true
```

Ordinary `status`, `status --json`, `doctor`, and `doctor --json` remain
read-only and must report a not-requested visible probe as `unverified` rather
than as a passing or failing live check. A separately explicit active title
probe may create an owned fixture, emit one bounded safe title through the
production path, observe the exact correlated tab, and clean it up. The exact
CLI spelling is an implementation decision; it must not rewrite persistent
configuration or transiently rename the Owner's current tab.

### Diagnostics integration

Expose the common classification in `status`, `status --json`, `doctor`, and
`doctor --json` without leaking unrelated Windows Terminal settings or
shell-profile contents. Passive output must distinguish `visible_probe=not_run`
and `authority=unverified` from a classified active result.

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
ADMIN_LABEL_SCENARIO=PASS
ACTUAL_ELEVATED_SCENARIO=<PASS|UNPROVEN|BLOCKED>
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
