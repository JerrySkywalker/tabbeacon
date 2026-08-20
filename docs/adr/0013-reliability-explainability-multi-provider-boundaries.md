# ADR 0013 — Reliability, Explainability & Multi-Provider Boundaries

## Status

Accepted for the v0.5.1/v0.6 planning track.

## Context

Public v0.5.0 established localized Human management, Adaptive Workspace Naming, local alias preferences, import/export portability, live Control Center, Sessions, help, and guided safe repair. Real post-release dogfood then exposed several boundaries that should be solved architecturally rather than with one-off patches:

1. long-lived Windows workers execute directly from the package-installed `tabbeacon.exe`, preventing package replacement while workers are active;
2. normal Human `Full` presentation selects two simultaneous animated activity channels;
3. root terminal workspace identity can still be recalculated from each accepted Hook event's `cwd`, allowing tool/subagent-related cwd changes to influence the visible alias even though explicit subagent lifecycle events are isolated;
4. Hook, naming, title and provider decisions are internally typed but not sufficiently inspectable in the Human UI;
5. the provider-neutral core is ready for a second provider, but production admission must be based on real provider behavior, not architecture alone;
6. Agy/Antigravity CLI is a promising second provider, but Owner authentication/environment availability is not guaranteed in all locations.

## Decision 1 — v0.5.1 precedes second-provider admission

TabBeacon will publish a Codex-only v0.5.1 Reliability & Explainability release before any Agy production work.

v0.5.1 owns:

- upgrade preflight and precise diagnostics;
- presentation-channel cleanup;
- Root Workspace Anchor;
- subagent/background count-only observability;
- Hook Inspector;
- naming score/title provenance explanation;
- provider-neutral Integrations/capability/session/badge foundations;
- upgrade-safe runtime worker images.

After public v0.5.1 there is a hard planning stop. Agy work requires a new Owner-present execution with a real authenticated environment.

## Decision 2 — root workspace identity is session-scoped authority

The root tab's workspace identity belongs to the provider session, not to the latest event cwd.

A typed Root Workspace Anchor is established only by provider events whose admitted profile has root-binding authority. Ordinary tool/lifecycle events may carry cwd observations but cannot silently rename the root tab.

Explicit subagent or subagent-attributed events never rebind the root workspace. Alternate cwd/workspace observations may be retained only as bounded content-minimal mismatch facts for diagnostics/explainability.

A later provider-specific profile may define authorized rebind events, but weakening the global invariant requires a later ADR.

## Decision 3 — subagent observability is count-only by default

TabBeacon may expose active subagent/background-task counts only when provider evidence proves them. It does not expose or persist raw agent/task IDs, prompt content, assistant content, tool content, or persistent activity history.

Subagent/background state is observational metadata and cannot become root workspace/title identity authority.

## Decision 4 — explanation is a typed product surface

Internal typed decisions should be projected into privacy-safe Human/machine views:

- Hook inventory, owner, enabled/trust/currentness state, handler kind, timeout and fingerprint;
- Adaptive Naming strategy, score and score components;
- root workspace binding source;
- automatic/override/effective alias provenance;
- provider capability/authority matrix;
- `Why this title?` semantic and presentation provenance;
- upgrade worker/runtime ownership relevant to replaceability.

Explainability does not grant mutation authority. Default Hook inspection redacts arbitrary command strings.

## Decision 5 — normal activity uses one primary animated channel

`ActivityMode::Both` remains a backward-compatible stable machine value. Existing explicit settings remain valid.

Normal Human presets should not encourage two simultaneous animated activity indicators. Dual indicators become advanced/explicit. No passive migration rewrites existing user settings.

## Decision 6 — long-lived workers run from immutable runtime images

The package-installed CLI executable remains the one-shot public entrypoint. Long-lived session/turn workers should execute from a per-user immutable version/content-hash-bound runtime image under TabBeacon state.

This separates package replacement from worker lifetime.

Runtime images:

- are generated from the trusted local TabBeacon release binary/package;
- are atomically published and content verified;
- are session/turn worker implementation artifacts, not a global daemon;
- may coexist across releases while valid workers remain;
- are garbage-collected only with ownership/lease proof;
- do not implement self-update or network download.

## Decision 7 — provider management is open and capability-based

Management/TUI uses a provider-neutral registry and capability profile. A provider is not considered production-supported merely because its identifier exists or the core could represent its states.

Capability status distinguishes supported/proven, unsupported, unavailable, and heuristic where admitted. Unsupported capability is not an error by itself.

Provider badges default to `auto` and are used for disambiguation without bloating ordinary single-provider titles.

## Decision 8 — Agy is admitted only after real-environment qualification

Agy is the intended second production provider for v0.6, but its adapter is not implemented before a real Owner-authenticated admission spike.

The spike rechecks current official behavior and proves actual session/workspace/lifecycle/title/Hook/config ownership semantics. Docs/source/mocks cannot substitute for real admission.

Backend preference is the narrowest structured direct-launch surface proven by the spike: structured title-state callback first if sufficient, Hooks where needed, hybrid only with explicit authority/reconciliation rules.

The daily command remains literally `agy`; wrappers, PATH shadowing, PTY hosts, TUI/transcript scraping, and global daemons are rejected.

## Decision 9 — v0.6 proves coexistence, not nominal adapter presence

v0.6 release requires real Codex-only, Agy-only, and concurrent Codex+Agy acceptance. Provider/session/root-anchor/worker/config namespaces must not cross-contaminate. Shared workspace alias registry/preferences remain shared by canonical workspace identity as intended.

## Consequences

Positive:

- Windows upgrades become compatible with active workers;
- subagent/tool workflows cannot silently rename the root tab;
- unusual titles and Hook/naming decisions become diagnosable;
- second-provider work reuses stable management/runtime foundations;
- Agy claims remain grounded in real behavior;
- direct CLI workflow and fail-open principles remain intact.

Costs:

- Root Workspace Anchor introduces bounded durable session binding state;
- runtime image ownership/GC is higher-risk implementation work;
- provider capability UI/schema becomes richer;
- Agy release timing depends on Owner environment availability;
- v0.6 intentionally spans two releases rather than maximizing features in one train.

## Rejected alternatives

### Keep resolving title identity from every event cwd

Rejected because tool/subagent cwd is observation metadata, not stable root-tab ownership.

### Fix Windows upgrades only by documenting `Stop-Process`

Rejected as the permanent design because normal package upgrades should not require killing healthy session workers solely due to executable mapping. A preflight remains useful as maintenance/fallback.

### Remove `ActivityMode::Both`

Rejected because it is an existing machine/persisted value. Preserve compatibility and change Human defaults instead.

### Add Agy immediately from docs/mocks

Rejected because provider capability and ownership semantics must be proven against a real authenticated CLI environment.

### Use an `agy` wrapper or TabBeacon PTY host

Rejected by zero-workflow-change/fail-open product invariants.

### Make naming weights user-editable

Rejected because it would undermine deterministic policy/history and portability; v0.6 exposes explanation, not policy scripting.

### Merge Hook reliability history/health statistics into TabBeacon

Rejected as product-scope drift. Hook inventory/current health belongs here; longitudinal reliability analytics belong to a separate observability product.