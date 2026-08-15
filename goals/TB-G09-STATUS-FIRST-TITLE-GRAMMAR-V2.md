# TB-G09 — Status-First Title Grammar v2

## Status

Candidate implementation; acceptance evidence pending.

## Admission

```text
GOAL_ID=TB-G09
STARTING_HEAD=17926b7dd9369f080d96bc3c15d2a163258f443e
BRANCH=agent/tb-g09-status-first-title-v2
TITLE_GRAMMAR=<status-slot> <repository-alias>
STATUS_SLOT_POSITION=left
REPOSITORY_POSITION=right
DEFAULT_SEMANTIC_WORD_SUFFIXES=false
```

## Scope

- presentation-safe repository identity, semantic title status, and final title
  composition remain separate;
- presentation maps semantic state and typed activity settings to the left
  status slot;
- the Codex runtime passes only reconciled semantics and the resolved offline
  repository alias;
- deterministic presentation and visual fixtures prove compact status-first
  behavior, sanitization, truncation, ownership modes, spinner presets, and
  reset;
- user and architecture documentation describes the new default grammar.

## Acceptance

- `Ready` renders `○ OWH`;
- working spinner frames render as `<frame> OWH` and never move the alias;
- `ResultReady`, `Approval`, and `Question` render `✓ OWH`, `! OWH`, and
  `? OWH`;
- default titles contain no lifecycle-word suffix;
- title native/off and activity native/off semantics remain intact;
- offline alias collision/worktree behavior, tab color, progress, sanitization,
  bounds, and reset remain intact;
- L0/L1/L2 pass at `EXPECTED_HEAD`;
- trusted interactive L3 visual CI passes at the same exact head.

## Required validation

```text
pwsh -NoProfile -File ./scripts/ci/run-local-ci.ps1 -ExpectedHead <sha>
GitHub hosted CI at <sha>
Visual CI on tabbeacon-visual-zenbookduo at <sha>
```

## Non-goals

- no long-lived worker or production animation lifecycle (`TB-G11`);
- no Codex Hook profile/turn/agent change (`TB-G10`);
- no setup, trust, production install, App Server, Claude, or OpenCode change;
- no visual-runner infrastructure redesign.

## Evidence

```text
EXPECTED_HEAD=pending
CODE_HEAD=pending
VISUAL_HEAD=pending
LOCAL_VALIDATION=pending
CI=pending
VISUAL_CI=pending
```
