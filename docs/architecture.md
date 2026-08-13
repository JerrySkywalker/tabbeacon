# System Architecture

## 1. Architectural objective

TabBeacon converts heterogeneous coding-agent lifecycle signals into a small, stable terminal presentation contract.

The architecture deliberately separates:

1. **provider integrations** — how Codex/Claude/OpenCode expose events;
2. **normalization** — how raw events become provider-neutral evidence;
3. **reconciliation** — how competing/fresh evidence becomes one session snapshot;
4. **repository identity** — how a working directory maps to a stable short key;
5. **presentation policy** — how semantic state maps to title/progress/color;
6. **terminal backend** — how the visual state is encoded for Windows Terminal.

## 2. Provider and backend model

A provider may have more than one backend.

```text
Codex
  hooks        (production v0.1)
  app-server   (experimental)

Claude
  hooks        (future)
  richer source(s) only if required

OpenCode
  plugin       (future first backend)
  SSE          (future enhanced backend)
```

A backend is a source of observations, not the owner of global UI state.

## 3. Ingress shapes

Ingress is intentionally not forced into one transport abstraction.

- **One-shot ingress:** a hook launches `tabbeacon` for one event, passes structured input, then exits.
- **Streaming ingress:** a long-lived API/SSE/WebSocket source yields a stream of events.

Both ingress shapes end at the same normalizer boundary.

## 4. Provider-neutral evidence

Conceptual model (exact Rust types land in `TB-G01`):

```text
AgentSessionKey
  provider
  native_session_id

AgentEvidence
  session
  source
  authority
  observed_at
  patch

StatePatch
  phase?
  attention?
  health?
  metadata?
```

Authority must distinguish at least:

- authoritative runtime state;
- lifecycle/provider-emitted evidence;
- heuristic observation.

A heuristic may not silently overrule fresher authoritative evidence.

## 5. Semantic state axes

State is not a single flat enum.

```text
Phase
  Ready
  Working
  WaitingUser
  Ended

Attention
  None
  ResultReady
  Approval
  Question

Health
  Normal
  Warning
  Interrupted
  Failed
```

This permits states such as `Working + Warning` without pretending warning means the task has stopped.

A future stalled detector, if ever added, must remain explicitly heuristic and must not be represented as confirmed failure.

## 6. Reconciliation

The reconciler consumes evidence and creates a `SessionSnapshot`.

Rules to formalize in `TB-G01`:

- freshness matters;
- authority matters;
- source-specific evidence may be upgrade-only;
- explicit clears must not be confused with missing fields;
- stale attention must be retired only by evidence strong enough to contradict it;
- identical provider events must be idempotent;
- session identity must not use PID as the canonical key.

## 7. Session identity

Canonical session identity is:

```text
provider + native session id
```

PID, process start time, cwd, repository, thread title, and terminal tab are metadata/binding data, not the durable primary key.

## 8. Repository identity

Repository identity is local and offline-first.

Preferred evidence order:

1. local Git remote identity (for example normalized origin URL);
2. another suitable local remote;
3. local Git common-dir/root identity;
4. cwd basename fallback.

The abbreviation engine allocates a stable local key. Collision handling expands new assignments without unnecessarily renaming existing keys.

## 9. Presentation contract

The reconciled session and repository identity produce a typed `VisualState` containing only presentation-safe data such as:

```text
title
progress_kind
tab_color
attention_indicator
reset_policy
```

Raw provider strings must never be concatenated directly into terminal control sequences.

## 10. Windows Terminal backend

The first terminal backend uses terminal-native control sequences for:

- title;
- task/progress state;
- content-driven tab/frame color;
- reset.

The tab-color mechanism is treated as a Windows Terminal capability with graceful fallback; title/progress remain usable if a future Terminal version changes the color implementation.

## 11. Codex backend policy

v0.1 production integration uses global Codex hooks because it preserves direct `codex` launch and fail-open behavior.

The app-server backend is an experimental fidelity track. It may provide richer warning/failure/interruption semantics, but it must not become a hidden wrapper/remote-launch dependency without a later ADR and evidence that zero-workflow-change/fail-open invariants are preserved.
