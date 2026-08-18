# TabBeacon v0.4 Planning Admission

## Status

PLANNED / ADMITTED FOR IMPLEMENTATION after merge of the v0.4 planning PR.
The production baseline is public `v0.3.0`; v0.4 is a human-interface and guided-management release, not a provider-expansion release.

## Stable goal IDs

Existing roadmap IDs `TB-G20` and `TB-G30` remain reserved for future Claude and OpenCode provider tracks. v0.4 therefore uses the stable IDs below:

```text
TB-G40  CLI Foundation & Output Contracts
  ↓
TB-G41  Unified Management / Action Model
  ↓
TB-G42  Human Status & Doctor v2
  ↓
TB-G43  Guided Setup v3
  ↓
TB-G44  Interactive Control Center
  ↓
TB-G46  UX Reliability & Accessibility
  ↓
TB-G46R v0.4 Hardening & Release
```

Optional non-blocking side track after G41:

```text
TB-G45X Sessions & Live Overview
```

`X` retains the repository convention: experimental work does not block the adjacent production release unless explicitly promoted.

## Version theme

**TabBeacon v0.4 — Human Interface & Guided Management**

v0.3 proved reliability. v0.4 makes the product easy to understand and configure without weakening the machine interfaces or safety boundaries already established.

## Mandatory product contracts

```text
DAILY_COMMAND=codex
GUIDED_SETUP_FULLSCREEN=false
GUIDED_SETUP_ENUM_TYPING_REQUIRED=false
PRESET_SELECTION_ATOMIC=true
STATUS_DEFAULT=HUMAN_FIRST
DOCTOR_DEFAULT=HUMAN_FIRST
STATUS_JSON_COMPATIBLE=true
DOCTOR_JSON_COMPATIBLE=true
CONTROL_CENTER_FULLSCREEN=true
CONTROL_CENTER_STAGED_APPLY=true
LIVE_PREVIEW=true
HOOK_TRUST_BYPASS=false
GLOBAL_DAEMON_ADDED=false
WRAPPER_ADDED=false
PROVIDER_ADDED=false
```

## Governance

Use Fast Lane v2 from `dev_governance_files/QUALITY_GATES.md`.

- focused tests while iterating;
- one settled candidate, one final hosted code CI per material risk surface;
- human-output changes require deterministic output tests, not repeated real-UI evidence;
- full-screen TUI layout should primarily use Ratatui `TestBackend`-style deterministic buffer tests;
- one bounded real Windows Terminal smoke is sufficient for TUI terminal-mode admission unless a later change alters terminal-state ownership;
- preserve JSON and direct CLI automation interfaces while improving human defaults.

Detailed frozen plan: `goals/TB-V04-HUMAN-INTERFACE-GUIDED-MANAGEMENT.md`.
