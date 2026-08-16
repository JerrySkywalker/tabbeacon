# AGENTS.md — TabBeacon development governance

This repository uses an evidence-first workflow adapted for a terminal UI project. The original conservative VMCell-style process remains the safety baseline, while routine post-v0.1 work uses the risk-based Fast Lane in `dev_governance_files/FAST_LANE.md`.

## 1. Writer model

- Ordinary development has exactly one active Implementer writer per worktree/branch.
- Architect, supervisor, reviewer, and auditor roles are read-oriented unless a goal explicitly transfers write authority.
- Do not run competing writers against the same worktree or branch.
- Preserve foreign/local work; never reset or clean it merely to make a goal proceed.
- Git writes belong to the active Implementer for the goal.

## 2. Scope discipline

Every implementation goal must define:

- exact repository and starting head;
- allowed files or subsystem scope;
- acceptance criteria;
- risk class / required gates;
- explicit non-goals.

Do not fold unrelated cleanup into a goal because it is nearby.

## 3. Branch and merge discipline

After bootstrap:

1. start from current authoritative `main`;
2. require a clean owned worktree;
3. create a focused branch when the change is independently revertible;
4. implement and run focused local tests while iterating;
5. settle one final candidate head;
6. push/open PR;
7. run only the gates required by the changed risk surface;
8. accept evidence only when required exact-head bindings match;
9. merge intentionally;
10. verify remote `main` after merge.

Do not split work or create extra commits solely to manufacture more governance checkpoints.

## 4. Risk-based Fast Lane

The authoritative routine-development policy is `dev_governance_files/FAST_LANE.md`.

Default principles:

- documentation-only changes do not require Rust build/test, Visual CI, or provider L4;
- ordinary internal code gets focused tests plus one final-head hosted code CI;
- Visual CI is required only when title/progress/color/VT/animation presentation can change;
- provider L4 is required only when provider/config/trust boundaries change or a focused real integration behavior cannot be proven synthetically;
- unchanged blockers latch after one audit and are not re-audited until relevant state changes;
- dedicated auditors are reserved for destructive configuration, security/privacy, concurrency/ownership, ambiguous defects, and release/publication work;
- `TB-G14` release closure still uses the full applicable evidence matrix.

The goal is fewer repeated checks, not weaker correctness.

## 5. Exact-head evidence

For any gate that is required, a green run is insufficient unless its checkout SHA equals the candidate SHA.

Use these concepts consistently when applicable:

- `CODE_HEAD`: SHA validated by code/logic CI;
- `VISUAL_HEAD`: SHA validated by visual CI;
- `EXPECTED_HEAD`: candidate PR head SHA.

Cancelled, skipped, neutral, superseded, wrong-runner, wrong-checkout, or wrong-SHA evidence does not count as PASS.

Do not create a new source commit solely to restate already-proven acceptance metadata when PR text or a durable receipt can carry that evidence.

## 6. Evidence dispositions

Use only explicit dispositions:

- `PASS` — requirement proved by evidence;
- `FAIL` — requirement disproved;
- `BLOCKED` — required validation could not execute because of an external/precondition blocker;
- `UNPROVEN` — evidence is insufficient to claim either pass or fail.

Do not silently convert `UNPROVEN` into success.

## 7. Product invariants

Implementations must preserve:

- zero workflow change for daily agent launch (`codex` remains `codex`);
- fail-open agent usability;
- offline-first **workspace identity**, with existing Git repository identity preserved as the stable Git specialization;
- provider-neutral core state;
- provider/backend isolation;
- typed terminal presentation state;
- visual behavior testability;
- no hidden launcher, PATH shadow, PTY wrapper, or global resident daemon baseline.

## 8. Provider boundaries

The core consumes normalized evidence. It must not depend on provider-specific event types.

Provider implementations may have multiple backends, for example:

- Codex: hooks (default) and app-server (experimental);
- Claude: hooks, with richer sources added only when needed;
- OpenCode: plugin and potentially SSE.

A backend must declare its capabilities and evidence authority. Heuristics must not masquerade as authoritative state.

## 9. Visual changes

Changes that can alter tab title, progress behavior, state palette, VT encoding, animation, or Windows Terminal presentation require exact-head visual evidence.

Do not run Visual CI for changes that cannot alter presentation, and do not approve an applicable visual change from prose or unit tests alone.

## 10. Destructive/configuration writes

Setup/uninstall/migration code must prove ownership before overwriting or deleting external configuration. Preserve unrelated user hooks and settings. Never bypass hook trust/review mechanisms.

Real Owner configuration must not be mutated by unattended tests unless the goal explicitly authorizes that production action.

## 11. Blocker latch

Once an unchanged blocker is confirmed, record a stable blocker fingerprint and set:

```text
BLOCKER_LATCHED=true
```

Re-evaluate only after relevant source head, trust state, Owner evidence, authoritative main, or external prerequisite changes. Repeating the same full audit is not progress.

## 12. Completion format

Use a concise receipt containing only gates relevant to the change. A typical code goal reports:

```text
DISPOSITION=<PASS|FAIL|BLOCKED|UNPROVEN>
GOAL_ID=<id>
EXPECTED_HEAD=<sha>
CODE_HEAD=<sha-or-N/A>
CI=<PASS|FAIL|BLOCKED|N/A>
VISUAL_CI=<PASS|FAIL|BLOCKED|N/A>
L4=<PASS|FAIL|BLOCKED|N/A>
UNRELATED_DRIFT_TOUCHED=<true|false>
OWNER_ACTION=<none-or-specific-action>
```

Do not require irrelevant fields merely for ceremony.
