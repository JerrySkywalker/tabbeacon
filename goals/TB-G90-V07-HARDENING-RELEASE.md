# TB-G90 — v0.7 Hardening & Release

## Status

PLANNED after accepted G89 and an accepted G86 Native Tab Icon final
disposition.

## Purpose

Close the v0.7 train as a truthful public release. Revalidate the settled
runtime and documentation candidate, apply the 0.7.0 version/release metadata,
prove fresh and v0.6.1-upgrade consumers, visually review the new public brand/
README surface, and publish only under the repository's explicit public-release
authorization boundary.

A native-icon disposition of `NO_GO` is fully compatible with a successful
v0.7.0 release.

## Preconditions

Required planning/product state:

```text
G83=ACCEPTED
G84=ACCEPTED_OR_SUPERSEDED_BY_OFFICIAL_API
G85=ACCEPTED_OR_SAFETY_TERMINATED
G86=ACCEPTED
G87=ACCEPTED
G88=ACCEPTED
G89=ACCEPTED
NATIVE_TAB_ICON_DISPOSITION=<GO_EXPERIMENTAL|GO_PRODUCTION_CANDIDATE|NO_GO>
CURRENT_PUBLIC_RELEASE=v0.6.1
```

If G86 has no final disposition, do not release v0.7 merely because docs are
ready.

## Production native-icon boundary

Before release, verify:

```text
PRODUCTION_RUNTIME_NATIVE_ICON=false
NORMAL_CLI_NATIVE_ICON=false
```

unless an entirely separate, explicitly authorized roadmap amendment after G86
has changed the v0.7 contract. This Goal by itself does not grant that
amendment.

If G86 returned `GO_EXPERIMENTAL` or `GO_PRODUCTION_CANDIDATE`, release notes
must describe the research result without implying normal runtime support.

If `NO_GO`, release notes may explain that stock WT native icon was investigated
and intentionally not productized because the admitted safety/reliability bar
was not met.

## A. Fresh remote / branch admission

Start from exact current admitted `main` after G89.

Verify:

- no overlapping v0.7 release writer;
- no unmerged risk-relevant v0.7 candidate;
- working tree/worktree clean;
- current public release remains v0.6.1;
- package version has not already been bumped/published by another transaction;
- G86 disposition/ADR and G87–G89 docs are reachable from the release source.

Use one focused release branch/worktree according to current repository
conventions.

## B. Version and release metadata

Only in G90 change the package target version:

```text
0.6.1 -> 0.7.0
```

Update the current changelog/release notes/upgrade guide truth required by the
repository.

Release notes must distinguish:

### Open-source presentation work

- TabBeacon SVG brand identity;
- English canonical README + Simplified Chinese README;
- Rust + Windows CI two-badge project-health surface;
- separate supported-coding-agent table/capability guide;
- docs portal and user/design/development guides;
- CONTRIBUTING v2 and docs CI.

### Native Tab Icon research

Record exact G86 disposition and what it means.

Do not claim production native icon unless that is genuinely true under a later
separately accepted scope change.

### Deferred providers

State or preserve truth that Claude/OpenCode remain deferred. Do not imply v0.7
adds provider coverage.

## C. README / brand release review

Because v0.7 materially changes the repository's public appearance, perform an
explicit final visual review of:

- README hero in GitHub light context;
- README hero in GitHub dark context where feasible;
- logo/mark scaling;
- small mark at 16/24/32 px;
- Rust badge truth;
- Windows CI badge truth;
- exactly two hero badges;
- English/Chinese language switch;
- real Windows Terminal screenshot privacy/readability;
- Supported Coding Agents table;
- Releases/crates.io/Docs/MIT navigation;
- syntax highlighting/code examples;
- docs portal navigation.

Do not fabricate visual PASS if the rendering cannot be inspected. Use an Owner
visual gate where required by repository policy.

## D. Documentation gates

Run the G89 docs checks from the exact release candidate.

Required:

```text
INTERNAL_MARKDOWN_LINKS_VALID=true
README_LANGUAGE_LINKS_RECIPROCAL=true
README_BADGE_COUNT=2
README_AGENT_BADGES=false
CRITICAL_EN_ZH_INVARIANTS=PASS
REQUIRED_BRAND_ASSETS_EXIST=true
SVG_WELL_FORMED=true
SVG_ACTIVE_CONTENT=false
DOCS_PORTAL_LINKS_VALID=true
STALE_CURRENT_RELEASE_MARKERS=0_after_release_metadata_is_set
```

Check that current-facing docs say v0.7.0 only when referring to the release
candidate/target appropriately. Do not rewrite historical v0.6.x records.

## E. Runtime / package quality gates

Even if most v0.7 changes are experimental/docs, release the same Rust product
with full package discipline.

At minimum run current repository equivalents of:

```text
cargo test --all-targets --locked
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo package --locked
```

Plus existing local/hosted quality gates required by `QUALITY_GATES.md` and
release scripts.

If experimental native-icon code is retained, verify package boundaries:

```text
EXPERIMENTAL_HELPER_INSTALLED=false
EXPERIMENTAL_HELPER_IN_PUBLIC_PACKAGE=false_or_explicitly_safe_and_intended
NORMAL_TABBEACON_RUNTIME_USES_XAML=false
```

The preferred default is that experiment-only helper material does not enter the
normal crates.io/package installation surface.

## F. Windows artifacts

Build final Windows x64 release artifact according to current release process.

Required:

- exact release-source provenance;
- Windows x64 ZIP;
- matching SHA-256 sidecar;
- package/content audit;
- binary reports `tabbeacon 0.7.0`;
- no unrelated experiment/evidence/private files in the archive.

## G. Fresh install consumer

Use a clean disposable consumer root and official release candidate/package
surface as appropriate before publication.

Prove:

```text
FRESH_INSTALL=PASS
INSTALLED_VERSION=0.7.0
```

Run basic read-only/help/doctor/setup smoke appropriate to current release
process without mutating Owner production config.

## H. v0.6.1 -> v0.7.0 upgrade consumer

Use a disposable, representative v0.6.1 installation/config state.

Prove:

- official replacement path works;
- ownership-safe upgrade-preflight behavior remains truthful;
- existing supported provider configuration/preferences migrate/preserve;
- unrelated Hooks/config preserved;
- manual trust boundary not bypassed;
- README/docs polish does not accidentally change provider behavior;
- v0.7 version/source reported correctly.

Required:

```text
V061_TO_V070_UPGRADE=PASS
HOOK_TRUST_BYPASS=false
UNRELATED_CONFIG_PRESERVED=true
```

## I. Native-icon regression truth

Run only the evidence needed to ensure packaging/release changes did not falsify
G86 conclusions.

Do not rerun a destructive/full XAML matrix merely because the version number
changed if exact source identity proves the experiment result unchanged.

Release metadata must contain exactly the accepted disposition.

Required:

```text
RELEASE_NATIVE_ICON_DISPOSITION=G86_DISPOSITION
PRODUCTION_RUNTIME_NATIVE_ICON=false
```

unless a separately authorized amendment says otherwise.

## J. Release candidate PR

Open one focused release PR to `main`.

Required before merge/publication:

- exact-head hosted CI PASS;
- docs CI PASS;
- release/brand/docs focused review PASS;
- no unresolved security/privacy finding;
- release artifacts/rehearsals tied to the exact candidate or safely reproduced
  post-merge according to current release policy.

Do not publish from an unmerged/unaccepted branch merely because local tests
pass.

## K. Public release authorization

The public mutation boundary remains explicit.

Without Owner/repository-authorized public release instruction, stop at:

```text
WAITING_FOR_OWNER_RELEASE_AUTHORIZATION
```

Do not create crates.io 0.7.0, tag v0.7.0, or GitHub Release v0.7.0 merely
because the release candidate is green.

## L. Public release transaction

After explicit authorization and post-merge exact-main verification:

1. publish crates.io `tabbeacon 0.7.0`;
2. create/push immutable `v0.7.0` tag at the exact release SHA;
3. create GitHub Release `v0.7.0`;
4. upload verified Windows x64 ZIP and SHA-256 sidecar;
5. verify public metadata/assets;
6. run clean crates.io consumer install;
7. run clean GitHub-asset download/hash/version smoke;
8. reconcile current public-release governance truth.

If a public surface succeeds and a later surface fails, report a truthful
partial-public-release state. Do not force-retag/delete/overwrite evidence to
pretend rollback.

## M. Post-release truth

After all public surfaces pass, current-facing repository truth must state:

```text
CURRENT_PUBLIC_RELEASE=v0.7.0
```

Historical v0.6.1 release evidence remains intact.

If a metadata-only post-release closeout PR is required by existing convention,
perform it separately/focused according to current governance.

## N. Cleanup

Remove only exact-owned disposable release/test roots through supported cleanup
paths. Do not bypass environment guards for cosmetic cleanup.

Preserve release provenance and G86 safety evidence.

## Risk vector

```text
CODE_CHANGED=version_release_metadata_and_any_settled_v07_changes
PRESENTATION_CHANGED=public_repository_surface
PROVIDER_CHANGED=false
USER_PERSISTENT_CONFIG_CHANGED=false_expected
SECURITY_OR_PRIVACY_CHANGED=brand_screenshot_docs_and_experiment_boundary_review
RELEASE_BOUNDARY=true
```

## Acceptance before public authorization

```text
PACKAGE_VERSION=0.7.0
NATIVE_TAB_ICON_DISPOSITION=<GO_EXPERIMENTAL|GO_PRODUCTION_CANDIDATE|NO_GO>
PRODUCTION_RUNTIME_NATIVE_ICON=false
README_CANONICAL_LANGUAGE=en-US
README_ZH_CN=true
README_BADGE_COUNT=2
README_BADGE_RUST=true
README_BADGE_WINDOWS_CI=true
README_AGENT_BADGES=false
DOCS_CI=PASS
README_VISUAL_REVIEW=PASS
BRAND_VISUAL_REVIEW=PASS
TESTS=PASS
CLIPPY=PASS
CARGO_PACKAGE=PASS
WINDOWS_ZIP=PASS
SHA256_SIDECAR=PASS
FRESH_INSTALL=PASS
V061_TO_V070_UPGRADE=PASS
HOSTED_EXACT_HEAD_CI=PASS
RELEASE_REVIEW_FINDINGS=0
CLAUDE_PROVIDER=DEFERRED
OPENCODE_PROVIDER=DEFERRED
```

## Final public acceptance

```text
CRATES_IO_VERSION=0.7.0
TAG=v0.7.0
TAG_SHA=RELEASE_SHA
GITHUB_RELEASE=v0.7.0
GITHUB_RELEASE_TARGET=RELEASE_SHA
WINDOWS_ASSET_HASH=PASS
CRATES_IO_FRESH_CONSUMER=PASS
GITHUB_ASSET_FRESH_CONSUMER=PASS
PUBLIC_VERSION_CONSISTENT=true
CURRENT_PUBLIC_RELEASE=v0.7.0
```

## Estimated effort

**4–6 effective engineering hours** for a settled candidate, excluding time
waiting for an explicit Owner release authorization/visual inspection.

## Next

After v0.7 post-release closeout, choose the next roadmap deliberately.

Possible future tracks may include native-icon productization **only if G86
returned GO_PRODUCTION_CANDIDATE and a new trust-boundary plan is accepted**.
Claude/OpenCode remain deferred until separately admitted.