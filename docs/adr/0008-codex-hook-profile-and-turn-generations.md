# ADR 0008 — Exact Codex Hook Profile and Turn Generations

- Status: Accepted
- Date: 2026-08-16
- Goal: TB-G10
- Amends: ADR 0007
- Acceptance: TB-G10 isolated real-Codex L4 passed without trust bypass or
  real Owner profile mutation

## Context

An activity worker must never let an old turn or a thread-spawned subagent
overwrite the root tab's current presentation. The original Hook adapter
treated each one-shot process independently and admitted seven lifecycle
events. That was sufficient for static v0.1 presentation but did not retain a
cross-process turn generation.

The installed release is `codex-cli 0.147.0`. The official
`openai/codex` tag `rust-v0.147.0`, peeled to commit
`be6e8eac029b183056b7e4402879f15d2c85f61b`, proves eleven Hook event names,
required `turn_id` on turn-scoped events, optional thread-spawn subagent
identity on applicable events, explicit `SubagentStart`/`SubagentStop`, and
`PreCompact`/`PostCompact`. Later upstream source is not compatibility
evidence for this profile.

## Decision

Freeze production normalization to the exact profiles
`codex-hooks-rust-v0.147.0` and `codex-hooks-rust-v0.149.0`. The 0.149 source
audit found a new MCP handler family while retaining the bounded command Hook
contract. TabBeacon reconciles only its exact owned command declarations and
preserves external MCP groups unchanged. An unstudied Codex version has no
admitted profile; doctor reports failure rather than inheriting the contract
from a version floor.

The owned user Hook surface is the exact eleven-event release surface:

```text
PreToolUse       PermissionRequest  PostToolUse
PreCompact       PostCompact
SessionStart     SessionEnd         UserPromptSubmit
SubagentStart    SubagentStop       Stop
```

Unknown events are ignored fail-open. Root `PreCompact` and `PostCompact`
preserve presentation. Any explicit subagent lifecycle event, or an applicable
event carrying `agent_id` or `agent_type`, is ignored before root generation
state or terminal output can change.

`session_id` remains the durable session identity. A process-safe atomic ledger
under the TabBeacon state root stores only SHA-256 digests of session and turn
identifiers, a monotonically increasing local generation, the current turn,
and a bounded retired-turn set. It never stores cwd, prompt text, assistant
content, tool input/output, credentials, or arbitrary Hook payload fields.

Root `UserPromptSubmit` is the causal new-turn boundary. A new admitted turn
retires the prior turn and increments the local generation. Turn-scoped tool,
permission, compact, and stop events apply only when their `turn_id` matches
the current generation. One turn may be adopted mid-session if the prompt Hook
was absent during installation or a prior fail-open loss; after a current turn
exists, every different non-prompt turn is stale. A retired prompt cannot
revive activity. Root session start/end boundaries retire current turn state.

Generation-state corruption, lock failure, repository failure, terminal
failure, and missing TabBeacon binaries all lose decoration only. They never
return a blocking decision to Codex. No health state is inferred.

Setup replaces only manifest-proven TabBeacon groups and preserves unrelated
notifiers, plugins, multiple same-event groups, and unknown event declarations.
Changed or newly added groups remain inactive until the user reviews them in
Codex `/hooks`; TabBeacon never writes or bypasses trust.

## Consequences

- G11 can use the admitted local generation as its stale-worker boundary if
  its independent feasibility gate passes.
- Hook state remains small and content-minimal, but it is intentionally not a
  full session history.
- An unstudied Codex release is diagnostic failure even if its version number
  is newer.
- Upgrading from the seven-event v0.1 declaration set requires a fresh
  `/hooks` review for the exact eleven-event set.
- Presentation bytes and title grammar are unchanged by this decision; L3 is
  therefore not required for TB-G10.
