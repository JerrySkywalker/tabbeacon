# Product principles

TabBeacon is a small, explainable presentation layer for coding-agent tabs.
These principles describe its public behavior; the repository governance files
remain the source for high-risk implementation procedures.

## Preserve the native workflow

- Daily provider commands remain literal `codex` and `agy`.
- No launcher wrapper, fake executable, PATH shadow, PTY host, or global daemon
  is the baseline architecture.
- A presentation failure must not stop the underlying provider.

## Treat authority as evidence-bound

- The core receives provider-neutral normalized evidence, not provider-specific
  event types.
- Compatibility claims require current, bounded evidence; version ordering is
  not authority.
- User preferences, provider integration state, Hook trust, and live runtime
  state are separate boundaries.
- Presentation never grants compatibility, trust, or configuration authority.

## Keep identity useful and private

- Workspace identity is offline-first; Git identity is a stable specialization.
- Normal presentation minimizes content and does not persist prompts, assistant
  content, or tool content.
- Read-only diagnostics expose bounded, explainable facts rather than secrets
  or raw environment data.

## Mutate only proven-owned state

- Setup, repair, import, uninstall, and process drain require exact ownership.
- Ambiguous configuration and processes are preserved.
- Hook trust remains a manual provider/Owner decision.

## Make visual state explainable

- Provider identity, runtime state, and workspace identity have distinct roles.
- Title, spinner, tab color, and Windows Terminal progress are presentation
  channels, not an authority system.
- `TitleMarkBackend` is the production terminal backend. Native tab icons are
  [NO_GO](native-tab-icon.md) under the accepted current-host safety evidence.
