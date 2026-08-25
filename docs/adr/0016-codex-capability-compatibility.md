# ADR 0016 — Codex Capability Compatibility

- Status: Accepted
- Date: 2026-08-25
- Goal: TB-V061-12H-FORWARD-COMPAT-VISUAL-001
- Supersedes: the version-admission decision of ADR 0008 only

## Context

ADR 0008 correctly froze exact Codex profiles while the Hook transport was
still unstable. That prevented a new release number from silently granting
ownership mutation or trust-sensitive configuration writes. After the
two-contract command/MCP history and a bounded local self-description surface
became available, exact release numbers became an operational fragility: a
normal Codex update could disable an otherwise compatible integration.

## Decision

Use local capability and protocol evidence, not a version registry, to decide
compatibility. The states are `Full`, `Degraded`, `Incompatible`, and
`Unproven`. Version output is diagnostic-only.

Required local Hooks evidence authorizes only a conservative bounded contract,
and only after the existing ownership/trust preflight passes. Optional schema
evidence enriches discovery but may not be a runtime network dependency. An
existing manifest-owned hybrid MCP declaration keeps its proven transport;
fresh setup does not infer it from release ordering.

An unproven detector blocks destructive or rewriting management actions but
does not disable a separately proven existing runtime. An incompatible required
capability refuses affected mutation. Unknown fields and additive events remain
fail-open in provider parsing.

## Consequences

- `99.99.99` with the same local capability evidence is supported.
- An upstream version bump alone cannot block setup, repair, reconciliation,
  or runtime continuity.
- Ownership safety and manual Hook trust remain unchanged.
- Future protocol changes are admitted by local evidence and bounded contract
  review, not by appending version numbers to a registry.
