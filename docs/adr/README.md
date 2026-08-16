# Architecture Decision Records

ADRs capture decisions that constrain future implementation.

- `0001-product-scope.md`
- `0002-zero-workflow-change-and-fail-open.md`
- `0003-evidence-reconciliation-architecture.md`
- `0004-windows-terminal-vt-presentation.md`
- `0005-codex-backend-policy.md`
- `0006-offline-repository-identity.md`
- `0007-codex-hooks-provider.md`
- `0008-codex-hook-profile-and-turn-generations.md`
- `0009-workspace-identity-generalization.md`
- `0010-session-scoped-ephemeral-activity-worker.md`

ADR 0009 generalizes the presentation-facing identity from Git repository-only to workspace identity. Existing repository identity remains the Git specialization; ordinary non-Git directories gain a local fallback before production G11.

ADR 0010 admits the G11 direct ephemeral worker with hashed session/turn/terminal ownership, atomic leases, bounded predecessor handoff, upgrade ownership, and fail-open cleanup.

A later ADR may supersede an earlier decision, but implementation code must not silently reverse one.
