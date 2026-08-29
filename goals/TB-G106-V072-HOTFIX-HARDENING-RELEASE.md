# TB-G106 — v0.7.2 Hotfix Hardening & Public Release

## Purpose

Release the Codex subagent Hook stability hotfix as public `v0.7.2` after the
legacy transport migration and real subagent qualification are accepted.

## Changed-risk vector and required gates

```text
CODE_CHANGED=false_expected
PRESENTATION_CHANGED=false
PROVIDER_CHANGED=false_release_metadata_only
USER_PERSISTENT_CONFIG_CHANGED=true_disposable_consumer_only
SECURITY_OR_PRIVACY_CHANGED=true_package_artifact_and_public_boundary
RELEASE_BOUNDARY=true
```

G106 requires fresh package/release/consumer evidence, artifact audit, release
security/ownership review, and exact-head hosted CI. The G105 real-provider,
Visual, trust, and migration evidence may be reused only when the release
candidate changes no Hook/profile/runtime/normalizer/config-ownership source and
the receipt records an empty bounded relevant-risk diff. Any such source change
requires fresh applicable G105 evidence. No Owner production mutation is in the
G106 source or consumer scope.

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

## Fresh phase admission

Before creating the release branch, record the fresh post-implementation state:

```text
REPOSITORY=JerrySkywalker/tabbeacon
EXPECTED_START_HEAD=<exact HOTFIX_IMPL_MERGE_SHA on origin/main>
CHECKED_OUT_HEAD=EXPECTED_START_HEAD
EXPECTED_REMOTE_MAIN=EXPECTED_START_HEAD
WORKTREE=<one clean owned release worktree>
```

The release-branch source boundary is package/version metadata, changelog,
and v0.7.2 release/upgrade notes, standard artifact/consumer procedures, and no
more. `CURRENT_PUBLIC_RELEASE` and `CURRENT_PUBLIC_TARGET` remain v0.7.1 until
all public surfaces succeed; update them only in the post-publication closeout.
No migration/profile/runtime change, new provider, PR #100 content, or
production Codex configuration/trust mutation is admitted by G106. A changed
release head requires fresh exact-head CI/review; a public tag, crate version,
or release already observed to exist is a hard stop for that mutation and must
be reconciled rather than replayed.

## A. Version/release metadata

Only in this Goal bump:

```text
0.7.1 -> 0.7.2
```

Update Cargo metadata/lockfile if required, changelog, release notes, and
upgrade notes according to existing repository convention. Do not claim a new
current public release in this pre-publication PR.

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
It must also cover the Windows live-child replacement boundary: from a disposable
legacy MCP installation with an active exact-owned MCP child, first run the
existing read-only `tabbeacon upgrade-preflight --plain`. If it reports
`REPLACEABILITY=blocked_by_owned_tabbeacon_mcp`, either use the existing
ownership-qualified `--drain` path for only that exact child or allow it to exit
naturally and restart the consumer before replacement. Re-run preflight before
the package replacement and require `REPLACEABILITY=ready` or
`REPLACEABILITY=no_known_tab_beacon_lock`; every other result hard-stops package
replacement. Prove Codex, third-party MCP children, and any unproven process
were not terminated. Never exercise drain against Owner production state.

Required additional release-consumer truth:

```text
LIVE_LEGACY_MCP_UPGRADE=PASS
LIVE_MCP_PREFLIGHT=PASS
LIVE_MCP_DRAIN_OR_NATURAL_EXIT=PASS
LIVE_MCP_REPLACEABILITY=<ready|no_known_tab_beacon_lock>
NONOWNED_PROCESSES_PRESERVED=true
```

## D. Real subagent regression

Re-use or reproduce G105 evidence only when exact source identity and repository
policy permit. If release-candidate changes affect Hook/migration/runtime code,
repeat the real qualification.

Final required truth:

```text
REAL_CODEX_SUBAGENT_QUALIFICATION=PASS
REAL_MIGRATED_LEGACY_SUBAGENT_QUALIFICATION=PASS
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

This planning document does not itself authorize a public mutation. Before the
public transaction, verify that the admitted execution Goal carries explicit,
current Owner authorization for `GOAL_ID=TB-V072-FULL-SUBAGENT-HOOK-HOTFIX-TO-PUBLIC-RELEASE-001`
and the exact v0.7.2 crates.io publication, immutable tag, GitHub Release,
Windows assets, and public-consumer proofs. Record
`PUBLIC_RELEASE_AUTHORIZATION=EXPLICIT` with that admission. If the
authorization is absent, stale, or narrower than the proposed transaction,
hard-stop with `OWNER_RELEASE_AUTHORIZATION=UNPROVEN`.

For the production-non-adoption exception, also verify that the same external
authority explicitly prohibits Owner production Codex/configuration/trust
mutation and record:

```text
OWNER_PRODUCTION_NON_ADOPTION_AUTHORIZATION=EXPLICIT
PRODUCTION_CODEX_CONFIGURATION_MUTATED=false_expected
PRODUCTION_HOOK_TRUST_MUTATED=false_expected
```

If public-release authority is silent about Owner adoption, the default
official-channel convergence rule remains applicable; do not use the exception.

When that verification and all applicable G103-G106 gates pass, do not stop
again solely for generic release authorization.

This does not authorize bypassing a failed gate.

## H. Public release transaction

After the release PR merge, re-fetch and bind every public mutation to the
actual release source, not the earlier post-implementation predecessor:

```text
GOAL_ID=TB-V072-FULL-SUBAGENT-HOOK-HOTFIX-TO-PUBLIC-RELEASE-001
RELEASE_SHA=<exact merged release source>
EXPECTED_REMOTE_MAIN=RELEASE_SHA
CHECKED_OUT_HEAD=RELEASE_SHA
PUBLIC_RELEASE_AUTHORIZATION=EXPLICIT
```

If `origin/main` differs, hard-stop the public transaction and reconcile before
any irreversible action. The package, tag, GitHub Release, assets, and consumer
proofs must all bind to `RELEASE_SHA`.

Then:

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
DEFAULT_INSTALL_RESOLVES_CURRENT=0.7.2
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

For this repository, the post-publication closeout is required: after all public
surfaces pass, open a metadata-only PR that updates only current-facing release
truth and its deterministic docs contract. Its admitted scope is the current
release declarations in `README.md`, `README.zh-CN.md`, `SECURITY.md`,
`docs/README.md`, `docs/getting-started.md`,
`docs/development/release-process.md`, `dev_governance_files/ROADMAP.md`, and
`dev_governance_files/DEVELOPMENT_PAUSE.md`; the v0.7.2 release record/notes;
and the exact v0.7.2 patterns in `scripts/ci/check-docs.ps1`. Do not rewrite
historical receipts. Run the docs checker, a fresh exact-head hosted docs CI,
and focused review before merging this closeout.

Do not resume v0.7.3 automatically.

## K. Production boundary

Do not mutate Owner production Codex config or Hook trust merely because the
release succeeds. Production adoption is a separate user action unless an
explicit qualification step under G105/G106 required a narrowly authorized
Owner-present operation.

`RELEASE_CRITERIA.md` separately requires Owner official-channel convergence
after every public release. Use this Goal's production-non-adoption exception
only when Section G has recorded
`OWNER_PRODUCTION_NON_ADOPTION_AUTHORIZATION=EXPLICIT` from the same external
Owner authority that authorizes publication. If that field is absent, the default
official-channel convergence rule remains applicable and public closeout stops.
When the field is present, complete the public release and disposable consumers
but do not claim Owner-convergence closeout: record
`OWNER_OFFICIAL_CHANNEL_CUTOVER=BLOCKED` with reason `NOT_AUTHORIZED`, preserve
the Owner configuration and trust, and leave only that separate adoption action
blocked for explicit Owner direction. Public release evidence must not be
relabeled as Owner installation or Hook-runtime proof.

```text
OWNER_OFFICIAL_CHANNEL_CUTOVER=BLOCKED
OWNER_OFFICIAL_CHANNEL_CUTOVER_REASON=NOT_AUTHORIZED
OWNER_INSTALL_SOURCE=UNPROVEN_NOT_INSPECTED
OWNER_INSTALL_SOURCE_PROVEN=false
OWNER_GIT_REV_INSTALL=UNPROVEN
OWNER_PRODUCTION_NON_ADOPTION_AUTHORIZATION=EXPLICIT
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
