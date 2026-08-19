# TabBeacon v0.4.1 → v0.5 execution roadmap

## Status

PLANNED from post-v0.4 closeout baseline `30bf8bd97527b36fe9211437d8d6c086890e62a8`.

This file is the compact execution index for the next production stage. The authoritative product intent and invariants live in [`../goals/TB-V05-LOCALIZED-EXPERIENCE-ADAPTIVE-WORKSPACE-IDENTITY.md`](../goals/TB-V05-LOCALIZED-EXPERIENCE-ADAPTIVE-WORKSPACE-IDENTITY.md); each numbered Goal below owns its implementation and exit gates.

## Product theme

**v0.5 — Localized Experience & Adaptive Workspace Identity**

Three visible product pillars:

1. **Localized Experience** — English and Simplified Chinese human interfaces, cleaner semantic CLI rendering, and one shared human presentation layer.
2. **Adaptive Workspace Identity** — deterministic scoring-based workspace abbreviations with Unicode/CJK awareness, stable local aliases, and explicit device-local override.
3. **Live Control Center** — live read-only refresh, Workspace/Sessions/Interface screens, guided safe repair, and help/accessibility polish.

A fourth portability layer is first-class in v0.5: top-level `tabbeacon export` / `tabbeacon import` for user-configurable state only.

## Dependency sequence

```text
v0.4.0 RELEASED + post-release G45X on main
        ↓
TB-G47   v0.4.1 Dogfood Maintenance & Release
        ↓
TB-G50   Unified Human Presentation & i18n Foundation
        ↓
TB-G51   Localized Guided Setup & Interface Preferences
        ↓
TB-G52   Adaptive Workspace Naming Engine v2
        ↓
TB-G53   Local Workspace Preferences & Alias Control
        ↓
TB-G54   Settings Export / Import
        ↓
TB-G55   Live Control Center
        ↓
TB-G56   Guided Repair, Help & Accessibility
        ↓
TB-G56R  v0.5 Hardening & Release
```

Default execution is sequential. A long autonomous train may continue into the next Goal only after the predecessor is accepted/merged and the next Goal remains within the same repository authority. Do not manufacture a stop merely because one Goal completes early.

## Goal index

| Goal | Scope | Estimated effort |
| --- | --- | ---: |
| G47 | publish Sessions; key-repeat fix; remove `wt.exe` popup probe; Human output cleanup; minimal semantic color; v0.4.1 release | 6–10 h |
| G50 | typed HumanDocument renderer; locale/color policy; en-US/zh-CN catalogs; stable machine-channel boundary | 7–10 h |
| G51 | language-first Setup; Interface preferences/page; full Human UI localization; CJK width handling | 5–8 h |
| G52 | Unicode/style-aware tokenization; alias candidate generation; deterministic integer scoring; naming corpus | 8–12 h |
| G53 | registry v2 migration; device-local preference overlay; alias show/preview/explain/set/reset; collision refusal | 7–11 h |
| G54 | top-level export/import; staged diff; portable identity digest; round-trip and privacy boundary | 7–10 h |
| G55 | live Control Center refresh; Workspace/Sessions/Interface screens; staged-draft-safe refresh | 7–11 h |
| G56 | guided safe repair; `?` help overlay; reduced-motion/accessibility polish; final UX consistency | 4–7 h |
| G56R | fresh/upgrade/import migration release closure; real WT bilingual smoke; packaging/publication | 4–6 h |
| **Total** | **v0.4.1 through v0.5.0** | **55–85 h** |

## Cross-cutting invariants

```text
DAILY_COMMAND=codex
FAIL_OPEN=true
HOOK_TRUST_BYPASS=false
PROVIDER_ADDED=false
GLOBAL_DAEMON_ADDED=false
PATH_SHADOW_ADDED=false
PTY_WRAPPER_ADDED=false
PROJECT_LOCAL_CONFIG=false
PROJECT_FILES_MUTATED_FOR_PREFERENCES=false
RAW_PROMPT_ASSISTANT_TOOL_CONTENT_PERSISTED=false
RAW_NATIVE_SESSION_IDS_EXPOSED=false
JSON_KEYS_LOCALIZED=false
PLAIN_KEYS_LOCALIZED=false
```

Human-visible text may be localized. Automation contracts, persisted enum spellings, IDs, and structured field names remain stable English machine tokens.

## Local-state model targeted by v0.5

```text
Existing PresentationSettingsStore
          +
InterfacePreferenceStore
          +
StableAliasRegistry v2
          +
WorkspacePreferenceStore
          ↓
     User configuration
          ↓
 tabbeacon export/import
```

Workspace preferences are stored under the user-local TabBeacon state root. TabBeacon must not create `.tabbeacon`, `.tabbeacon.toml`, `tabbeacon.toml`, or equivalent project-local preference files.

## Long-train policy

For unattended 8–12 hour development, start from this roadmap plus the current Goal and the active Jerry Harness route. Use `COMPRESSED_TRAIN_V1` where appropriate. Ordinary correctable code/test/harness/runner failures should consume the admitted correction budget rather than terminating the train immediately. Same-head/same-signature blind reruns remain prohibited.

Typical train partitions:

```text
Train A: G47 release + begin G50
Train B: close G50 + G51 + begin G52
Train C: close G52 + G53
Train D: G54 + begin G55
Train E: close G55 + G56
Train F: G56R v0.5 release + post-release closeout
```

Partitions are estimates, not hard ceremony. Progress and risk gates decide actual boundaries.

## Explicitly deferred

Do not fold these into v0.5 merely because time remains:

- Claude provider (`TB-G20`);
- OpenCode provider (`TB-G30`);
- Codex App Server production backend;
- remote/web dashboard;
- session/process control;
- global daemon;
- wrapper/PATH interception/PTY host;
- self-update service;
- automatic Hook trust;
- repository-local TabBeacon config;
- automatic cross-device sync service.
