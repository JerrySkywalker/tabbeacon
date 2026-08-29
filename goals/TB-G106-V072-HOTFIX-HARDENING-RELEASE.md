# TB-G106 — v0.7.2 Hotfix Hardening & Public Release

## Purpose

Release the Codex subagent Hook stability hotfix as public `v0.7.2` after the
legacy transport migration and real subagent qualification are accepted.

## Preconditions

Required:

```text
G103=COMPLETE
G104=COMPLETE
G105=COMPLETE
CURRENT_PUBLIC_RELEASE=v0.7.1
TARGET_PUBLIC_RELEASE=v0.7.2
DESIRED_CODEX_TRANSPORT=command_v1
LEGACY_MCP_HYBRID_NEW_ADMISSION=false
PR100_MERGED=false
```

Start from exact fresh remote `main` after accepted hotfix implementation. Do not
merge or rebase promotional PR #100 into this release candidate.

## A. Version/release metadata

Only in this Goal bump:

```text
0.7.1 -> 0.7.2
```

Update Cargo metadata/lockfile if required, changelog, release notes, upgrade
notes/current-facing truth according to existing repository convention.

Release notes must describe:

- the visible subagent Hook failure symptom;
- legacy MCP Hybrid convergence to command v1 for exact-owned installations;
- preservation of third-party Hooks/MCP/config;
- manual Hook trust review requirement after changed Hook declarations;
- no new provider or presentation feature.

Do not include the frozen promo GIF/social-preview work as a v0.7.2 feature.

## B. Quality gates

Run current repository equivalents of at least:

```powershell
cargo test --all-targets --locked
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo package --locked
```

Plus docs, package, release, ownership, migration, and exact-head hosted gates
required by `dev_governance_files/QUALITY_GATES.md`.

Required:

```text
TESTS=PASS
CLIPPY=PASS
CARGO_PACKAGE=PASS
DOCS_CI=PASS
HOSTED_EXACT_HEAD_CI=PASS
RELEASE_REVIEW_FINDINGS=0
HIGH_RISK_FINDINGS=0
```

Known unchanged host timing families must be classified truthfully; do not
weaken runtime timeouts or unrelated semantics to obtain green output.

## C. Migration release regression

From the exact release candidate re-prove:

```text
FRESH_COMMAND_V1_INSTALL=PASS
LEGACY_MCP_TO_COMMAND_UPGRADE=PASS
THIRD_PARTY_HOOKS_PRESERVED=true
THIRD_PARTY_MCP_SERVERS_PRESERVED=true
UNRELATED_CODEX_CONFIG_PRESERVED=true
HOOK_TRUST_BYPASS=false
MIGRATION_IDEMPOTENT=true
```

A disposable v0.7.1 representative consumer should exercise the upgrade path.

## D. Real subagent regression

Re-use or reproduce G105 evidence only when exact source identity and repository
policy permit. If release-candidate changes affect Hook/migration/runtime code,
repeat the real qualification.

Final required truth:

```text
REAL_CODEX_SUBAGENT_QUALIFICATION=PASS
PRETOOLUSE_HOOK_FAILED_COUNT=0
POSTTOOLUSE_HOOK_FAILED_COUNT=0
ROOT_PRESENTATION_MUTATED_BY_CHILD=false
```

## E. Package/artifact

Build normal public artifacts:

```text
PACKAGE_VERSION=0.7.2
WINDOWS_ZIP=PASS
SHA256_SIDECAR=PASS
BINARY_VERSION=tabbeacon 0.7.2
```

No promo PR #100 files, local UIA recovery scratch, private receipts, or
unrelated evidence may leak into the crate or Windows ZIP.

## F. Release PR

Open one focused hotfix release PR to `main`. Before merge require exact-head
hosted CI and focused release/security/ownership review.

Merge only the exact accepted head and record:

```text
RELEASE_SHA=<exact admitted release source>
```

## G. Public authorization

The Owner has explicitly authorized public v0.7.2 release once all applicable
G103-G106 gates pass. Do not stop again solely for generic release authorization.

This does not authorize bypassing a failed gate.

## H. Public release transaction

After post-merge exact-main verification:

1. publish crates.io `tabbeacon 0.7.2`;
2. create/push immutable `v0.7.2` tag at `RELEASE_SHA`;
3. create GitHub Release `v0.7.2` targeting `RELEASE_SHA`;
4. upload verified Windows x64 ZIP and SHA-256 sidecar;
5. verify public metadata/assets;
6. run fresh crates.io exact consumer;
7. run fresh GitHub asset/hash/version consumer.

Never move an existing public tag or attempt to overwrite a crates.io version.
If publication is partially successful and a later surface fails, report
`PARTIAL_PUBLIC_RELEASE` and repair forward.

## I. Public consumers

Normal user path must remain valid:

```powershell
cargo install tabbeacon
```

Exact release verification:

```powershell
cargo install tabbeacon --version 0.7.2 --locked
```

Required:

```text
DEFAULT_CRATES_IO_INSTALL=PASS
EXACT_CRATES_IO_INSTALL=PASS
EXACT_CONSUMER_VERSION=0.7.2
GITHUB_ASSET_FRESH_CONSUMER=PASS
WINDOWS_ASSET_HASH=PASS
```

## J. Post-release truth

After all public surfaces pass, update current-facing truth to:

```text
CURRENT_PUBLIC_RELEASE=v0.7.2
CURRENT_PUBLIC_TARGET=v0.7.2
ACTIVE_FEATURE_DEVELOPMENT=PAUSED
PROMOTION_TARGET_RELEASE=v0.7.3
PROMO_PR=100
PROMO_PR_STATE=FROZEN_DRAFT
```

If repository convention requires a metadata-only closeout PR, create, validate,
and merge it separately.

Do not resume v0.7.3 automatically.

## K. Production boundary

Do not mutate Owner production Codex config or Hook trust merely because the
release succeeds. Production adoption is a separate user action unless an
explicit qualification step under G105/G106 required a narrowly authorized
Owner-present operation.

```text
PRODUCTION_HOOK_TRUST_MUTATED=false_expected
PRODUCTION_AGY_CONFIGURATION_MUTATED=false
```

## Final public acceptance

```text
CRATES_IO_VERSION=0.7.2
TAG=v0.7.2
TAG_SHA=RELEASE_SHA
GITHUB_RELEASE=v0.7.2
GITHUB_RELEASE_TARGET=RELEASE_SHA
PUBLIC_VERSION_CONSISTENT=true
CURRENT_PUBLIC_RELEASE=v0.7.2
ACTIVE_FEATURE_DEVELOPMENT=PAUSED
PR100_MERGED=false
```
