# Development scope after v0.7.2

```text
CURRENT_PUBLIC_TARGET=v0.7.2
CURRENT_PUBLIC_RELEASE=v0.7.2
ACTIVE_FEATURE_DEVELOPMENT=PAUSED_EXCEPT_V073_PROMOTION
PROMOTION_TARGET_RELEASE=v0.7.3
```

The v0.7.2 Codex subagent Hook stability hotfix is publicly complete. The Owner
now admits one narrow post-v0.7.2 exception whose purpose is to finish the
public presentation/distribution surface before an extended dogfood pause:

```text
v0.7.3 — Discoverability, Demo & Distribution Polish
```

See [`ROADMAP_V073.md`](ROADMAP_V073.md) and
[`V073_DEVELOPMENT_CHECKLIST.md`](V073_DEVELOPMENT_CHECKLIST.md).

## Active narrow exception

Only the following work is active:

- re-admit/reconcile the previously frozen PR #100 discovery/promo evidence
  against post-v0.7.2 `main`;
- GitHub description/topics/social-preview work;
- deterministic real-Windows-Terminal promotional GIF/poster generation using
  exact-owned window capture and cleanup;
- README/crates.io/package-content polish;
- v0.7.3 hardening, release, public-consumer verification, and closeout.

```text
V073_IMPLEMENTATION=ADMITTED_AFTER_PLANNING_PR_MERGE
PROMO_PR=100
PROMO_PR_STATE=FROZEN_DRAFT_TO_RECONCILE
PR100_DIRECT_MERGE_ALLOWED=false_until_reconciled
TARGET_PUBLIC_RELEASE=v0.7.3
```

The historical PR number is not more important than safe source history. If
PR #100 cannot be reconciled cleanly against current `main`, preserve it as
historical evidence and create a clean successor Train-A PR.

## Product-positioning boundary

The public discovery position is Codex-led:

```text
POSITIONING_PRIMARY=CODEX_AND_MORE
CODEX_ACQUISITION_PRIORITY=true
```

`Codex and more` is a discovery phrase, not a compatibility wildcard. Current
support truth must remain explicit on nearby durable surfaces:

```text
CODEX_SUPPORT=production_capability_based
AGY_SUPPORT=production_exact_1.1.19
CLAUDE_SUPPORT=deferred
OPENCODE_SUPPORT=deferred
```

No deferred provider may be implied as partially supported.

## Runtime/provider freeze

v0.7.3 is not a runtime feature train.

```text
RUNTIME_BEHAVIOR_CHANGED=false_expected
PROVIDER_BEHAVIOR_CHANGED=false
NEW_PROVIDER_ADDED=false
HOOK_SEMANTICS_CHANGED=false_expected
HOOK_TIMEOUT_CHANGED=false
HOOK_TRUST_BOUNDARY_CHANGED=false
NATIVE_TAB_ICON_DISPOSITION=NO_GO
```

A newly discovered P0/P1 runtime defect may interrupt this train only through a
fresh, explicit hotfix admission. It must not be silently repaired inside
promotion/media work.

## Temporary Windows Terminal safety

The exact-owned lifecycle proven in v0.7.2 is mandatory for every active v0.7.3
visual/promo qualification path.

```text
EXACT_TABITEM_TO_ANCESTOR_HWND=true
NO_DESKTOP_CAPTURE=true
OWNER_WINDOWS_CLOSED=0
BROAD_WINDOWS_TERMINAL_KILL=false
OWNED_TEMP_WT_REMAINING=0_after_each_run
```

The retired top-level dynamic `Window.Name` assumption must not return.

## Deferred, not active

`V08_OPTIONS.md` remains **NON_AUTHORITATIVE** and `ROADMAP_V08.md` remains
**NOT_CREATED**. None of the following is admitted by v0.7.3:

- Operational Reliability v2;
- Provider Platform v2;
- Multi-Agent Presentation UX v3;
- Distribution / Terminal Reach beyond the bounded v0.7.3 package/public-surface
  work;
- Windows Terminal upstream Native Icon experiment;
- Claude provider;
- OpenCode provider;
- Codex App Server;
- Windows installer or TabBeacon Winget/Scoop package;
- PATH mutation or auto-update.

## Publication boundary

Implementation admission does not by itself authorize the irreversible public
v0.7.3 transaction. G102 must carry or obtain explicit Owner release
authorization before crates.io publication, tag creation, GitHub Release, or
public asset upload.

```text
PUBLIC_RELEASE_MUTATION=false_until_G102_authorized
PRODUCTION_CODEX_CONFIGURATION_MUTATED=false
PRODUCTION_HOOK_TRUST_MUTATED=false
PRODUCTION_AGY_CONFIGURATION_MUTATED=false
```

## Current active state

After this planning PR is accepted:

```text
CURRENT_PUBLIC_RELEASE=v0.7.2
CURRENT_PUBLIC_TARGET=v0.7.2
ACTIVE_FEATURE_DEVELOPMENT=PAUSED_EXCEPT_V073_PROMOTION
PROMOTION_TARGET_RELEASE=v0.7.3
V073_IMPLEMENTATION=ADMITTED
V08_OPTIONS_STATUS=NON_AUTHORITATIVE
ROADMAP_V08_CREATED=false
NEXT_RECOMMENDED_GOAL=TB-V073-12H-DISCOVERY-DEMO-DISTRIBUTION-001
```

After verified public v0.7.3 closeout, the intended state becomes:

```text
CURRENT_PUBLIC_RELEASE=v0.7.3
CURRENT_PUBLIC_TARGET=v0.7.3
ACTIVE_FEATURE_DEVELOPMENT=PAUSED
ROADMAP_V08_CREATED=false
NEXT_RECOMMENDED_GOAL=LONG_TERM_DOGFOOD_NO_ACTIVE_DEVELOPMENT
```

Recommended dogfood duration is four weeks minimum and six to eight weeks
preferred before deciding whether to create an authoritative v0.8 roadmap.
