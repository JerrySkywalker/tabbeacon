# Quality Gates

## Principle

TabBeacon uses risk-based validation. `dev_governance_files/FAST_LANE.md` defines the default post-v0.1 execution policy.

A gate is required because the change can invalidate that class of behavior, not because every Goal must mechanically run every lane.

For every gate that is required, exact-head evidence remains mandatory.

## Gate hierarchy

### L0 — Repository policy

Typical checks:

- expected files/licenses present;
- LF/whitespace policy for changed text;
- no generated runtime state or secret material committed.

For docs-only work, this may be the only required gate.

### L1 — Rust static/build gate

Full repository suite:

```text
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build --locked --all-targets
```

The full suite is mandatory for release candidates and may be delegated to one final-head hosted CI for ordinary code work. During implementation, prefer focused tests over repeatedly running the entire suite.

A small Rust change should normally run:

- `cargo fmt --check`;
- focused affected tests locally;
- one final-head hosted CI covering the full repository suite.

Do not repeat the complete L1 suite after a metadata-only acceptance update when product code did not change.

### L2 — Functional integration gate

Use focused integration tests for the changed subsystem: provider normalization, reconciliation, workspace/repository identity, setup/uninstall, worker lifecycle, configuration, or diagnostics as applicable.

L2 is not a requirement to rerun unrelated integration matrices.

### L3 — Visual gate

Required only for changes that can alter Windows Terminal presentation, including:

- title composition/glyphs;
- activity animation;
- progress semantics;
- tab/frame color;
- VT encoding/reset behavior;
- product visual fixture/oracle semantics.

Requires an approved interactive desktop session and exact-head evidence.

Do not run L3 for code/doc changes that cannot change presentation.

### L4 — Provider E2E gate

Required when the change crosses a production provider/configuration/trust boundary or when a focused real-provider behavior cannot be proven synthetically.

Typical triggers:

- Hook declaration or trusted hash changes;
- admitted Codex wire/profile changes;
- setup/uninstall/migration semantics;
- real Hook lifecycle assumptions;
- final integrated animator/provider smoke;
- release closure.

Do not run L4 merely because provider code exists in the repository. Do not request Owner trust when Hook declarations are unchanged.

## Change-class matrix

| Change class | Focused local | Final hosted CI | L3 Visual | L4 Provider |
| --- | --- | --- | --- | --- |
| Docs/planning only | diff sanity | optional under Fast Lane | no | no |
| Ordinary internal Rust | yes | yes, once at final head | no unless visible | no unless provider boundary |
| Workspace identity fallback | yes | yes | only if title semantics materially change | focused non-Git smoke |
| Animator production | yes | yes | yes | focused integrated smoke |
| Setup/trust changes | yes | yes | only if visible | yes when declaration/real integration changes |
| Diagnostics only | yes | yes | normally no | focused only if needed |
| Release candidate | full | yes | yes | yes |

## Exact-head contract

For every required lane:

```text
EXPECTED_HEAD == checked_out_head == evidence_head
```

For a required visual lane:

```text
EXPECTED_HEAD == CODE_HEAD == VISUAL_HEAD
```

A run is not acceptable evidence when it is cancelled, skipped, neutral, superseded, checked out at a different SHA, or executed on an unapproved runner class.

## One-final-head acceptance

Preferred sequence:

```text
focused implementation tests
-> settle candidate
-> one final hosted code CI
-> optional L3/L4 according to risk
-> merge
```

Avoid rerunning the same full gates after changes that affect only PR text or external evidence receipts.

## Runner identity

Hosted CI may use GitHub-hosted Windows runners for code gates while the code remains compatible with them.

Visual CI must run in an explicitly approved interactive Windows desktop session. Do not treat a service/Session-0 runner as valid visual evidence merely because the job starts.

When self-hosted runners are used, workflows must assert expected runner/machine identity and relevant toolchain paths rather than trusting labels alone.

## Failure classification

Record failures as one of:

- product/code defect;
- test defect;
- runner/environment defect;
- external dependency/service defect;
- evidence mismatch/wrong SHA;
- unproven.

Do not patch product code to compensate for a runner defect without evidence that the product itself is wrong.

## Blocker behavior

After one audit identifies the same unchanged external/Owner blocker, latch it and stop repeating the full audit. Re-run only when source/trust/evidence/prerequisite state changes.

## Release exception

`TB-G14` intentionally restores the complete closure matrix. Fast Lane is for iteration efficiency; it does not reduce public-release evidence.
