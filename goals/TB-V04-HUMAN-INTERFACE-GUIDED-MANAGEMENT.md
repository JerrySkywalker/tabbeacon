# TabBeacon v0.4 — Human Interface & Guided Management

## Status

COMPLETE. v0.4.0 was publicly released from
`0e0c81279b4f90c9d67f9b841405401d532e7c24`; post-release G45X subsequently
landed without changing the release source or tag.

## Product objective

v0.3 answered whether TabBeacon can reliably own and animate a Codex Windows Terminal tab. v0.4 answers whether a human can install, understand, configure, and repair TabBeacon without memorizing internal enum spellings or reading machine-oriented `KEY=VALUE` output.

The release introduces four first-class interaction layers backed by one management/domain model:

```text
1. Snapshot interface
   tabbeacon status / doctor
   scrollback-friendly human output

2. Guided setup
   tabbeacon setup
   Hermes-style inline wizard, not full-screen

3. Control Center
   tabbeacon / tabbeacon ui
   full-screen Ratatui management UI

4. Automation interface
   --json / --plain / direct config commands
   scripts, Codex, CI, automation
```

No frontend may create a second configuration or ownership system.

## UX findings motivating v0.4

The v0.3 guided setup accepts a preset and then continues asking for every individual value. Real dogfood showed that selecting `balanced` could then be unintentionally overridden by typing another spinner choice. v0.4 therefore makes preset selection atomic: choosing a preset ends option selection unless the user explicitly enters Customize.

The v0.3 `status` and `doctor` structured data are good machine interfaces, but the default human render remains close to machine `KEY=VALUE` output. v0.4 keeps the structured data model and makes human output concise, grouped, actionable, and copy-friendly.

## Product invariants retained

```text
DAILY_COMMAND=codex
FAIL_OPEN=true
GLOBAL_DAEMON_BASELINE=false
HOOK_TRUST_BYPASS=false
PROVIDER_NEUTRAL_CORE=true
OFFLINE_WORKSPACE_IDENTITY=true
NO_PATH_SHADOW=true
NO_PTY_WRAPPER=true
NO_LAUNCH_WRAPPER=true
RAW_PROMPT_TOOL_MODEL_CONTENT_PERSISTED=false
```

## Interaction architecture

### Snapshot

`tabbeacon status` answers “what is the current state?” and should normally fit in one terminal screen when healthy.

`tabbeacon doctor` answers “what is wrong and what should I do next?” and must provide an actionable next step for every human-visible warning/failure where one exists.

### Guided setup

`tabbeacon setup` is an inline, scrollback-preserving wizard. It must not enter alternate-screen mode. Selection should use arrow/select/confirm primitives so normal users do not type closed enum values.

Fresh users receive environment discovery, a brief privacy statement, a recommended preset path, preview, staged change summary, Apply/Cancel, and any manual trust handoff.

Returning users receive a short health summary and choices such as Appearance, Integration, Repair detected issues, Full setup, or Cancel.

`setup --quick` addresses only missing/stale/action-required items. `setup --full` deliberately revisits the complete flow with current values as defaults.

### Control Center

On an interactive TTY, `tabbeacon` may enter the full-screen Control Center; `tabbeacon ui` is the explicit entry point. Non-interactive execution must never hang waiting for TUI input and should print deterministic help/guidance.

Mandatory screens:

```text
Overview
Appearance
Codex Integration
Diagnostics
Preview
```

All configuration edits are staged until Apply. Quit/Revert must be lossless.

### Automation

Keep existing direct commands. `status --json` and `doctor --json` remain stable structured contracts. Add `--plain` for the legacy/key-value human-compatible form if default human rendering changes.

## Preferred Rust stack

```text
CLI grammar        clap derive
shell completion   clap_complete
inline setup       dialoguer (+ console/theme support if useful)
full-screen TUI    ratatui
terminal backend   crossterm
core/domain        existing TabBeacon modules
```

The exact crate versions are implementation decisions, but the frontend/domain separation is normative.

## Management model

Introduce one shared management projection, conceptually including:

```text
ManagementSnapshot
HealthIssue
RecommendedAction
ChangePlan
```

Remediation actions must have explicit safety classes such as:

```text
READ_ONLY
MANUAL_ACTION
PREVIEWABLE_SAFE_REPAIR
OWNER_EXPLICIT_REQUIRED
UNSUPPORTED_AUTOMATION
```

Hook trust remains manual. Existing ownership-safe Windows Terminal title-policy repair may be surfaced as previewable safe remediation; unrelated user configuration remains preserved.

## Dependency DAG

```text
v0.3.0 RELEASED
      ↓
TB-G40 — CLI Foundation & Output Contracts
      ↓
TB-G41 — Unified Management / Action Model
      ↓
TB-G42 — Human Status & Doctor v2
      ↓
TB-G43 — Guided Setup v3
      ↓
TB-G44 — Interactive Control Center
      ↓
TB-G46 — UX Reliability & Accessibility
      ↓
TB-G46R — v0.4 Hardening & Release

post-release:
v0.4.0 RELEASED
      ↓
TB-G45X — Sessions & Live Overview — COMPLETE
```

The mandatory production path remained sequential. G45X was non-blocking and
landed only after v0.4.0 publication and public-consumer verification.

## Goal summary and effort

| Goal | Scope | Effective effort |
| --- | --- | ---: |
| G40 | clap CLI, output modes, PowerShell completion, TTY behavior | 3–5 h |
| G41 | shared management/action/change-plan model | 2–4 h |
| G42 | human-first status/doctor, JSON/plain compatibility | 3–5 h |
| G43 | Hermes-style guided setup, quick/full/custom, atomic presets | 5–7 h |
| G44 | Ratatui Control Center, staged settings, preview | 7–11 h |
| G46 | TUI terminal restoration, resize, accessibility, non-TTY | 3–5 h |
| G46R | upgrade/fresh-install hardening and public release | 3–5 h |
| **Mandatory** | | **26–42 h** |
| G45X | optional Sessions & Live Overview | +4–7 h |

Mandatory center estimate is roughly 33–34 effective engineering hours.

## Non-goals

Do not add during v0.4:

- Claude provider;
- OpenCode provider;
- Codex App Server production backend;
- web/remote dashboard;
- session control or process killing;
- global resident daemon;
- wrapper/PATH interception/PTY host;
- self-update system;
- automatic Hook trust;
- arbitrary user-defined scripts/VT sequences/spinner code.

## Release definition

```text
TABBEACON_V04=PASS
DAILY_COMMAND=codex

GUIDED_SETUP=PASS
GUIDED_SETUP_FULLSCREEN=false
GUIDED_SETUP_ENUM_TYPING_REQUIRED=false
PRESET_SELECTION_ATOMIC=true
CUSTOM_OPTIONS_ONLY_AFTER_EXPLICIT_CUSTOMIZE=true
WRITE_BEFORE_APPLY=false

STATUS_DEFAULT=HUMAN_FIRST
DOCTOR_DEFAULT=HUMAN_FIRST
STATUS_JSON_COMPATIBLE=true
DOCTOR_JSON_COMPATIBLE=true
LEGACY_PLAIN_MODE=true

CONTROL_CENTER=PASS
CONTROL_CENTER_FULLSCREEN=true
CONTROL_CENTER_STAGED_APPLY=true
LIVE_PREVIEW=true
TUI_EXIT_RESTORES_TERMINAL=true

HOOK_TRUST_BYPASS=false
OWNER_CONFIG_PRESERVED=true
PROVIDER_ADDED=false
GLOBAL_DAEMON_ADDED=false
WRAPPER_ADDED=false

V0_4_RELEASE=PASS
```
