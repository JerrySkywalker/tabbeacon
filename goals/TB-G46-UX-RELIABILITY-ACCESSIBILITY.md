# TB-G46 — UX Reliability & Accessibility

## Status

PLANNED after accepted G44. Mandatory v0.4 hardening before release closure.

## Purpose

Prove that the new human interfaces behave safely across terminal exits, errors, resize, non-TTY use, and accessibility constraints. Full-screen terminal state is a new product risk and needs explicit bounded validation.

## TUI terminal-state requirements

Cover:

```text
normal q exit
Apply then exit
Revert then exit
Ctrl+C
error exit
panic/unwind-safe cleanup where practical
terminal resize
minimum supported terminal size
```

Critical invariant:

```text
TUI_EXIT_RESTORES_MAIN_SCREEN=true
TUI_EXIT_LEAVES_RAW_MODE=false
SHELL_USABLE_AFTER_TUI=true
```

## Accessibility/readability

- keyboard-only navigation is complete;
- mouse is optional;
- color is decorative, not the sole carrier of state;
- monochrome/no-color mode remains meaningful;
- Unicode glyphs have understandable text labels/fallbacks where needed;
- narrow terminals degrade to a compact layout or clear minimum-size message rather than corrupt rendering.

## Non-TTY

Redirected stdout/stderr and automation paths must never enter raw mode or alternate screen. Human snapshot commands should remain copyable and stable enough for support/debugging while JSON remains the machine contract.

## Validation

- deterministic terminal-backend tests for cleanup paths;
- Ratatui buffer tests for resize/minimum layouts;
- no-color tests;
- non-TTY tests;
- one bounded real Windows Terminal Control Center smoke proving enter/navigation/exit leaves the originating shell usable;
- one final hosted exact-head code CI.

Do not create a large UIA scenario matrix; one representative terminal-state smoke is the risk gate.

## Exit gate

```text
TUI_EXIT_RESTORES_TERMINAL=true
SHELL_USABLE_AFTER_TUI=true
KEYBOARD_ONLY_COMPLETE=true
COLOR_NOT_SOLE_SIGNAL=true
NARROW_TERMINAL=PASS
NON_TTY_NO_FULLSCREEN=true
WINDOWS_TERMINAL_TUI_SMOKE=PASS
CODE_CI=PASS
```

Estimated effort: **3–5 h**.
