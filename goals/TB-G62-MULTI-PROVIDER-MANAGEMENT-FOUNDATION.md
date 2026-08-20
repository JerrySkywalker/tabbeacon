# TB-G62 — Multi-Provider Management Foundation

## Status

PLANNED after accepted G61. v0.5.1 remains Codex-only in production.

## Purpose

Generalize management/TUI/session projections so TabBeacon can represent more than one provider without admitting Agy yet. This Goal is architecture and UX preparation, not second-provider implementation.

## Integrations model

Replace Codex-specific top-level management assumptions with a provider-neutral registry/projection.

Conceptually:

```text
ProviderRegistry
  -> ProviderProbe
  -> ProviderCapabilityProfile
  -> ProviderIntegrationSnapshot
  -> ManagementSnapshot
```

Provider IDs remain checked open identifiers rather than a closed enum.

## Control Center

Rename/reshape `Codex Integration` into `Integrations / 集成` while preserving Codex detail. With only Codex installed the screen remains compact; it must not display fake Agy status before G64/G65.

A provider detail should be able to report:

```text
provider
installed/available
version
compatibility/admission state
observation backend
Hook/integration state where applicable
capability matrix
title ownership/presentation participation
manual actions
```

## Capability matrix

Expose proven capability/authority per provider, with explicit unavailable/unsupported states. At minimum model:

```text
Phase
Attention
Approval/question
Health
Session identity
Workspace/root binding
Subagents/background tasks
Title
WT progress/color/activity channels
Hook/integration inspectability
```

Do not claim capability solely because the provider-neutral core has a field. Capability belongs to actual admitted evidence.

## Provider-aware Sessions

Sessions rows gain provider identity, e.g.:

```text
TB — Codex — working — 52s
```

Provider identity is a safe product label, not the raw native session ID.

## Provider badge policy

Add a provider-neutral presentation preference:

```text
provider_badge = auto | always | off
```

Default `auto` should preserve compact single-provider titles and add disambiguation only when useful/necessary for simultaneous provider sessions.

Conceptual future grammar:

```text
⠋ TB       # unambiguous single provider
⠋ TB·C     # Codex when provider disambiguation active
⠋ TB·A     # Agy when admitted later
```

The exact badge glyph/token must be deterministic, sanitized, width-bounded and visually tested. G62 needs only Codex behavior and the provider-neutral policy; Agy badge proof belongs to G67.

## Setup / probe foundation

Guided Setup should discover registered/admitted providers through typed probes rather than hardcoding all future provider choices into presentation logic.

v0.5.1 registered production providers:

```text
codex
```

Agy must not be registered as production-ready before G65. If code includes a dormant provider ID/test fixture, it must not present itself as installed/supported to users.

## Hooks integration

G60 Hook inventory should plug into the provider integration model. Providers without Hooks may report `not_applicable` rather than fabricating a Hook surface.

## Testing

- Codex-only behavior remains compact and backward compatible;
- Integrations screen renders one provider cleanly;
- provider capability profile has explicit unsupported/unavailable semantics;
- Sessions include provider safely;
- provider badge auto/always/off round-trip and import/export;
- auto badge does not change ordinary unambiguous Codex title unexpectedly;
- setup provider registry is deterministic/offline;
- dormant/unadmitted provider cannot appear supported;
- JSON/plain schema changes are versioned/compatible as required;
- en-US/zh-CN/no-color/narrow TUI.

## Risk vector

```text
CODE_CHANGED=true
PRESENTATION_CHANGED=true
PROVIDER_CHANGED=true  # management architecture, not new provider
USER_PERSISTENT_CONFIG_CHANGED=true
SECURITY_OR_PRIVACY_CHANGED=true
RELEASE_BOUNDARY=false
```

Use focused config migration/ownership proof and privacy review. No real Agy call is admitted.

## Acceptance

```text
INTEGRATIONS_MODEL_PROVIDER_NEUTRAL=true
CODEX_INTEGRATION=PASS
PROVIDER_CAPABILITY_MATRIX=PASS
SESSIONS_PROVIDER_AWARE=true
PROVIDER_BADGE_POLICY=PASS
PROVIDER_BADGE_DEFAULT=auto
SETUP_PROVIDER_REGISTRY=true
AGY_PROVIDER=false
AGY_LOGIN_ATTEMPTED=false
JSON_MACHINE_CONTRACT_STABLE_OR_VERSIONED=true
PRIVACY_REVIEW=PASS
CODE_CI=PASS
```

## Estimated effort

**7–10 effective engineering hours.**

## Next

`TB-G63 — Upgrade-Safe Worker Runtime`.