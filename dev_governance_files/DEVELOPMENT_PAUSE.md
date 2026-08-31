# Development pause after v0.7.2

```text
CURRENT_PUBLIC_TARGET=v0.7.2
CURRENT_PUBLIC_RELEASE=v0.7.2
ACTIVE_FEATURE_DEVELOPMENT=PAUSED
PROMOTION_TARGET_RELEASE=v0.7.3
```

The Owner continues to pause TabBeacon feature development after the completed
Codex subagent Hook stability hotfix:

```text
v0.7.2 — Codex Subagent Hook Stability Hotfix
```

The completed hotfix roadmap is [`ROADMAP_V072.md`](ROADMAP_V072.md). TB-G103
through TB-G106 are retained as its historical execution sequence.

## Completed narrow exception

The narrow exception completed only the following work:

- reproduce/classify the legacy MCP Hybrid subagent Hook failure;
- migrate exact-owned legacy TabBeacon MCP Hybrid integrations to the already
  admitted command-v1 Hook transport;
- prove preservation of third-party Hooks/MCP/config and manual Hook trust;
- run real Codex subagent qualification;
- harden and publicly release v0.7.2 after all gates passed.

The exception is closed and does not admit unrelated runtime/platform work.

## Promotion train is frozen, not discarded

The previously admitted Discoverability & Automated Demo train is retargeted to
**v0.7.3** and remains frozen pending a new explicit Owner admission.

See [`ROADMAP_V073.md`](ROADMAP_V073.md).

```text
PROMO_PR=100
PROMO_PR_STATE=FROZEN_DRAFT
PR100_MERGE_ALLOWED=false
GITHUB_PROMO_METADATA_APPLIED=false
```

The existing remote PR #100 and any still-present local UIA recovery commit are
preserved as future v0.7.3 evidence. The hotfix must not merge, discard, or
silently rebase them as part of v0.7.2.

## Deferred, not active

`V08_OPTIONS.md` remains **NON_AUTHORITATIVE** and `ROADMAP_V08.md` remains
**NOT_CREATED**. None of the following is active:

- Operational Reliability v2;
- Provider Platform v2;
- Multi-Agent Presentation UX v3;
- Distribution / Terminal Reach beyond the frozen v0.7.3 promotion train;
- Windows Terminal upstream Native Icon experiment;
- Claude provider;
- OpenCode provider;
- Codex App Server.

Native Windows Terminal tab icons remain `NO_GO`; this hotfix does not reopen
XAML Diagnostics, process attachment, or native-icon mutation.

## Current pause state

The public v0.7.2 closeout is complete:

```text
CURRENT_PUBLIC_RELEASE=v0.7.2
CURRENT_PUBLIC_TARGET=v0.7.2
ACTIVE_FEATURE_DEVELOPMENT=PAUSED
PROMOTION_TARGET_RELEASE=v0.7.3
V073_IMPLEMENTATION=FROZEN
V08_OPTIONS_STATUS=NON_AUTHORITATIVE
ROADMAP_V08_CREATED=false
NEXT_RECOMMENDED_GOAL=DOGFOOD_ONLY_NO_ACTIVE_DEVELOPMENT
```

Future v0.7.3 implementation still requires a new explicit Owner admission
against then-current `main`. No automatic continuation from hotfix release into
promotion work is allowed.

Publication and disposable consumers do not authorize Owner production
adoption. The v0.7.2 transaction preserved that boundary:

```text
OWNER_PRODUCTION_NON_ADOPTION_AUTHORIZATION=EXPLICIT
OWNER_OFFICIAL_CHANNEL_CUTOVER=BLOCKED
OWNER_OFFICIAL_CHANNEL_CUTOVER_REASON=NOT_AUTHORIZED
PRODUCTION_CODEX_CONFIGURATION_MUTATED=false
PRODUCTION_HOOK_TRUST_MUTATED=false
PRODUCTION_AGY_CONFIGURATION_MUTATED=false
```

```text
NEW_PROVIDER_ADDED=false
CLAUDE_PROVIDER=DEFERRED
OPENCODE_PROVIDER=DEFERRED
CODEX_APP_SERVER=DEFERRED
NATIVE_TAB_ICON_DISPOSITION=NO_GO
```
