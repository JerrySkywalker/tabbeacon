# Supported coding agents

TabBeacon normalizes provider evidence into a provider-neutral presentation
model. A provider is production-supported only to the extent its current
contract is admitted by evidence.
See the [English/Chinese terminology](terminology.md) for the distinct meanings
of capability, evidence, installed integration, Hook trust, and effective setting.

## Current support

| Coding agent | Status | Daily command | Compatibility policy |
| --- | --- | --- | --- |
| Codex CLI | Production | `codex` | Capability-based local admission; version text is diagnostic only. |
| Agy CLI | Production | `agy` | Exact admitted Agy 1.1.19 profile only. |
| Claude Code | Deferred | N/A | No production integration. |
| OpenCode | Deferred | N/A | No production integration. |

Deferred does not mean partially supported. It means no production provider is
enabled or implied.

Cursor Agent is being investigated for the v0.8.0 train. Its original command
is `agent` on the observed ZenBook Duo installation. [Current Cursor Hook documentation](https://cursor.com/docs/hooks)
names a structured `stop.status` outcome, but exact Windows Terminal color
output and original-command lifecycle have only one isolated product observation.
The Owner saw working/completed colors and no native-title interference; the
later session left matching release/end state, while an earlier start-only route
did not record an end. Per-Hook output results and mixed-session behavior remain
unproven. Cursor is not
included in the current production support table until those gates pass.
The candidate route guard now keeps bounded, process-safe generation history
and rejects superseded turns after the 32nd round. This deterministic guard
does not by itself prove a real Hook delivery or terminal binding; its bounded
history may conservatively refuse a new generation if its membership filter
reaches capacity.

## v0.8.0 candidate evidence boundary

The candidate's saved per-CLI preferences and deterministic event fixtures do
not change the production table above. Installed Hook delivery, Hook trust,
terminal binding, and effective visible output are separate observations.
`native` leaves a presentation channel to the CLI or terminal; `off` disables
TabBeacon output on that channel. Neither option suppresses a provider's own
title. See [Native](terminology.md#tb-t11), [Off](terminology.md#tb-t12), and
[Ownership](terminology.md#tb-t10).

| Candidate fact | Codex CLI | Cursor Agent |
| --- | --- | --- |
| Structured interruption | The exact 0.156.1/0.157.1 source-audited profile, isolated owned declaration and trust gate, public Hook CLI, and runtime dispatch have deterministic proof. Daily installed `Interrupt` delivery and visible output remain unproven. | No admitted live interruption route. |
| Structured warning or failed main turn | Unproven for the original HTTP 404 class in [Issue #116](https://github.com/JerrySkywalker/tabbeacon/issues/116). | Real structured `stop(completed)` delivery was observed by the earlier isolated collector; warning/failure outcomes remain unproven. |
| Strict native presentation | Owned-channel release has deterministic tests; final visible transition proof is pending. | One isolated original-command color run preserved the native title by Owner observation; continuous owned Visual and mixed-session transitions remain unproven. |

An event from a child tool or terminal text alone cannot establish a failed
main turn. The candidate keeps unknown events and unavailable evidence
unproven; it does not infer health from visible text, ANSI, or responses.

## Setup paths

Use the owned setup path once, then retain the provider's literal daily command:

```powershell
# Codex CLI: review Hook trust manually when the provider asks.
tabbeacon setup codex
codex

# Agy CLI: only the admitted 1.1.19 title-callback profile.
tabbeacon setup agy
agy
```

`tabbeacon setup` provides the guided combined flow. Setup does not create a
provider wrapper or grant Hook trust. Read [Codex Hooks](codex-hooks.md) and
[Agy setup](agy-setup.md) before changing an existing provider configuration.

## Capability matrix

`Supported` means the admitted provider contract supplies the fact. `Unavailable`
means the current contract does not provide it. `Not proven` is deliberately not
converted into a claim.

| Capability | Codex CLI | Agy CLI 1.1.19 |
| --- | --- | --- |
| Provider identity | Supported | Supported |
| Stable workspace identity | Supported | Supported when current/project roots agree |
| Working state | Supported | Supported (`initializing` / `working`) |
| Ready state | Supported | Supported (`idle`) |
| Result-ready | Supported | Unavailable |
| Approval or question | Supported when evidenced | Unavailable |
| Tab color | Supported | Unavailable |
| Windows Terminal progress | Supported | Unavailable |
| Activity animation | Supported | Unavailable |
| Session projection | Supported | Not proven |
| Integration diagnostics | Supported | Supported for the admitted profile |
| Compatibility policy | Capability-based | Exact admitted profile |

The Agy callback returns a plain title and does not infer approval, failure,
interruption, warning, health, stop authority, background-task count, or model
content from arbitrary output. See [Agy setup](agy-setup.md) and
[ADR 0015](adr/0015-agy-1-1-19-production-profile.md).

## Codex compatibility

Codex compatibility has four evidence states:

| State | Meaning |
| --- | --- |
| Full | Required Hook evidence and optional schema fingerprint succeeded. |
| Degraded | Required Hook evidence succeeded; optional schema evidence is unavailable. |
| Incompatible | A required capability is explicitly absent or disabled. |
| Unproven | Local discovery did not finish safely. |

Neither a newer version nor an older version number grants support. A safe
failure leaves literal `codex` usable and preserves unowned configuration. Read
[Codex compatibility](CODEX_COMPATIBILITY_V3.md) for the complete contract.

## Trust and configuration ownership

Provider compatibility is not configuration ownership. Codex Hook trust stays
manual, and setup refuses a TabBeacon-like declaration it cannot prove it owns.
Agy setup owns only its admitted title callback member and preserves unrelated
settings. An explicit Agy `preserve-native` preference removes that owned
callback; an unowned title declaration is never removed by TabBeacon. Neither
provider is wrapped, PATH-shadowed, or hosted in a PTY.

## Terminal presentation boundary

The production terminal backend is `TitleMarkBackend`. Native Windows Terminal
tab icons are [NO_GO](design/native-tab-icon.md) under accepted current-host
safety evidence. Provider metadata cannot enable a different backend.
