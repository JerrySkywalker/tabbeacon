# TB-G05R — Trust-boundary and compatibility hardening

## Status

`ACTIVE — SUBORDINATE HARDENING LAB`

This goal is a non-production hardening line beneath `TB-G05-CODEX-HOOKS`.
It does not replace, retarget, or authorize mutation of pull request #6.

## Admission

- Repository: `JerrySkywalker/tabbeacon`
- Run: `TB-G05R-HARDENING-LAB-001`
- Starting commit: `11f0876c62b29208ba0b0243678ff4f65ae6cfc4`
- Production G05 pull request: <https://github.com/JerrySkywalker/tabbeacon/pull/6>
- Production G05 branch: `tb-g05-codex-hooks`
- Hardening branch: `tb-g05r-trust-boundary-hardening`
- Production candidate disposition: frozen unless a reproduced P0/P1 correctness,
  security, or compatibility defect justifies a proposed replacement.

## Purpose

Use the owner-controlled hook-trust interval to test the G05 provider and
configuration boundary against current and nearby Codex versions, adversarial
hook payloads, Windows command parsing, configuration diversity, filesystem
failures, concurrency, crash-consistency, and lifecycle transitions.

The lab may add tests, scripts, research notes, and evidence to this branch.
It must not consume the remaining owner trust action or claim real-owner smoke
evidence.

## Allowed scope

- `goals/TB-G05R-TRUST-BOUNDARY-AND-COMPATIBILITY-HARDENING.md`
- `hardening/TB-G05R-HARDENING-LAB-001/**`
- G05 provider/configuration/runtime code and tests only when a demonstrated
  release-blocking defect requires a repair on this hardening branch
- `Cargo.toml` and `Cargo.lock` only when a concrete dependency defect requires
  a justified change

## Non-goals

- no write to the owner's real Codex configuration or trust state;
- no trust bypass for the owner's hooks;
- no merge or retarget of pull request #6;
- no G06X or G07 implementation;
- no change to G01 semantics or G02 presentation policy without a demonstrated
  defect;
- no launcher wrapper, fake `codex.exe`, PATH shadowing, daemon, or TUI scraping;
- no visual CI unless presentation behavior changes;
- no change to another repository.

## Finding policy

Every finding is classified before implementation as one of:

- `PRODUCT_DEFECT`
- `TEST_ORACLE_DEFECT`
- `UPSTREAM_COMPATIBILITY`
- `WINDOWS_ENVIRONMENT`
- `FILESYSTEM`
- `SECURITY_BOUNDARY`
- `TRUST_BOUNDARY`
- `DOCUMENTATION`
- `UNPROVEN`

Severity uses `P0`, `P1`, `P2`, `P3`, or `INFORMATIONAL`. Only a reproduced P0
or P1 defect may justify a proposed replacement G05 candidate. The production
branch remains untouched unless separately authorized.

## Acceptance evidence

- exact-head freeze receipt for PR #6 and CI run `31811205030`;
- current upstream Codex source/release and compatibility baseline;
- isolated multi-version compatibility matrix;
- isolated trust-model forensics;
- deterministic hook fuzz and Windows command-quoting evidence;
- configuration chaos and atomicity/crash-consistency evidence;
- fail-open, multi-session, lifecycle, and synthetic end-to-end evidence;
- dependency and static security reviews;
- owner return runbook;
- content digest for the final evidence directory.

All mutable Codex test homes live under ignored build/lab output. Tests must not
write `%USERPROFILE%\\.codex`.

## Repository validation

When repository files change, run:

```text
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build --locked --all-targets
scripts/local-ci.ps1
```

If the repository local CI wrapper has another path on the admitted candidate,
record and use that exact path rather than inventing a new gate.

## Completion boundary

G05 remains `BLOCKED` on `OWNER_HOOK_TRUST_REVIEW`. This lab may prove every
independent boundary available without owner trust, but it must leave the owner
action irreducible and explicit.
