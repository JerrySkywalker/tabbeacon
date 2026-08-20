# TB-G66 — Agy Presentation & Management Parity

## Status

PLANNED after accepted G65.

## Purpose

Bring the admitted Agy provider into the same terminal-management experience as Codex where G64/G65 proved equivalent capability, while remaining explicit where parity is impossible.

## Presentation parity

Prove and implement the maximum safe subset of:

```text
title identity/status
tab/frame semantic color
Windows Terminal progress ring
session-scoped title animation
result-ready / approval attention
reset/exit behavior
```

The selected Agy backend may require returning a plain title string on stdout. If so, preserve that protocol exactly and use direct terminal output only where G64 proved it does not corrupt Agy behavior.

If a presentation channel cannot be supported safely, report it as unsupported and omit it. Do not add wrappers or TUI scraping to force parity.

## Activity worker

If Agy animation is feasible, reuse G63 upgrade-safe runtime worker images and provider/session/turn namespace isolation. Do not create a second provider-specific global worker architecture.

Agy workers must not lock the package-installed CLI executable and must not write to Codex-bound terminal sessions.

## Provider badge

Complete real Agy badge behavior under G62 policy:

```text
provider_badge=auto|always|off
```

Example when disambiguation is active:

```text
⠋ TB·A
```

The exact badge token must be stable, width-safe, localized only in explanatory UI (not in machine token), and collision-safe with the title grammar.

## Integrations / capability matrix

Control Center must show Codex and Agy as independent providers with truthful capability rows. Unsupported Agy rows remain visible as unsupported/unavailable rather than disappearing ambiguously.

## Sessions

Agy sessions use the same privacy-safe projection:

```text
workspace alias
provider
semantic state
age/recency
worker health
subagent/background count when proven
```

No raw conversation ID, transcript path, model prompt, tool data, or task IDs.

## Hooks screen

If Agy Hooks are part of the production backend or are safely inspectable, expose them through the provider-neutral G60 Hook inventory. If the title-state command is the only backend and Hooks are not configured, Hook capability should show `not_applicable`/`not_configured` truthfully.

Do not claim TabBeacon ownership of unrelated Agy Hooks.

## Setup / TUI

Guided Setup and Integrations should allow configuring an admitted Agy integration using the same Preview/Apply/ownership semantics as other persistent changes.

No internal enum typing for normal use.

## Explainability

`Why this title?` must work for Agy, showing safe provider/root workspace/state/presentation provenance. Workspace score explanation remains provider-neutral.

## Testing

- title callback/protocol exactness;
- real Windows Terminal title behavior;
- WT color/ring proof if admitted;
- Agy animation worker lifecycle if admitted;
- provider badge auto/always/off;
- Agy Integrations/capability rows;
- Agy Sessions privacy;
- Agy Why-this-title;
- Hook screen correct not-applicable/configured semantics;
- bilingual/no-color/narrow TUI;
- Agy settings Apply/Revert/Cancel ownership safety;
- no Codex regressions;
- real Agy smoke across ready/working/approval/result/exit states that G64 profile supports.

## Risk vector

```text
CODE_CHANGED=true
PRESENTATION_CHANGED=true
PROVIDER_CHANGED=true
USER_PERSISTENT_CONFIG_CHANGED=true
SECURITY_OR_PRIVACY_CHANGED=true
RELEASE_BOUNDARY=false
```

Use one representative real Agy + real Windows Terminal presentation pack, focused config ownership proof, and privacy review.

## Acceptance

```text
AGY_TITLE=PASS
AGY_TAB_COLOR=PASS_OR_UNSUPPORTED
AGY_WT_RING=PASS_OR_UNSUPPORTED
AGY_ANIMATION=PASS_OR_UNSUPPORTED
AGY_PROVIDER_BADGE=PASS
AGY_INTEGRATIONS=PASS
AGY_CAPABILITY_MATRIX=PASS
AGY_SESSIONS=PASS
AGY_HOOK_INVENTORY=PASS_OR_NOT_APPLICABLE
AGY_WHY_THIS_TITLE=PASS
AGY_SETUP_TUI=PASS
ZH_CN_AGY=PASS
EN_US_AGY=PASS
REAL_AGY_WINDOWS_TERMINAL_SMOKE=PASS
PRIVACY_REVIEW=PASS
CODE_CI=PASS
```

## Estimated effort

**7–11 effective engineering hours.**

## Next

`TB-G67 — Multi-Provider Concurrency & Polish`.