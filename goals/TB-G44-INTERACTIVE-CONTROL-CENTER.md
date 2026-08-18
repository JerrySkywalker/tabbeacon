# TB-G44 — Interactive Control Center

## Status

COMPLETE. The bounded Ratatui/Crossterm frontend provides all five screens,
staged Apply/Revert, live preview, dirty-quit confirmation, non-TTY guidance,
and deterministic buffer coverage. The accepted code candidate passed hosted
Windows exact-head CI; G46 is next for terminal-state hardening and smoke
evidence.

## Purpose

Add a pure-Rust full-screen daily management interface without replacing the inline setup wizard or the direct CLI/machine interfaces.

## Entrypoints

```text
tabbeacon
  interactive TTY -> Control Center
  non-TTY         -> deterministic help/guidance

tabbeacon ui
  explicit Control Center entry
```

Do not change the daily coding-agent command: Codex still starts as `codex`.

## Technology

Preferred frontend stack:

```text
ratatui
crossterm
```

The TUI is a frontend over the shared management/configuration model; it must not write config/hooks/WT policy directly.

## Mandatory screens

```text
Overview
Appearance
Codex Integration
Diagnostics
Preview
```

## Interaction

Keyboard-first, with mouse optional and never required. Suggested conventions:

```text
↑↓ Navigate
←→ Change
Enter Select
a Apply
r Revert
q Quit
```

Exact keys may evolve if documented consistently.

## Staged edits

Configuration changes update an in-memory draft and live preview only.

```text
TUI_EDITS_STAGED=true
LIVE_PREVIEW=true
WRITE_BEFORE_APPLY=false
CANCEL_LOSSLESS=true
```

Quit with unsaved changes must ask to keep editing or discard; it must never silently persist.

## Layout/testing

Use Ratatui buffer/TestBackend-style deterministic tests for normal, narrow, and minimum supported terminal sizes. Real Windows Terminal UIA is not the primary layout test mechanism.

## Safety

- no automatic Hook trust;
- no raw native session/prompt/tool display;
- no global daemon;
- no session control/process kill;
- no shell profile edits.

## Validation

- deterministic screen/buffer tests;
- navigation/focus tests;
- staged Apply/Revert/quit-with-dirty-state tests;
- resize/minimum-width tests;
- no-color/monochrome semantics;
- one final hosted CI;
- terminal-state restoration belongs to G46 and may gate final production admission.

## Exit gate

```text
CONTROL_CENTER=PASS
OVERVIEW_SCREEN=PASS
APPEARANCE_SCREEN=PASS
INTEGRATION_SCREEN=PASS
DIAGNOSTICS_SCREEN=PASS
PREVIEW_SCREEN=PASS
CONTROL_CENTER_STAGED_APPLY=true
LIVE_PREVIEW=true
CANCEL_LOSSLESS=true
NON_TTY_NO_FULLSCREEN=true
CODE_CI=PASS
```

Estimated effort: **7–11 h**.
