# AGENTS.md — TabBeacon development governance

This repository uses evidence-first, risk-based development governance. The conservative
VMCell-style ownership model remains the safety baseline; gate selection is governed by
`dev_governance_files/QUALITY_GATES.md` (Fast Lane v2). `FAST_LANE.md` is only a compact
execution reference.

## 1. Writer model

- Ordinary development has exactly one active Implementer writer per worktree/branch.
- Architect, supervisor, reviewer, and auditor roles are read-oriented unless a Goal explicitly transfers write authority.
- Do not run competing writers against the same worktree or branch.
- Preserve foreign/local work; never reset or clean it merely to make a goal proceed.
- Git writes belong to the active Implementer for the goal.

## 2. Scope discipline

Every implementation Goal must define:

- exact repository and starting head;
- allowed files or subsystem scope;
- acceptance criteria;
- changed-risk vector / required gates;
- explicit non-goals.

Do not fold unrelated cleanup into a Goal because it is nearby.

## 3. Branch and merge discipline

After bootstrap:

1. start from current authoritative `main` or an explicitly admitted predecessor branch;
2. require a clean owned worktree;
3. create/use one focused branch when the change is independently revertible;
4. implement with focused tests while iterating;
5. settle one final candidate head;
6. push/open or update the focused PR;
7. run only gates selected by `QUALITY_GATES.md`;
8. reuse accepted evidence when its relevant risk surface is unchanged;
9. merge intentionally after required gates pass;
10. verify remote `main` after merge.

Do not split work or create extra commits solely to manufacture governance checkpoints.

## 4. Fast Lane v2

Before validation classify:

```text
CODE_CHANGED
PRESENTATION_CHANGED
PROVIDER_CHANGED
USER_PERSISTENT_CONFIG_CHANGED
SECURITY_OR_PRIVACY_CHANGED
RELEASE_BOUNDARY
```

Mandatory principles:

- validation is risk-surface based;
- one invariant/failure family normally receives one representative proof;
- a traceability matrix is not automatically a set of independent release gates;
- ordinary code gets focused tests plus one final hosted code CI;
- presentation changes add one final owned UIA/Visual pack;
- provider L4 is required only for a changed real provider/profile/trust boundary or an otherwise unprovable real-provider claim;
- persistent user-configuration changes add one focused ownership/restore/drift safety family;
- unchanged-risk evidence may be reused across heads after a bounded risk diff;
- documentation-only acceptance updates do not invalidate unrelated code/Visual/L4 evidence;
- dedicated auditors are exceptional, not a default pipeline stage;
- an unchanged blocker is latched and not repeatedly re-audited.

The goal is fewer repeated checks, not weaker correctness.

## 5. Exact-head and reused evidence

Fresh required evidence binds to the settled candidate:

```text
EXPECTED_HEAD == checked_out_head == evidence_head
```

Reused evidence may come from an earlier accepted head only when the relevant risk diff
is empty and the receipt records the reuse explicitly.

Do not create a source commit solely to restate already-proven acceptance metadata when
PR text or durable evidence can carry it.

## 6. Evidence dispositions

Use explicit dispositions only:

- `PASS` — requirement proved;
- `FAIL` — requirement disproved;
- `BLOCKED` — an external/precondition boundary prevented required execution;
- `UNPROVEN` — evidence is insufficient for pass or fail;
- `REUSED` — an accepted prior proof remains valid because the relevant risk surface did not change;
- `N/A` — the gate is outside the current changed-risk vector.

Do not silently convert `UNPROVEN` into success.

## 7. Product invariants

Implementations must preserve:

- daily agent launch remains literally `codex`;
- fail-open agent usability;
- offline-first workspace identity with Git identity as the stable specialization;
- provider-neutral core state;
- provider/backend isolation;
- typed terminal presentation state;
- visual behavior testability;
- no hidden launcher, fake `codex.exe`, PATH shadow, PTY wrapper, or global resident daemon baseline.

## 8. Provider boundaries

The core consumes normalized evidence and must not depend on provider-specific event types.
Provider backends declare their capabilities and evidence authority. Heuristics must not
masquerade as authoritative state.

A newer Codex version does not inherit support from an admitted version merely because
its version number is greater.

## 9. Visual changes

Changes that can alter tab title, progress, palette, VT bytes, animation, or the product
visual oracle require one final representative owned UIA/Visual proof.

Do not rerun Visual for non-presentation changes. Do not approve an applicable visible
change from prose or internal unit tests alone.

## 10. Destructive/configuration writes

Setup/uninstall/migration/remediation code must prove exact ownership before overwriting
or deleting external configuration. Preserve unrelated user hooks/settings and never
bypass trust/review mechanisms.

Real Owner configuration must not be mutated by unattended tests unless the Goal
explicitly authorizes that production action.

## 11. Audit and blocker discipline

A separate auditor is required only for changed persistent/destructive external writes,
security/privacy boundaries, high-risk concurrency/ownership, ambiguous defects,
release/publication, or explicit Implementer request.

Do not chain auditors over unchanged evidence.

Once an unchanged blocker is confirmed, record a stable fingerprint and:

```text
BLOCKER_LATCHED=true
```

Re-evaluate only after relevant source, evidence, trust state, Owner action, or external
prerequisite changes.

## 12. Completion format

Receipts report only gates relevant to the changed risk. Typical form:

```text
DISPOSITION=<PASS|FAIL|BLOCKED|UNPROVEN>
GOAL_ID=<id>
EXPECTED_HEAD=<sha>
CODE_CI=<PASS|FAIL|N/A>
VISUAL=<PASS|REUSED|N/A>
L4=<PASS|REUSED|N/A>
PERSISTENT_CONFIG_SAFETY=<PASS|REUSED|N/A>
BLOCKER_LATCHED=<true|false>
OWNER_ACTION=<none-or-specific>
```

Do not require irrelevant fields for ceremony.

## Local Rust build-storage discipline

- Daily agent launch remains literally `codex`.
- Temporary and non-canonical TabBeacon worktrees route ordinary Cargo artifacts to
  the admitted shared local target, `V:\build\tabbeacon\codex-target`, instead of
  accumulating worktree-local targets.
- The canonical checkout, `V:\src\tabbeacon`, may retain its local HOT target.
- Goal-specific Cargo targets are forbidden unless technical isolation is explicitly
  required and documented with `REASON`, `EXPECTED_STORAGE_GB`, `TARGET_PATH`, and
  `RETENTION_AFTER_GOAL`.
- Capacity pressure fails safely: never delete repository source, `.git`, worktree
  roots, or evidence as automatic remediation.
- Storage governance must not introduce a Codex wrapper, fake `codex.exe`, PATH
  shadow, PTY host, or global daemon.
