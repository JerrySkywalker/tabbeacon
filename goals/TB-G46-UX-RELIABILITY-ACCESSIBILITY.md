# TB-G46 — UX Reliability & Accessibility

## Status

COMPLETE after accepted G44. The bounded real Windows Terminal fixture and the
final hosted exact-head code CI passed before PR 41 merged.

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

The real-terminal proof uses the feature-gated
`tabbeacon-terminal-smoke-fixture` binary in a disposable `wt.exe` window. It
drives production app events internally rather than injecting operating-system
keys, enters and leaves the production Crossterm/Ratatui terminal lifecycle,
stages and reverts an in-memory appearance draft, and has no Apply callback or
settings authority. The same child shell writes the post-TUI sentinel; an
external observer verifies exact-title window and child-process completion.

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

Candidate evidence:

```text
WINDOWS_TERMINAL_TUI_SMOKE=PASS
TUI_EXIT_RESTORES_TERMINAL=true
SHELL_USABLE_AFTER_TUI=true
OWNER_MUTATIONS=none
```

Estimated effort: **3–5 h**.
