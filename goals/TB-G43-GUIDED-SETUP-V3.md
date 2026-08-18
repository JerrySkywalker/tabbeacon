# TB-G43 — Guided Setup v3

## Status

PLANNED after accepted G42.

## Purpose

Replace typed-enum setup with a Hermes-style inline, scrollback-preserving guided flow. This is the first v0.4 flagship feature and is deliberately distinct from the later full-screen Control Center.

## Interaction contract

```text
ALTERNATE_SCREEN=false
SCROLLBACK_PRESERVED=true
ENUM_TYPING_REQUIRED=false
PRESET_SELECTION_ATOMIC=true
CUSTOM_OPTIONS_ONLY_AFTER_EXPLICIT_CUSTOMIZE=true
WRITE_BEFORE_APPLY=false
```

Use arrow/select/confirm primitives (for example `dialoguer`) rather than requiring users to type internal values such as `title-spinner`, `braille`, or `muted-dark`.

## Fresh-install flow

1. welcome and short privacy statement;
2. environment discovery: Windows Terminal, Codex/profile, TabBeacon binary, integration/title-policy state;
3. choose presentation: Recommended, Minimal, Full, Native, Customize;
4. selecting a preset shows its resolved settings and asks Use / Customize / Back; it does not then re-ask every field;
5. Customize drills into Title, Tab color, Activity, Spinner, Theme using selectable values;
6. preview using the production renderer path;
7. staged change summary including what will and will not be modified;
8. Apply / Cancel;
9. clear manual `/hooks` trust handoff if required.

## Returning-user flow

Show compact current health/configuration then offer choices such as:

```text
Appearance
Integration
Repair detected issues
Full setup
Cancel
```

## Quick/full modes

```text
tabbeacon setup --quick
  only missing/stale/action-required items

tabbeacon setup --full
  revisit the complete flow with current values as defaults
```

If quick setup finds no work, exit immediately with a concise healthy summary.

## Safety

- no Hook trust bypass;
- no settings/config/hook write until final Apply;
- preserve existing ownership manifests and unrelated config;
- cancellation is lossless;
- non-TTY invocation must return actionable guidance, never hang.

## Validation

- deterministic prompt-flow tests using injected terminal/input backend rather than brittle real keystroke automation;
- fresh install / returning / quick healthy / quick stale / custom / cancel / setup-failure rollback scenarios;
- preset atomicity regression proving the v0.3 dogfood problem cannot recur;
- one final hosted CI;
- one bounded manual/automated Windows Terminal smoke for final interaction polish if needed.

## Exit gate

```text
GUIDED_SETUP=PASS
GUIDED_SETUP_FULLSCREEN=false
SCROLLBACK_PRESERVED=true
GUIDED_SETUP_ENUM_TYPING_REQUIRED=false
PRESET_SELECTION_ATOMIC=true
CUSTOM_OPTIONS_ONLY_AFTER_EXPLICIT_CUSTOMIZE=true
WRITE_BEFORE_APPLY=false
CANCEL_LOSSLESS=true
QUICK_SETUP=PASS
FULL_SETUP=PASS
HOOK_TRUST_BYPASS=false
NON_TTY_NO_HANG=true
CODE_CI=PASS
```

Estimated effort: **5–7 h**.
