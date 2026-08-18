# ADR 0011 — Human Interface and Guided Management Architecture

## Status

Accepted for the v0.4 planning track after the planning PR merges.

## Context

TabBeacon v0.3 has reliable Codex/Windows Terminal integration, structured diagnostics, ownership-safe setup, and a typed presentation model. Its human interaction is still dominated by hand-written argument parsing, `KEY=VALUE` status output, and setup prompts that require users to understand internal option spellings.

v0.4 needs both an easy first-run/reconfiguration flow and a full-screen daily management interface. Treating those as the same interface would harm either setup scrollback/copyability or daily discoverability.

## Decision

TabBeacon adopts four frontend layers over one shared management/domain model:

```text
Snapshot interface   status / doctor
Guided setup         inline, non-full-screen
Control Center       full-screen TUI
Automation interface JSON/plain/direct commands
```

### Guided setup

The setup wizard remains in the normal terminal scrollback. It uses selection/confirmation primitives rather than requiring users to type closed enums. Preset selection is atomic; deeper fields appear only after explicit Customize.

Preferred implementation is a lightweight Rust prompt library such as `dialoguer`, not Ratatui alternate-screen mode.

### Control Center

The Control Center is a separate full-screen frontend using Ratatui + Crossterm or equivalent pure-Rust terminal tooling. It edits only an in-memory draft until Apply and uses the production presentation renderer for preview.

### CLI foundation

Use `clap` derive or equivalent typed Rust CLI parsing and `clap_complete`-style completion generation. Existing v0.3 commands and JSON contracts remain supported.

### Shared management model

Human frontends do not own configuration, Hook, trust, or Windows Terminal mutation semantics. They consume one typed management projection and typed change/action plans backed by existing stores/integration modules.

Conceptually:

```text
ManagementSnapshot
HealthIssue
RecommendedAction
ChangePlan
```

Every remediation action has an explicit safety class. Hook trust is always manual and cannot be automated by any frontend.

## Consequences

Positive:

- first-run setup stays simple, copyable, and scrollback-friendly;
- daily management gains discoverability without forcing a TUI on scripts;
- status/doctor become readable without sacrificing JSON automation;
- one management model prevents setup/TUI/doctor from disagreeing;
- deterministic TUI buffer tests can cover layout without a large Windows UIA matrix.

Costs:

- new dependencies for CLI/prompts/TUI/terminal handling;
- terminal raw/alternate-screen cleanup becomes a new reliability surface;
- human/plain/JSON output contracts must be maintained deliberately.

## Rejected alternatives

### One full-screen TUI for everything

Rejected because first-run setup, SSH/support, and copyable remediation benefit from normal scrollback.

### Only improve the existing line-input wizard

Rejected because daily configuration/status discovery benefits from a persistent full-screen control surface.

### Frontends write config directly

Rejected because it would duplicate v0.3 ownership and rollback semantics.

### Add provider expansion in v0.4

Rejected. v0.4 is a human-interface release; Claude/OpenCode/App Server work remains on separate roadmap tracks.

## Invariants

```text
DAILY_COMMAND=codex
GUIDED_SETUP_FULLSCREEN=false
CONTROL_CENTER_FULLSCREEN=true
WRITE_BEFORE_APPLY=false
HOOK_TRUST_BYPASS=false
AUTOMATION_JSON_RETAINED=true
FRONTENDS_SHARE_DOMAIN_MODEL=true
```
