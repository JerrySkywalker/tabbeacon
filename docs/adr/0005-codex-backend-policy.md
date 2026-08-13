# ADR 0005 — Codex Backend Policy

- Status: Accepted
- Date: 2026-08-13

## Decision

Codex global hooks are the production v0.1 event backend. Codex app-server is an experimental higher-fidelity backend track.

The app-server track may be promoted only when it preserves direct `codex` launch, fail-open behavior, and an acceptable stability contract.

## Consequences

The core and presentation layers must not depend on Codex hook event names. App-server experiments can coexist with hooks by producing the same `AgentEvidence` contract.
