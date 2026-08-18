# TabBeacon Roadmap

Roadmap IDs are stable governance identifiers. `X` suffixes denote experimental work that does not block the adjacent production release unless promoted by a later decision.

## Current production baseline

The v0.2 production line is complete through public `0.2.0` distribution.
The Codex-first v0.2 implementation train is complete: `TB-G10A` through
`TB-G14` are **COMPLETE**.

Authoritative v0.2.0 release head and tag:

```text
RELEASE_SHA=0b1d5136833a05bf94b7d32c414a21da2f5ac78e
TAG=v0.2.0
```

Public distribution: crates.io `tabbeacon 0.2.0` and the GitHub `v0.2.0`
Release, including the verified Windows x64 ZIP and SHA-256 sidecar.

The mandatory v0.1 path is complete: `TB-B00`, `TB-G01`, `TB-G02`, `TB-G03`, `TB-G04`, `TB-G05`, `TB-G07`, and `TB-G08` all landed and were released. The Codex App Server experiment (`TB-G06X`) and future Claude/OpenCode provider tracks (`TB-G20`, `TB-G30`) remain deliberately deferred and do not count as Codex-only product debt.

## Active production sequence — v0.3 Codex Presentation Reliability & Motion

The released v0.2 line is historical baseline and remains the source of its
public release record. Near-term implementation is the sequential v0.3
Codex-first presentation reliability and motion track, beginning only after
this planning admission:

```text
TB-G15 — Title Authority Observatory
  ↓
TB-G16 — Animation Engine v2 and Defaults
  ↓
TB-G17 — Title Ownership / Conflict Remediation
  ↓
TB-G18 — Session Convergence and Recovery
  ↓
TB-G19 — Codex Compatibility Registry v2
  ↓
TB-G19R — v0.3 Hardening and Release
```

Detailed execution plan: [`../goals/TB-V03-CODEX-PRESENTATION-RELIABILITY.md`](../goals/TB-V03-CODEX-PRESENTATION-RELIABILITY.md).
The track keeps the stock `codex` + Codex Hooks + Windows Terminal workflow:
no launcher wrapper, global daemon, App Server dependency, Claude provider, or
OpenCode provider is admitted by this roadmap transition.

TB-G15 through TB-G19R are **COMPLETE**. G18 merged through same-SHA recovery
PR #30 after the GraphQL ready-path outage; G19 merged as PR #31 after its
exact-head hosted CI. v0.3.0 was published from
`ddb2a218b9fe7601a09caaa1c8c3a0c1d0af9419` as `v0.3.0`, with crates.io,
the public GitHub Release, and the verified Windows x64 ZIP plus SHA-256
sidecar all available.
Pixel capture remains a latched runner-environment limitation, while exact-tab
UIA title evidence continues to cover title admission and working-frame motion.

## Active production sequence — v0.4 Human Interface & Guided Management

The released v0.3 line remains the public baseline. v0.4 improves local human
management while retaining the stock `codex` launch path, Codex Hooks, and
automation-safe JSON/direct-command interfaces:

```text
TB-G40 — CLI Foundation & Output Contracts — COMPLETE
  ↓
TB-G41 — Unified Management / Action Model — COMPLETE
  ↓
TB-G42 — Human Status & Doctor v2 — COMPLETE
  ↓
TB-G43 — Guided Setup v3 — COMPLETE
  ↓
TB-G44 — Interactive Control Center — COMPLETE
  ↓
TB-G46 — UX Reliability & Accessibility — NEXT
  ↓
TB-G46R — v0.4 Hardening & Release
```

Detailed plan: [`../goals/TB-V04-HUMAN-INTERFACE-GUIDED-MANAGEMENT.md`](../goals/TB-V04-HUMAN-INTERFACE-GUIDED-MANAGEMENT.md).
No new provider, launcher wrapper, PATH shadow, global daemon, or hook-trust
bypass is admitted by this track.

## TB-B00 — Repository Bootstrap — COMPLETE

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

## TB-G01 — Unified Agent Core Contract — COMPLETE

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

## TB-G02 — Windows Terminal Presentation — COMPLETE

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

## TB-G03 — Visual CI Foundation — COMPLETE

Build deterministic machine visual verification:

- launch real Windows Terminal in an interactive desktop session;
- identify the target tab via UI Automation;
- verify title semantics via UIA;
- capture tab/window screenshots;
- validate progress animation by frame-delta ROI;
- validate color by background ROI range rather than full-image golden equality;
- retain structured evidence bundles.

This goal must not depend on a real model/network call; it uses fixtures.

## TB-G04 — Offline Repository Identity — COMPLETE

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

## TB-G05 — First Provider: Codex Hooks — COMPLETE

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

## TB-G06X — Codex App-Server Experimental Backend — FUTURE BACKLOG

Research a higher-fidelity Codex backend without blocking the Hooks production path.

Possible future investigation:

- app-server protocol/version gating;
- in-process vs remote event model;
- approval/warning/failure/interruption fidelity;
- read-only observation possibilities that preserve direct `codex` launch;
- whether upstream support is needed for observing the embedded app-server stream.

This track is intentionally unscheduled. The Codex-first v0.2 line must not depend on App Server, an experimental wrapper, or remote transport solely for TabBeacon.

## TB-G07 — Autonomous E2E and Hardening — COMPLETE

Connect the production path end to end:

`Codex hook -> evidence -> reconciliation -> repository identity -> visual state -> Windows Terminal -> machine verdict`.

Cover multi-tab, same-repo multi-session, worktrees, collisions, Ctrl+C, normal exit, missing TabBeacon binary, hook failure, config drift, and fail-open behavior.

## TB-G08 — Public v0.1 Release — COMPLETE

Release only after exact-head code CI, visual CI, setup/uninstall smoke tests, and release criteria are all green for the same candidate SHA.

Completed distribution channels now include:

- GitHub Releases;
- crates.io;
- Windows x64 prebuilt archive.

Windows ARM64 remains optional future packaging work when real validation is available.

# Codex-first v0.2 track

## Track objective

Make TabBeacon effectively feature-complete for the Owner's daily multi-tab Codex CLI workflow while preserving the original product invariants:

- daily launch remains literally `codex`;
- Codex Hooks remain the production observation backend;
- TabBeacon failure never blocks Codex;
- no fake `codex.exe`, PATH shadow, PowerShell `codex` function, PTY wrapper, or launch wrapper;
- no global resident daemon is introduced as the animation baseline;
- repository identity remains offline-first;
- provider-neutral core boundaries remain intact even though near-term product work is Codex-only;
- Hook evidence must not invent authoritative `Warning`, `Interrupted`, or `Failed` states when the backend cannot prove them.

## Title grammar contract for v0.2

The default title grammar is status-first:

```text
<status-slot> <repository-alias>
```

The status glyph or animation is always on the **left**; the stable repository identity is always on the **right**.

Examples:

```text
○ OWH
⠋ OWH
⠙ OWH
✓ OWH
! OWH
? OWH
```

The default compact grammar does not append redundant English lifecycle words such as `working`, `result-ready`, or `approval`. Animation changes only the left status slot; the repository alias must remain positionally stable.

## Dependency order

```text
TB-G09
  ↓
TB-G10
  ↓
TB-G10A
  ↓
TB-G11
  ↓
TB-G12
  ↓
TB-G13
  ↓
TB-G14 (COMPLETE)
```

Do not parallelize these goals by default. Turn/agent identity decisions in `TB-G10` directly constrain worker identity and stale-event rejection in `TB-G11`.

## TB-G09 — Status-First Title Grammar v2 — COMPLETE

**Purpose:** establish one compact presentation grammar before adding real animation.

Deliverables:

- separate stable repository identity from the mutable status slot;
- default layout becomes `<status-slot> <repository-alias>`;
- working indicator/spinner frames render on the left;
- result-ready, approval, question, and ready glyphs render on the left;
- remove default redundant lifecycle word suffixes from production titles;
- keep title sanitization, length bounds, dynamic tab color, and optional Windows Terminal progress behavior intact;
- update deterministic presentation fixtures and visual assertions for positional stability.

Exit gate:

```text
TITLE_GRAMMAR=status-first
STATUS_SLOT_POSITION=left
REPOSITORY_POSITION=right
REPOSITORY_POSITION_STABLE_DURING_ACTIVITY=true
DEFAULT_SEMANTIC_WORD_SUFFIXES=false
```

Because this changes visible title semantics, exact-head L3 visual evidence is required.

Estimated effective engineering effort: **2–4 h**.

## TB-G10 — Codex Hook Compatibility and Turn/Agent Awareness — COMPLETE

**Purpose:** harden the Hooks backend against Codex lifecycle evolution before a long-lived animation worker depends on it.

Deliverables:

- probe and classify the installed Codex Hook capability/profile rather than assuming every upstream event is reliable everywhere;
- retain `session_id` as durable session identity while admitting stable turn-level metadata where available;
- use `turn_id` or an equivalent proven generation key to reject stale prior-turn terminal updates;
- distinguish root-agent and subagent lifecycle metadata when the installed Codex version exposes reliable fields;
- prevent subagent events from accidentally terminating or replacing the root tab's activity state;
- explicitly classify compact lifecycle (`PreCompact`/`PostCompact` or equivalent) when supported and proven;
- keep unknown or unsupported Hook events forward-compatible and fail-open;
- add compatibility reporting to doctor-level internals for use by later goals;
- never infer health states merely from tool exit codes, missing events, logs, or TUI text.

Exit gate:

- current supported Codex version/profile is identified deterministically;
- stale prior-turn events cannot override a newer admitted turn;
- admitted subagent events cannot corrupt root-session tab state;
- unsupported/new events remain safely ignored;
- setup preserves unrelated hooks and the official trust boundary.

This goal should use upstream/source research as evidence, but production behavior must be frozen against an actually admitted Codex version/profile rather than upstream `main` alone.

Current candidate profile is `codex-hooks-rust-v0.147.0`, audited from the
official `rust-v0.147.0` tag. Local deterministic coverage is implemented. The
Owner-completed isolated real-Codex L4 passed at implementation head
`640e1ff1380c595148502f6eeaba8fc2bb983468`; Hook trust exists only in the
isolated recorder profile, the real Owner Codex configuration was not changed,
and no trust bypass was used. Codex 0.147.0 clamped the fixture's five-second
`SessionEnd` timeout to three seconds, while the real Hook still completed and
appeared in minimized evidence. The closeout acceptance-candidate head
`96e336fdd7de6d6f8fb15730816d5494b5d26158` passed local L0/L1/L2, hosted
exact-head CI run `31940268156`, and fresh isolated real-Codex L4. The final
COMPLETE metadata head must receive the same final-head gates before merge.

Estimated effective engineering effort: **4–8 h**.

## TB-G11 — Session-Scoped Ephemeral Activity Animator — COMPLETE

**Purpose:** replace the v0.1 static title-spinner fallback with real working-state animation without changing the `codex` launch command.

### Mandatory feasibility gate

Before broad implementation, prove all of the following in a focused spike:

- a child/worker can outlive the one-shot Hook process without exceeding or inheriting the Hook timeout incorrectly;
- it can continue updating the correct originating Windows Terminal tab/session;
- two simultaneous Codex tabs cannot cross-write each other's titles;
- closing the tab/session causes bounded cleanup;
- Codex and TabBeacon remain usable if worker creation or worker execution fails.

If this cannot be proved, the goal must stop `BLOCKED`/`UNPROVEN`. Do **not** silently replace the design with a machine-global always-running daemon.

### Worker contract

A viable worker must be session/turn scoped and carry only minimum non-sensitive state, conceptually including:

```text
session identity
turn/generation identity
terminal binding
repository alias
semantic activity state
spinner preset
lease/last-update metadata
```

It must not persist prompt text, assistant content, tool input/output, or arbitrary Hook payload content.

Deliverables after feasibility passes:

- start/update the worker when reliable Hooks prove active work;
- stop/supersede it on reliable result-ready, approval, question, end, or newer-turn evidence;
- bounded stale-worker TTL/lease cleanup;
- worker crash and missing-binary fail-open behavior;
- upgrade-safe generation ownership so an obsolete worker cannot keep overwriting a newer release/session;
- animate only the left status slot while keeping the right repository alias stationary;
- preserve optional WT-ring activity as an independently configurable channel;
- visual CI proves multiple distinct animation frames plus stable repository text.

Expected working title sequence:

```text
⠋ OWH
⠙ OWH
⠹ OWH
⠸ OWH
```

Expected terminal transition example:

```text
○ OWH
  ↓
⠋ OWH
  ↓
⠙ OWH
  ↓
✓ OWH
```

Estimated effective engineering effort: **8–16 h** after feasibility admission.

## TB-G12 — Guided Setup and Configuration Wizard v2 — COMPLETE

**Purpose:** turn the existing sequential CLI configuration primitives into a coherent first-run and reconfiguration workflow.

The existing `config show`, `config wizard`, `config set`, presets, reset, and preview commands remain supported; this goal evolves them rather than replacing them with a second configuration system.

Deliverables:

- guided `tabbeacon setup` onboarding entry point while retaining `tabbeacon setup codex` as the scriptable provider-specific command;
- detect and report Windows Terminal, Codex, TabBeacon version, Hook profile, and integration state;
- arrow-key or similarly lightweight interactive selection for the small closed set of presentation options;
- inline/live preview before commit;
- explicit Apply/Cancel semantics so exploratory changes do not persist accidentally;
- presets remain first-class;
- successful setup clearly hands off to the official Codex `/hooks` trust review and then `tabbeacon doctor`;
- no trust bypass, no automatic approval of Hook definitions;
- no full-screen terminal application requirement and no heavy TUI framework unless evidence shows the lightweight approach is insufficient.

Preferred UX is a compact TUI-like wizard, not a general terminal application.

Estimated effective engineering effort: **4–8 h**.

## TB-G13 — Operational Status and Machine-Readable Diagnostics — COMPLETE

**Purpose:** make TabBeacon easy to audit manually and by Codex itself.

Deliverables:

- `tabbeacon status` human-readable operational summary;
- `tabbeacon status --json` machine-readable summary;
- `tabbeacon doctor --json` or equivalent stable structured diagnostics;
- report TabBeacon version/binary path, Codex version/profile, Hook declaration/trust/ownership status, presentation settings, and animator worker counts/health where applicable;
- expose stale-worker or compatibility warnings without leaking prompts, Hook payload bodies, credentials, or unrelated user configuration;
- retain nonzero exit behavior for actual doctor failure and distinguish warnings from failures.

The structured interface should be suitable for a Codex maintenance task to consume without scraping human prose.

Estimated effective engineering effort: **2–4 h**.

## TB-G14 — Codex-Only v0.2 Hardening and Release — COMPLETE

**Purpose:** close the Codex-first track with one production release candidate rather than adding another feature.

Release closure completed for `0.2.0` at
`0b1d5136833a05bf94b7d32c414a21da2f5ac78e` / `v0.2.0`: exact-head L0–L4,
functional matrix, crates.io publication, GitHub Release, Windows x64 ZIP,
and public consumer verification all passed.

Required real-world scenarios include:

- multiple simultaneous Codex tabs;
- same-repository multiple sessions;
- linked worktrees;
- stale prior-turn events;
- subagent lifecycle where supported;
- compact lifecycle where supported;
- normal Stop/result-ready;
- PermissionRequest/attention transition;
- Ctrl+C/interruption paths only to the extent Hooks can prove them without inventing health authority;
- Codex crash/abnormal disappearance cleanup;
- Windows Terminal tab close;
- animator worker crash;
- missing TabBeacon binary;
- binary upgrade/relocation;
- settings changes;
- SessionEnd cleanup;
- fail-open behavior throughout.

Visual evidence must prove the v0.2 title grammar and animation contract at the exact release candidate SHA:

```text
status glyph/frame changes on the left
repository alias remains stable on the right
semantic tab color remains correct
final static state replaces animation when reliable Hook evidence arrives
```

Release only after all applicable L0–L4 gates are green at one exact candidate head. Publish GitHub Release and crates.io intentionally from the same accepted source version.

Estimated effective engineering effort: **3–6 h**.

## Codex-first v0.2 historical completion definition

The Codex-only v0.2 product line is complete: `TB-G09` through `TB-G14` landed
and v0.2.0 was publicly released. The following estimate is retained as the
historical completion definition, not current near-term work.

Expected remaining effective engineering effort from v0.1.1 baseline:

```text
G09   2–4 h
G10   4–8 h
G11   8–16 h
G12   4–8 h
G13   2–4 h
G14   3–6 h
----------------
Total 23–46 h
```

The largest uncertainty is `TB-G11` feasibility and lifecycle isolation. Do not hide that uncertainty by pre-committing to a daemon architecture.

# Future backlog — unscheduled

The following are explicitly outside the Codex-first v0.2 completion definition:

- `TB-G06X` — Codex App Server experimental backend;
- `TB-G20` — Claude provider, starting with native hooks;
- `TB-G30` — OpenCode provider, starting with provider plugin integration; SSE may be an enhanced backend;
- Windows ARM64 packaging until validated on appropriate hardware/CI;
- full-screen configuration TUI;
- shell-completion polish;
- self-update/package-manager behavior;
- Winget/Scoop packaging.

These may be promoted by a later Owner decision. They reuse the same core, reconciliation, repository identity, presentation, and visual-CI layers but do not block the Codex Hooks workflow from being considered complete.
