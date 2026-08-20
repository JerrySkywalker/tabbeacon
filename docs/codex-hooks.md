# Codex Hooks Integration

TabBeacon integrates with Codex once, then daily launch remains:

```powershell
codex
```

There is no TabBeacon launcher wrapper.

## Setup

Build or install `tabbeacon`, then run the guided first-run or reconfiguration
flow:

```powershell
tabbeacon setup
```

The lightweight flow reads the current TabBeacon settings and Codex doctor
state without creating settings, configuration, Hook, or trust state. It shows
Windows Terminal availability, TabBeacon/Codex version and Hook profile,
existing Hook state, presentation choices, and a temporary renderer-backed
preview. Select a built-in preset or closed typed values, then choose `Apply`
or `Cancel`. `Cancel` leaves settings, Codex configuration, and Hook
declarations unchanged. `Apply` atomically saves the complete typed settings
draft and delegates to the existing ownership-aware setup implementation.

For scripts or provider-only setup, retain the direct command:

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

`tabbeacon setup` is the recommended cohesive flow; the compact commands stay
available for repeatable, scriptable changes.

Default Setup, Config, and Uninstall output is concise Human-facing prose.
Scripts that intentionally consume the legacy key/value receipts can request
`--plain`, for example `tabbeacon config show --plain`. Human output uses
restrained automatic color only on an interactive terminal; redirected output
remains monochrome.

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

When an originally absent Codex `config.toml` needs no title change,
`title=native` or `title=off` leaves it absent. Later repeat setup, title
reconciliation, doctor, and ownership-safe uninstall treat that absence as the
same empty supported configuration rather than creating or requiring a file.

`tab_color=native` and `tab_color=off` clear a TabBeacon-owned frame color and
then stop applying semantic dynamic colors. `activity=native` emits no
TabBeacon activity decoration; `activity=off` additionally clears any owned
progress state. `wt-ring` uses Windows Terminal OSC `9;4`; its foreground is
terminal-controlled and cannot be RGB-configured by TabBeacon.

`title-spinner` starts a direct ephemeral worker while reliable Hook evidence
proves active work. `both` combines the same title animation with Windows
Terminal's progress ring; `title-indicator` remains static. The worker is bound
to the originating terminal and turn generation, self-expires, and stops on
result, attention, or session-end evidence. Worker failure loses decoration
only and never blocks Codex. The built-in `codex` preset is the reduced bullet
pair `•`/`◦`; other fixed frame sets are braille, quadrant, line, and pulse. No
arbitrary frame strings are accepted.

Worker leases contain only hashed session/turn/terminal and executable-owner
identity, generation/order metadata, a safe workspace alias, semantic active
state, and the selected built-in spinner. Prompt, assistant, tool, credential,
and raw Hook payload content is neither passed to nor persisted by the worker.

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

Doctor reports declaration exactness, currentness, trust review or
trusted-hash drift, disabled Hooks, title ownership, executable presence,
version compatibility, the exact Hook profile, and manifest consistency. A
trusted-hash mismatch does not imply that the Hook declaration itself was
modified. Declaration presence alone is not reported as active.

## Upgrade preflight

Use the bounded local preflight before a package replacement when an active
worker may still have the installed executable mapped:

```powershell
tabbeacon upgrade-preflight
tabbeacon upgrade-preflight --json
```

The default command is read-only. It reports the current executable/version,
the installed Cargo target when present, whether a `TabBeacon` worker lock is
known to remain, and only content-minimal ownership classifications for
matching processes. It never opens the target for writing or prints arbitrary
process command lines, Hook payloads, or native session IDs.

`tabbeacon upgrade-preflight --drain` is an explicit local operation. Before a
stop, TabBeacon rechecks that the exact process uses the inspected executable,
has the internal activity-worker arguments, and matches a current valid local
lease. Unowned or ambiguous processes are preserved. It never targets Codex,
Windows Terminal, PowerShell, Cargo, or another `tabbeacon` executable path.
After a requested drain leaves no matching process, it performs the final
replacement-access probe without writing bytes to the target.

The admitted production profile is `codex-hooks-rust-v0.147.0`, audited from
the official `rust-v0.147.0` source tag. It is turn-aware, thread-spawn
subagent-aware, and compact-aware. A newer version does not inherit that
profile merely because its version number is higher.

## Operational diagnostics

Use `status` for a bounded read-only operational report, or either JSON mode
for machine consumption:

```powershell
tabbeacon status
tabbeacon status --json
tabbeacon doctor --json
```

Both JSON documents use stable `schema_version: 1`. `status --json` emits the
complete operational model: TabBeacon version and binary path, Codex version
and admitted profile, owned-integration/trust/title state, effective
presentation choices, lease-based activity counts, workspace identity health,
and the nested doctor verdict. `doctor --json` emits that same typed doctor
projection with its checks, aggregate verdict, and structured warnings and
failures. JSON modes write only the JSON document to stdout; no human prose is
mixed into it.

Stable enum spellings are lower snake case: doctor and declaration verdicts
are `pass`, `warning`, or `fail`; Hook trust is `active`, `review_required`,
`failed`, or `unavailable`; title ownership is `tabbeacon`, `native_or_off`,
`conflict`, or `unavailable`; settings source is `default`, `configured`,
`invalid`, or `unavailable`; worker health is `healthy`, `warning`, or
`unavailable`; and registry health is `absent`, `healthy`, `corrupt`, or
`unavailable`.

Activity counts are leases, not claims that an operating-system worker process
is alive. Registry reporting is health and count only. Neither human nor JSON
diagnostics expose prompt or assistant content, tool input/output, credentials,
headers, raw Hook payloads, raw session/turn identifiers, alias assignments or
canonical identities, unrelated Codex configuration, or an environment dump.
`doctor --json` preserves `doctor` exit behavior (failure is nonzero; warning
or pass succeeds). `status` is observational and succeeds after it emits a
valid report even if the nested doctor verdict is failing.

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

## Exact compatibility registry

TabBeacon classifies Codex versions through one offline typed registry. v0.3 admits
only `codex-hooks-rust-v0.147.0`; a numerically newer version never inherits that
support. Read-only `status` and `doctor` expose `supported`, `known_unadmitted`, or
`unknown_or_unavailable` alongside the admitted profile identifier when one exists.

`scripts/compare-codex-compatibility.ps1` compares two local source checkouts or two
Git references over the bounded Hook, identity, lifecycle, timeout, and title-ownership
surface. Its `SAFE_COMPATIBLE`, `REQUIRES_REVIEW`, and `BREAKING_OR_UNPROVEN` result
informs a later admission review; it does not update runtime profiles or access a network.

## Uninstall

```powershell
tabbeacon uninstall codex
```

Uninstall preflights every owned element before mutation. It removes only the
exact TabBeacon hook groups and restores only the prior title value. If an
owned declaration or title setting changed after setup, uninstall refuses and
leaves unrelated configuration untouched.
