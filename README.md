# TabBeacon

TabBeacon is a terminal-native identity and live-status layer for coding-agent sessions.

The project is intentionally narrow: it keeps the user's existing terminal and agent CLI workflow, then makes each terminal tab act as a compact status beacon. The first supported product path is **Codex CLI in stock Windows Terminal**.

## Product invariants

- **Zero workflow change:** after one-time setup, daily use remains `codex`.
- **Fail open:** if TabBeacon is absent or broken, Codex must remain usable.
- **Offline-first identity:** repository naming must not require GitHub or network access.
- **Provider-neutral core:** Codex, Claude, OpenCode, and future agents are adapters beneath one evidence/state model.
- **Terminal-native presentation:** v0.2 targets Windows Terminal using terminal control sequences rather than replacing the PTY or terminal UI.
- **Machine-verifiable UI:** title, animation, and color behavior must eventually be covered by visual CI.

## Current status

The provider-neutral core, Windows Terminal presentation layer, deterministic
visual infrastructure, Git and non-Git workspace identity, first Codex hooks
provider, session-scoped ephemeral title animator, guided setup flow, and
read-only operational diagnostics are implemented. See
[`dev_governance_files/ROADMAP.md`](dev_governance_files/ROADMAP.md) and the
[`Codex hooks integration guide`](docs/codex-hooks.md).

The default TabBeacon-owned tab title uses a compact status-first grammar:

```text
○ OWH
⠋ OWH
✓ OWH
! OWH
? OWH
```

The mutable status slot stays on the left and the stable offline repository
alias stays on the right. Default titles do not append lifecycle prose.

## Current product scope

The Codex-first product is intentionally limited to:

- Windows Terminal;
- Codex CLI;
- pure Rust product code;
- automatic repository abbreviation;
- tab title ownership;
- independently configurable title, activity, and dynamic tab-color channels;
- muted-dark and classic semantic palettes;
- static title indicators and a fail-open ephemeral title animator;
- global Codex integration with no change to the `codex` launch command;
- autonomous functional and visual verification.

Claude and OpenCode support are architectural extension points, not part of the v0.2 Codex-first release.

## Installation

TabBeacon has two first-class installation channels:

- **Windows binary users:** download the prebuilt Windows x64 ZIP from
  [GitHub Releases](https://github.com/JerrySkywalker/tabbeacon/releases).
- **Rust/Cargo users:** install the public CLI from crates.io:

  ```powershell
  cargo install tabbeacon --locked
  ```

  To install this release exactly:

  ```powershell
  cargo install tabbeacon --version 0.2.0 --locked
  ```

The default Cargo install surface is the public `tabbeacon` command only.
After installing, start the guided first-run or reconfiguration flow with:

```powershell
tabbeacon setup
```

It previews typed presentation choices before Apply, then reuses the existing
ownership-safe Codex setup path. `tabbeacon setup codex` remains available for
scripted provider-only setup. Complete Codex `/hooks` trust review only when
prompted. Daily agent use remains literally `codex`, not `tabbeacon codex`.

### Operational diagnostics

Use the read-only status commands to inspect the current installation without
scraping the human `doctor` output:

```powershell
tabbeacon status
tabbeacon status --json
tabbeacon doctor
tabbeacon doctor --json
```

The version-1 JSON schemas contain only bounded status, configuration choices,
safe counts, and the TabBeacon binary path. They never emit prompt or assistant
content, Hook payloads, credentials, raw session/turn identifiers, alias
registry identities, or an environment dump. `doctor --json` writes JSON only
to stdout and keeps the normal doctor exit contract: a failure is nonzero while
warning and pass are successful. `status --json` remains observational and
successful even when its nested doctor verdict is a failure.

### Rust library target

The published package includes the `tabbeacon` library target used internally
by the CLI and its tests. TabBeacon v0.2.x is CLI-first and does not promise a
mature public Rust library API beyond normal SemVer expectations.

## Non-goals for v0.2

TabBeacon is not a PTY host, session manager, worktree manager, agent orchestrator, prompt router, remote-control service, terminal replacement, or web dashboard.

## Development

The repository pins Rust 1.97.1. The local quality gate is:

```powershell
pwsh -NoProfile -File ./scripts/ci/run-local-ci.ps1
```

For daily presentation choices, use `tabbeacon setup`, `tabbeacon config show`,
the compact `tabbeacon config wizard`, or the documented presets in the
[`Codex hooks integration guide`](docs/codex-hooks.md). Settings are user-global
under `%LOCALAPPDATA%\TabBeacon`, never in a repository.

All production changes after the bootstrap commit follow feature-branch, pull-request, exact-head CI, and evidence rules defined in [`AGENTS.md`](AGENTS.md) and [`dev_governance_files/QUALITY_GATES.md`](dev_governance_files/QUALITY_GATES.md).

## License

MIT.
