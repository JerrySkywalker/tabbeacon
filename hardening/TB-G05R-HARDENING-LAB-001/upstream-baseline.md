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
Upstream `main` has advanced and requires source-level classification without
assuming that unreleased refactoring is a stable compatibility break.

## Sources

- Current official hooks documentation: <https://learn.chatgpt.com/docs/hooks>
- Stable release: <https://github.com/openai/codex/releases/tag/rust-v0.147.0>
- Observed upstream main: <https://github.com/openai/codex/commit/23094236acac6fdc22f67a408ea8ccb8fac8e6e1>

Detailed schema, discovery, trust, command execution, title configuration, and
version classifications are added after the source and binary probes complete.
