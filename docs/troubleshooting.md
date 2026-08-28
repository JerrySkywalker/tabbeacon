# Troubleshooting

Start with observation, not mutation. TabBeacon is fail-open for coding-agent
use and preserves configuration or processes when ownership is not proven.

## First checks

```powershell
tabbeacon status --json
tabbeacon doctor --json
tabbeacon hooks --json
tabbeacon sessions --json
tabbeacon upgrade-preflight --json
```

These commands make bounded observations. They do not grant Hook trust or
terminate a provider.

## Hook trust needs review

Codex Hook trust is deliberately manual. Use the supported setup flow, inspect
the proposed exact declarations, and complete the provider's normal review.
Do not bypass trust, edit `hooks.json` blindly, or copy a declaration from an
unrelated machine.

## Hook timeout or slow PowerShell startup

First collect evidence with `tabbeacon doctor --json` and inspect the affected
Hook declaration. A shell startup delay can be environmental; increasing a
timeout without evidence can hide the cause and expand the failure window.
Use the exact supported configuration and report the bounded receipt if the
problem remains.

## Upgrade preflight is blocked

Run the read-only preflight first:

```powershell
tabbeacon upgrade-preflight --json
```

`REPLACEABILITY=blocked_by_owned_tabbeacon_mcp` means a live process was proven
to be a TabBeacon-owned MCP child by its ephemeral lease, PID creation time,
canonical executable path/hash, and content hash. Let relevant sessions exit
naturally when possible. Only after a fresh ownership-qualified preflight may
you consider the explicit owned-only drain:

```powershell
tabbeacon upgrade-preflight --drain
```

`--drain` is not a general process cleanup command. It never authorizes
terminating Codex, Windows Terminal, PowerShell, Cargo, or ambiguous processes.

## Ambiguous processes are preserved

An ambiguous process is intentionally preserved because ownership is not
proven. The safe normal path is:

```text
read-only diagnosis
→ allow relevant sessions to exit naturally
→ ownership-qualified preflight
→ exact owned drain only when proven
```

Do not use `taskkill` by process name, kill all Codex sessions, terminate a
process tree, or use an image name/PID alone as proof.

## Codex compatibility says degraded, incompatible, or unproven

`Full` and `Degraded` are distinct healthy evidence states; degraded means an
optional schema observation was unavailable. `Incompatible` or `Unproven`
prevents affected mutation rather than guessing. Check the bounded diagnostic,
keep using literal `codex`, and review [Codex compatibility](CODEX_COMPATIBILITY_V3.md).

## A title is missing or falls back to PowerShell

Confirm that the session is Windows Terminal and run `tabbeacon doctor --json`.
Windows Terminal title policy, active profile, or settings-source ambiguity can
limit title output. Diagnose first; do not hand-edit Windows Terminal settings
or use an XAML/native-icon workaround. The supported production path remains
the title-based backend.

## Workspace identity looks unexpected

Run:

```powershell
tabbeacon alias show
tabbeacon alias preview
tabbeacon alias explain
```

Git identity is a stable specialization, not a requirement. Set an explicit
device-local alias only after reviewing the suggestions and scope.

## Agy version or profile mismatch

Agy production support is limited to the admitted **1.1.19** structured-title
profile. Use `tabbeacon agy version` for read-only comparison. Do not force a
different Agy version into the callback or treat a version string as a broad
compatibility range.

## Native icon question

Native Windows Terminal tab icons are [NO_GO](design/native-tab-icon.md) under
the accepted current-host evidence. Do not attempt XAML Diagnostics, process
instrumentation, or an icon workaround from a troubleshooting session.
