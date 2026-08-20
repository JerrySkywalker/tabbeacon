# TB-G67R — v0.6.0 Hardening & Release

## Status

PLANNED final release closure after accepted G64–G67 and public v0.5.1.

## Purpose

Publish TabBeacon v0.6.0 with Agy as the second production provider while preserving the v0.5.1 reliability/explainability baseline and all original direct-command/fail-open invariants.

## Mandatory upgrade paths

Prove at least:

```text
public v0.5.0 -> v0.6.0
public v0.5.1 -> v0.6.0
fresh v0.6.0 install
```

The v0.5.1 path is authoritative for migration from Root Workspace Anchor, Hook Inspector, provider badge, Integrations model, and upgrade-safe worker images.

## Provider release matrix

Required real release acceptance:

```text
Codex-only
Agy-only
Codex + Agy concurrent
```

For both providers, verify only capabilities frozen by their admitted profiles. Unsupported Agy presentation/health/background capabilities remain explicitly unsupported and do not fail release solely for lack of parity.

## Daily command proof

```text
Codex launch = codex
Agy launch   = agy
```

No wrapper, PATH shadow, PTY host, or global TabBeacon daemon may be introduced for either provider.

## Reliability/explainability regression

Revalidate v0.5.1 outcomes:

- normal presets avoid redundant dual activity;
- explicit legacy `both` remains compatible;
- Root Workspace Anchor resists tool/subagent cwd drift;
- subagent/background counts remain content-minimal;
- Hook Inspector does not expose arbitrary commands;
- trust/currentness wording remains precise;
- naming score breakdown equals engine math;
- `Why this title?` works for both providers;
- Integrations/capability matrix are truthful;
- provider-aware Sessions and badges work;
- worker runtime images allow package replacement with active workers.

## Agy release gate

Re-run real Agy acceptance against the exact release candidate and record the exact admitted Agy version/range. Prove:

- setup/ownership safety;
- real structured backend input;
- session/root workspace behavior;
- lifecycle/approval semantics where supported;
- title/presentation channels actually admitted;
- fail-open behavior;
- uninstall/restore;
- no raw transcript/tool/prompt/assistant content persisted or rendered.

If current Agy behavior/version drift invalidates the frozen profile, stop and update/requalify through the provider-governance path. Do not publish with guessed compatibility.

## Multi-provider release gate

Real Windows Terminal scenario must include concurrent Codex and Agy tabs and prove:

- no cross-title writes;
- no cross-root-anchor binding;
- no cross-worker lease/generation contamination;
- provider badge semantics;
- shared workspace alias preference behavior;
- terminal restoration and shell usability after TUI/sessions.

## Human/localization gates

Both en-US and zh-CN cover:

```text
status
doctor
setup
Integrations
Hooks
Workspace score explanation
Why this title
Sessions
Appearance/Interface
upgrade preflight
Agy provider surfaces
```

Machine JSON/plain tokens remain locale-independent.

## Release review

Focused independent release-boundary review covers:

- provider capability truthfulness;
- Codex/Agy config ownership and restore;
- Hook trust remains manual;
- Hook inventory privacy;
- root-anchor/session privacy;
- runtime image ownership/GC;
- provider namespace isolation;
- package contents;
- no hidden wrapper/daemon/self-update path.

## Publication sequence

Read-before-retry every irreversible mutation:

```text
settled release candidate
 -> full locked/static/build CI
 -> focused release regressions
 -> cargo package dry-run/content inspection
 -> Windows x64 release build + ZIP + SHA256
 -> real Codex/Agy/multi-provider WT acceptance
 -> merge accepted source
 -> cargo publish 0.6.0
 -> verify crates.io non-yanked package
 -> immutable v0.6.0 tag
 -> GitHub Release/assets
 -> clean crates.io consumer
 -> clean GitHub ZIP consumer
```

Do not blindly retry ambiguous publish/tag/release/upload operations.

## Completion

```text
V0_5_1_RELEASE=PASS
TB_G64=COMPLETE
TB_G65=COMPLETE
TB_G66=COMPLETE
TB_G67=COMPLETE
VERSION=0.6.0
CODEX_PROVIDER=PASS
AGY_PROVIDER=PASS
CODEX_AGY_CONCURRENT=PASS
DAILY_COMMAND_CODEX=codex
DAILY_COMMAND_AGY=agy
ROOT_WORKSPACE_ANCHOR=PASS
HOOK_INSPECTOR=PASS
NAMING_EXPLAINABILITY=PASS
WHY_THIS_TITLE=PASS
INTEGRATIONS_MULTI_PROVIDER=PASS
PROVIDER_CAPABILITY_MATRIX=PASS
PROVIDER_BADGE=PASS
UPGRADE_SAFE_WORKER_RUNTIME=PASS
FRESH_INSTALL=PASS
V050_TO_V060_UPGRADE=PASS
V051_TO_V060_UPGRADE=PASS
REAL_AGY_RELEASE_SMOKE=PASS
REAL_MULTI_PROVIDER_WT_SMOKE=PASS
RELEASE_REVIEW=PASS
CRATES_IO_PUBLISHED=true
GITHUB_RELEASE_PUBLISHED=true
WINDOWS_X64_ASSET_PUBLISHED=true
PUBLIC_CONSUMERS=PASS
V0_6_RELEASE=PASS
```

## Non-goals

No Claude/OpenCode provider, Codex App Server production backend, remote/web dashboard, process/session control, cloud sync, automatic Hook trust, repository-local configuration, self-update, or editable naming-score weights.

## Estimated effort

**4–6 effective engineering hours after G64–G67 are accepted.**