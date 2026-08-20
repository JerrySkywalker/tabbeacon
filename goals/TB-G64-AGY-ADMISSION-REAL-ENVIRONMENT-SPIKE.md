# TB-G64 — Agy Admission & Real-Environment Spike

## Status

PLANNED only after public v0.5.1 and an explicit Owner-confirmed real authenticated Agy environment.

## Purpose

Qualify Agy/Google Antigravity CLI as a candidate second production provider using real observed behavior before implementing a production adapter.

## Hard prerequisites

```text
V0_5_1_RELEASE=PASS
OWNER_PRESENT=true
AGY_AUTHENTICATED_REAL_ENVIRONMENT=true
```

If authentication, account eligibility, network/IP policy, or local environment prevents real Agy use, stop:

```text
DISPOSITION=BLOCKED_OWNER_ENVIRONMENT
```

Mocks, docs-only assumptions, source-only reasoning, or inferred behavior cannot replace the real admission spike.

## Official surfaces to verify

At planning time Antigravity documents:

- command Hooks around tool/model/stop lifecycle, with common metadata including conversation identity and workspace paths;
- a custom terminal-title command that receives structured state JSON whenever agent state changes;
- `/title` control and a custom title command configuration;
- `/hooks`, `/tasks`, and related management surfaces.

These are discovery inputs, not frozen production truth. G64 must re-check the then-current official docs/version and prove actual local behavior.

## Candidate backend order

Prefer the narrowest backend that provides reliable state while preserving direct `agy` launch:

1. **structured title-state command** if it supplies stable lifecycle/workspace/session state and allows TabBeacon to own terminal presentation safely;
2. **Agy Hooks** for lifecycle/tool/stop evidence when needed;
3. a hybrid only when each source has a clearly bounded authority and conflict policy.

Do not introduce a wrapper, PATH shadow, PTY host, global daemon, TUI scraping, transcript scraping, or hidden network interception.

## Real spike questions

Prove and record:

### Identity

- stable conversation/session identifier across one session and resume/fork behavior;
- workspace/project fields and whether they represent launch cwd, project root, mounted workspaces, or dynamic tool cwd;
- whether root workspace can be bound deterministically to G59 Root Workspace Anchor.

### Lifecycle

Observe actual transitions among states such as ready/idle, thinking/working/tool-use/initializing, user-approval pending, stop/completion, background tasks, errors/interruption where available.

Freeze only states actually observed/documented strongly enough for production authority.

### Title integration

Determine whether a TabBeacon title command can:

- receive structured JSON reliably;
- return the plain title string Agy expects;
- simultaneously use the existing owned-console/terminal backend for WT progress/color/activity without corrupting Agy stdout protocol;
- coexist with Agy's own `/title` lifecycle and settings ownership;
- fail open when TabBeacon is missing/broken.

### Hooks

If Hooks are needed, prove:

- supported Hook events and input/output contracts;
- timeout behavior;
- global vs workspace-specific configuration precedence;
- ownership-safe installation and uninstall options;
- whether Hook handlers can remain observational without altering Agy decisions;
- privacy boundary: do not consume transcript/tool content merely because the payload exposes paths/content-capable fields.

### Background tasks/subagents

Determine what count/status can be observed without raw task/agent identifiers or log scraping.

## Capability profile

Produce one frozen `AgyCapabilityProfile` covering:

```text
version/admission range
backend(s)
session identity
root workspace evidence
Phase authority
Attention/Approval authority
Health authority
subagent/background-task count support
title ownership
WT progress/color feasibility
animation-worker feasibility
Hook inventory feasibility
setup/uninstall ownership
unknown-event policy
fail-open behavior
```

Each capability must be classified as proven supported, unsupported, unavailable, or explicitly heuristic. No optimistic parity claims.

## Repository mutation

G64 may add fixtures/profile definitions/ADR evidence needed to freeze the qualification, but must not install production Agy integration into Owner state beyond disposable/explicitly approved spike configuration.

Any real Agy settings mutation requires Owner approval and exact backup/restore verification.

## Testing / evidence

- record exact Agy version/build observed;
- bounded real-session state trace with no prompt/tool content retained;
- title callback state samples minimized to approved fields;
- disposable setup/restore receipt if configuration is touched;
- fail-open test when TabBeacon callback fails;
- Windows Terminal ownership/presentation feasibility result;
- independent provider/privacy review.

## Risk vector

```text
CODE_CHANGED=possible
PRESENTATION_CHANGED=possible
PROVIDER_CHANGED=true
USER_PERSISTENT_CONFIG_CHANGED=possible
SECURITY_OR_PRIVACY_CHANGED=true
RELEASE_BOUNDARY=false
```

This is a real-provider admission gate; use the minimum required L4/provider acceptance. No production publication.

## Acceptance

```text
REAL_AGY_ENVIRONMENT=PASS
AGY_VERSION_RECORDED=true
AGY_SESSION_IDENTITY=PROVEN_OR_UNSUPPORTED
AGY_ROOT_WORKSPACE_EVIDENCE=PROVEN
AGY_LIFECYCLE_PROFILE=FROZEN
AGY_APPROVAL_EVIDENCE=PROVEN_OR_UNSUPPORTED
AGY_BACKGROUND_OBSERVABILITY=PROVEN_OR_UNSUPPORTED
AGY_TITLE_BACKEND_FEASIBILITY=PASS_OR_REJECTED
AGY_HOOK_BACKEND_FEASIBILITY=PASS_OR_NOT_REQUIRED
AGY_FAIL_OPEN=PASS
AGY_CAPABILITY_PROFILE=PASS
PRIVACY_REVIEW=PASS
OWNER_STATE_RESTORED=true
```

If no safe direct-launch backend is feasible, stop `BLOCKED/UNPROVEN`; do not replace the design with a wrapper.

## Estimated effort

**3–5 effective engineering hours once real Agy login is available.**

## Next

`TB-G65 — Agy Provider Foundation`.