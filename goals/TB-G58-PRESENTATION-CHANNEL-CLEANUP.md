# TB-G58 — Presentation Channel Cleanup

## Status

PLANNED after accepted G57.

## Purpose

Remove redundant dual activity from normal Human presets without breaking existing machine configuration or explicit user choices.

## Current behavior

`ActivityMode::Both` intentionally drives both title animation and the Windows Terminal progress ring. The current `full` preset selects `Both`, which produces two simultaneous animated activity indicators.

## Frozen behavior

Existing machine token compatibility remains:

```text
activity = "both"
```

Existing explicit `both` user settings must remain readable and unchanged during normal reads/upgrades.

The Human product surface changes instead:

- ordinary Recommended: title spinner + semantic tab color;
- Minimal: static title indicator;
- Terminal Ring: WT ring as the primary activity channel;
- Native;
- Customize;
- dual activity remains advanced/explicit, not the desirable ordinary "maximum" preset.

The exact Human label replacing or redefining `Full` may be chosen during implementation, but a fresh/default guided setup must not enable two simultaneous activity animations merely because the user selected the most feature-rich ordinary preset.

## Preset migration policy

No silent rewrite of persisted config.

If an existing config explicitly contains `both`, retain it until the user applies another presentation choice. If a symbolic preset record exists, preserve semantics according to existing storage contracts; do not rewrite project/user files simply on startup.

## TUI / Setup

Appearance and Guided Setup should make activity channels understandable:

```text
Title spinner
Static title indicator
Terminal ring
Dual indicators (Advanced)
Native
Off
```

Spinner preset controls matter only when title spinner activity is selected. The UI may de-emphasize/disable irrelevant spinner controls but must not corrupt stored values.

## Testing

- fresh Recommended/default has one primary animated activity channel;
- revised feature-rich Human preset has one primary animated activity channel;
- explicit legacy `both` round-trips unchanged;
- direct config machine token remains stable;
- import/export preserves explicit `both`;
- TUI/Setup explain dual indicators as advanced/explicit;
- real-WT representative smoke confirms no duplicate normal-preset activity;
- reduced-motion behavior remains semantically correct.

## Risk vector

```text
CODE_CHANGED=true
PRESENTATION_CHANGED=true
PROVIDER_CHANGED=false
USER_PERSISTENT_CONFIG_CHANGED=true  # only explicit user Apply paths
SECURITY_OR_PRIVACY_CHANGED=false
RELEASE_BOUNDARY=false
```

## Acceptance

```text
NORMAL_PRESET_DUAL_ACTIVITY=false
LEGACY_ACTIVITY_BOTH_SUPPORTED=true
LEGACY_ACTIVITY_BOTH_AUTO_REWRITTEN=false
HUMAN_ACTIVITY_CHOICES_CLEAR=true
IMPORT_EXPORT_COMPATIBLE=true
WINDOWS_TERMINAL_SMOKE=PASS
CODE_CI=PASS
```

## Estimated effort

**4–6 effective engineering hours.**

## Next

`TB-G59 — Root Workspace Anchor & Subagent Observability`.