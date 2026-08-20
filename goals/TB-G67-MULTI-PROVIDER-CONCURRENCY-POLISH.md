# TB-G67 — Multi-Provider Concurrency & Polish

## Status

PLANNED after accepted G66.

## Purpose

Prove that Codex and Agy can coexist concurrently without cross-provider identity, session, worker, Hook, title, or configuration contamination, then finish the multi-provider Human UX before release.

## Concurrency matrix

Required real scenarios include:

```text
same repo:
  Codex tab
  Agy tab

different repos:
  multiple Codex tabs
  multiple Agy tabs
mixed:
  Codex + Agy + subagents/background tasks
resume/restart/rebind cases
```

## Namespace isolation

Provider/session/turn/generation/worker state must be namespaced strongly enough that:

- Agy cannot update a Codex terminal binding;
- Codex cannot update an Agy terminal binding;
- one provider's root workspace anchor cannot replace another provider's anchor;
- stale worker/lease/generation from one provider cannot suppress or overwrite another;
- provider-specific configuration ownership remains separate;
- shared StableAliasRegistry and WorkspacePreferenceStore remain correctly shared by workspace identity, not provider session.

## Provider badge auto semantics

Finalize `auto` behavior using actual concurrent evidence.

Preferred principle:

- no badge when a title is unambiguous and single-provider simplicity is preserved;
- badge when concurrent providers for the same effective workspace or otherwise ambiguous title identity make it useful;
- `always` forces provider badge;
- `off` suppresses it even in ambiguity, with explainability warning/description rather than hidden mutation.

Do not require constant global scans per title frame. Use bounded local session/lease projections.

## Sessions / Integrations UX

Sessions must make provider identity and root workspace stability easy to scan while remaining content-minimal. Integrations should show each provider independently and summarize overall attention without conflating unsupported capability with unhealthy capability.

## Subagents/background work

Count-only observations from Codex/Agy should coexist without becoming root title authority. If one provider does not prove background-task counts, show unavailable rather than zero when zero would falsely imply authoritative absence.

## Hook inventory

G60 Hook screen should present provider grouping cleanly when both providers use/declare Hooks. Providers using a non-Hook backend should not generate fake Hook entries.

## Why this title?

Explainability must correctly identify which provider/session/root anchor currently owns each title and why provider badge is shown or hidden.

## Resource bounds

Multi-provider support must not introduce:

- global polling daemon;
- unbounded filesystem scans;
- continuous transcript/log parsing;
- network polling merely for TabBeacon presentation;
- per-frame Git/provider subprocess calls.

Idle overhead remains negligible and local.

## Testing

Required families:

- same-workspace Codex+Agy concurrent titles;
- different-workspace concurrent titles;
- provider badge auto/always/off;
- worker image namespace isolation;
- Root Workspace Anchor isolation;
- session/generation stale-event isolation;
- shared alias override semantics;
- provider config ownership isolation;
- mixed Hook inventory;
- unsupported capability rendering;
- bilingual/narrow/no-color TUI;
- terminal close/restart/resume;
- one provider missing/broken does not degrade the other's functionality;
- real Windows Terminal multi-tab smoke.

## Risk vector

```text
CODE_CHANGED=true
PRESENTATION_CHANGED=true
PROVIDER_CHANGED=true
USER_PERSISTENT_CONFIG_CHANGED=true
SECURITY_OR_PRIVACY_CHANGED=true
RELEASE_BOUNDARY=false
```

Use representative real multi-provider/WT acceptance and independent privacy/ownership review.

## Acceptance

```text
CODEX_AGY_CONCURRENT=PASS
PROVIDER_NAMESPACE_ISOLATED=true
ROOT_ANCHOR_CROSS_PROVIDER_CONTAMINATION=false
WORKER_CROSS_PROVIDER_CONTAMINATION=false
PROVIDER_CONFIG_CROSS_MUTATION=false
SHARED_ALIAS_REGISTRY=PASS
PROVIDER_BADGE_AUTO=PASS
PROVIDER_BADGE_ALWAYS=PASS
PROVIDER_BADGE_OFF=PASS
SESSIONS_MULTI_PROVIDER=PASS
INTEGRATIONS_MULTI_PROVIDER=PASS
HOOKS_MULTI_PROVIDER=PASS_OR_NOT_APPLICABLE
WHY_THIS_TITLE_MULTI_PROVIDER=PASS
REAL_WT_MULTI_TAB_SMOKE=PASS
PRIVACY_REVIEW=PASS
CODE_CI=PASS
```

## Estimated effort

**5–8 effective engineering hours.**

## Next

`TB-G67R — v0.6.0 Hardening & Release`.