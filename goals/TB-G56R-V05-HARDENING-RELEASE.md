# TB-G56R — v0.5 Hardening & Release

## Status

PLANNED final production closure after accepted G47, G50, G51, G52, G53, G54, G55, and G56.

## Purpose

Publish one accepted v0.5.0 source proving bilingual Human UX, Adaptive Workspace Identity, local alias preferences, export/import portability, and the Live Control Center while preserving every original TabBeacon safety invariant.

## Mandatory upgrade paths

Prove at least:

```text
public v0.4.0
  -> v0.5.0
  -> existing Presentation settings preserved
  -> old stable alias assignments preserved
  -> Interface defaults resolve safely
  -> post-v0.4 Sessions available
  -> Setup/Status/Doctor/Control Center healthy
```

and:

```text
public v0.4.1
  -> v0.5.0
  -> no regression of G47 dogfood fixes
  -> registry/preference migration
  -> bilingual/live/naming features available
```

Also cover a clean/fresh v0.5 installation.

## Fresh-install path

Use isolated state and prove:

```text
install v0.5.0 candidate
  ↓
tabbeacon setup
  ↓
Language / 语言 selection
  ↓
Presentation + Interface draft
  ↓
Preview
  ↓
Apply
  ↓
manual Hook trust handoff only if required
  ↓
status / doctor / Control Center
```

No internal enum typing should be required for normal Setup.

## Localization release gate

Prove both supported Human locales:

```text
en-US
zh-CN
```

Required surfaces:

- status;
- doctor;
- guided Setup;
- config Human summary;
- sessions;
- alias commands;
- export/import summaries;
- all mandatory Control Center screens;
- help and guided repair.

Machine JSON/plain outputs must remain locale-independent.

Run one representative real Windows Terminal smoke in each locale, or one smoke that switches locale in-process and proves both layouts while retaining the same terminal lifecycle.

## Adaptive naming release gate

Validate the accepted G52 corpus against the release head.

Required claims:

```text
naming deterministic/offline
Unicode/grapheme/display width safe
single-token names avoid trivial one-character collapse when better candidates exist
acronym boundaries preserved
hash fallback bounded
existing old aliases preserved after upgrade
new aliases use adaptive-v2
```

Do not regenerate legacy aliases merely for release neatness.

## Workspace preference release gate

Prove:

- device-local preference overlay;
- project directory remains untouched;
- candidate selection/custom set/reset;
- collision refusal;
- linked-worktree semantics;
- concurrent-drift refusal;
- normal Human output does not expose raw canonical private identity/path.

## Export / Import release gate

Run isolated round-trip:

```text
configured v0.5 state
  -> tabbeacon export
  -> fresh state
  -> tabbeacon import preview
  -> Apply
  -> export again
```

Canonical user-configurable semantics must match after normalization.

Explicitly inspect the export package and prove absence of:

```text
Hook trust
runtime sessions/leases
credentials/tokens/cookies
raw native session/turn IDs
Windows Terminal machine/profile state
PowerShell profile state
absolute private workspace paths
```

Prove Git portable identity digest behavior and the accepted truthful policy for ordinary directory preferences.

## Live Control Center release gate

Real Windows Terminal smoke must cover:

```text
enter actual TUI/raw-mode/alternate screen
live refresh occurs
navigate Overview -> Workspace -> Sessions -> Interface
stage one Appearance or Interface change
Revert
open/close Help
exercise one safe read-only/preview repair path if practical
exit
same-shell sentinel proves terminal restored and shell usable
```

No Owner production preferences need to be mutated; use isolated/disposable state.

## Human output release gate

Default Human interfaces must contain no stray machine-only receipt/debug flags. Semantic color should be restrained and monochrome-safe. Redirected output in auto color mode must not contain unwanted ANSI.

## Automation compatibility

Required:

```text
status JSON/plain compatibility
 doctor JSON/plain compatibility
 sessions JSON/plain compatibility
 locale independence
 stable existing CLI commands
 direct config commands
 non-TTY deterministic behavior
```

New `alias`, `export`, and `import` machine contracts must be versioned/explicit where structured output is provided.

## Safety / privacy review

One focused release-boundary independent review covers:

- Hook trust remains manual;
- provider boundary unchanged;
- Interface and Workspace local stores;
- alias registry migration;
- import transaction/rollback semantics;
- export exclusions;
- guided repair action classification;
- project-local configuration prohibition;
- sessions privacy;
- package contents.

Do not repeat unrelated historical provider/convergence matrices when their risk paths are unchanged.

## Release code/package gates

Use one settled release candidate:

- full locked Rust/static/build CI;
- focused release regression suites;
- cargo package dry-run and content inspection;
- Windows x64 release build;
- ZIP content inspection;
- SHA-256 sidecar;
- real WT v0.5 smoke;
- release review.

## Publication sequence

Read-before-retry every external mutation.

```text
accepted immutable release source
  ↓
cargo publish 0.5.0
  ↓
verify crates.io non-yanked package
  ↓
create immutable v0.5.0 tag at release source
  ↓
GitHub Release
  ↓
Windows x64 ZIP + SHA256 assets
  ↓
clean public crates.io consumer
  ↓
clean GitHub-asset consumer
```

If publication response is ambiguous, inspect remote state before retrying. Never duplicate publish/tag/release operations blindly.

## Public-consumer verification

From clean isolated consumer locations verify:

- `cargo install tabbeacon --version 0.5.0 --locked` with supported Rust;
- GitHub Windows archive executes and reports 0.5.0;
- fresh Setup reaches bilingual path;
- `tabbeacon status`, `doctor`, `sessions`, `alias`, `export` basic surfaces work;
- non-TTY help/structured commands do not hang.

## Product invariants

```text
DAILY_COMMAND=codex
FAIL_OPEN=true
HOOK_TRUST_BYPASS=false
PROVIDER_ADDED=false
GLOBAL_DAEMON_ADDED=false
PATH_SHADOW_ADDED=false
PTY_WRAPPER_ADDED=false
PROJECT_LOCAL_CONFIG=false
PROCESS_SESSION_CONTROL=false
REMOTE_CONTROL=false
SELF_UPDATE=false
```

## Completion definition

```text
TB_G47=COMPLETE
TB_G50=COMPLETE
TB_G51=COMPLETE
TB_G52=COMPLETE
TB_G53=COMPLETE
TB_G54=COMPLETE
TB_G55=COMPLETE
TB_G56=COMPLETE

VERSION=0.5.0
LOCALES=en-US,zh-CN
HUMAN_RENDERER_SHARED=true
JSON_LOCALE_INDEPENDENT=true
PLAIN_LOCALE_INDEPENDENT=true
HUMAN_MACHINE_FLAGS_SEPARATED=true

ADAPTIVE_NAMING_V2=PASS
NAMING_CORPUS=PASS
EXISTING_ALIASES_PRESERVED=true
PROJECT_LOCAL_CONFIG=false
LOCAL_ALIAS_OVERRIDE=PASS
OVERRIDE_COLLISION_REFUSED=true

EXPORT=PASS
IMPORT=PASS
EXPORT_IMPORT_ROUND_TRIP=PASS
HOOK_TRUST_EXPORTED=false
PRIVATE_PATHS_EXPORTED=false

CONTROL_CENTER_LIVE=true
WORKSPACE_SCREEN=PASS
SESSIONS_SCREEN=PASS
INTERFACE_SCREEN=PASS
GUIDED_SAFE_REPAIR=PASS
HELP_OVERLAY=PASS
TUI_EXIT_RESTORES_TERMINAL=true

FRESH_INSTALL=PASS
V04_TO_V05_UPGRADE=PASS
V041_TO_V05_UPGRADE=PASS
RELEASE_REVIEW=PASS
CRATES_IO_PUBLISHED=true
GITHUB_RELEASE_PUBLISHED=true
WINDOWS_X64_ASSET_PUBLISHED=true
PUBLIC_CONSUMERS=PASS

V0_5_RELEASE=PASS
```

## Non-goals

Do not use release pressure to introduce Claude/OpenCode providers, App Server production backend, cloud sync, remote dashboard, process/session control, daemon, wrapper/PATH interception, automatic Hook trust, repository-local settings, or self-update.

## Estimated effort

**4–6 effective engineering hours after all predecessor Goals are accepted.**
