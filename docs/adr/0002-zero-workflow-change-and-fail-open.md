# ADR 0002 — Zero Workflow Change and Fail Open

- Status: Accepted
- Date: 2026-08-13

## Decision

After one-time provider setup, the user's normal launch command must remain the provider's native command (for v0.1: `codex`).

TabBeacon failure may remove decoration but must not make Codex unavailable.

## Consequences

- Do not require `tabbeacon codex` for normal v0.1 use.
- Do not replace/shadow `codex.exe` merely to gain richer status.
- Provider setup must be ownership-safe and reversible.
