# ADR 0003 — Evidence/Reconciliation Architecture

- Status: Accepted
- Date: 2026-08-13

## Decision

Provider backends normalize raw events into provider-neutral `AgentEvidence`. A reconciler folds evidence into orthogonal `Phase`, `Attention`, and `Health` state.

Backend capability and evidence authority are explicit. Provider-specific raw event names do not appear in the core contract.

## Rationale

Hooks are one-shot while app-server/SSE-style sources are streaming. Unifying transport shape would distort one side; unifying evidence semantics keeps the core stable while backends evolve.
