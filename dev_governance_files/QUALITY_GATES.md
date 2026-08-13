# Quality Gates

## Gate hierarchy

### L0 — Repository policy

- expected files and licenses present;
- LF policy satisfied for configured text files;
- `git diff --check` / equivalent whitespace validation passes;
- no generated runtime state committed.

### L1 — Rust static/build gate

Using the repository-pinned Rust toolchain:

```text
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build --locked --all-targets
```

The declared `rust-version` and CI toolchain must match for the minimum-supported-version claim to be proven.

### L2 — Functional integration gate

Applies once the relevant subsystem exists. Verify provider normalization, state reconciliation, repository identity, and setup/uninstall behavior without relying on screenshots.

### L3 — Visual gate

Applies after `TB-G03` to changes that can alter Windows Terminal presentation. Requires an interactive desktop session and exact-head evidence for title, animation, and state color behavior.

### L4 — Provider E2E gate

Applies once a provider is production-supported. Exercises the real provider integration while keeping model/network behavior as deterministic as possible.

## Exact-head contract

For every required CI/evidence lane:

```text
EXPECTED_HEAD == checked_out_head == evidence_head
```

For visual changes after visual CI exists:

```text
EXPECTED_HEAD == CODE_HEAD == VISUAL_HEAD
```

A run is not acceptable evidence when it is cancelled, skipped, neutral, superseded, checked out at a different SHA, or executed on an unapproved runner class.

## Runner identity

Hosted CI may use GitHub-hosted Windows runners for L0/L1 while the code remains compatible with them.

Visual CI must run in an explicitly approved **interactive Windows desktop session**. Do not treat a Windows service/Session-0 runner as valid visual evidence merely because the job process starts.

When self-hosted runners are introduced, workflows must assert the expected runner/machine identity and relevant toolchain paths rather than trusting labels alone.

## Failure classification

Record failures as one of:

- product/code defect;
- test defect;
- runner/environment defect;
- external dependency/service defect;
- evidence mismatch/wrong SHA;
- unproven.

Do not patch product code to compensate for a runner defect without evidence that the product itself is wrong.
