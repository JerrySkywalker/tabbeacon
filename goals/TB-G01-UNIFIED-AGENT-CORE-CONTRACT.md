# TB-G01 — Unified Agent Core Contract

## Goal

Define and test the provider-neutral evidence contract and deterministic
per-session reconciliation model. This goal establishes the Rust API boundary
that future provider backends normalize into; it does not observe a provider or
render a terminal.

## Starting point

- Repository: `JerrySkywalker/tabbeacon`
- STARTING_MAIN=`215e12eff470c4dfc012f407d65561059679c8cd`
- Feature branch: `tb-g01-unified-agent-core-contract`

## Authorized scope

- `src/core/**`;
- `src/lib.rs`, only if an export or crate-level documentation adjustment is
  required for the core public API;
- focused core-contract tests under `tests/`;
- this goal contract.

No dependency, workflow, provider, presentation, repository-identity, setup,
or external configuration change is authorized unless it is separately
approved.

## Contract to implement

1. Expose provider-neutral public types for `AgentProvider`,
   `AgentSessionKey`, `EvidenceSource`, `EvidenceAuthority`,
   `EvidenceConfidence`, `BackendCapabilities`, `AgentEvidence`, `Phase`,
   `Attention`, `Health`, `StatePatch`, and `SessionSnapshot`/reconciler
   result.
2. Canonical session identity is exactly provider plus non-empty native session
   ID. A PID, cwd, repository identity, terminal tab, or provider-specific raw
   event name is not part of the key or core API.
3. Model state on three independent axes:
   `Phase::{Ready, Working, WaitingUser, Ended}`;
   `Attention::{None, ResultReady, Approval, Question}`; and
   `Health::{Normal, Warning, Interrupted, Failed}`. In particular,
   `Working + Warning` must be representable.
4. Model every patch field as one of unchanged, set, or explicit clear. An
   omitted field must never clear prior state. An explicit clear resets only
   that axis to its documented neutral value and carries normal reconciliation
   provenance.
5. Associate each evidence record with a normalized source, explicit authority
   and confidence classes, an observation time, and a stable provider-neutral
   tie-break key. Backend capabilities must state which axes and authority
   levels a backend can assert; a backend declaration is not itself evidence.
6. Reconcile each axis independently and deterministically. For a field with
   an existing winner, reject an older observation and reject weaker authority.
   For equal authority and timestamp, choose the documented stable
   confidence then source/tie-break ordering rather than arrival order. A
   stronger observation must also be no older than the current winner.
   Confidence is a tie-quality discriminator; it must not let a weaker
   authority displace a stronger one. Applying an identical evidence record
   repeatedly is idempotent.
7. Preserve winning provenance per axis sufficiently to enforce the stale and
   authority rules. A heuristic cannot override an authoritative or lifecycle
   winner merely because it arrives later. A stale attention assertion cannot
   revive after a stronger or equally authoritative newer clear/contradiction.
8. Keep all provider-specific transport types, event names, hooks, app-server
   concerns, strings intended for terminal control sequences, filesystem/Git
   identity, and OS handles outside this public core contract.

## Acceptance criteria

1. The contract compiles as a dependency-free Rust core API under the pinned
   `1.97.1` toolchain and is publicly documented where semantics are not
   self-evident.
2. Focused tests prove session-key equality and separation, including the fact
   that no process identifier is needed for canonical identity.
3. Focused tests prove independent phase/attention/health updates, including
   `Working + Warning`.
4. Focused tests distinguish unchanged from explicit clear.
5. Focused tests prove deterministic reconciliation for freshness, authority,
   equal-time ties, stale evidence, and repeated identical input.
6. Focused tests prove a heuristic cannot displace an authoritative/lifecycle
   winner and that a stale attention event cannot revive after a valid newer
   clear.
7. No product integration appears outside the authorized scope. In particular,
   Codex hooks, Windows Terminal VT output, offline repository identity, and
   all TB-G02+ functionality remain absent.
8. L0/L1 local validation passes, then the focused candidate is pushed in a
   pull request and the hosted Windows exact-head code CI reports the same
   candidate SHA as `EXPECTED_HEAD` and `CODE_HEAD`.

## Required validation and evidence

Before PR creation, run:

```text
pwsh -NoProfile -File .\\scripts\\ci\\run-local-ci.ps1
```

After committing the candidate, run the same command with
`-ExpectedHead <candidate SHA>`. Hosted validation is the existing
`Windows / Hosted / Exact Head` PR job. Visual validation is not yet applicable.

Completion evidence must state:

```text
GOAL_ID=TB-G01
EXPECTED_HEAD=<candidate SHA>
CODE_HEAD=<candidate SHA>
VISUAL_HEAD=N/A
LOCAL_VALIDATION=<PASS|FAIL|BLOCKED|UNPROVEN>
CI=<PASS|FAIL|BLOCKED|UNPROVEN>
VISUAL_CI=N/A
UNRELATED_DRIFT_TOUCHED=false
```

## Explicit non-goals

- Codex hooks, Codex App Server, or any other Codex provider implementation;
- Claude or OpenCode provider implementation, including hooks, plugins, SSE,
  or another backend;
- Windows Terminal integration, VT/OSC rendering, title/progress/color policy,
  or visual CI;
- repository discovery, repository abbreviation, Git/worktree identity, or
  local identity state;
- setup, doctor, uninstall, configuration mutation, process management,
  telemetry, or network access;
- TB-G02 and every later roadmap goal.

## Completion rule

TB-G01 is complete only after an exact candidate commit is reviewed through a
PR and the required hosted exact-head code CI is PASS for that commit. Local
tests alone are insufficient for completion.
