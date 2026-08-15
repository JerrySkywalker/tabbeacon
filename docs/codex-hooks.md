# Codex Hooks Integration

TabBeacon integrates with Codex once, then daily launch remains:

```powershell
codex
```

There is no TabBeacon launcher wrapper.

## Setup

Build or install `tabbeacon`, then run:

```powershell
tabbeacon setup codex
```

Setup adds exact TabBeacon command groups to the supported user-global
`~/.codex/hooks.json` layer and sets this supported Codex title option:

```toml
[tui]
terminal_title = []
```

Unrelated hooks and TOML settings remain in place. Before either external file
changes, exact backups and an ownership manifest are written below:

```text
%LOCALAPPDATA%\TabBeacon\codex-integration
```

Setup is idempotent. It refuses a TabBeacon-like declaration that it cannot
prove it owns.

## Required trust review

Codex does not run a new or changed unmanaged hook until its normalized
definition hash is trusted. TabBeacon deliberately does not bypass or write
that trust decision.

After setup:

1. launch `codex` normally;
2. open `/hooks`;
3. inspect the TabBeacon definitions and trust them if they match the setup
   output;
4. run `tabbeacon doctor`.

Doctor reports declaration presence, modified owned hooks, trusted versus
inactive hooks, title ownership, executable presence, version compatibility,
and manifest consistency. Declaration presence alone is not reported as
active.

## State fidelity

The hook backend represents only evidence Codex emits directly:

| Codex hook | TabBeacon state |
| --- | --- |
| `SessionStart` startup/resume/clear | ready |
| `SessionStart` compact | preserve current state |
| `UserPromptSubmit` | working |
| `PreToolUse` / `PostToolUse` | reinforce working |
| `PermissionRequest` | approval required |
| `Stop` | result ready |
| `SessionEnd` | reset |

`Stop` is a hook stop point, not an authoritative app-server completed
verdict. Tool failures, shell exit codes, missing events, and timeouts never
become authoritative failed/warning/interrupted states.

## Fail-open behavior

The installed Codex `0.147.0` release requires these hooks to be synchronous,
so TabBeacon uses the minimum one-second Codex timeout. The Windows command
neutralizes a missing or nonzero TabBeacon executable, and the internal hook
ingress is silent and always successful. Repository, state, or terminal output
failure loses decoration only; it does not return a Codex block decision.

## Uninstall

```powershell
tabbeacon uninstall codex
```

Uninstall preflights every owned element before mutation. It removes only the
exact TabBeacon hook groups and restores only the prior title value. If an
owned declaration or title setting changed after setup, uninstall refuses and
leaves unrelated configuration untouched.
