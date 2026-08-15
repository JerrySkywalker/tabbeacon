# TabBeacon

TabBeacon is a terminal-native identity and live-status layer for coding-agent sessions.

The project is intentionally narrow: it keeps the user's existing terminal and agent CLI workflow, then makes each terminal tab act as a compact status beacon. The first supported product path is **Codex CLI in stock Windows Terminal**.

## Product invariants

- **Zero workflow change:** after one-time setup, daily use remains `codex`.
- **Fail open:** if TabBeacon is absent or broken, Codex must remain usable.
- **Offline-first identity:** repository naming must not require GitHub or network access.
- **Provider-neutral core:** Codex, Claude, OpenCode, and future agents are adapters beneath one evidence/state model.
- **Terminal-native presentation:** v0.1 targets Windows Terminal using terminal control sequences rather than replacing the PTY or terminal UI.
- **Machine-verifiable UI:** title, animation, and color behavior must eventually be covered by visual CI.

## Current status

The provider-neutral core, Windows Terminal presentation layer, deterministic
visual infrastructure, offline repository identity, and first Codex hooks
provider are implemented. See
[`dev_governance_files/ROADMAP.md`](dev_governance_files/ROADMAP.md) and the
[`Codex hooks integration guide`](docs/codex-hooks.md).

## v0.1 scope

v0.1 is intentionally limited to:

- Windows Terminal;
- Codex CLI;
- pure Rust product code;
- automatic repository abbreviation;
- tab title ownership;
- independently configurable title, activity, and dynamic tab-color channels;
- muted-dark and classic semantic palettes;
- static title activity indicators with a safe one-shot spinner fallback;
- global Codex integration with no change to the `codex` launch command;
- autonomous functional and visual verification.

Claude and OpenCode support are architectural extension points, not v0.1 release requirements.

## Non-goals for v0.1

TabBeacon is not a PTY host, session manager, worktree manager, agent orchestrator, prompt router, remote-control service, terminal replacement, or web dashboard.

## Development

The repository pins Rust 1.97.1. The local quality gate is:

```powershell
pwsh -NoProfile -File ./scripts/ci/run-local-ci.ps1
```

For daily presentation choices, use `tabbeacon config show`, the compact
`tabbeacon config wizard`, or the documented presets in the
[`Codex hooks integration guide`](docs/codex-hooks.md). Settings are user-global
under `%LOCALAPPDATA%\TabBeacon`, never in a repository.

All production changes after the bootstrap commit follow feature-branch, pull-request, exact-head CI, and evidence rules defined in [`AGENTS.md`](AGENTS.md) and [`dev_governance_files/QUALITY_GATES.md`](dev_governance_files/QUALITY_GATES.md).

## License

MIT.
