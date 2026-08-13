# TabBeacon Roadmap

Roadmap IDs are stable governance identifiers. `X` suffixes denote experimental work that does not block the adjacent production release unless promoted by a later decision.

## TB-B00 — Repository Bootstrap

**Purpose:** establish a public, governed Rust repository without implementing runtime product features.

Deliverables:

- Rust 1.97.1 build skeleton;
- MIT license and public project metadata;
- line-ending policy;
- VMCell-style governance and evidence contracts;
- architecture and ADR baseline;
- hosted Windows CI with exact-head checkout assertion;
- local CI script;
- PR template.

Exit gate: bootstrap files exist on `main`; exact-head hosted CI for the bootstrap head passes. If CI cannot run, the repository may exist but B00 remains `BLOCKED` or `UNPROVEN` rather than being declared complete.

## TB-G01 — Unified Agent Core Contract

Define provider-neutral types and reconciliation behavior:

- `AgentProvider`;
- `AgentSessionKey` (`provider + native session id`);
- `AgentEvidence`;
- `EvidenceSource` and authority/confidence classes;
- backend capabilities;
- `Phase`, `Attention`, `Health`;
- `StatePatch` and deterministic reconciliation;
- stale-evidence and tie-breaking rules.

No provider integration and no Windows Terminal output in this goal.

## TB-G02 — Windows Terminal Presentation

Implement typed `VisualState -> VT` rendering, including:

- title control;
- progress ring states;
- dynamic content/tab frame color;
- reset behavior;
- graceful fallback if a color capability is unavailable;
- deterministic presentation fixture.

Default semantic palette contract:

- ready: terminal default;
- working: green + indeterminate progress;
- result-ready: blue;
- approval attention: yellow;
- warning: orange (progress may continue if phase is working);
- interrupted: purple;
- failed: red.

Question attention remains a distinct semantic state even if the default v0.1 palette initially shares a human-attention color.

## TB-G03 — Visual CI Foundation

Build deterministic machine visual verification:

- launch real Windows Terminal in an interactive desktop session;
- identify the target tab via UI Automation;
- verify title semantics via UIA;
- capture tab/window screenshots;
- validate progress animation by frame-delta ROI;
- validate color by background ROI range rather than full-image golden equality;
- retain structured evidence bundles.

This goal must not depend on a real model/network call; it uses fixtures.

## TB-G04 — Offline Repository Identity

Implement local repository discovery and stable abbreviation:

- local Git discovery;
- canonical remote identity when available;
- local-only fallback identity;
- deterministic abbreviation;
- collision expansion;
- stable local history;
- atomic/process-safe state updates;
- worktree support;
- repository move/reclone behavior.

No GitHub API or network lookup is allowed for normal identity resolution.

## TB-G05 — First Provider: Codex Hooks

Implement the first production provider backend using global Codex hooks.

Requirements:

- one-time `tabbeacon setup codex`;
- daily command remains `codex`;
- preserve unrelated hooks/config;
- disable competing Codex terminal-title ownership only through supported configuration;
- `doctor` and ownership-safe uninstall;
- normalize hook payloads into `AgentEvidence`;
- emit only states the hook evidence can support reliably;
- no TUI text scraping for authoritative health/failure states.

## TB-G06X — Codex App-Server Experimental Backend

Research a higher-fidelity Codex backend without blocking v0.1.

Investigate:

- app-server protocol/version gating;
- in-process vs remote event model;
- approval/warning/failure/interruption fidelity;
- read-only observation possibilities that preserve direct `codex` launch;
- whether upstream support is needed for observing the embedded app-server stream.

Do not make the production `codex` command depend on an experimental wrapper/remote transport solely for TabBeacon.

## TB-G07 — Autonomous E2E and Hardening

Connect the production path end to end:

`Codex hook -> evidence -> reconciliation -> repository identity -> visual state -> Windows Terminal -> machine verdict`.

Cover multi-tab, same-repo multi-session, worktrees, collisions, Ctrl+C, normal exit, missing TabBeacon binary, hook failure, config drift, and fail-open behavior.

## TB-G08 — Public v0.1 Release

Release only after exact-head code CI, visual CI, setup/uninstall smoke tests, and release criteria are all green for the same candidate SHA.

Publish targets:

- GitHub Releases;
- crates.io when `publish = false` is intentionally removed;
- Windows x64 at minimum; Windows ARM64 when validated.

## Post-v0.1 extension tracks

- `TB-G20` — Claude provider, starting with native hooks.
- `TB-G30` — OpenCode provider, starting with provider plugin integration; SSE may be an enhanced backend.

These tracks reuse the same core, reconciliation, repository identity, presentation, and visual-CI layers.
