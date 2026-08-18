# TB-G40 — CLI Foundation & Output Contracts

## Status

PLANNED. First mandatory v0.4 implementation goal.

## Purpose

Replace the growing hand-written argument matcher with a typed CLI foundation while preserving every supported v0.3 automation command and JSON contract.

## Deliverables

- migrate command parsing to `clap` derive or an equivalent typed Rust command model;
- retain all supported v0.3 commands and exit semantics;
- define explicit human / JSON / plain output modes;
- add `tabbeacon ui` admission point without implementing the full TUI yet;
- define interactive-TTY vs non-TTY behavior;
- add PowerShell shell completion generation via `clap_complete` or equivalent;
- separate CLI parsing/dispatch from domain operations enough for later guided/TUI frontends to reuse commands without parsing stdout.

## Compatibility contract

These must remain valid:

```text
tabbeacon setup codex
tabbeacon status --json
tabbeacon doctor --json
tabbeacon config show
tabbeacon config set <key> <value>
tabbeacon config preset <name>
tabbeacon config reset
tabbeacon preview
tabbeacon title-policy inspect|repair|restore
tabbeacon uninstall codex
```

Internal hidden worker/fixture commands must remain source-compatible for existing tests/runtime or be migrated atomically with all callers.

## Human/machine output contract

G40 establishes the modes but does not need to complete the G42 pretty renderer:

```text
default human mode
--json stable structured mode
--plain legacy/key-value mode
```

No JSON document may gain human prose.

## TTY contract

```text
INTERACTIVE_TTY=true  -> future UI entry may be admitted
INTERACTIVE_TTY=false -> never enter raw/alternate-screen mode; deterministic help or requested command output
```

## Validation

- parser/command compatibility tests;
- exit-code regression tests;
- JSON byte/shape regression where already frozen;
- completion generation smoke;
- non-TTY tests;
- one final hosted exact-head code CI.

No Visual/UIA/L4 by default.

## Exit gate

```text
CLI_TYPED=true
EXISTING_COMMANDS_COMPATIBLE=true
STATUS_JSON_COMPATIBLE=true
DOCTOR_JSON_COMPATIBLE=true
PLAIN_MODE_ADMITTED=true
POWERSHELL_COMPLETION=PASS
NON_TTY_NO_FULLSCREEN=true
CODE_CI=PASS
```

Estimated effort: **3–5 h**.
