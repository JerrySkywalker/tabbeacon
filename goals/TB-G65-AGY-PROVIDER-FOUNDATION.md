# TB-G65 — Agy Provider Foundation

## Status

PLANNED only after accepted G64 real-environment capability profile.

## Purpose

Implement Agy as the second production provider through the narrowest real surface proven by G64 while preserving direct `agy` launch and fail-open operation.

## Daily workflow invariant

```text
DAILY_COMMAND=agy
WRAPPER=false
PATH_SHADOW=false
PTY_HOST=false
GLOBAL_DAEMON=false
```

Users continue to start Agy normally. TabBeacon may require one-time ownership-aware setup of Agy-supported configuration, but no launch interception is admitted.

## Provider adapter

Implement a provider module that translates the G64-frozen real surface into provider-neutral core evidence:

```text
Agy raw structured state / Hooks
  -> Agy normalizer
  -> AgentEvidence
  -> SessionReconciler
  -> Root Workspace Anchor
  -> PresentationPolicy
```

The exact module/backend naming follows the capability profile. Do not hard-code docs-era assumptions that G64 did not prove.

## Authority discipline

Map only proven fields:

- session/conversation identity;
- root workspace evidence;
- Phase lifecycle;
- Attention/Approval where proven;
- Health only if a source actually proves it;
- background/subagent counts only if content-minimal evidence exists.

Unsupported capability remains explicit `unavailable`/`unsupported` in management.

## Setup / ownership

Provide one-time ownership-aware setup/uninstall/doctor behavior for the selected Agy integration surface.

Requirements:

- preserve unrelated Agy settings/hooks/custom title/statusline configuration;
- exact backup/snapshot before mutation;
- refuse unsupported/ambiguous shapes;
- conditional/atomic write where possible;
- uninstall restores only exact TabBeacon-owned state;
- no automatic permission/trust escalation;
- do not modify workspace-local files by default when a user-global supported surface exists;
- if G64 proves only a workspace-local supported mechanism, explicit product/ADR approval is required before use because project-local TabBeacon state is normally forbidden.

## Root workspace

Use G59 Root Workspace Anchor. Agy dynamic tool/workspace observations must not cause title alias drift after root binding unless the provider profile proves an authorized rebind event.

## Hook/title callback privacy

Raw Agy payloads stop at the provider boundary. Do not persist or render transcript paths, tool call arguments, error text, prompt/assistant content, artifact contents, account identifiers, tokens, or arbitrary model output.

Retain only the minimum typed fields required for provider-neutral evidence, ordering, workspace binding, and approved counts.

## Fail-open

If TabBeacon callback/setup/runtime fails:

- Agy remains usable;
- do not block tool calls/model invocations merely to preserve TabBeacon UI;
- if an Agy Hook contract requires JSON output, return the neutral/pass-through response defined by the frozen profile;
- no error loop or repeated permission prompt caused by TabBeacon.

## Management integration

G62 Integrations / capability matrix / provider-aware Sessions should now register Agy as admitted only when setup/version/profile is actually supported.

Doctor/status should distinguish:

```text
installed + supported
available but not configured
known but unadmitted version
unsupported/unavailable
configuration drift
```

## Testing

Required families:

- frozen G64 fixture normalization;
- malformed/unknown state fail-open;
- session identity stability;
- root workspace binding and mismatch isolation;
- approval/attention mapping where proven;
- unsupported health remains unavailable;
- background count privacy;
- ownership-safe setup/idempotence/uninstall;
- preserve unrelated Agy settings/hooks/title/statusline config;
- non-content persistence assertions;
- real Agy smoke using Owner-authenticated environment;
- Agy remains usable with TabBeacon binary missing/failing;
- Codex provider regressions remain green without cross-provider state.

## Risk vector

```text
CODE_CHANGED=true
PRESENTATION_CHANGED=true
PROVIDER_CHANGED=true
USER_PERSISTENT_CONFIG_CHANGED=true
SECURITY_OR_PRIVACY_CHANGED=true
RELEASE_BOUNDARY=false
```

Use real-provider L4 acceptance, focused persistent-config safety proof, and independent privacy/ownership review.

## Acceptance

```text
AGY_PROVIDER=true
DAILY_COMMAND_AGY=agy
AGY_WRAPPER=false
AGY_SETUP_OWNERSHIP_SAFE=true
AGY_UNINSTALL_RESTORES=true
AGY_FAIL_OPEN=PASS
AGY_SESSION_IDENTITY=PASS
AGY_ROOT_WORKSPACE_ANCHOR=PASS
AGY_PHASE_MAPPING=PASS
AGY_APPROVAL_MAPPING=PASS_OR_UNSUPPORTED
AGY_HEALTH_NOT_FABRICATED=true
AGY_BACKGROUND_COUNT=PASS_OR_UNSUPPORTED
RAW_AGY_CONTENT_PERSISTED=false
INTEGRATIONS_AGY=PASS
SESSIONS_AGY=PASS
REAL_AGY_SMOKE=PASS
PRIVACY_REVIEW=PASS
CODE_CI=PASS
```

## Estimated effort

**8–12 effective engineering hours.**

## Next

`TB-G66 — Agy Presentation & Management Parity`.