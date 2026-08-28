# TB-G102 — v0.7.2 Hardening & Public Release

## Purpose

Close the v0.7.2 maintenance train as a truthful public release after G99–G101
have been accepted. This release must preserve production runtime/provider
behavior while publishing the discoverability, deterministic demo, and Cargo
presentation improvements.

## Preconditions

This roadmap document does not itself authorize G102 implementation. A fresh
explicit Owner admission is required before any G102 repository write, version
bump, publication, tag, or release mutation.

Required:

```text
G99=ACCEPTED
G100=ACCEPTED
G101=ACCEPTED
CURRENT_PUBLIC_RELEASE=v0.7.1
TARGET_PUBLIC_RELEASE=v0.7.2
RUNTIME_BEHAVIOR_CHANGED=false
PROVIDER_BEHAVIOR_CHANGED=false
```

Start from exact fresh remote `main`, verify no overlapping v0.7.2 release
writer, and use one focused release branch/worktree.

## A. Version/release metadata

Only in G102 bump:

```text
0.7.1 -> 0.7.2
```

Update Cargo metadata, lockfile if required, changelog, v0.7.2 release notes,
and current release/upgrade truth according to existing repository convention.
Do not rewrite historical release evidence.

Release notes should describe three bounded themes:

1. GitHub discoverability/metadata and social-preview asset;
2. automated deterministic real-Windows-Terminal promotional demo; and
3. README/crates.io distribution polish with the simple normal Cargo install.

Explicitly state that runtime/provider behavior did not change.

## B. Final discovery/media review

Require:

```text
GITHUB_DESCRIPTION=PASS
GITHUB_TOPICS_COUNT=6..10
GITHUB_TOPICS_RELEVANT=true
SOCIAL_PREVIEW_SVG=PASS
SOCIAL_PREVIEW_PNG=PASS
SOCIAL_PREVIEW_DIMENSIONS=1280x640
PROMO_GIF=PASS
PROMO_POSTER=PASS
PROMO_PRIVACY_REVIEW=PASS
```

Revalidate media from the exact release candidate. Do not fabricate a visual
PASS if the committed GIF/poster is unreadable or privacy-unsafe.

If GitHub still lacks a supported social-preview upload API, the actual Settings
UI upload may remain an Owner-only non-release-blocking action, provided the
validated final asset is committed and the limitation is recorded truthfully.

## C. Normal quality gates

Run current repository equivalents of at least:

```powershell
cargo test --all-targets --locked
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo package --locked
```

Plus current docs/check scripts and release gates required by
`dev_governance_files/QUALITY_GATES.md`.

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

Known unchanged host-only timing families must be classified under existing
quality-gate semantics; do not modify runtime timeouts merely to make a
promotion-only release green.

## D. Package/media separation

Audit the exact `.crate` archive.

Required:

```text
PACKAGE_VERSION=0.7.2
PROMO_GIF_IN_CRATE=false
SOCIAL_PREVIEW_IN_CRATE=false
PROMO_BUILD_EVIDENCE_IN_CRATE=false
```

No temporary PNG frames, palette files, FFmpeg binaries, private evidence, or
promo scratch roots may enter the package or Windows artifact.

## E. Windows artifact

Build the existing official Windows x64 ZIP and matching SHA-256 sidecar under
the normal release procedure.

Required:

```text
WINDOWS_ZIP=PASS
SHA256_SIDECAR=PASS
BINARY_VERSION=tabbeacon 0.7.2
```

This artifact remains a release artifact; v0.7.2 does not create a new Windows
installer or package-manager install path.

## F. Pre-publication consumers

Use disposable clean roots and current release procedure to prove candidate
install/upgrade behavior without mutating Owner production configuration.

At minimum prove the v0.7.1 -> v0.7.2 upgrade path and basic CLI smoke while
preserving unrelated provider config/hooks.

## G. Release PR

Open one focused PR to `main` containing only the v0.7.2 release transaction and
settled G99–G101 work if not already merged separately.

Before merge require exact-head hosted CI, docs/package/media checks, and focused
release/privacy review.

Merge only the exact accepted head and record the admitted `RELEASE_SHA`.

## H. Public release authorization

The public mutation boundary remains explicit. If the active Owner instruction
that launches G102 authorizes publication of v0.7.2, proceed after all gates
pass. Otherwise stop at `WAITING_FOR_OWNER_RELEASE_AUTHORIZATION`.

Do not infer public-release authority merely from this planning document.

## I. Public release transaction

After explicit authorization and post-merge exact-main verification:

1. publish crates.io `tabbeacon 0.7.2`;
2. create/push immutable tag `v0.7.2` at `RELEASE_SHA`;
3. create GitHub Release `v0.7.2`;
4. upload verified Windows x64 ZIP + SHA-256 sidecar;
5. verify public metadata/assets.

Never move/overwrite an existing public tag or crates.io version.

## J. Two public crates.io consumers

After propagation, prove two distinct clean consumer semantics.

### Normal user path

```powershell
cargo install tabbeacon
```

Required:

```text
DEFAULT_CRATES_IO_INSTALL=PASS
DEFAULT_INSTALL_RESOLVES_CURRENT=0.7.2
```

### Exact release path

```powershell
cargo install tabbeacon --version 0.7.2 --locked
```

Required:

```text
EXACT_CRATES_IO_INSTALL=PASS
EXACT_CONSUMER_VERSION=0.7.2
```

No `--git`, `--path`, local worktree binary, or pre-populated target may stand
in for either public consumer.

## K. GitHub asset consumer

From a fresh disposable root, download the public Windows asset + sidecar,
verify SHA-256, extract, launch, and prove `tabbeacon 0.7.2`.

Required:

```text
GITHUB_ASSET_FRESH_CONSUMER=PASS
WINDOWS_ASSET_HASH=PASS
```

## L. Post-release truth and pause

Only after all public surfaces pass, reconcile current-facing truth to:

```text
CURRENT_PUBLIC_RELEASE=v0.7.2
CURRENT_PUBLIC_TARGET=v0.7.2
```

If a metadata-only closeout PR is required, create/validate/merge it separately
under existing convention.

Then resume the deliberate pause:

```text
ACTIVE_FEATURE_DEVELOPMENT=PAUSED
V08_OPTIONS_STATUS=NON_AUTHORITATIVE
ROADMAP_V08_CREATED=false
NEXT_RECOMMENDED_GOAL=DOGFOOD_ONLY_NO_ACTIVE_DEVELOPMENT
```

Do not begin v0.8, a third provider, native-icon work, or reliability/platform
feature trains.

## Owner production boundary

Do not mutate the Owner's installed TabBeacon/Codex/Agy configuration or Hook
trust merely because v0.7.2 is released.

```text
PRODUCTION_CODEX_CONFIGURATION_MUTATED=false
PRODUCTION_HOOK_TRUST_MUTATED=false
PRODUCTION_AGY_CONFIGURATION_MUTATED=false
```

## Risk vector

```text
CODE_CHANGED=release_metadata_and_settled_promo_docs_tooling
PRESENTATION_CHANGED=public_repository_and_promo_assets
PROVIDER_CHANGED=false
USER_PERSISTENT_CONFIG_CHANGED=false_expected
SECURITY_OR_PRIVACY_CHANGED=promo_media_final_review
RELEASE_BOUNDARY=true
```

## Final public acceptance

```text
CRATES_IO_VERSION=0.7.2
TAG=v0.7.2
TAG_SHA=RELEASE_SHA
GITHUB_RELEASE=v0.7.2
GITHUB_RELEASE_TARGET=RELEASE_SHA
DEFAULT_CRATES_IO_INSTALL=PASS
EXACT_CRATES_IO_INSTALL=PASS
GITHUB_ASSET_FRESH_CONSUMER=PASS
PUBLIC_VERSION_CONSISTENT=true
CURRENT_PUBLIC_RELEASE=v0.7.2
ACTIVE_FEATURE_DEVELOPMENT=PAUSED
```
