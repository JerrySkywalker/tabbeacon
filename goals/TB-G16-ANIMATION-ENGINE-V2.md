# TB-G16 — Animation Engine v2 and Defaults

## Status

PLANNED. Depends on accepted TB-G15 evidence so animation changes do not mask
an unresolved title-authority problem. It inherits the post-release worker and
owned-tab UIA safety baseline and must not turn diagnosed persistent contention
into an endless title fight.

## Purpose

Make working-state title animation a real default product behavior rather than an implemented-but-opt-in capability. Standardize the default cadence at 100 ms and eliminate stale pre-G11 defaults/comments.

## Required product defaults

For new/default installs:

```text
title=tabbeacon
tab_color=tabbeacon
activity=title-spinner
spinner=braille
theme=muted-dark
TARGET_FRAME_INTERVAL_MS=100
```

Presets v2:

```text
native   -> native title + native color + native activity
minimal  -> TabBeacon title + native color + static indicator
balanced -> TabBeacon title + TabBeacon color + 100 ms braille spinner
full     -> TabBeacon title + TabBeacon color + 100 ms braille spinner + WT ring
```

`balanced` becomes the recommended default.

## Existing-user migration rule

Do not silently rewrite an existing `%LOCALAPPDATA%\TabBeacon\config.toml`.

Guided setup should recognize a legacy v0.2 static profile where practical and offer the v0.3 balanced profile as a recommended migration. Persistence still requires explicit Apply.

An absent settings file uses the new v0.3 defaults automatically.

## Animation engine requirements

### 100 ms cadence

Replace the predecessor's nominal 180 ms frame interval with a normative 100 ms target.

Use monotonic deadline scheduling rather than accumulating `sleep + render` drift. Overrun handling must be bounded and must never busy-loop.

### Stable identity

Only the mutable left status slot advances. Workspace alias text on the right must remain byte/position stable across frames.

### Working lifecycle

Reliable active-work evidence starts/updates the existing session/turn/terminal-scoped worker. Reliable result/attention/end/newer-generation evidence stops/supersedes it and hands off to a static final state.

### Bounded static settle

Introduce or admit a short-lived bounded static-state convergence mechanism for startup/result/attention states when TB-G15 evidence shows a startup overwrite race. This must terminate automatically and must not become an always-running service.

A persistent external writer must remain classified `contended`; do not hide it by endless reassertion.

## Performance envelope

Measure at least:

```text
1 active animated tab
4 active animated tabs
8 active animated tabs
```

Record CPU/memory/write-rate observations and prove:

```text
NO_BUSY_LOOP=true
ONE_WORKER_PER_ACTIVE_SESSION=true
MEMORY_BOUNDED=true
NO_WORKER_LEAK=true
NO_VISIBLE_TERMINAL_LAG=true
```

Do not add arbitrary user-configurable frame milliseconds unless evidence shows a real need. The default is 100 ms.

## Documentation debt cleanup

Remove/update targeted comments and docs that still describe animation as future/unproven or claim static indication is required until worker feasibility exists.

Do not rewrite historical records that were correct at the time.

## Validation

- focused settings/preset tests;
- worker timing/deadline tests;
- generation/supersession tests;
- 1/4/8-tab performance evidence;
- exact-head Visual CI proving at least three distinct valid working frames within one second and stable alias text;
- one final hosted code CI;
- no generic repeated audit.

## Exit gate

```text
DEFAULT_ACTIVITY=title-spinner
DEFAULT_SPINNER=braille
TARGET_FRAME_INTERVAL_MS=100
DEADLINE_SCHEDULING=PASS
VISIBLE_WORKING_FRAMES_GE_3_WITHIN_1S=PASS
WORKSPACE_ALIAS_STABLE=true
LEGACY_CONFIG_SILENTLY_REWRITTEN=false
TABS_1_4_8_PERFORMANCE=PASS
NO_GLOBAL_DAEMON=true
```

## Exit receipt

```text
GOAL_ID=TB-G16
DISPOSITION=<PASS|FAIL|BLOCKED|UNPROVEN>
STARTING_MAIN=<sha>
EXPECTED_HEAD=<sha>
DEFAULT_ACTIVITY=<value>
DEFAULT_SPINNER=<value>
TARGET_FRAME_INTERVAL_MS=<value>
PERFORMANCE_1_TAB=<...>
PERFORMANCE_4_TABS=<...>
PERFORMANCE_8_TABS=<...>
CI=<...>
VISUAL=<...>
OWNER_ACTION=<none-or-specific>
```
