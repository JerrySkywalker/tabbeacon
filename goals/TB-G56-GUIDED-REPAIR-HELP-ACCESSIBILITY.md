# TB-G56 — Guided Repair, Help & Accessibility

## Status

PLANNED after accepted G55.

## Purpose

Finish the v0.5 Human experience by making safe remediation understandable, keyboard behavior discoverable, motion/color optional, and narrow/monochrome operation predictable without broadening TabBeacon into an autonomous configuration manager.

## Guided remediation

Reuse the existing management/action safety classes. The UI may expose a repair action only according to its typed class.

Normative behavior:

```text
READ_ONLY
  -> inspect/show only

MANUAL_ACTION
  -> show exact human steps; no automated Apply

PREVIEWABLE_SAFE_REPAIR
  -> Preview -> explicit Apply -> refresh/verify

OWNER_EXPLICIT_REQUIRED
  -> explain boundary and require explicit Owner action outside unattended automation

UNSUPPORTED_AUTOMATION
  -> explain that TabBeacon cannot perform the action safely
```

Hook trust remains `MANUAL_ACTION`. The Control Center may tell the user to launch `codex` and use `/hooks`, but must never fabricate trust state or mark hooks trusted automatically.

Existing ownership-safe Windows Terminal title-policy repair is a suitable representative `PREVIEWABLE_SAFE_REPAIR` when the action model classifies it that way.

## Repair UX

Diagnostics should make the following visible for each actionable issue:

```text
What is wrong
Why it matters
Recommended action
What will change
What will not change
Whether the action is automatic/manual/unsupported
```

Previewable repair must build a typed plan before mutation and show preserved state. After Apply, refresh diagnostics and report verified result.

No repair action may silently change unrelated Windows Terminal, Codex, PowerShell, repository, or user settings.

## Help overlay

Add `?` as the primary contextual help entry point in the Control Center.

The help overlay must explain at least:

```text
page navigation
field/value navigation
Enter/select behavior
Apply
Revert
Quit / dirty quit
Help
language/color controls
Workspace alias semantics
Sessions read-only boundary
```

Help is localized in en-US/zh-CN and width-aware. It must be dismissible without changing draft state.

Footer hints may stay concise and context-sensitive; the overlay carries the complete discoverability contract.

## Accessibility / reduced motion

Finalize `Reduced motion` if it was only scaffolded earlier.

Reduced motion affects Human/TUI visual animation intensity only. It must not change semantic lifecycle state, worker correctness, Hook behavior, or machine contracts.

Possible behavior:

- avoid nonessential animated preview cycling;
- prefer static indicators in the management TUI where motion is decorative;
- preserve the actual production tab activity semantics unless the user explicitly changes the existing Activity setting.

Do not conflate `Reduced motion` with disabling TabBeacon activity reporting globally.

## Color and monochrome

Revalidate all mandatory states with `color=never` and a monochrome terminal. Health/warning/failure/dirty/manual-action distinctions must have text/glyph/layout meaning independent of color.

High-contrast custom themes are not required in v0.5, but style choices should avoid hard-coded assumptions that make a future accessibility theme impossible.

## Keyboard and focus behavior

Complete a consistent keyboard model across all screens:

- one physical Press -> one page/field action;
- bounded repeat only where deliberately admitted for long lists;
- no action on Release;
- focus is always visible without color alone;
- `Esc` cancels/dismisses overlays or returns one level where safe;
- dirty state cannot be silently discarded;
- help/repair overlays do not leak events into the underlying screen;
- Ctrl+C cleanup remains safe.

## Narrow terminal behavior

Every screen/overlay must either:

- render a compact readable layout; or
- render a clear localized `Terminal too small` message with required minimum dimensions.

No panic, integer underflow, broken border, or unusable hidden confirmation control at minimum width/height.

## Setup / CLI consistency pass

Use the shared Human renderer to perform one final consistency pass over:

```text
status
doctor
setup
config summaries
sessions
alias commands
export/import summaries
```

Goals:

- consistent success/warning/failure vocabulary;
- consistent final-summary placement;
- no stray machine flags in Human mode;
- consistent next-action wording;
- consistent bilingual terminology for Workspace/Alias/Session/Integration/Apply/Revert.

This is a polish pass, not permission to redesign the machine schemas.

## Testing

Required families:

- each action safety class renders the correct affordance;
- manual Hook trust cannot become an automated action;
- previewable title-policy repair Preview/Cancel/Apply in isolated state;
- preserved unrelated state after representative repair;
- `?` help open/close no draft mutation;
- en-US/zh-CN help buffers;
- overlay event isolation;
- reduced-motion behavior;
- no-color/monochrome semantic distinction;
- keyboard-only completion of every mandatory screen/action;
- key repeat and Release filtering;
- narrow/minimum terminal layouts;
- Ctrl+C/error cleanup with overlays;
- Human CLI consistency snapshots/semantic assertions.

## Risk vector

```text
CODE_CHANGED=true
PRESENTATION_CHANGED=true
PROVIDER_CHANGED=false
USER_PERSISTENT_CONFIG_CHANGED=true   # Interface reduced-motion + repair paths where applicable
SECURITY_OR_PRIVACY_CHANGED=true      # remediation authority surface
RELEASE_BOUNDARY=false
```

Use ordinary hosted exact-head CI, one focused independent safety review of remediation authority, focused persistent-config ownership proof where repair writes external/user config, and one representative real WT/TUI accessibility smoke. No provider L4 unless provider/trust semantics themselves change.

## Acceptance

```text
GUIDED_SAFE_REPAIR=PASS
ACTION_SAFETY_CLASSES_ENFORCED=true
HOOK_TRUST_AUTOMATION=false
REPAIR_PREVIEW_FIRST=true
REPAIR_CANCEL_LOSSLESS=true
UNRELATED_CONFIG_PRESERVED=true
HELP_OVERLAY=PASS
HELP_ZH_CN=PASS
HELP_EN_US=PASS
KEYBOARD_ONLY_COMPLETE=true
KEY_REPEAT_BOUNDED=true
KEY_RELEASE_IGNORED=true
FOCUS_VISIBLE_WITHOUT_COLOR=true
REDUCED_MOTION=PASS
COLOR_NOT_SOLE_SIGNAL=true
NO_COLOR_TUI=PASS
NARROW_TERMINAL=PASS
HUMAN_OUTPUT_CONSISTENT=true
HUMAN_MACHINE_FLAGS_SEPARATED=true
TUI_EXIT_RESTORES_TERMINAL=true
WINDOWS_TERMINAL_SMOKE=PASS
SAFETY_REVIEW=PASS
CODE_CI=PASS
```

## Non-goals

No new provider, automatic Hook trust, arbitrary repair scripts, process/session control, remote dashboard, daemon, wrapper, project-local config, cloud sync, or self-update.

## Estimated effort

**4–7 effective engineering hours.**

## Next

`TB-G56R — v0.5 Hardening & Release`.
