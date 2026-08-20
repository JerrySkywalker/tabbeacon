# TabBeacon v0.5.1 / v0.6 Master Goal — Reliability, Explainability & Multi-Provider

## Status

PLANNED from public v0.5.0 baseline `eb4e2ec0132ef4fa116d2a44c163a135a7f7e40f`.

## Objective

Advance TabBeacon in two deliberately separated releases.

### v0.5.1 — Reliability & Explainability

Keep production scope Codex-only. Resolve real v0.5 dogfood defects and make presentation, identity, Hooks, naming, provider capabilities, sessions, and upgrade behavior inspectable and stable.

### v0.6.0 — Multi-Provider Reliability & Explainability

Only after v0.5.1 is public and the Owner has a real authenticated Agy environment, admit Agy as the second production provider. Preserve direct `codex` / `agy` commands, fail-open operation, provider-neutral core semantics, and strict capability truthfulness.

## Frozen user-observed inputs

The planning baseline includes these accepted findings:

1. Windows `cargo install --force` can fail at executable replacement while session-scoped TabBeacon workers are still running from the installed `tabbeacon.exe`. Real dogfood observed two owning TabBeacon processes; after stopping them, the executable became replaceable and the same install succeeded.
2. The Human `Full` presentation preset currently uses `ActivityMode::Both`, producing both a title spinner and the Windows Terminal progress ring. This is visually redundant for normal use.
3. In some subagent/tool-heavy Codex flows, the final tab workspace alias can shift to another directory/project. Explicit subagent events are already ignored, so the likely correctness boundary is root-workspace ownership: accepted root events currently resolve workspace from each event's `cwd`.
4. The Control Center needs a first-class Hook inventory surface rather than only aggregate Hook/trust status.
5. Workspace candidate ranking needs strategy, score, and component explanation rather than only ordered aliases.
6. A `Why this title?` surface is required so users can inspect provider, root workspace binding, alias source, semantic state, and presentation source without guessing.
7. Multi-provider management foundations (Integrations, capability matrix, provider-aware Sessions, provider badge, provider setup/probe registry) are useful before a second provider exists.
8. Agy should be the second production provider, but implementation must be delayed until the Owner can use a real authenticated Agy environment. No simulated admission is acceptable.

## Product pillars

### 1. Stable identity ownership

A terminal tab's root workspace must be session-scoped, not whichever `cwd` happened to appear on the latest root-visible provider event.

Conceptual model:

```text
provider session
  -> root workspace anchor
  -> canonical workspace identity
  -> effective alias
  -> stable title identity
```

Subagents, tool worktrees, nested directories, and background tasks may be observed, but they must not silently rebind the root tab identity.

### 2. Explainability

Every visible decision should be inspectable through typed, privacy-safe projections:

- Hook inventory / ownership / trust / currentness;
- Adaptive Naming strategy, score and component breakdown;
- current title provenance;
- provider capability claims;
- root workspace binding source;
- worker/runtime ownership relevant to upgrades.

### 3. Upgrade reliability

The installed CLI executable must not remain the long-lived worker image. v0.5.1 should introduce upgrade-safe runtime worker images so normal package replacement does not fail because old session workers keep the install binary mapped.

### 4. Provider-neutral management

The Control Center and machine interfaces should manage providers through common typed projections. Codex remains the only production provider in v0.5.1. Agy may be admitted only after G64 proves the real environment.

## Release boundary

### v0.5.1 dependency DAG

```text
G57 -> G58 -> G59 -> G60 -> G61 -> G62 -> G63 -> G63R
```

After G63R publishes v0.5.1, STOP. Do not continue to G64 unless a new Owner-present execution explicitly confirms a usable real Agy environment.

### v0.6 dependency DAG

```text
PUBLIC v0.5.1
  -> G64 real Agy admission spike
  -> G65 Agy provider foundation
  -> G66 Agy presentation/management parity
  -> G67 multi-provider concurrency/polish
  -> G67R public v0.6.0
```

## Agy hard boundary

Before G64:

```text
AGY_PROVIDER=false
AGY_PRODUCTION_CONFIG_MUTATED=false
AGY_LOGIN_ATTEMPTED_BY_UNATTENDED_TRAIN=false
AGY_CAPABILITIES_CLAIMED=false
```

G64 must use a real Owner-authenticated Agy CLI. If the environment cannot authenticate or official behavior cannot be observed, return `BLOCKED_OWNER_ENVIRONMENT`. Do not replace that evidence with mocks, docs-only assumptions, or guessed event semantics.

## Presentation policy decisions

- `ActivityMode::Both` remains a stable machine token for backward compatibility.
- Existing explicit user `both` settings are preserved.
- The ordinary Human `Full`/recommended surface must stop presenting dual animated activity as the desirable maximum setting.
- One primary activity channel is the default Human behavior; dual indicators are advanced/explicit.
- Provider badge defaults to `auto`: invisible for unambiguous single-provider use, visible when useful to disambiguate simultaneous providers.

## Hook Inspector privacy boundary

Hook inventory may expose safe fields such as provider, event, ownership, enabled state, trust/currentness, handler kind, timeout, source location class, and fingerprint.

Do not print arbitrary handler command lines by default. Third-party commands may contain private paths, arguments, or credentials. Any future reveal operation must be explicit and Human-only; machine defaults remain redacted.

## Naming explainability boundary

Expose the current deterministic Adaptive Naming score and its existing components. Do not make scoring weights user-editable in this track. Stable generated alias history and explicit local overrides remain authoritative.

## Subagent observability boundary

Allowed:

```text
active_subagent_count
background_task_count (only when provider evidence proves it)
root_binding_stable / mismatch observation
```

Forbidden:

```text
raw agent IDs in Human surfaces
prompt text
assistant text
tool input/output
persistent activity history
process kill/resume/focus control
```

## Upgrade-safe runtime worker boundary

A runtime worker image may be copied/published under the per-user TabBeacon runtime state root and bound to a binary/version/hash. It exists only to let a session/turn-scoped worker outlive the one-shot Hook invocation without locking the package-installed CLI executable.

It must not become:

- a global daemon;
- a self-update service;
- an installer replacement;
- a cross-user service;
- a remote control process.

Old runtime images may be garbage-collected only after no valid lease can require them and deletion is ownership-safe.

## Provider capability truthfulness

A provider exposes only what real evidence proves. Capability reporting must distinguish unsupported, unavailable, heuristic, lifecycle, and stronger authority when applicable. No provider parity claim may be achieved by converting absence of evidence into a guessed state.

## Global invariants

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

## Validation strategy

Use the repository Fast Lane / active QUALITY_GATES policy:

- focused local tests during iteration;
- one settled exact-head hosted CI gate for code candidates;
- representative real Windows Terminal proof when title/TUI/runtime behavior changes;
- focused persistent-config/ownership proof when user state changes;
- focused security/privacy review when Hook/session/provider inventory exposure changes;
- Agy L4/real-environment proof only after G64 admission;
- release-boundary proof only in G63R/G67R.

Do not repeat unrelated provider, visual, configuration, or historical matrices when their risk surface did not change.

## Non-goals

No Claude/OpenCode provider, Codex App Server production backend, cloud sync, web dashboard, process/session control, automatic Hook trust, repository-local settings, self-update, arbitrary Hook execution, editable naming weights, or Hook reliability-history analytics.

## Completion

v0.5.1 completes only when G57–G63 are accepted and G63R publishes public 0.5.1 with clean consumers.

v0.6.0 completes only when a real G64 Agy capability profile is accepted, G65–G67 are accepted, and G67R publishes public 0.6.0 with Codex-only, Agy-only, and concurrent-provider consumer evidence.