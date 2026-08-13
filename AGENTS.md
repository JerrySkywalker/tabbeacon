# AGENTS.md — TabBeacon development governance

This repository uses a VMCell-style evidence-first workflow adapted for a terminal UI project.

## 1. Writer model

- Ordinary development has exactly one active Implementer writer.
- Architect, supervisor, reviewer, and auditor roles are read-oriented unless a goal explicitly transfers write authority.
- Do not run competing writers against the same worktree or branch.
- Git writes belong to the active Implementer for the goal.

## 2. Scope discipline

Every implementation goal must define:

- exact repository and starting head;
- allowed files or subsystem scope;
- acceptance criteria;
- validation commands;
- expected evidence;
- explicit non-goals.

Do not fold unrelated cleanup into a goal because it is nearby.

## 3. Branch and merge discipline

`TB-B00` is the one bootstrap exception because the repository began empty. After bootstrap:

1. start from current `main`;
2. require a clean working tree;
3. create a focused feature branch;
4. implement and validate locally;
5. push the exact candidate commit;
6. open a PR;
7. accept CI only when it is bound to that exact PR head SHA;
8. audit evidence before merge;
9. merge intentionally;
10. verify local/remote `main` after merge.

## 4. Exact-head evidence

A green run is insufficient unless its checkout SHA equals the candidate SHA.

Use these concepts consistently:

- `CODE_HEAD`: SHA validated by code/logic CI.
- `VISUAL_HEAD`: SHA validated by visual CI when visual CI is required.
- `EXPECTED_HEAD`: candidate PR head SHA.

For a release candidate, every required head must equal `EXPECTED_HEAD`.

Cancelled, skipped, neutral, superseded, wrong-runner, wrong-checkout, or wrong-SHA evidence does not count as PASS.

## 5. Evidence dispositions

Use only explicit dispositions:

- `PASS` — requirement proved by evidence;
- `FAIL` — requirement disproved;
- `BLOCKED` — required validation could not execute because of an external/precondition blocker;
- `UNPROVEN` — evidence is insufficient to claim either pass or fail.

Do not silently convert `UNPROVEN` into success.

## 6. Product invariants

Implementations must preserve:

- zero workflow change for daily agent launch;
- fail-open agent usability;
- offline-first repository identity;
- provider-neutral core state;
- provider/backend isolation;
- typed terminal presentation state;
- visual behavior testability.

## 7. Provider boundaries

The core consumes normalized evidence. It must not depend on provider-specific event types.

Provider implementations may have multiple backends, for example:

- Codex: hooks (default) and app-server (experimental);
- Claude: hooks, with richer sources added only when needed;
- OpenCode: plugin and potentially SSE.

A backend must declare its capabilities and evidence authority. Heuristics must not masquerade as authoritative state.

## 8. Visual changes

Once `TB-G03` lands, changes to tab title, progress behavior, state palette, VT encoding, or Windows Terminal presentation require visual evidence at the exact candidate head.

Do not approve a visual change from prose or unit tests alone when the visual gate is applicable.

## 9. Destructive/configuration writes

Setup/uninstall code must prove ownership before overwriting or deleting external configuration. Preserve unrelated user hooks and settings. Never bypass hook trust/review mechanisms.

## 10. Completion format

A completed governed goal should report at least:

```text
DISPOSITION=<PASS|FAIL|BLOCKED|UNPROVEN>
GOAL_ID=<id>
EXPECTED_HEAD=<sha>
CODE_HEAD=<sha-or-N/A>
VISUAL_HEAD=<sha-or-N/A>
LOCAL_VALIDATION=<PASS|FAIL|BLOCKED|UNPROVEN>
CI=<PASS|FAIL|BLOCKED|UNPROVEN>
VISUAL_CI=<PASS|FAIL|BLOCKED|UNPROVEN|N/A>
UNRELATED_DRIFT_TOUCHED=<true|false>
OWNER_ACTION=<none-or-specific-action>
```
