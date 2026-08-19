# TB-G55 — Live Control Center

## Status

PLANNED after accepted G54.

## Purpose

Turn the v0.4 snapshot-based Control Center into a live local management surface that refreshes read-only state without adding a daemon, remote control plane, or async runtime solely for UI refresh.

## Target information architecture

Mandatory screens after G55:

```text
Overview / 概览
Appearance / 外观
Workspace / 工作区
Sessions / 会话
Codex Integration / Codex 集成
Diagnostics / 诊断
Interface / 界面
```

Preview may remain as a dedicated screen, panel, or contextual pane if the accepted UX still provides the same staged-preview semantics.

## Live refresh model

The current Control Center receives one initial management snapshot. G55 adds a bounded refresh cadence, target roughly **500 ms–1 s**.

Preferred loop:

```text
render
  ↓
wait until input or next refresh deadline
  ├─ key event -> handle staged interaction
  └─ refresh deadline -> collect read-only state
  ↓
merge refreshed read-only state without destroying draft state
```

Do not add Tokio, a global daemon, background network service, or provider polling merely for this UI.

## Draft / refresh separation

Live refresh must never overwrite an in-memory dirty draft.

Conceptually maintain separate state:

```text
OperationalSnapshot   # refreshed
CurrentSettings       # accepted persisted baseline
DraftSettings         # user edits
InterfaceDraft
WorkspacePreferenceDraft if editing
```

Refresh updates only observational data and externally changed baselines that can be reconciled safely. If a persisted baseline changes concurrently while a related draft is dirty, surface a conflict and refuse stale Apply rather than silently rebasing the user's edits.

## Overview

Live overview should update health and useful counts without flicker:

```text
TabBeacon/Codex support
Hook/integration status
worker/session health
current workspace alias
attention/warning summary
```

Avoid dumping every diagnostic detail onto the Overview screen.

## Workspace screen

Expose G52/G53 naming and preference domain:

```text
Workspace/display hint
Automatic alias
Effective alias
Naming policy
Override state
Top candidate suggestions
```

Actions:

```text
use adaptive/default
select another suggestion
set custom alias
reset override
explain naming
```

All changes are local TabBeacon preferences. Display a clear localized statement equivalent to:

> Stored locally only. Project files are never modified.

Collision refusal and stale-draft semantics from G53 remain authoritative.

## Sessions screen

Bring post-v0.4 read-only Sessions into the TUI.

Display only approved fields:

```text
workspace alias
semantic state
age / recency
worker health
```

Do not display raw native session IDs, raw turn IDs, prompt/assistant/tool content, private canonical identities, process-control actions, kill buttons, resume/switch/focus control, or persistent activity history.

Long Sessions lists may support controlled key repeat, but not by consuming every queued `Repeat` event unboundedly. If repeat is admitted, use a deliberate initial delay and bounded cadence.

## Interface screen

Use G51 Interface preferences:

```text
Language
Color
Reduced motion if admitted
```

Language/color draft changes should update Human rendering immediately while remaining staged until Apply.

## Integration / Diagnostics

Continue to render shared `ManagementSnapshot`, `HealthIssue`, and `RecommendedAction` semantics rather than creating a second diagnostic model.

G55 may surface action affordances, but actual guided remediation belongs to G56 unless needed for coherent navigation.

## Navigation behavior

Preserve G47 input contract:

```text
Press   -> one page/field/value action
Repeat  -> ignored for page/field navigation by default
Release -> ignored
```

For long lists only, bounded repeat may be implemented with an initial delay roughly 250–350 ms and repeat interval roughly 80–120 ms, subject to deterministic tests and usable real-WT dogfood.

## Rendering / localization

All screen labels and Human messages support en-US/zh-CN through the shared Human layer.

CJK display width, narrow layouts, monochrome/no-color mode, and minimum terminal size remain first-class. Live refresh must not cause layout churn merely because counters change width by one digit; use stable field layouts where practical.

## Performance / resource bounds

Refresh is local and read-only. Establish a bounded budget:

- no unbounded filesystem walk per tick;
- no Git/network invocation per tick;
- no raw Hook payload scan per tick;
- no persistent write simply because a refresh occurred;
- avoid rebuilding expensive immutable data on every frame when it can be cached safely;
- CPU should remain negligible while idle relative to normal terminal use.

A coarse deterministic performance assertion or manual observation is sufficient; do not create a microbenchmark regime without evidence it is needed.

## Terminal lifecycle

Preserve the accepted G46 centralized terminal guard:

- alternate screen restored;
- raw mode disabled;
- cursor restored;
- normal/dirty quit and Ctrl+C clean up;
- draw/event/refresh errors clean up;
- same-shell usability remains proven.

One representative real Windows Terminal smoke must exercise live refresh, navigation through at least Workspace and Sessions, a staged edit + Revert, locale/interface rendering, and clean exit.

## Testing

Required families:

- refresh deadline updates OperationalSnapshot;
- no writes on refresh;
- dirty Appearance draft survives refresh;
- dirty Interface draft survives refresh;
- concurrent external settings change becomes explicit conflict;
- Workspace candidate/override state rendering;
- Workspace collision refusal through TUI action path;
- Sessions current/stale/invalid truthful rendering;
- no prohibited session/private fields in screen buffer;
- en-US/zh-CN buffer tests;
- normal/narrow/minimum widths;
- key repeat policy;
- no-color mode;
- non-TTY never enters Control Center;
- terminal cleanup across refresh/error paths;
- one real WT smoke.

## Risk vector

```text
CODE_CHANGED=true
PRESENTATION_CHANGED=true
PROVIDER_CHANGED=false
USER_PERSISTENT_CONFIG_CHANGED=true
SECURITY_OR_PRIVACY_CHANGED=true   # Sessions/Workspace display boundary
RELEASE_BOUNDARY=false
```

Use ordinary hosted exact-head code CI, one representative real WT/TUI pack, focused persistent-config safety where mutation paths are touched, and one focused privacy review for Sessions/Workspace screen exposure. Do not run provider L4 when provider semantics are unchanged.

## Acceptance

```text
CONTROL_CENTER_LIVE=true
REFRESH_CADENCE_BOUNDED=true
REFRESH_WRITES_STATE=false
DIRTY_DRAFT_SURVIVES_REFRESH=true
CONCURRENT_CONFLICT_VISIBLE=true
OVERVIEW_SCREEN=PASS
APPEARANCE_SCREEN=PASS
WORKSPACE_SCREEN=PASS
SESSIONS_SCREEN=PASS
INTEGRATION_SCREEN=PASS
DIAGNOSTICS_SCREEN=PASS
INTERFACE_SCREEN=PASS
WORKSPACE_PROJECT_FILES_MUTATED=false
SESSIONS_READ_ONLY=true
RAW_NATIVE_SESSION_IDS=false
PROMPT_CONTENT=false
KEY_REPEAT_BOUNDED=true
ZH_CN_TUI=PASS
EN_US_TUI=PASS
NARROW_TERMINAL=PASS
NO_COLOR_TUI=PASS
NON_TTY_NO_FULLSCREEN=true
TUI_EXIT_RESTORES_TERMINAL=true
SHELL_USABLE_AFTER_TUI=true
WINDOWS_TERMINAL_SMOKE=PASS
PRIVACY_REVIEW=PASS
CODE_CI=PASS
```

## Non-goals

No process/session control, remote dashboard, network refresh, provider addition, daemon, wrapper, automatic Hook trust, project-local config, or self-update.

## Estimated effort

**7–11 effective engineering hours.**

## Next

`TB-G56 — Guided Repair, Help & Accessibility`.
