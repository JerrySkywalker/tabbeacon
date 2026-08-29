# Release Criteria

A public TabBeacon candidate is releasable only when all applicable conditions
below are proved at one exact candidate SHA.

## Product

- daily launch remains `codex` after setup;
- fail-open behavior is demonstrated;
- repository identity is offline-first and stable across tested moves/worktrees/collisions;
- title, progress, and tab color behave according to the visual state contract;
- setup is idempotent and uninstall is ownership-safe;
- unrelated user Codex configuration/hooks survive setup and uninstall.

## Architecture

- provider-specific event types remain below the normalizer boundary;
- presentation code consumes typed visual state, not raw provider strings;
- Codex hooks remain the production backend unless a later ADR explicitly
  promotes another backend;
- experimental app-server work is not a hidden runtime dependency of the production path.

## Verification

- one final locked exact-head code CI PASS;
- focused current release smoke PASS;
- presentation, provider, performance, and configuration-safety evidence is
  fresh only when its relevant risk surface changed, otherwise explicitly
  REUSED with an empty bounded risk diff;
- exact-head equality holds for every fresh evidence lane.

## Packaging

- release artifacts are reproducible from the tagged source within the documented toolchain;
- license and notices are present;
- release notes describe limitations and the Windows Terminal/Codex scope;
- crates.io publication is intentional, not an accidental consequence of CI.

## Owner official-channel convergence

```text
OWNER_OFFICIAL_CHANNEL_CONVERGENCE_REQUIRED=true
OWNER_OFFICIAL_CHANNEL=crates.io
```

An exact-Git Cargo installation is permitted only for unreleased RC or dogfood
qualification, for example:

```powershell
cargo install --git <repo> --rev <exact-sha> --locked --force
```

It must not remain the normal Owner installation after that version is public.
For every public release, closeout must converge the Owner dogfood machine to
the official stable channel. The current Rust CLI channel is crates.io; unless
another release-policy change deliberately replaces it, the v0.5.2 cutover is
equivalent to:

```powershell
rustup run 1.97.1 cargo install tabbeacon --version 0.5.2 --locked --force
```

Use an explicit Rust 1.97.1 selection whenever the active toolchain is not
already admitted. The cutover proof must inspect Cargo-owned installation
metadata (`.crates.toml`, `.crates2.json`, or an equally authoritative Cargo
source record) and minimally prove a crates.io registry source, not a Git
revision. `tabbeacon --version`, executable location, and binary existence are
not sufficient proof.

After cutover, closeout MUST run `tabbeacon setup codex` as ownership-aware
reconciliation, preserving existing Codex configuration ownership, Hook trust,
third-party Hooks/MCP servers, and user presentation settings. Reconciliation
must not automatically change trust. Closeout then runs static Doctor and the
exact hybrid runtime probe.

Every public-release closeout receipt reports:

```text
OWNER_INSTALL_SOURCE=<content-minimal-cargo-source-proof>
OWNER_INSTALL_SOURCE_PROVEN=<true|false>
OWNER_GIT_REV_INSTALL=<true|false|UNPROVEN>
OWNER_OFFICIAL_CHANNEL_CUTOVER=<PASS|FAIL|BLOCKED|UNPROVEN>
OWNER_OFFICIAL_CHANNEL_CUTOVER_REASON=<none-or-bounded-reason>
OWNER_OFFICIAL_CHANNEL=crates.io
```

### TB-V072 explicit production-non-adoption exception

The default convergence requirement remains mandatory except for
`TB-V072-FULL-SUBAGENT-HOOK-HOTFIX-TO-PUBLIC-RELEASE-001`. That Goal explicitly
authorizes the public v0.7.2 transaction while prohibiting any Owner production
Codex/configuration/trust mutation. For that exact Goal only, public release and
current-truth closeout may proceed after all applicable public gates pass, while
the Owner-convergence portion is truthfully recorded as:

```text
OWNER_OFFICIAL_CHANNEL_CUTOVER=BLOCKED
OWNER_OFFICIAL_CHANNEL_CUTOVER_REASON=NOT_AUTHORIZED
OWNER_INSTALL_SOURCE_PROVEN=false
OWNER_GIT_REV_INSTALL=UNPROVEN
```

This narrow exception neither authorizes an Owner cutover nor alters the default
for another release. It preserves a separate explicit-Owner-admission action for
official-channel adoption and its `setup codex`/Doctor/runtime proof.
