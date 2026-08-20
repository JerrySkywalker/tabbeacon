# TB-G63R — v0.5.1 Hardening & Release

## Status

PLANNED final Codex-only closure after accepted G57–G63.

## Purpose

Publish a reliability/explainability maintenance release that resolves real v0.5 dogfood defects and completes the provider-neutral management foundation without admitting Agy yet.

## Mandatory release scope

v0.5.1 must contain accepted outcomes from:

```text
G57 upgrade preflight / trust wording / Issue #45 closeout
G58 activity-channel cleanup
G59 Root Workspace Anchor / subagent counts
G60 Hook Inspector
G61 Naming + title explainability
G62 Integrations/capability/provider-aware management foundation
G63 upgrade-safe worker runtime
```

## Agy hard stop

Release candidate and public package must still report:

```text
AGY_PROVIDER=false
AGY_PRODUCTION_ADMISSION=false
```

No Agy login, setup mutation, or production capability claim is permitted in G63R.

## Upgrade gates

Prove at least:

```text
public v0.5.0 -> v0.5.1
```

Preserve:

- Presentation settings including explicit legacy `activity=both`;
- Interface settings;
- StableAliasRegistry and local overrides;
- Hook ownership/trust boundary;
- export/import semantics;
- Codex 0.147.0 admitted integration behavior or the then-current explicitly admitted profile;
- Root Workspace Anchor initializes safely without breaking existing sessions/config.

## Active-worker upgrade gate

A release-critical Windows scenario:

```text
v0.5.x binary installed
  -> spawn/prove active TabBeacon worker
  -> worker runs from versioned/hash-bound runtime image
  -> replace/install v0.5.1 package binary while worker is active
  -> package replacement succeeds
  -> worker/session remains bounded/fail-open
  -> new invocation uses v0.5.1/new runtime image
```

This gate closes the dogfood defect that originally produced `os error 5` at Cargo replacement.

## UX/reliability gates

Prove:

- normal Human presentation has no redundant dual activity unless explicitly chosen;
- root workspace alias remains stable under subagent/tool cwd variation;
- Hook inventory accurately reflects ownership/trust/currentness without leaking arbitrary commands;
- naming score/component explanation matches engine values;
- `Why this title?` reports safe provenance;
- Integrations screen and provider-aware Sessions work Codex-only;
- provider badge `auto` does not bloat unambiguous ordinary Codex titles;
- upgrade preflight reflects runtime-image architecture;
- en-US/zh-CN/no-color/narrow layouts remain usable.

## Release review

Focused independent review covers:

- process ownership/drain safety;
- runtime image path/hash/GC safety;
- Hook inventory privacy and manual trust boundary;
- root anchor privacy/stale cleanup;
- provider-neutral management without accidental Agy admission;
- settings/import/export migrations;
- package contents.

## Publication sequence

Use one settled release candidate and read-before-retry external mutation:

```text
accepted source
 -> version 0.5.1
 -> locked/static/build CI
 -> package inspection
 -> Windows x64 build/ZIP/SHA256
 -> real WT / active-worker upgrade smoke
 -> merge
 -> crates.io 0.5.1
 -> immutable v0.5.1 tag
 -> GitHub Release/assets
 -> clean public crates.io consumer
 -> clean public ZIP consumer
```

No half-publication and no blind retry after ambiguous external responses.

## Completion

```text
TB_G57=COMPLETE
TB_G58=COMPLETE
TB_G59=COMPLETE
TB_G60=COMPLETE
TB_G61=COMPLETE
TB_G62=COMPLETE
TB_G63=COMPLETE
VERSION=0.5.1
UPGRADE_PREFLIGHT=PASS
ROOT_WORKSPACE_ANCHOR=PASS
HOOK_INSPECTOR=PASS
NAMING_EXPLAINABILITY=PASS
WHY_THIS_TITLE=PASS
INTEGRATIONS_FOUNDATION=PASS
UPGRADE_SAFE_WORKER_RUNTIME=PASS
V050_TO_V051_UPGRADE=PASS
ACTIVE_WORKER_PACKAGE_REPLACE=PASS
FRESH_INSTALL=PASS
RELEASE_REVIEW=PASS
CRATES_IO_PUBLISHED=true
GITHUB_RELEASE_PUBLISHED=true
WINDOWS_X64_ASSET_PUBLISHED=true
PUBLIC_CONSUMERS=PASS
AGY_PROVIDER=false
V0_5_1_RELEASE=PASS
```

## Mandatory post-release stop

After public v0.5.1 and closeout, the next state is:

```text
NEXT_GOAL=TB-G64-AGY-ADMISSION-REAL-ENVIRONMENT-SPIKE
OWNER_AGY_ENVIRONMENT_REQUIRED=true
AUTO_CONTINUE_TO_G64=false
```

No unattended train may cross this boundary merely because wall-clock budget remains.

## Estimated effort

**4–6 effective engineering hours after G57–G63 are accepted.**