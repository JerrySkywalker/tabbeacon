# TB-G10 — Codex Hook Compatibility and Turn/Agent Awareness

## Status

IMPLEMENTATION CANDIDATE. Local deterministic coverage is implemented, but the
required isolated real-Codex L4 smoke is blocked until the Owner reviews the
new isolated eleven-event Hook set through Codex `/hooks`. This goal is not
COMPLETE and must not be merged while L4 remains blocked.

## Admission

```text
GOAL_ID=TB-G10
STARTING_HEAD=f130f2e1bd436da8c23c36bfc80445c904bdc229
BRANCH=agent/tb-g10-codex-hook-compatibility
CODEX_VERSION=0.147.0
HOOK_PROFILE=codex-hooks-rust-v0.147.0
UPSTREAM_TAG=rust-v0.147.0
UPSTREAM_COMMIT=be6e8eac029b183056b7e4402879f15d2c85f61b
TURN_AWARE=true
AGENT_AWARE=true
COMPACT_AWARE=true
UNKNOWN_EVENT_POLICY=ignore-fail-open
```

## Scope

- exact installed-release Hook capability/profile classification;
- content-minimal turn/generation admission across one-shot processes;
- stale prior-turn rejection before terminal output;
- root/thread-spawn subagent isolation;
- explicit `PreCompact`/`PostCompact` preservation;
- ownership-safe expansion to the exact eleven-event Hook surface;
- doctor reporting and user/architecture documentation.

## Acceptance

- a newer root prompt supersedes the prior turn;
- stale stop, working, permission, and prompt events cannot overwrite or revive
  the current generation;
- subagent start, activity, and stop cannot mutate root presentation;
- unknown events and missing optional metadata remain fail-open;
- prompt, assistant, and tool bodies do not affect or enter persistent state;
- unrelated notifier, plugin, same-event, and unknown Hook declarations survive
  setup, upgrade, and uninstall;
- local L0/L1/L2 and exact-head hosted CI pass;
- isolated real-Codex L4 observes admitted payload/profile after official
  `/hooks` trust review.

## L4 blocker

The isolated fixture has no trust entries. `tabbeacon doctor` reports all
eleven exact groups as `REVIEW_REQUIRED`, and a real `codex exec` invocation
without the prohibited trust-bypass flag produces no TabBeacon turn state.
Owner trust is intentionally not manufactured during the unattended train.

## Non-goals

- no real Owner Codex configuration, trust, notifier, or installation change;
- no G11 worker, daemon, wrapper, PATH shadow, PTY, or App Server work;
- no title, palette, VT byte, progress, or visual-fixture change;
- no health inference from tool output, exit codes, missing events, or logs.
