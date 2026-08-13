# TB-B00 — Repository Bootstrap

## Goal

Materialize the first governed TabBeacon repository baseline from an empty public remote.

## Authorized scope

Bootstrap and policy files only:

- Rust project skeleton;
- project metadata/license;
- CI and local validation script;
- governance/roadmap/evidence files;
- architecture and ADRs;
- PR template.

No Codex hooks, VT renderer, repository abbreviation, app-server client, or visual-capture implementation is authorized in B00.

## Preconditions

- repository is public and owned by the project owner;
- repository starts empty;
- default branch target is `main`;
- no unrelated repository content exists.

## Acceptance criteria

1. Repository contains the approved architecture/governance baseline.
2. Rust source is a dependency-free bootstrap skeleton and makes no runtime integration claims.
3. Rust toolchain is pinned to 1.97.1 and `Cargo.toml` declares the same `rust-version`.
4. Hosted Windows CI checks out and asserts the exact candidate SHA.
5. CI enforces formatting, Clippy warnings-as-errors, tests, locked build, and line-ending policy.
6. Bootstrap head is pushed to `main`.
7. Exact-head CI result is recorded as PASS, FAIL, BLOCKED, or UNPROVEN; it is not assumed.

## Expected evidence

```text
GOAL_ID=TB-B00
EXPECTED_HEAD=<bootstrap head>
CODE_HEAD=<CI checkout head>
VISUAL_HEAD=N/A
LOCAL_VALIDATION=<...>
CI=<...>
VISUAL_CI=N/A
UNRELATED_DRIFT_TOUCHED=false
```

## Completion rule

B00 is complete only when the repository bootstrap exists and the required exact-head code CI is PASS. Repository materialization without runnable CI is a valid partial result but remains `BLOCKED` or `UNPROVEN`.
