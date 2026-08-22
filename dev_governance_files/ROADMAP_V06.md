# TabBeacon v0.5.1 → v0.6 execution roadmap

## Status

ACTIVE from public v0.5.1. The next admitted maintenance train is v0.5.2
High-Performance Codex Runtime; it is deliberately bounded before the G64 Agy
hard stop.

This file is the compact long-train entrypoint for the next production stages. Product intent, invariants, release boundaries, and the Agy hard stop are defined in [`../goals/TB-V06-RELIABILITY-EXPLAINABILITY-MULTI-PROVIDER.md`](../goals/TB-V06-RELIABILITY-EXPLAINABILITY-MULTI-PROVIDER.md). Each numbered Goal below owns its exact implementation and exit gates.

## Product direction

Two release stages are intentionally separated:

1. **v0.5.1 — Reliability & Explainability**: remain Codex-only while resolving real dogfood defects, stabilizing root workspace identity, making Hooks/naming/title decisions inspectable, preparing provider-neutral management, and eliminating the Windows worker executable-lock upgrade defect.
2. **v0.6.0 — Multi-Provider Reliability & Explainability**: only after public v0.5.1 and an Owner-available real Agy login environment, admit Agy as the second production provider and prove Codex/Agy coexistence.

Agy implementation is deliberately last. No unattended train before the v0.5.1 release may create, simulate, or claim production Agy support.

## Dependency sequence

```text
v0.5.0 RELEASED
    ↓
TB-G57   Dogfood Maintenance & Upgrade Preflight
    ↓
TB-G58   Presentation Channel Cleanup
    ↓
TB-G59   Root Workspace Anchor & Subagent Observability
    ↓
TB-G60   Hook Inspector & Trust Explainability
    ↓
TB-G61   Naming / Title Explainability
    ↓
TB-G62   Multi-Provider Management Foundation
    ↓
TB-G63   Upgrade-Safe Worker Runtime
    ↓
TB-G63R  v0.5.1 Hardening & Release
    ↓
PUBLIC v0.5.1
    ↓
TB-V052  v0.5.2 High-Performance Codex Runtime
    ↓
PUBLIC v0.5.2 (only after its own release decision)
    ↓
HARD STOP: DOGFOOD / OWNER AGY ENVIRONMENT REQUIRED
    ↓
TB-G64   Agy Admission & Real-Environment Spike
    ↓
TB-G65   Agy Provider Foundation
    ↓
TB-G66   Agy Presentation & Management Parity
    ↓
TB-G67   Multi-Provider Concurrency & Polish
    ↓
TB-G67R  v0.6.0 Hardening & Release
```

Default execution is sequential. A long autonomous train may continue only after the predecessor is accepted/merged and the next Goal remains within its authority. `TB-G63R` is a mandatory train boundary: after public v0.5.1, stop unless the Owner is present and a real usable Agy environment is explicitly available.

## Goal index

| Goal | Scope | Estimated effort |
| --- | --- | ---: |
| G57 | Windows upgrade preflight/drain; executable-lock regression; trust wording precision; close stale Issue #45 after verification | 4–6 h |
| G58 | clean Human activity model; remove dual-spinner `Full` default; preserve legacy `both` machine compatibility | 4–6 h |
| G59 | session-scoped Root Workspace Anchor; prevent cwd/subagent alias drift; count-only subagent/background observability | 8–12 h |
| G60 | provider-neutral Hook inventory CLI/TUI; trust/currentness/ownership explanation; safe command redaction | 6–9 h |
| G61 | Workspace candidate score/strategy breakdown; `Why this title?`; root-binding/title provenance | 6–9 h |
| G62 | Integrations model; capability matrix; provider-aware Sessions; provider badge policy; setup/provider registry foundation | 7–10 h |
| G63 | version/hash-bound worker runtime images; handoff/GC; remove long-lived lock on install binary | 8–12 h |
| G63R | v0.5.1 upgrade/dogfood/release closure and public consumers | 4–6 h |
| **v0.5.1 subtotal** | **Codex-only Reliability & Explainability** | **47–70 h** |
| V052 | v0.5.2 High-Performance Codex Runtime: phase-attributed sub-second Windows Hook path; Fast Anchor Path; regression gates; Owner dogfood/re-enable packet | 8–12 h |
| G64 | real Agy environment admission spike; freeze proven capability profile | 3–5 h |
| G65 | production Agy provider adapter with direct `agy` daily command and fail-open setup | 8–12 h |
| G66 | Agy title/state/workspace/approval plus proven presentation and management parity | 7–11 h |
| G67 | Codex + Agy concurrency, provider badges, sessions/integration polish, namespace isolation | 5–8 h |
| G67R | v0.6.0 hardening/release/public consumers | 4–6 h |
| **Agy/v0.6 subtotal** | **second-provider production admission** | **27–42 h** |
| **Total** | **v0.5.1 through v0.6.0** | **74–112 h** |

## v0.5.1 admitted product outcomes

```text
UPGRADE_PREFLIGHT=true
WINDOWS_EXECUTABLE_LOCK_DIAGNOSABLE=true
FULL_PRESET_DUAL_ACTIVITY=false
LEGACY_ACTIVITY_BOTH_READABLE=true
ROOT_WORKSPACE_IS_SESSION_SCOPED=true
SUBAGENT_CANNOT_REBIND_ROOT=true
SUBAGENT_COUNT_OBSERVABLE=true
HOOK_INVENTORY=true
HOOK_TRUST_DIAGNOSTICS_PRECISE=true
WORKSPACE_SCORE_EXPLAINABLE=true
WHY_THIS_TITLE=true
INTEGRATIONS_MODEL=true
PROVIDER_CAPABILITY_MATRIX=true
SESSIONS_PROVIDER_AWARE=true
PROVIDER_BADGE_POLICY=true
UPGRADE_SAFE_WORKER_RUNTIME=true
AGY_PROVIDER=false
```

## v0.6 admitted product outcomes

```text
AGY_PROVIDER=true
DAILY_COMMAND_AGY=agy
AGY_REAL_ENVIRONMENT_PROVEN=true
CODEX_AGY_CONCURRENT=true
PROVIDER_CAPABILITY_MUST_BE_PROVEN=true
PROVIDER_NAMESPACE_ISOLATED=true
```

Unsupported Agy capabilities must be reported as unavailable/unsupported. No heuristic may be upgraded to authoritative state merely to claim parity.

## Cross-cutting invariants

```text
DAILY_COMMAND_CODEX=codex
DAILY_COMMAND_AGY=agy
FAIL_OPEN=true
NO_WRAPPER=true
NO_PATH_SHADOW=true
NO_PTY_HOST=true
GLOBAL_DAEMON_ADDED=false
SELF_UPDATE=false
HOOK_TRUST_BYPASS=false
AUTO_HOOK_TRUST=false
PROJECT_LOCAL_CONFIG=false
PROJECT_FILES_MUTATED_FOR_PREFERENCES=false
RAW_PROMPT_CONTENT_PERSISTED=false
RAW_ASSISTANT_CONTENT_PERSISTED=false
RAW_TOOL_CONTENT_PERSISTED=false
RAW_NATIVE_SESSION_IDS_HUMAN_EXPOSED=false
PROCESS_SESSION_CONTROL=false
REMOTE_CONTROL=false
ROOT_WORKSPACE_IS_SESSION_SCOPED=true
PROVIDER_CAPABILITY_MUST_BE_PROVEN=true
```

Long-lived worker runtime images are permitted only as session/turn-scoped fail-open implementation artifacts. They are not a machine-global daemon and are not a self-update mechanism.

## Explicitly deferred

Do not fold these into v0.5.1/v0.6 merely because time remains:

- Claude provider;
- OpenCode provider;
- Codex App Server production backend;
- web/remote dashboard;
- process kill/session resume/tab focus control;
- cloud sync;
- self-update;
- automatic Hook trust;
- repository-local TabBeacon settings;
- user-editable Adaptive Naming score weights;
- Hook reliability/history statistics (belongs to a separate health-observability product).

## Suggested 12h train partitions

```text
Train A: G57 + G58 + begin G59
Train B: close G59 + G60
Train C: G61 + G62
Train D: G63
Train E: G63R -> PUBLIC v0.5.1 -> HARD STOP

Maintenance exception: TB-V052 may run after public v0.5.1 and before G64. It
does not admit Agy and ends at a v0.5.2 release-candidate / Owner dogfood
transaction unless a separately authorized public-release Goal is accepted.

Owner/Agy environment becomes available

Train F: G64 + begin G65
Train G: close G65 + G66
Train H: G67 + G67R -> PUBLIC v0.6.0
```

Partitions are estimates. Risk gates and truthful Goal state remain authoritative.
