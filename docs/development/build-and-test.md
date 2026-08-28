# Build and test

This guide is for external contributors. Choose the smallest evidence set that
covers your change; a documentation typo does not require an interactive
Windows Terminal visual run.

## Prerequisites

- Windows for Windows-specific behavior.
- Rust **1.97.1** or newer (the package MSRV).
- PowerShell 7 (`pwsh`) for repository scripts.
- Windows Terminal only when a changed visual path requires a real visual gate.

Check the local toolchain:

```powershell
rustc --version
cargo --version
pwsh --version
```

## Build

```powershell
cargo build --locked
```

For a focused Rust test while iterating:

```powershell
cargo test --locked <test-name>
```

Run the full Rust test suite and lint before a code PR:

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build --locked --all-targets
```

## Documentation checks

The offline docs gate is quick and appropriate for documentation changes:

```powershell
pwsh -NoProfile -File scripts/ci/check-docs.ps1
```

The repository-wide local CI adds format, clippy, tests, build, provider-script
contract checks, and the docs gate:

```powershell
pwsh -NoProfile -File scripts/ci/run-local-ci.ps1
```

Use a shared noncanonical Cargo target when working outside the canonical
worktree, following local repository guidance. Do not commit target output or
temporary evidence roots.

## Risk-based gates

- Documentation-only changes: run the docs check and relevant link review.
- Ordinary code: focused tests during iteration plus the final hosted exact-head
  CI run.
- Changed title, progress, palette, VT bytes, animation, or visual oracle: add
  one final representative owned visual proof.
- Provider profile, trust, persistent configuration, process targeting, or
  privacy-sensitive work: read the applicable ADR/governance documents and use
  the additional safety gate they require.

## High-risk experiments

Keep experiments isolated from normal runtime behavior. Use exact-owned,
disposable paths and preserve unrelated Owner configuration. Do not attach to,
terminate, instrument, or capture an active Owner/development terminal. Native
tab icon research is [NO_GO](../design/native-tab-icon.md) and is not a
contributor workaround path.

## Evidence roots

Build artifacts and visual evidence may belong under an exact-owned build root.
Do not check transient screenshots, logs, raw provider payloads, secrets, or
private paths into the repository. If safe cleanup is refused, record the exact
owned path and leave it intact rather than bypassing the guard.
