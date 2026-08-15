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

### TB-G01 core model

The core is intentionally open to new providers: `AgentProvider` is a checked,
opaque provider identifier rather than a closed enum. `AgentSessionKey` is the
pair of that identifier and a non-empty native session ID. Provider adapters
own any translation from their raw event types before they create these values.

An `AgentEvidence` carries the session key, a checked provider-neutral source
identity, authority, confidence, `SystemTime` observation time, a stable
tie-break key, and a `StatePatch`. A patch field is exactly one of
`Unchanged`, `Set(value)`, or `Clear`; clear resets only its own axis to the
documented neutral value (`Ready`, `None`, or `Normal`).

`BackendCapabilities` describes which of the three axes a backend may assert
at which authority levels. It is a declaration, not proof of a state change.
No backend implementation or provider event name is represented in the core.

Reconciliation keeps one independently provenanced winner per axis. A
candidate cannot replace a winner when it is older or lower authority. Among
candidates that are neither older nor weaker, the winner is selected in this
order: newer observation time, higher authority, higher confidence,
lexicographically greater normalized source identity, lexicographically
greater tie-break key, then the ordered patch action/value. The final patch
ordering handles a malformed duplicate tie-break without relying on arrival or
collection iteration order. The core is deterministic for a given normalized
evidence stream; adapters remain responsible for providing stable timestamps
and tie-break keys.

## 7. Session identity

Canonical session identity is:

```text
provider + native session id
```

PID, process start time, cwd, repository, thread title, and terminal tab are metadata/binding data, not the durable primary key.

## 8. Repository identity

Repository identity is local and offline-first.

Preferred evidence order:

1. a usable locally configured `origin` URL;
2. another usable local remote, ordered by remote name and URL;
3. a digest of the repository's sorted local root commits;
4. for an unborn repository only, a digest of the local Git common-dir path.

Discovery uses only a closed set of local Git metadata commands and supports
ordinary repositories and linked worktrees. Common HTTPS, SSH URL, and SCP-like
SSH forms normalize to a scheme- and user-neutral host/path key without DNS or
provider access. A remote-backed reclone therefore retains identity; a
committed originless repository retains identity across a move. An unborn,
originless repository has no content identity yet, so its fallback is path-local
until stronger evidence appears.

The abbreviation policy tokenizes separator and camel-case boundaries, then
emits deterministic readable expansions followed by stable hash candidates.
The machine-local alias registry serializes first assignment under a process
lock and atomically publishes digest-named immutable generations. Existing
assignments never change merely because a new collision appears. Generated
state lives in the per-user TabBeacon application-state directory, never in the
repository or dotfiles.

## 9. Presentation contract

The reconciled session and repository identity produce a typed `VisualState` containing only presentation-safe data such as:

```text
repository_alias
title_status
progress_kind
tab_color
reset_policy
```

Raw provider strings must never be concatenated directly into terminal control sequences.

### TB-G02 presentation model

The presentation policy accepts a `SemanticPresentationInput`: the orthogonal
`Phase`/`Attention`/`Health` values plus one stable repository alias. A helper
creates this input from a `SessionSnapshot`; deterministic fixtures construct it
directly without a provider. The policy produces either
`PresentationAction::Apply` with a typed `VisualState`, or
`PresentationAction::Reset` for ordinary ended sessions. Failed, interrupted,
warning, and attention evidence retain the normative priority over an ended
phase; an ended session with no higher-priority semantic state resets to the
neutral status slot.

`VisualState` keeps a control-free `TitleIdentity` derived from the resolved
repository alias, typed `TitleStatus`, semantic tab color, and progress
semantic as separate fields. The Windows
Terminal renderer alone maps `TitleStatus` and typed activity settings to a
glyph/frame and composes the default grammar `<status-slot> <repository-alias>`.
The provider and core never carry terminal glyphs. `Question` and `Approval`
remain distinct semantic colors even when the default palette intentionally
maps both to yellow. `Reset` explicitly clears progress and the dynamic frame
color while rendering the neutral status-first title.

The final title type replaces all Unicode control characters and limits the
composed output to a fixed number of Unicode scalar values, using an ellipsis
for truncation. Only this sanitized type reaches the encoder. The Windows
Terminal renderer uses static OSC/CSI envelopes with ST terminators: OSC `0`
for title, OSC `9;4` for progress, and OSC `4`/`104` at frame-background
color-table index `264` for dynamic tab/frame color. The frame-color sequence
is capability gated: without that capability, the exact title and progress
bytes remain and only color bytes are omitted.

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

### TB-G05 hook backend

The production hook adapter reads one bounded JSON object, retains only the
event name, session ID, cwd, and stable ordering material, and produces
provider-neutral lifecycle evidence. It declares lifecycle authority for phase
and attention and no health authority. Prompt text, tool input/output, and
assistant content are neither persisted nor used in titles.

The admitted Codex `0.147.0` release requires synchronous command hooks. The
user-global declarations therefore use its minimum one-second timeout and a
Windows shell fail-open suffix; only Codex's explicit exit-code-2 contract can
block an operation, while TabBeacon's command always resolves to success.

Each supported event supplies a complete transition for a one-shot reconciler.
Compact deliberately produces no presentation write. The hook runtime passes a
resolved G04 alias and the reconciled semantic snapshot to presentation as
separate inputs. Presentation selects the left status slot and composes the
status-first title before writing VT through the Windows console handle rather
than hook stdout, which Codex captures.

Configuration management is separate from event normalization. Typed
presentation preferences live under the per-user TabBeacon state root, are
read fresh by one-shot hook invocations, and fall back safely when absent or
malformed. The presentation renderer receives semantic state plus these
preferences, then independently decides whether to emit title, dynamic frame
color, and Windows Terminal progress. Themes resolve semantic `TabColor` only
at the renderer boundary; provider adapters never carry RGB choices.

Codex integration preserves unrelated global hooks and TOML, atomically
replaces changed files, and records exact backups and ownership locally. When
`title=tabbeacon`, it owns the supported `[tui].terminal_title = []` setting.
When `title=native` or `title=off`, it restores the pre-install title setting
instead of leaving both title systems disabled. Hook trust remains exclusively
in the supported Codex review flow.

## 12. TB-G03 visual verification infrastructure

Visual verification is test infrastructure above the presentation system under
test, not a provider or terminal-control feature:

```text
G02 deterministic fixture
        ↓
FixtureDriver (unique safe test title)
        ↓
TerminalTestSession (owned WT window)
        ↓
TargetLocator (read-only UIA)
        ↓
CaptureBackend (owned-window pixels)
        ↓
VisualOracle (title/color/animation)
        ↓
EvidenceWriter
```

The G03 fixture child renders the existing typed G02 action through the
production renderer, holds the visual state for a bounded interval, then emits
the G02 reset action before it exits. The launcher intentionally does not pass
a Windows Terminal `--title` override: UIA can resolve the unique token only
after the production G02 VT bytes set it. The launcher neither controls
existing Terminal windows nor closes a process it cannot prove it owns.

UIA supplies the semantic tab-title assertion and target geometry. The first
capture backend uses a safe Windows GDI screenshot adapter over the UIA-owned
window rectangle, so it is deliberately visibility-dependent: it runs only
after UIA reports that the unique test window is keyboard-focused. Loss of that
precondition is a classified visual-environment blocker, never an inferred
presentation failure. The pure oracle, ROI selection, fixed-point color
metrics, animation deltas, evidence model, and exact-head check have no UIA
or provider dependency.
