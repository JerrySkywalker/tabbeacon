# TB-G17 — Title Ownership and Conflict Remediation

## Status

PLANNED. Depends on accepted TB-G15 classification and TB-G16 animation
behavior from the reconciled post-release v0.3 sequence.

## Purpose

Turn title-authority diagnosis into safe remediation for Windows Terminal and shell-side contention without broad user-configuration mutation.

## Required classifications

The implementation must be able to distinguish at least:

```text
application-title suppressed by Windows Terminal policy
static/custom tab title policy
visible title overwritten after TabBeacon write
unknown external writer
healthy TabBeacon authority
```

Do not claim a specific shell/plugin writer unless the evidence proves it.

## Windows Terminal policy inspection

Read only the bounded settings needed to understand the active/current profile title path, including relevant semantics equivalent to:

```text
suppressApplicationTitle
tabTitle
profile identity/current-session relationship
```

Do not dump unrelated Windows Terminal configuration into diagnostics.

## Remediation contract

### Exact ownership only

Automatic repair is permitted only when TabBeacon can prove the exact target and prior value required for a minimal change.

Any persistent edit must use the same general safety model as Codex integration:

```text
preflight
backup
ownership record
minimal atomic mutation
idempotent repeat
ownership-safe uninstall/restore
```

### Ambiguous profile

If the active Windows Terminal profile cannot be determined safely:

```text
REMEDIATION=DIAGNOSE_ONLY
```

Do not edit every profile.

### Shell-side contention

If visible evidence proves a later shell/application title writer, provide actionable diagnostics. Do not edit PowerShell profiles, starship/oh-my-posh/posh-git configuration, or unrelated startup files automatically without a future explicit ownership design.

### No endless title fight

Do not solve persistent contention by shortening the frame interval indefinitely or adding an unbounded reassert loop. Bounded convergence is allowed; continuous contention remains a warning/degraded title channel until the conflicting writer is resolved.

## Guided setup integration

`tabbeacon setup` should surface title-authority problems before Apply where practical and explain the difference between:

```text
Codex title ownership
Windows Terminal application-title policy
external/shell contention
```

If an ownership-safe Windows Terminal fix is available, present it as a distinct explicit action/choice rather than silently applying it as a side effect of unrelated presentation settings.

## Diagnostics

Expose a concise remediation summary in human and JSON status/doctor outputs without leaking unrelated settings.

Example conceptual fields:

```text
title.authority
title.conflict_class
title.remediation_available
title.remediation_scope
```

## Validation

Cover at least:

```text
suppressApplicationTitle=true
tabTitle/static policy where relevant
normal application-title allowed profile
external overwrite simulation
ambiguous active profile
ownership-safe apply/repeat/uninstall
unowned config drift refusal
```

Use exact-head Visual proof for any change that alters the visible title behavior or remediation oracle.

## Exit gate

```text
WT_TITLE_POLICY_DIAGNOSIS=PASS
SUPPRESSED_TITLE_CLASSIFICATION=PASS
CONTENDED_TITLE_CLASSIFICATION=PASS
AMBIGUOUS_PROFILE_FAIL_CLOSED=PASS
OWNERSHIP_SAFE_REMEDIATION=PASS
UNOWNED_SHELL_PROFILE_MUTATED=false
ENDLESS_TITLE_FIGHT=false
```

## Exit receipt

```text
GOAL_ID=TB-G17
DISPOSITION=<PASS|FAIL|BLOCKED|UNPROVEN>
STARTING_MAIN=<sha>
EXPECTED_HEAD=<sha>
WT_POLICY_DIAGNOSIS=<...>
CONTENTION_DIAGNOSIS=<...>
REMEDIATION=<PASS|N/A|BLOCKED>
UNOWNED_CONFIG_MUTATED=false
CI=<...>
VISUAL=<...|N/A>
OWNER_ACTION=<none-or-specific>
```
