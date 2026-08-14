# Codex upstream baseline

## Frozen G05 baseline

```text
FROZEN_PR_HEAD=11f0876c62b29208ba0b0243678ff4f65ae6cfc4
FROZEN_CODEX_VERSION=0.147.0
FROZEN_CODEX_RELEASE_SOURCE=be6e8eac029b183056b7e4402879f15d2c85f61b
FROZEN_UPSTREAM_OBSERVATION=4eff3b788ba629acc944ed6db6502c362fc08e0a
```

## Re-baseline observation

Observed 2026-08-14 without changing the installed Codex package:

```text
INSTALLED_CODEX=0.147.0
NPM_LATEST_STABLE=0.147.0
LATEST_STABLE_RELEASE=rust-v0.147.0
LATEST_STABLE_RELEASE_SOURCE=be6e8eac029b183056b7e4402879f15d2c85f61b
LATEST_STABLE_PUBLISHED=2026-08-07T01:41:49Z
UPSTREAM_MAIN=23094236acac6fdc22f67a408ea8ccb8fac8e6e1
UPSTREAM_MAIN_OBSERVED=2026-08-14T15:24:10Z
```

The installed version and latest stable release equal the G05 frozen version.
Upstream `main` has advanced but has not produced a newer stable package.

## Re-baselined behavior

- Codex 0.147.0 exposes 11 hook events: `PreToolUse`, `PermissionRequest`,
  `PostToolUse`, `PreCompact`, `PostCompact`, `SessionStart`, `SessionEnd`,
  `UserPromptSubmit`, `SubagentStart`, `SubagentStop`, and `Stop`.
- User-global hooks are discovered from `CODEX_HOME/hooks.json`. Every matching
  source is considered; a project hook does not silently replace a user hook.
- Windows command hooks select `commandWindows` and execute it through
  `cmd.exe /C`. TabBeacon declares synchronous (`async=false`) one-second hooks.
  It does not depend on ordering among multiple matching handlers.
- Non-managed hooks require review. Codex stores per-handler enablement and the
  trusted normalized hash beneath `hooks.state` in `config.toml`; `/hooks` and
  startup review are the normal interactive review paths.
- `[tui] terminal_title = []` remains the supported way to stop Codex from
  owning the title. Stable and observed upstream `main` use the same behavior.

## Drift classification

| Surface | Stable 0.147.0 versus observed main | Class | Effect on G05 |
| --- | --- | --- | --- |
| Hook event set used by TabBeacon | unchanged | NO_EFFECT | Seven declared events remain valid. |
| User hook discovery and trust identity | unchanged | NO_EFFECT | Existing trust calculations remain valid. |
| Command hook execution | internal runtime refactoring; synchronous command contract retained | COMPATIBLE | One-second fail-open declaration remains valid. |
| Additional MCP-oriented hook types | unreleased source addition | NO_EFFECT | TabBeacon emits command hooks only. |
| Terminal title configuration | unchanged | NO_EFFECT | Empty selection remains supported. |

No upstream change justified changing the frozen hook declarations. The one
release issue found by this lab was local doctor handling of Codex's existing
`enabled=false` state, not upstream drift.

## Sources

- Current official hooks documentation: <https://learn.chatgpt.com/docs/hooks>
- Stable release: <https://github.com/openai/codex/releases/tag/rust-v0.147.0>
- Observed upstream main: <https://github.com/openai/codex/commit/23094236acac6fdc22f67a408ea8ccb8fac8e6e1>
