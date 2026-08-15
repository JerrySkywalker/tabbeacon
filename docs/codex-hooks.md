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

When upgrading to a binary at a new path, run `tabbeacon setup codex` from
that new binary. TabBeacon migrates exactly the eleven manifest-proven command
groups and updates its manifest atomically; it never adopts a lookalike hook,
an unsafe recorded executable path, or a different Codex configuration root.

## Presentation configuration

Presentation preferences are user-global, never repository-local:

```text
%LOCALAPPDATA%\TabBeacon\config.toml
```

The default is a comfortable v0.1 profile:

```toml
[presentation]
title = "tabbeacon"
tab_color = "tabbeacon"
activity = "title-indicator"
spinner = "codex"
theme = "muted-dark"
```

Use the compact commands below; values are closed typed choices, so settings
cannot inject a script, command, or raw terminal sequence.

```powershell
tabbeacon config show
tabbeacon config wizard
tabbeacon config set title tabbeacon
tabbeacon config set tab-color native
tabbeacon config set activity title-indicator
tabbeacon config set spinner braille
tabbeacon config set theme muted-dark
tabbeacon config preset balanced
tabbeacon config reset
```

Supported values are:

| Setting | Values |
| --- | --- |
| `title` | `tabbeacon`, `native`, `off` |
| `tab_color` | `tabbeacon`, `native`, `off` |
| `activity` | `title-spinner`, `title-indicator`, `wt-ring`, `both`, `native`, `off` |
| `spinner` | `codex`, `braille`, `quadrant`, `line`, `pulse` |
| `theme` | `muted-dark`, `classic` |

`title=tabbeacon` makes TabBeacon safely own the terminal title and disables
Codex's competing title setting through the existing ownership manifest.
`title=native` restores the prior supported Codex setting. `title=off` also
restores Codex native ownership: TabBeacon emits no title, but does not leave
both title producers disabled. Changes are atomic and process-safe; a malformed
settings file falls back to defaults for hook handling, while `config reset`
explicitly rewrites documented defaults.

`tab_color=native` and `tab_color=off` clear a TabBeacon-owned frame color and
then stop applying semantic dynamic colors. `activity=native` emits no
TabBeacon activity decoration; `activity=off` additionally clears any owned
progress state. `wt-ring` uses Windows Terminal OSC `9;4`; its foreground is
terminal-controlled and cannot be RGB-configured by TabBeacon.

`title-spinner` is accepted as a preference, but v0.1 intentionally uses one
deterministic static frame from the chosen preset rather than spawning a
long-lived animation worker from a one-shot Codex hook. The built-in `codex`
preset is the reduced bullet pair `•`/`◦`, inspired by Codex's textual activity
language without claiming to reproduce its shimmer. Other fixed frame sets are
braille, quadrant, line, and pulse. No arbitrary frame strings are accepted.

When `title=tabbeacon`, the default title grammar is status-first:

```text
<status-slot> <repository-alias>
```

For example: `○ OWH`, `⠋ OWH`, `✓ OWH`, `! OWH`, and `? OWH`. The
repository alias remains the stable offline identity. Only the left status slot
changes, and default titles do not append `working`, `result-ready`, `approval`,
`question`, `waiting`, or other lifecycle prose. `title=native` and
`title=off` retain their existing ownership behavior.

`muted-dark` is the intended v0.1 default for long multi-tab sessions. It uses
lower-saturation semantic fills: working `#1B4E3A`, result-ready `#1E3E88`,
approval/question `#776824`, warning `#81340E`, interrupted `#48395F`, and
failed `#5E1E35`. These values are deliberately separated beyond the visual
oracle's classification tolerance. `classic` retains the original G02 values
for compatibility.

## Preview

Preview does not mutate Codex hook trust or configuration. It cycles ready,
working, result-ready, approval, and reset through the current choices and
cleans up before exit:

```powershell
tabbeacon preview
tabbeacon preview --theme muted-dark
tabbeacon preview --spinner braille
```

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
the exact Hook profile, and manifest consistency. Declaration presence alone is not reported as
active.

The admitted production profile is `codex-hooks-rust-v0.147.0`, audited from
the official `rust-v0.147.0` source tag. It is turn-aware, thread-spawn
subagent-aware, and compact-aware. A newer version does not inherit that
profile merely because its version number is higher.

## State fidelity

The hook backend represents only evidence Codex emits directly:

| Codex hook | TabBeacon state |
| --- | --- |
| `SessionStart` startup/resume/clear | ready |
| `SessionStart` compact | preserve current state |
| `UserPromptSubmit` | working |
| `PreToolUse` / `PostToolUse` | reinforce working |
| `PermissionRequest` | approval required |
| `PreCompact` / `PostCompact` | preserve current state |
| `SubagentStart` / `SubagentStop` | ignore for root presentation |
| `Stop` | result ready |
| `SessionEnd` | reset |

Turn-scoped root events must match the current admitted `turn_id`.
`UserPromptSubmit` opens a new local generation and retires the prior turn;
stale stop/activity/prompt events cannot overwrite or revive it. Any applicable
event carrying thread-spawn subagent identity is isolated from root state.

The process-safe generation ledger stores only hashed session/turn identifiers,
a local generation, current turn, and a bounded retired-turn set. Prompt text,
assistant content, tool input/output, credentials, and arbitrary payload bodies
are neither persisted nor used for ordering.

`Stop` is a hook stop point, not an authoritative app-server completed
verdict. Tool failures, shell exit codes, missing events, and timeouts never
become authoritative failed/warning/interrupted states.

## Fail-open behavior

The installed Codex `0.147.0` release requires these hooks to be synchronous,
so TabBeacon uses the minimum one-second Codex timeout. The Windows command
neutralizes a missing or nonzero TabBeacon executable, and the internal hook
ingress is silent and always successful. Generation, repository, state, or terminal output
failure loses decoration only; it does not return a Codex block decision.

## Uninstall

```powershell
tabbeacon uninstall codex
```

Uninstall preflights every owned element before mutation. It removes only the
exact TabBeacon hook groups and restores only the prior title value. If an
owned declaration or title setting changed after setup, uninstall refuses and
leaves unrelated configuration untouched.
