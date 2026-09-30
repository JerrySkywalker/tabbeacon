# Agy setup

TabBeacon supports exactly Agy 1.1.19 and 1.2.7 on Windows through Agy's
user-global structured title callback. For 1.2.7, public support is bounded to the
exact callback contract and proved real Working/default-color/noninterference;
visible Ready and attended native switching are outside the v0.8.0 promise.
Install the provider integration once:

```powershell
tabbeacon setup agy
```

Daily launch remains the native command:

```powershell
agy
```

Setup owns only the `title` member in
`~/.gemini/antigravity-cli/settings.json`. It enables the supported callback,
preserves unrelated settings, refuses a foreign title owner or ownership
drift, and records an exact pre-install backup. Remove only TabBeacon's owned
declaration with:

```powershell
tabbeacon uninstall agy
```

When unrelated settings are unchanged, uninstall restores the original bytes.
When Agy has legitimately changed unrelated settings, uninstall preserves
those settings and removes only the exact owned title member. Reparse points,
unsupported versions, malformed ownership state, and unsafe executable paths
are refused.

## Admitted capabilities

Agy's admitted title profiles provide a stable conversation identity, equal current/project
workspace roots, and the exact deterministic lifecycle mapping `idle` (Ready) plus
`initializing`/`working` (Working). The callback returns a plain title.

Approval, result-ready, stop authority, health, background-task count,
failure, interruption, warning, direct tab color, Windows Terminal progress,
and animation are unavailable or unsupported. TabBeacon does not infer them
from arbitrary Agy output.

The callback stores only hashes, a safe workspace alias, bounded state tokens,
and timestamps. Prompt, assistant, tool, transcript, model, account, raw
session, and raw path content are not persisted. If TabBeacon is missing or a
callback fails, native Agy remains usable.

No wrapper, PATH shadow, PTY host, Hook configuration, or resident daemon is
installed.

## v0.8.0 exact 1.2.7 qualification

The Owner authorized narrow re-admission on 2026-09-29. The actual Windows
1.2.7 executable has a valid Google signature. An isolated native TUI startup
delivered the original structured title command input with exact version,
equal current/project workspace and `initializing`; session identity was absent
at that startup boundary and is deliberately not invented.

v0.8.0 keeps exact 1.1.19 and adds only exact 1.2.7 to the existing title
contract. Payload version must match its profile, and unknown/adjacent versions,
missing identities, foreign callbacks and ownership drift remain fail safe.
Setup/reconcile/preserve-native/uninstall retain the same minimal `title` member
and backup contract. This is no arbitrary future-version range or new backend.
The [official title contract](https://antigravity.google/docs/cli/title/) describes
structured stdin and plain stdout. A real authenticated 1.2.7 mixed session
showed Working title `Agy⭕AGYWORKS` without cross-provider interference.
After the response the same title remained visible; real Ready/idle title
conversion is unproven. Isolated tests map a delivered `idle` callback to Ready.
The Owner stopped the additional real preserve-native/title-only switch, so
that L4 scenario remains NOT_EXECUTED_OWNER_STOP; owned configuration and visual
fixtures retain their narrower proofs. Real Ready remains UNPROVEN. These attended
follow-ups are not required for the narrowed v0.8.0 public scope and are not
retroactively passed. Agy 1.2.13 remains unsupported.
