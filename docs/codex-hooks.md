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

Codex 0.149 uses a hybrid owned transport: one named, session-scoped stdio MCP
server, ten content-minimal normal-event `mcp_tool` lifecycle declarations,
and exactly one TabBeacon-owned `SessionEnd` command declaration. Its tool
is explicitly omitted from Codex's direct, deferred, and Code Mode model-facing
surfaces. The owned server declares exactly `env_vars = ["WT_SESSION"]`.
Codex clears a local stdio child's inherited environment, so this one
allow-listed value is required for Windows Terminal activity binding; TabBeacon
does not inherit the parent environment or forward `WT_PROFILE_ID`. Ordinary
events are calls over the already-connected MCP channel, so they do not start
Pwsh7, Windows PowerShell, or cmd. The templates carry only event, session,
turn, CWD, source, and explicit subagent identity fields—never prompt,
assistant, or tool content. Codex does not admit a `SessionEnd` MCP Hook;
therefore its synchronous, fail-open command Hook has `timeout = 1` and
`async = false`. That command is the authoritative cleanup boundary. MCP EOF
can release only the server's in-memory binding as a best-effort fallback and
retains bounded stale-state recovery; it is never real SessionEnd proof.

The source-audited 0.147 profile retains the command fallback. For a shell-safe,
whitespace-free native `.exe` path it emits a compact direct `commandWindows`
invocation; hostile paths retain the encoded PowerShell compatibility envelope.
That fallback remains synchronous with its one-second timeout and is silent and
fail-open after ingress starts.

Static `tabbeacon doctor` deliberately reports the Hook runtime as unproven:
matching declarations and trust state do not prove that Codex can execute the
transport. For 0.149 it separately reports `mcp.terminal-binding`; the owned
MCP declaration must forward exactly `WT_SESSION` before presentation can be
healthy. `tabbeacon doctor --probe-hook-runtime` runs one manifest-exact
representative declaration in isolated temporary `LOCALAPPDATA`. Its 0.149
path mirrors the cleared Codex local-stdio environment, adds only the owned
terminal binding, then separately proves MCP initialize, Working activity, two
distinct spinner writes at the worker cadence, and Stop supersession without a
command shell. It next models Codex 0.149 terminating the MCP child before
transport EOF and independently invokes the exact SessionEnd command
declaration. That second proof requires generation and root-anchor retirement,
activity-lease revocation, Windows Terminal progress clear, and indexed
`OSC 104;264` frame-color reset. The report exposes
`MCP_EVENT_TRANSPORT_PROVEN` and `REAL_SESSION_END_CLEANUP_PROVEN` separately;
an artificial EOF can establish only `EOF_CLEANUP_CAPABLE`. The activity proof
has a ten-second diagnostic bound; the terminate-before-EOF phase is bounded
to one second and the isolated cleanup diagnostic to five seconds. The normal
Hook timeout and the SessionEnd command timeout remain one second. For 0.147
it uses the 900 ms COMSPEC command fallback.
The probe records no prompt, tool, assistant, title, or environment content
and never modifies Codex configuration or Hook trust.

For an exact-head 0.149 transport measurement, use
`scripts/measure-codex-mcp-hybrid-runtime.ps1`. It launches normal events
directly through the owned stdio server (zero shell processes), measures an
eight-session warm concurrency distribution, and separately measures the one
SessionEnd command after the terminate-before-EOF model in isolated state.
An Owner upgrading the original 0.149 MCP integration preserves the exact ten
MCP groups and their existing trust hashes. Only the newly introduced
SessionEnd command needs review in `/hooks`; TabBeacon never changes Hook trust
itself.

## Package upgrades with a live MCP child

On Windows, the session-scoped MCP child intentionally executes the installed
`tabbeacon.exe`, so it can temporarily hold that package file open. Before an
official-channel replacement, inspect it without changing anything:

```powershell
tabbeacon upgrade-preflight --plain
```

When the report says `REPLACEABILITY=blocked_by_owned_tabbeacon_mcp`, the
process is drainable only if an ephemeral local lease, PID creation time,
canonical executable path hash, and executable content hash all match the
manifest-proven TabBeacon MCP runtime. Image name, command text, or a Codex
parent name alone never grant drain authority. The report uses
`stale_or_invalid_lease` and `process_identity_mismatch` for safe refusals;
those processes remain untouched.

After reviewing the report, drain only the exact currently proven MCP children:

```powershell
tabbeacon upgrade-preflight --drain --plain
```

The drain rechecks each process immediately before terminating its exact handle.
It never kills Codex, a process tree, an arbitrary `tabbeacon.exe`, or a
third-party MCP server. Normal Hook events keep their session-scoped MCP hot
path; this ownership proof adds no recurring process query to lifecycle events.

For a release closeout, verify the installed Cargo source after the explicit
official installation command. The release helper takes a deliberately selected
Cargo home, exact package, and exact version; it emits no credentials or raw
metadata:

```powershell
scripts/verify-cargo-install-source.ps1 -CargoHome <isolated-or-owner-approved-cargo-home> -Package tabbeacon -Version <version>
```

An official crates.io cutover requires `OWNER_INSTALL_SOURCE=REGISTRY_OFFICIAL`
and `OWNER_GIT_REV_INSTALL=false`. Binary version, path, and existence are not
source proof. Run `tabbeacon setup codex` only as ownership-aware reconciliation
after the official cutover; it never changes Hook trust automatically.

## Orphaned owned-Hook repair

An interrupted Codex upgrade can leave a valid TabBeacon ownership manifest and
title baseline behind while its exact owned Hook groups are absent. Inspect
that narrow state first:

```powershell
tabbeacon repair codex
```

The default is a read-only preview. It is eligible to restore only groups that
are all of the following: manifest-owned, exact to the current source-audited
profile, and proven entirely absent from a safe known Hook wire shape. It also
requires an exact manifest target and title-ownership baseline. A retained
non-owned group may be from the verified pre-install backup or may have been
added later by a third party: it is preserved when it has the admitted Hook
envelope and affirmative external provenance: a non-TabBeacon plugin identity
or an MCP server/tool identity. An arbitrary post-install command does not
prove third-party ownership and is a baseline-drift hard stop. TabBeacon-like
or ambiguous groups, malformed/unknown Hook envelopes, and a tampered
pre-install backup remain hard stops. No external Hook group, MCP group,
configuration setting, backup, manifest, or trust state is changed by the
preview.

The preview emits `TARGET_DIGEST`. After reviewing the result, apply that exact
target digest under the ownership lock:

```powershell
tabbeacon repair codex --apply --expected-target-digest <TARGET_DIGEST>
```

Apply repeats the ownership preflight and refuses a target digest that changed
since preview. It appends only the missing exact owned groups to `hooks.json`.
It never adopts, deletes, rewrites, trusts, disables, or reorders external
groups; it inserts only new owned JSON fragments, preserving existing
third-party command and MCP group bytes where the valid known envelope is
representable. The manifest and original terminal-title restoration baseline
are also untouched. Plain and JSON repair diagnostics distinguish
`POSTINSTALL_THIRD_PARTY_PRESERVED`, `TABBEACON_LIKE_AMBIGUITY_BLOCKED`,
`BASELINE_DRIFT_BLOCKED`, and `CONCURRENT_DRIFT_REFUSAL`. Every successful
repair still requires the Owner to launch `codex`, review the definitions in
`/hooks`, and then run `tabbeacon doctor`; TabBeacon never auto-trusts Hooks.

When upgrading to a binary at a new path, run `tabbeacon setup codex` from
that new binary. TabBeacon migrates the exact manifest-proven transport (eleven
command groups for 0.147, or ten MCP groups plus one SessionEnd command and its
owned MCP server for 0.149)
and updates its manifest atomically; it never adopts a lookalike hook,
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
0.149 report additionally distinguishes a missing or broadened owned
terminal-binding allow-list from a healthy presentation configuration.
trusted-hash mismatch does not imply that the Hook declaration itself was
modified. Declaration presence alone is not reported as active.

## Hook Inspector

Inspect the same state per Hook without mutating Codex configuration or trust:

```powershell
tabbeacon hooks
tabbeacon hooks --json
tabbeacon hooks --plain
```

The inspector and the `Hooks / 钩子` Control Center screen expose a typed,
provider-neutral inventory: provider, event, proven owner, enabled state,
manual trust state, declaration currentness, source/handler class, timeout,
and a declaration fingerprint. Human output is localized; JSON and plain keys
remain stable lower-snake-case values. Every output marks command visibility
as `redacted`: it never prints arbitrary Hook command strings, raw config
state keys, or provider configuration paths.

For owned declarations it distinguishes `review_required`,
`hash_stale_or_changed`, `disabled`, and
`declaration_modified_or_missing`. A stale trusted hash means the declaration
can still be exact; it does not claim the declaration changed. Third-party,
unowned, malformed, and unsupported shapes remain read-only and are never
auto-trusted, edited, disabled, or adopted.

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

The admitted production profiles are `codex-hooks-rust-v0.147.0` and
`codex-hooks-rust-v0.149.0`, audited from the matching official source tags.
Both are turn-aware, thread-spawn subagent-aware, and compact-aware. The 0.149
audit uses `mcp_tool` through TabBeacon's own named session stdio server; the
server is hidden from the model and has no machine-global daemon lifetime.
TabBeacon reconciles only its exact owned server and Hook groups, and preserves
external MCP servers and groups unchanged. A newer version never inherits either profile merely
because its version number is higher.

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

`status --json` and `doctor --json` also expose stable
`mutation_authority` (`admitted` or `blocked`) and `runtime_continuity`
(`admitted`, `preserved_unadmitted`, or `unproven`). An unadmitted version is
always blocked from setup, repair, rewrite, and title reconciliation. Its
already-installed runtime may nevertheless remain
`preserved_unadmitted` only when the ownership manifest, exact declarations,
trusted hashes, known parseable Hook wire shape, managed executable, and title
ownership are independently exact. This is a runtime-continuity warning, not a
new profile admission.

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

The admitted transports are synchronous. The 0.149 MCP handler declares the
source-audited one-second timeout and has no `async` field; normal lifecycle
events remain on that connected MCP channel with no shell process. Its sole
SessionEnd command Hook is `async=false` with a one-second timeout and runs one
fail-open command process per session. The 0.147 command fallback remains
`async=false` with a one-second timeout. Neither transport returns a Codex
block decision for TabBeacon failures; decoration may be lost, but Codex
continues.

## Exact compatibility registry

TabBeacon classifies Codex versions through one offline typed registry. The
registry currently admits `codex-hooks-rust-v0.147.0` and
`codex-hooks-rust-v0.149.0`; a numerically newer version never inherits that
support. Read-only `status` and `doctor` expose `supported`, `experimental`,
`unknown`, or `unsupported` alongside the admitted profile identifier when one
exists. `supported` reports `Codex version is source-audited`; an `unknown`
detected version reports its detected version, `Registry: unknown`, `Hook
profile: unclassified`, and `Risk: manual review required`.

Unknown, experimental, and unsupported versions are fail-closed for `setup`
and title reconciliation: no hooks, trust state, title setting, manifest, or
backup is created or changed. They are also fail-closed for `repair --apply`.
Safe uninstall remains available only for exact manifest-owned declarations, so
an owner can remove a previously installed integration without adopting an
unproven version or Hook shape.

`scripts/compare-codex-compatibility.ps1` compares two local source checkouts or two
Git references over the bounded Hook, identity, lifecycle, timeout, and title-ownership
surface. Its machine-readable `tabbeacon-codex-hook-delta-v1` receipt reports
the source mode, relevant-file count, bounded protocol-signal count, and
`NONE_RELEVANT`, `REQUIRES_SOURCE_REVIEW`, or `BREAKING_OR_UNPROVEN` protocol
delta. The receipt intentionally says `EXACT_PRODUCTION_ADMISSION=NOT_GRANTED`:
it narrows a future source/protocol audit, but never updates a runtime profile
or infers compatibility from a version range.

## Uninstall

```powershell
tabbeacon uninstall codex
```

Uninstall preflights every owned element before mutation. It removes only the
exact TabBeacon hook groups and restores only the prior title value. If an
owned declaration or title setting changed after setup, uninstall refuses and
leaves unrelated configuration untouched.
