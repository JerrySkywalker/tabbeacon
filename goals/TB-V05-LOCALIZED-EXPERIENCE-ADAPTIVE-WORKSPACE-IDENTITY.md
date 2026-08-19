# TabBeacon v0.5 — Localized Experience & Adaptive Workspace Identity

## Status

PLANNED after the public v0.4.0 release and post-release G45X Sessions merge. The first production step is the bounded v0.4.1 maintenance release in G47; v0.5 implementation begins only after G47 is accepted.

Planning anchor: `30bf8bd97527b36fe9211437d8d6c086890e62a8`. Implementers must resolve the current authoritative `main` before starting a Goal rather than assuming this SHA remains current.

## Product objective

v0.4 made TabBeacon understandable and configurable by humans. v0.5 should make that experience **native to the user, adaptive to the workspace, portable across machines where safe, and live while the terminal is running**.

The release has three flagship capabilities plus one portability layer:

```text
Localized Experience
  en-US / zh-CN human interfaces
  clean semantic CLI output
  shared HumanDocument renderer

Adaptive Workspace Identity
  style/Unicode-aware tokenization
  deterministic candidate scoring
  stable generated alias
  device-local explicit override

Live Control Center
  periodic read-only refresh
  Workspace / Sessions / Interface screens
  guided safe remediation and help

Portable User Configuration
  tabbeacon export
  tabbeacon import
  no security/runtime-state transfer
```

## Findings motivating this stage

Real v0.4 dogfood established several maintenance facts:

- `tabbeacon sessions` landed after immutable v0.4.0 publication and therefore is not present in the crates.io v0.4.0 binary;
- Control Center Up/Down navigation consumes repeated key events too aggressively; issue #45 records the dogfood defect;
- guided Setup probes Windows Terminal by spawning `wt.exe`, which can open an unwanted Terminal window and must be replaced by non-launching detection;
- healthy Human setup/config flows still mix machine-style flags such as `SETUP=PASS`, `OWNER_ACTION=none`, `CONFIG_PATH`, and internal feasibility fields into normal user output;
- Human CLI rendering needs restrained semantic color while preserving monochrome meaning;
- current abbreviation logic is stable and deterministic but strongly prefers shortest initialisms, which can collapse single-token names such as `tabbeacon` to an unhelpful one-character alias;
- current Control Center is interactive but snapshot-based rather than live-refreshing.

These are product inputs, not reasons to weaken existing automation or ownership contracts.

## Product invariants retained

```text
DAILY_COMMAND=codex
FAIL_OPEN=true
GLOBAL_DAEMON_BASELINE=false
HOOK_TRUST_BYPASS=false
PROVIDER_NEUTRAL_CORE=true
OFFLINE_WORKSPACE_IDENTITY=true
NO_PATH_SHADOW=true
NO_PTY_WRAPPER=true
NO_LAUNCH_WRAPPER=true
PROJECT_LOCAL_CONFIG=false
RAW_PROMPT_TOOL_MODEL_CONTENT_PERSISTED=false
RAW_NATIVE_SESSION_IDS_EXPOSED=false
```

## Human / machine boundary

Localization and color apply only to Human presentation.

Must remain stable machine tokens:

```text
CLI command names
JSON keys and enum values
--plain keys and enum values
persisted machine enum spellings
error/diagnostic IDs
provider profile IDs
```

Human UI may display localized labels for those values, but it must never rewrite the underlying automation schema based on locale.

## Localized Experience architecture

Introduce one shared human presentation layer rather than adding more ad-hoc `println!()` strings. The exact Rust names may evolve, but the architecture must support a model equivalent to:

```text
HumanDocument
  title
  summary/status
  sections[]
    fields[]
    issues[]
    actions[]
```

Renderers consume this model for scrollback CLI and TUI surfaces. Domain/management code remains locale-neutral.

Supported locales for v0.5:

```text
auto
en-US
zh-CN
```

Resolution order:

```text
explicit command override if admitted
  ↓
TABBEACON_LANG if admitted
  ↓
local Interface preference
  ↓
OS locale
  ↓
en-US
```

The persistent Interface preference must be user-local, not project-local.

## Adaptive Workspace Identity architecture

The current `AbbreviationPolicy` and `StableAliasRegistry` remain the compatibility baseline. v0.5 upgrades candidate generation and selection without destroying existing assignments.

Pipeline:

```text
raw display name
  ↓
Unicode normalization
  ↓
style-aware tokenization
  ↓
candidate generation
  ↓
deterministic integer scoring
  ↓
display-width budget
  ↓
collision resolution
  ↓
stable generated alias
  ↓
optional local preference override
  ↓
effective alias
```

Tokenization must cover at least:

```text
kebab-case
snake_case
space separated
dot separated
camelCase
PascalCase
acronym-to-word boundaries (XMLHttpRequest)
digit boundaries
Unicode/CJK names
mixed Latin/CJK names
```

Candidate families should include, where applicable:

```text
INITIALISM
ACRONYM_AWARE
BALANCED_PREFIX
SINGLE_TOKEN_READABLE_PREFIX
CONSONANT_SKELETON
UNICODE_PREFIX
HASH_FALLBACK
```

Selection is score-based, not a chain of fragile one-off special cases. Scoring must be deterministic across supported platforms and should use integer components with a deterministic tie-break. Factors include token coverage, acronym preservation, recognizability, target display width, information loss, awkward one-character penalties, and collision cost.

Existing aliases do not silently change merely because the policy implementation advances. A v0.4 user keeps the current stable assignment unless the user explicitly requests a new adaptive choice.

## Local preference overlay

User override is local TabBeacon state, never repository content.

Conceptual model:

```text
GeneratedAliasRecord {
  generated_alias,
  policy_version
}

WorkspaceAliasPreference {
  override_alias: Option<RepositoryAlias>
}

EffectiveAlias = override_alias ?? generated_alias
```

The implementation may store generated identity state and user preference state separately; the preferred design is:

```text
%LOCALAPPDATA%\TabBeacon\repository-identity\
%LOCALAPPDATA%\TabBeacon\workspace-preferences\
```

No `.tabbeacon`, `.tabbeacon.toml`, `tabbeacon.toml`, or similar project-local preference file is admitted in v0.5.

An override is bound to canonical workspace identity rather than a raw visible path. Explicit override collision with another effective local alias is rejected; TabBeacon must not silently rename the other workspace, swap aliases, or append a hash to a value the user explicitly chose.

## Export / Import architecture

v0.5 adds top-level commands:

```text
tabbeacon export
tabbeacon import
```

The exported document is versioned and contains only user-configurable TabBeacon state intended for migration/backup, including applicable Presentation settings, Interface preferences, and Workspace alias preferences.

It must not export or import:

```text
Hook trust state
raw Hook declarations as trusted state
runtime sessions / leases
raw native session or turn IDs
credentials / cookies / tokens
Windows Terminal machine/profile state
PowerShell profile state
absolute private workspace paths
arbitrary diagnostic/runtime logs
```

For portable Git workspace preference matching, prefer a stable digest of canonical identity rather than exposing the raw canonical identity. Ordinary directory identity is path-derived and therefore device-local; it must not masquerade as portable cross-device identity.

Import is preview/stage first. Validate the entire document and construct one typed `ImportPlan` before mutation. Interactive import shows a diff and asks for Apply; non-interactive mutation requires an explicit apply flag. Partial failure must not silently leave an unreported half-imported state.

## Live Control Center architecture

The v0.4 Control Center starts from a snapshot. v0.5 should refresh read-only operational data on a bounded cadence (target roughly 500 ms–1 s) without adding Tokio or a resident daemon solely for refresh.

Target screens:

```text
Overview / 概览
Appearance / 外观
Workspace / 工作区
Sessions / 会话
Codex Integration / Codex 集成
Diagnostics / 诊断
Interface / 界面
```

A refresh may update read-only management/session/workspace state but must not overwrite an in-memory dirty settings draft. Apply/Revert remains explicit and ownership-aware.

## UX policy

- page and field navigation is edge-triggered by default: one physical press produces one navigation step;
- repeated key events must not race across multiple pages or values;
- long lists such as Sessions may later admit deliberate bounded repeat with an initial delay and controlled cadence;
- semantic color is optional decoration, never the sole carrier of health/state;
- TUI and CLI must remain usable in monochrome and on narrow terminals;
- language changes in the Interface page should update Human rendering without changing machine contracts;
- `?` opens a help overlay explaining keyboard and Apply/Revert/Quit semantics;
- safe repairs may be previewed/applied only through existing safety classes; Hook trust remains MANUAL_ACTION.

## Dependency DAG

```text
v0.4.0 RELEASED
 + G45X COMPLETE POST-RELEASE
          ↓
TB-G47 — v0.4.1 Dogfood Maintenance & Release
          ↓
TB-G50 — Unified Human Presentation & i18n Foundation
          ↓
TB-G51 — Localized Guided Setup & Interface Preferences
          ↓
TB-G52 — Adaptive Workspace Naming Engine v2
          ↓
TB-G53 — Local Workspace Preferences & Alias Control
          ↓
TB-G54 — Settings Export / Import
          ↓
TB-G55 — Live Control Center
          ↓
TB-G56 — Guided Repair, Help & Accessibility
          ↓
TB-G56R — v0.5 Hardening & Release
```

Default production path is sequential. A compressed long-running train may chain accepted Goals in this order rather than stopping after every merge.

## Goal summary and effort

| Goal | Scope | Estimate |
| --- | --- | ---: |
| G47 | dogfood fixes + official Sessions + v0.4.1 | 6–10 h |
| G50 | shared Human renderer, i18n/color contracts, interface preference foundation | 7–10 h |
| G51 | bilingual Setup/TUI integration and Interface page | 5–8 h |
| G52 | Naming Engine v2 + scoring + Unicode corpus | 8–12 h |
| G53 | registry v2, local preference overlay, alias CLI/TUI domain | 7–11 h |
| G54 | export/import, staged migration, privacy/round-trip | 7–10 h |
| G55 | live Control Center + Workspace/Sessions/Interface screens | 7–11 h |
| G56 | guided repair/help/accessibility/reduced-motion polish | 4–7 h |
| G56R | v0.5 release hardening/publication | 4–6 h |
| **Total** | **v0.4.1 through v0.5.0** | **55–85 h** |

## Validation strategy

Use Fast Lane v2. During implementation, prefer focused fixtures and deterministic tests. One settled candidate receives one canonical hosted exact-head code CI. Add presentation/real-WT evidence only when the changed risk affects the visible terminal/TUI behavior. Persistent local preference/import changes receive focused ownership/restore/drift safety proof. Public release receives one deliberate closure train.

Naming validation must include a versioned corpus of at least 30–50 representative names covering Latin styles, acronyms, digits, single-token brands, collisions, CJK, mixed Unicode, and width boundaries.

Localization validation must include both en-US and zh-CN Human snapshots/semantic assertions plus CJK terminal-width cases. Machine JSON/plain tests must prove locale independence.

## Non-goals

Do not add in v0.5:

- Claude provider;
- OpenCode provider;
- Codex App Server production backend;
- web or remote dashboard;
- process/session control or kill/switch/focus control plane;
- global daemon;
- wrapper/PATH interception/PTY host;
- automatic Hook trust;
- repository-local TabBeacon settings;
- background cloud synchronization service;
- self-update service.

## v0.5 release definition

```text
TABBEACON_V05=PASS
V0_4_1_RELEASE=PASS
DAILY_COMMAND=codex

HUMAN_RENDERER_SHARED=true
LOCALES=en-US,zh-CN
LANGUAGE_AUTO=true
JSON_LOCALE_INDEPENDENT=true
PLAIN_LOCALE_INDEPENDENT=true
HUMAN_MACHINE_FLAGS_SEPARATED=true
COLOR_AUTO_ALWAYS_NEVER=true

ADAPTIVE_NAMING_V2=PASS
NAMING_DETERMINISTIC=true
NAMING_OFFLINE=true
NAMING_CORPUS=PASS
EXISTING_ALIAS_MIGRATION_PRESERVED=true
PROJECT_LOCAL_CONFIG=false
LOCAL_ALIAS_OVERRIDE=PASS
OVERRIDE_COLLISION_REFUSED=true

EXPORT=PASS
IMPORT=PASS
IMPORT_PREVIEW_FIRST=true
EXPORT_IMPORT_ROUND_TRIP=PASS
HOOK_TRUST_EXPORTED=false
RUNTIME_SESSIONS_EXPORTED=false
PRIVATE_PATHS_EXPORTED=false

CONTROL_CENTER_LIVE=true
WORKSPACE_SCREEN=PASS
SESSIONS_SCREEN=PASS
INTERFACE_SCREEN=PASS
GUIDED_SAFE_REPAIR=PASS
HELP_OVERLAY=PASS
TUI_KEY_REPEAT_BOUNDED=true
TUI_EXIT_RESTORES_TERMINAL=true

HOOK_TRUST_BYPASS=false
PROVIDER_ADDED=false
GLOBAL_DAEMON_ADDED=false
WRAPPER_ADDED=false

V0_5_RELEASE=PASS
```
