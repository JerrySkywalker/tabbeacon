# ADR 0001 — Product Scope

- Status: Accepted
- Date: 2026-08-13

## Decision

TabBeacon is a lightweight terminal identity/status layer, not an agent session manager or terminal replacement.

v0.1 supports Codex CLI in stock Windows Terminal on Windows. The architecture remains provider-neutral for future Claude/OpenCode adapters.

## Consequences

v0.1 will not implement PTY hosting, worktree management, prompt routing, remote control, orchestration, or a dashboard.
