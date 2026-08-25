# ADR 0017 — Provider Visual Identity v2

- Status: Accepted
- Date: 2026-08-25
- Goal: TB-V061-12H-FORWARD-COMPAT-VISUAL-001

## Context

The previous compact provider badge was concatenated to the workspace alias.
That made provider identity look like repository identity and left no explicit
terminal boundary for a future native tab icon. Runtime state, provider, and
workspace must stay independently understandable in title, accessibility, and
future terminal rendering.

## Decision

Use fixed `ProviderVisualIdentity` metadata selected by the provider registry.
Render it as an independent leading title slot through `TitleMarkBackend`:

```text
[Provider] [Runtime state] [Workspace]
```

The production backend reports native tab icons unavailable and falls back to
the fixed text identity. `TerminalVisualBackend` owns terminal capability
negotiation; a provider cannot imply that a native terminal feature exists.
Unknown provider IDs receive the fixed `Unknown` identity, never their raw
value. No icon files, paths, URLs, or executable metadata are accepted from a
provider.

## Consequences

- Provider switching cannot rewrite a workspace alias.
- Runtime state cannot change provider identity.
- Native icon research remains decoration-only and separately gated.
- Existing `provider_badge` settings remain visibility preferences and do not
  grant provider, trust, or compatibility authority.
