# TB-G07 — Usable v0.1 presentation and autonomous E2E hardening

## Scope

Starting main: `2360e4f14051db0061cc1efe105df62e6c95d59e` after merged G05.

G07 adds only the provider-neutral presentation preference layer, configuration
CLI/wizard, muted-dark palette, safe Codex title-ownership reconciliation,
preview, and corresponding hook/visual evidence. The normalizer, reconciler,
repository identity, provider boundary, and daily `codex` launch remain
unchanged.

## User-visible contract

- Preferences live at `%LOCALAPPDATA%\TabBeacon\config.toml`, not in a
  repository.
- Every preference is a closed typed choice. Missing or malformed config must
  leave hook ingress fail-open and use safe defaults.
- Writes are atomic and serialize concurrent writers with a per-user lock.
- Title, tab color, and activity are independently controlled.
- `muted-dark` is the default; `classic` remains compatible.
- `tabbeacon preview` uses current settings but never changes Codex trust.
- `title-spinner` is `FALLBACK_ACCEPTED` for v0.1: one-shot hooks have no
  proven safe per-tab child-worker binding, so the chosen preset emits one
  deterministic static title frame instead of a continuous worker.

## Required evidence

- focused settings, presentation, Codex ownership, and visual-oracle tests;
- exact final local repository gate;
- exact-head hosted CI;
- one trusted visual run bound to the final candidate that observes title,
  semantic colors, activity fallback/progress as configured, and reset;
- real owner Codex smoke for normal `codex` use without a launcher wrapper.

## Non-goals

- G06X/app-server work;
- Claude or OpenCode providers;
- arbitrary executable configuration, arbitrary spinner frames, a global
  daemon/session manager, or a full-screen terminal UI;
- broad personal Codex hook cleanup.
