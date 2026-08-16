# TB-G10 — Codex Hook Compatibility and Turn/Agent Awareness

## Status

COMPLETE. The acceptance-candidate head passed local L0/L1/L2, exact-head
hosted CI, and fresh isolated real-Codex L4. Trust was granted only inside the
isolated recorder profile; the real Owner Codex profile was not changed and no
trust bypass was used. This status update is included in the final candidate
and therefore must receive the same final exact-head gates before merge.

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

## Owner L4 evidence

The Owner reviewed the eleven exact declarations through Codex `/hooks` and
granted trust only in the isolated recorder profile. The fail-closed verifier
then observed the required real `SessionStart`, `UserPromptSubmit`, `Stop`, and
`SessionEnd` lifecycle at the exact implementation head with content
minimization and candidate dispatch both passing:

```text
EXPECTED_HEAD=640e1ff1380c595148502f6eeaba8fc2bb983468
CANDIDATE_BINARY_SHA256=c4945f14fa877a6df7d8c707ef72f7c9036e1531c36f27b19898db3656edca39
SANITIZED_RECORD_COUNT=4
GENERATION_STATE_FILE_COUNT=1
CONTENT_MINIMIZATION=PASS
REQUIRED_REAL_CODEX_LIFECYCLE=PASS
EXACT_HEAD_CANDIDATE_DISPATCH=PASS
L4=PASS
OWNER_REAL_CODEX_CONFIG_MUTATED=false
TRUST_BYPASS_USED=false
```

The isolated fixture declares a five-second timeout. For `SessionEnd`, Codex
0.147.0 reported `clamping SessionEnd hook timeout to 3s`; the real Hook still
completed and the sanitized `SessionEnd` record was present. This is accepted
as observed installed-release behavior rather than a product failure.

## Acceptance basis before closure-status update

```text
EXPECTED_HEAD=96e336fdd7de6d6f8fb15730816d5494b5d26158
CODE_HEAD=96e336fdd7de6d6f8fb15730816d5494b5d26158
LOCAL_VALIDATION=PASS
CI=PASS
CI_RUN=31940268156
VISUAL_CI=N/A
FINAL_G10_L4=PASS
L4_CANDIDATE_BINARY_SHA256=db152c47c83f88c4b87a5cca67815a41f3dffda49014e9667896112b0dced680
OWNER_REAL_CODEX_CONFIG_MUTATED=false
TRUST_BYPASS_USED=false
```

The governed closeout maintains the final PR-head receipt externally because a
Git commit cannot embed its own SHA without changing that SHA.

## Non-goals

- no real Owner Codex configuration, trust, notifier, or installation change;
- no G11 worker, daemon, wrapper, PATH shadow, PTY, or App Server work;
- no title, palette, VT byte, progress, or visual-fixture change;
- no health inference from tool output, exit codes, missing events, or logs.
