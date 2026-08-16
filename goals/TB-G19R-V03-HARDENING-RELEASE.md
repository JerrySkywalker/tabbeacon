# TB-G19R — v0.3 Hardening and Release

## Status

PLANNED. Final closure Goal for the Codex-only v0.3 track. No new feature work is admitted here.

## Purpose

Prove that title authority, 100 ms motion, convergence, and Codex compatibility operate together in the actual daily Windows Terminal workflow, then publish v0.3 from one exact accepted source.

## Required release scenarios

### Shell / terminal

```text
normal PowerShell
Administrator: PowerShell
application-title allowed profile
suppressed application-title profile classification
contention simulation / known external overwrite path
```

### Workspace

```text
Git repository
linked worktree
non-Git directory
HOME
workspace alias collision
```

### Concurrency

```text
1 active Codex tab
4 active Codex tabs
8 active Codex tabs
same-workspace parallel sessions
```

### Presentation

```text
ready
100 ms working animation
result-ready
approval
question where evidence supports it
static minimal profile
balanced profile
full spinner + WT ring profile
native fallback
```

### Lifecycle / failure

```text
fresh launch
resume
clear
compact
subagent isolation
newer-turn supersession
SessionEnd
terminal close
Codex disappearance
worker crash
missing binary
binary relocation/upgrade
settings changes
fail-open behavior
```

## Animation release contract

For the default balanced profile:

```text
DEFAULT_ACTIVITY=title-spinner
DEFAULT_SPINNER=braille
TARGET_FRAME_INTERVAL_MS=100
```

Trusted visible evidence must show at least three distinct valid working frames inside one second while the workspace alias remains stable.

Performance at 1/4/8 active tabs must remain bounded without a busy loop or worker leak.

## Title-authority release contract

A healthy title channel must prove visible convergence. The release may not claim title health solely because OSC writes succeeded.

Regression `TB-REG-TITLE-OWNERSHIP-001` must be closed for normal and elevated PowerShell or truthfully classified degraded in a deliberately unsupported environment. The intended product result is healthy authority in both supported PowerShell contexts.

## Exact-head evidence

Release gates that depend on source/presentation/provider behavior bind to one exact release candidate head.

At minimum:

```text
CODE_HEAD == VISUAL_HEAD == PROVIDER_HEAD == RELEASE_HEAD
```

Use the repository's risk-based governance during iteration, then full release closure at the final candidate.

## Required release gates

- repository policy/source review;
- full locked Rust/static/build suite;
- functional convergence matrix;
- trusted Windows Terminal Visual CI;
- real Codex smoke against each newly admitted production profile;
- title-authority visible probe;
- normal + Administrator PowerShell dogfood;
- 1/4/8 animated-tab performance evidence;
- package/dry-run/content inspection;
- Windows x64 artifact/checksum;
- public distribution verification.

## Release limitations

Do not silently add:

- global daemon;
- wrapper/PATH shadow;
- automatic shell-profile edits;
- unsupported Codex profile inheritance;
- Claude/OpenCode/App Server production work.

## Completion definition

```text
TB_G15=COMPLETE
TB_G16=COMPLETE
TB_G17=COMPLETE
TB_G18=COMPLETE
TB_G19=COMPLETE

DEFAULT_ACTIVITY=title-spinner
DEFAULT_SPINNER=braille
TARGET_FRAME_INTERVAL_MS=100
VISIBLE_WORKING_FRAMES_GE_3_WITHIN_1S=PASS
WORKSPACE_ALIAS_STABLE=true

NORMAL_POWERSHELL=PASS
ADMIN_POWERSHELL=PASS
TB_REG_TITLE_OWNERSHIP_001=CLOSED
MULTI_TAB_1_4_8=PASS

CODEX_PROFILE_POLICY=explicit
DAILY_COMMAND=codex
GLOBAL_DAEMON_INTRODUCED=false

V0_3_RELEASE=PASS
```

## Exit receipt

```text
GOAL_ID=TB-G19R
DISPOSITION=<PASS_RELEASED|FAIL|BLOCKED|UNPROVEN>
STARTING_MAIN=<sha>
RELEASE_HEAD=<sha>
VERSION=<v0.3.x>
CODE_CI=<...>
VISUAL=<...>
REAL_CODEX=<...>
TITLE_AUTHORITY=<...>
NORMAL_POWERSHELL=<...>
ADMIN_POWERSHELL=<...>
ANIMATION_100MS=<...>
MULTI_TAB_1_4_8=<...>
PACKAGE=<...>
PUBLICATION=<...>
OWNER_ACTION=<none-or-specific>
NEXT_GOAL=<maintenance|v0.3.1-candidate>
```
