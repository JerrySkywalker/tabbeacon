# Supported coding agents

TabBeacon normalizes provider evidence into a provider-neutral presentation
model. A provider is production-supported only to the extent its current
contract is admitted by evidence.

## Current support

| Coding agent | Status | Daily command | Compatibility policy |
| --- | --- | --- | --- |
| Codex CLI | Production | `codex` | Capability-based local admission; version text is diagnostic only. |
| Agy CLI | Production | `agy` | Exact admitted Agy 1.1.19 profile only. |
| Claude Code | Deferred | N/A | No production integration. |
| OpenCode | Deferred | N/A | No production integration. |

Deferred does not mean partially supported. It means no production provider is
enabled or implied.

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
settings. Neither provider is wrapped, PATH-shadowed, or hosted in a PTY.

## Terminal presentation boundary

The production terminal backend is `TitleMarkBackend`. Native Windows Terminal
tab icons are [NO_GO](design/native-tab-icon.md) under accepted current-host
safety evidence. Provider metadata cannot enable a different backend.
