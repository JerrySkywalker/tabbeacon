# Fast Lane — Risk-Based Development Governance

## Purpose

The repository originally used a deliberately conservative evidence-first process while bootstrap, Windows Terminal visual infrastructure, Hook trust behavior, and release mechanics were still being established. Those foundations now exist.

For routine post-v0.1 development, repeating the complete audit/CI stack after every small change creates more delay than signal. This Fast Lane keeps the safety boundaries that protect users while removing repeated evidence that does not change the decision.

This document supplements `AGENTS.md` and `QUALITY_GATES.md`.

## Core rule

Validation follows **changed risk**, not Goal ceremony.

Run the smallest set of checks that can falsify the changed behavior, once at the final candidate head. Add stronger gates only when the change crosses the corresponding risk boundary.

Do not repeat a gate merely because a documentation receipt or PR description changed after the already-tested code head. Acceptance metadata should prefer PR text or external receipts instead of creating a new code SHA solely to restate evidence.

## Change classes

### Class D — documentation / planning only

Examples:

- roadmap text;
- ADRs;
- goal contracts;
- prose-only README/docs changes.

Default validation:

- diff/format sanity only;
- no Rust build/test required;
- no Visual CI;
- no L4;
- no separate auditor unless the change modifies security/governance policy materially.

A governance change that alters future gate requirements should receive one ordinary PR review/CI cycle under the rules in effect before that change is merged.

### Class C — ordinary internal code

Examples:

- pure identity/reconciliation helpers;
- typed settings logic;
- internal worker state that does not change visible bytes or external configuration.

Default validation:

- focused tests for changed subsystem;
- `cargo fmt --check`;
- Clippy/build only as needed for changed Rust scope or once through hosted CI;
- one final-head hosted code CI;
- no Visual CI unless presentation behavior changes;
- no L4 unless real provider integration changes.

### Class V — presentation-visible change

Examples:

- title grammar/glyphs;
- progress semantics;
- tab/frame color;
- VT encoding;
- animation behavior;
- visual fixtures/oracles that define product appearance.

Required additional gate:

- one exact-final-head L3 Visual CI.

Do not run Visual CI for changes that cannot alter presentation.

### Class P — provider/configuration/trust boundary change

Examples:

- Hook declaration set or command hash changes;
- provider wire-schema/profile changes;
- setup/uninstall ownership behavior;
- trust-sensitive migration;
- real provider ingress semantics.

Required additional gate:

- focused L4/provider smoke at the final candidate head when the changed behavior cannot be proven synthetically;
- Owner trust only when an actual declaration hash changed and the official product requires review.

Do not re-request Owner trust when declarations are unchanged.

### Class R — release candidate

`TB-G14` and any public release candidate use the full applicable closure matrix:

- L0 policy;
- full Rust/static/build suite;
- functional integration;
- Visual CI;
- real Codex/provider smoke;
- packaging/release checks;
- exact-head source binding.

The full matrix belongs here, not on every intermediate Goal.

## One-final-head rule

During implementation, developers may run focused local tests freely.

The governed acceptance sequence should normally be:

```text
implement
-> focused local tests
-> settle candidate
-> push once/few times as needed
-> one final-head hosted CI
-> Visual/L4 only if required by change class
-> merge
```

Avoid:

```text
full local suite
-> CI
-> metadata commit
-> full local suite again
-> CI again
-> auditor re-check
-> second auditor re-check
```

unless the code or relevant evidence actually changed.

## Blocker latch

A blocker is identified by a stable fingerprint containing the blocker class plus the relevant head/evidence root/required Owner action.

After one confirmation of an unchanged blocker:

```text
BLOCKER_LATCHED=true
```

Do not perform another full audit until one of these changes:

- source/PR head;
- trust state;
- Owner evidence;
- external prerequisite;
- authoritative main affecting the goal.

Repeated observation of the same unchanged blocker is not progress evidence.

## Audit policy

A dedicated auditor/reviewer pass is required only when at least one applies:

- destructive external configuration writes;
- security/privacy boundary changes;
- concurrency/ownership changes with plausible cross-session corruption;
- release/publication;
- a prior test exposed an ambiguous defect;
- the Implementer explicitly requests independent review.

Routine scoped changes do not need a separate audit role after deterministic tests and the required risk gates pass.

## PR policy

Use focused PRs for independently revertible production changes, but do not split work solely to manufacture more governance checkpoints.

A sequential train may complete several small, low-risk goals when:

- dependency order is preserved;
- each milestone is internally testable;
- no Owner decision/trust gate is crossed silently;
- the final merged state remains easy to revert or diagnose.

## Evidence policy

Prefer concise receipts. Do not create a source commit only to change `BLOCKED` to `PASS` after external evidence arrives unless source documentation itself must truthfully change for users.

PR body, issue/goal receipt, or durable external evidence may record acceptance without invalidating a tested code SHA.

## Safety rules retained

Fast Lane does NOT relax:

- one active writer per worktree/branch;
- preservation of foreign work;
- fail-open Codex usability;
- no Hook trust bypass;
- exact ownership before destructive config writes;
- no secret/prompt/tool-content persistence outside explicit product design;
- no fake `codex.exe`, PATH shadow, PTY wrapper, or hidden launcher;
- no global resident daemon as the default animator architecture;
- exact-head binding for the gates that are actually required;
- full release closure at `TB-G14`.

## Expected effect for remaining v0.2 work

- `TB-G10A`: focused identity tests + one code CI + one non-Git integration smoke; Visual only if visible composition semantics change.
- `TB-G11`: focused worker tests + one final code CI + one final Visual CI + focused provider smoke; no repeated full audits between worker iterations.
- `TB-G12`: focused wizard tests; Visual only for preview/presentation deltas; L4 only for setup/trust behavior actually changed.
- `TB-G13`: focused schema/diagnostic tests + code CI; no Visual by default.
- `TB-G14`: full closure matrix.
