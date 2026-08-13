# Project Charter

## Mission

TabBeacon turns terminal tabs into compact, reliable identity and status beacons for coding-agent sessions without replacing the user's terminal or changing the daily agent launch command.

## First product target

The first production target is Codex CLI inside stock Windows Terminal on Windows. Product code is written in Rust.

## Architectural intent

TabBeacon is not a Codex-specific state machine. The core accepts provider-neutral evidence and reconciles it into a session snapshot. Providers and their backends live below that contract; Windows Terminal presentation lives above it.

## Hard invariants

1. **Zero workflow change** — after setup, users continue to launch the underlying CLI directly.
2. **Fail open** — loss of TabBeacon may remove decoration, but must not make the agent CLI unavailable.
3. **Offline-first identity** — normal repository identity and abbreviation do not require network access.
4. **Stable identity** — previously assigned repository keys should not churn merely because new repositories appear.
5. **Evidence before color** — warning/error/interruption colors are emitted only from evidence strong enough to support their semantics.
6. **Provider isolation** — provider-specific raw events never become core API surface.
7. **Presentation isolation** — provider adapters never write Windows Terminal VT sequences directly.
8. **Machine-verifiable UI** — presentation changes must be amenable to deterministic fixture-driven visual testing.

## v0.1 deliverable

A public Windows-first Rust tool that, for Codex CLI:

- assigns a short stable repository identity;
- owns the tab title without competing title writers;
- shows working animation;
- shows state-dependent tab colors;
- distinguishes normal work, result-ready, and approval attention using evidence available to the production backend;
- integrates globally while preserving the `codex` command;
- includes autonomous functional and visual CI.

## Explicit v0.1 non-goals

- PTY hosting or replacement;
- session resurrection;
- terminal multiplexer/session manager behavior;
- agent orchestration;
- prompt routing;
- worktree management;
- remote control;
- dashboard/web UI;
- telemetry service;
- production Claude/OpenCode adapters.
