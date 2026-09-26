# Configuration

Configuration changes presentation preferences; it does not grant provider
compatibility, Hook trust, configuration ownership, or runtime authority.
The [English/Chinese terminology](terminology.md) keeps these states distinct.

## Guided setup and Control Center

Use guided setup for a cohesive first configuration or a bounded revisit:

```powershell
tabbeacon setup
tabbeacon setup --quick
tabbeacon setup --full
```

`tabbeacon ui` opens the current Control Center entry point without changing how
you launch a provider. Follow the displayed scope and review any proposed
provider change before applying it.

## Presentation preferences

Inspect the effective settings, then choose a direct value, a preset, or the
interactive wizard:

```powershell
tabbeacon config show
tabbeacon config set spinner braille
tabbeacon config set theme muted-dark
tabbeacon config preset balanced
tabbeacon config wizard
tabbeacon config reset
```

The typed settings cover title presentation, activity/spinner behavior, tab
color, theme, and named presets. Use `tabbeacon preview --theme muted-dark`
for a temporary visual preview; preview does not persist a change.

For the v0.8.0 development train, a partial provider override inherits each
unspecified field from the existing user-global default. `native` leaves that
channel to the provider or terminal; `off` disables TabBeacon output on the
channel. Neither value suppresses a provider's own title. Saving a preference
does not establish that the integration is installed, trusted, or applied to an
already running CLI session. The published v0.7.3 commands above remain the
current user-facing contract until the new CLI and integration are qualified.

The candidate CLI requires a separate preview and explicit apply verb:

```powershell
tabbeacon config provider cursor preview color-only
tabbeacon config provider cursor apply color-only
tabbeacon config provider cursor show
tabbeacon config provider cursor inherit --apply
```

`tabbeacon config wizard` now starts with a global/Codex/Agy/Cursor target
choice. A provider mode is previewed before a separate Apply/Cancel decision;
cancel leaves the saved preference untouched. The Control Center Integrations
screen shows requested/effective provider preferences and application status.
Keys `1`/`2`/`3` select Codex/Agy/Cursor, arrow keys choose an admitted mode,
`r` chooses inheritance, and `v` previews. Only `a` in that preview requests
the snapshot-guarded write; Esc cancels without a write. The saved setting still
does not prove current-session application.

The output separates requested and capability-limited effective channels.
`LIVE_APPLICATION=NOT_INSTALLED` means the admitted Agy title callback is absent;
`UNPROVEN` means the saved choice has not been proved active in the current CLI
session. Agy's explicit `preserve-native` mode removes only an owned Agy title
callback when applied; `tabbeacon setup agy` also respects that saved mode.
Current Cursor terminal routing is unqualified, so
its requested color remains capability-limited; this command does not install
Cursor Hooks or claim the color appeared.

## Human interface preferences

Language, color, and reduced-motion preferences are user-local interface
preferences:

```powershell
tabbeacon interface show
tabbeacon interface set --help
```

They are separate from provider integration state and from live session
evidence.

## Workspace identity

TabBeacon derives a stable, offline-first workspace identity, specializing Git
identity when it is available. Inspect candidates before setting a local alias:

```powershell
tabbeacon alias show
tabbeacon alias preview
tabbeacon alias explain
tabbeacon alias set --help
```

An explicit alias is device-local. It is not a provider setting and does not
change the repository, terminal, or daily command.

## Export and import

Portable preferences use canonical `tabbeacon-export-v1` or
`tabbeacon-export-v2` JSON. Export creates a new file by default; import
previews before it can apply:

```powershell
tabbeacon export --output tabbeacon-settings.json
tabbeacon import tabbeacon-settings.json
tabbeacon import tabbeacon-settings.json --apply
```

Review the plan before `--apply`. Non-interactive import never mutates merely
because a file was supplied.
Exports without provider overrides retain `tabbeacon-export-v1`; exports with
partial provider overrides use `tabbeacon-export-v2`. The v0.8.0 candidate
accepts both. The v2 document contains preferences only, never Hook trust,
provider authentication, terminal binding, or installed integration state.
If an import would change Codex terminal-title ownership, preview still shows
the proposed change without writing. Explicit Apply uses the owned Codex
reconciliation path after snapshot-guarded preference writes. A changed store
or an unprovable owned Hook/configuration refuses success. A failed external
write is reported as `partial_state` even when preference compensation
succeeds; inspect the owned integration before retrying. Process interruption
recovery now has an owned local transaction journal. A later explicit
`--apply` checks the exact three preference paths and original or planned
bytes before restoring them. It calls the owned Codex title reconciler only
if the prior process reached the Hook boundary. The title reconciler has a
separate exact-byte journal for its own `config.toml` and ownership manifest
write boundary. External drift or uncertain Hook ownership leaves
`partial_state` and the journal for review. The import journal
temporarily contains exact local preference bytes so unrelated fields can be
restored; it is not exported. Isolated process-exit tests cover the write
boundaries, while real user configuration and Hook trust remain outside the
test and require independent safety review. Import never copies Hook trust or
authentication.

## Separate boundaries

| Surface | What it controls | What it cannot grant |
| --- | --- | --- |
| User preferences | title, activity, colors, theme, presets, language | provider compatibility or Hook trust |
| Provider integration | exact owned provider declarations | a user visual preference |
| Hook trust | an Owner-reviewed Codex decision | automatic setup authority |
| Runtime/session state | evidence-driven current presentation | persistent configuration ownership |

For provider ownership details, see [Codex Hooks](codex-hooks.md) and
[Agy setup](agy-setup.md).
