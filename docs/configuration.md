# Configuration

Configuration changes presentation preferences; it does not grant provider
compatibility, Hook trust, configuration ownership, or runtime authority.

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

Portable settings use canonical `tabbeacon-export-v1` JSON. Export creates a
new file by default; import previews before it can apply:

```powershell
tabbeacon export --output tabbeacon-settings.json
tabbeacon import tabbeacon-settings.json
tabbeacon import tabbeacon-settings.json --apply
```

Review the plan before `--apply`. Non-interactive import never mutates merely
because a file was supplied.

## Separate boundaries

| Surface | What it controls | What it cannot grant |
| --- | --- | --- |
| User preferences | title, activity, colors, theme, presets, language | provider compatibility or Hook trust |
| Provider integration | exact owned provider declarations | a user visual preference |
| Hook trust | an Owner-reviewed Codex decision | automatic setup authority |
| Runtime/session state | evidence-driven current presentation | persistent configuration ownership |

For provider ownership details, see [Codex Hooks](codex-hooks.md) and
[Agy setup](agy-setup.md).
