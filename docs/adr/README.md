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
- `0010-title-authority-and-motion.md`
- `0011-human-interface-and-guided-management.md`
- `0012-localized-human-presentation-and-workspace-preferences.md`
- `0013-reliability-explainability-multi-provider-boundaries.md`
- `0014-agy-preadmission-qualification-boundary.md`

ADR 0009 generalizes the presentation-facing identity from Git repository-only to workspace identity. Existing repository identity remains the Git specialization; ordinary non-Git directories gain a local fallback before production G11.

ADR 0010 (session-scoped ephemeral activity worker) admits the G11 direct ephemeral worker with hashed session/turn/terminal ownership, atomic leases, bounded predecessor handoff, upgrade ownership, and fail-open cleanup. ADR 0010 (title authority and motion) separately records the later v0.3 title/motion authority decision; the duplicate numeric prefix is historical and retained.

ADR 0011 defines the shared v0.4 Human management architecture: Human status/doctor, inline guided Setup, full-screen Control Center, and stable automation interfaces over one management/domain model.

ADR 0012 defines the v0.5 Human/i18n boundary, deterministic Adaptive Naming v2, device-local workspace preference overlay, top-level import/export portability, and daemonless Live Control Center architecture.

ADR 0013 defines the v0.5.1/v0.6 boundaries: session-scoped Root Workspace ownership, count-only subagent observability, typed Hook/naming/title explainability, upgrade-safe runtime worker images, provider capability truthfulness, and real-environment-only Agy admission after public v0.5.1.

ADR 0014 constrains pre-admission Agy preparation to disposable,
content-minimal qualification machinery. Agy remains unavailable to ordinary
product registration until the later Owner-present G64 admission is accepted.

A later ADR may supersede an earlier decision, but implementation code must not silently reverse one.
