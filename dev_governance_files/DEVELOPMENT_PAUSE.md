# Development pause after v0.7.1

```text
CURRENT_PUBLIC_TARGET=v0.7.1
CURRENT_PUBLIC_RELEASE=v0.7.1
TARGET_PUBLIC_RELEASE=v0.7.2
ACTIVE_FEATURE_DEVELOPMENT=PAUSED_EXCEPT_V072_HOTFIX
```

The Owner continues to pause broad TabBeacon feature development. A real Codex
subagent dogfood defect now admits one narrow production hotfix exception:

```text
v0.7.2 — Codex Subagent Hook Stability Hotfix
```

The hotfix roadmap is [`ROADMAP_V072.md`](ROADMAP_V072.md). Its only active
implementation sequence is TB-G103 through TB-G106.

## Active narrow exception

Allowed work:

- reproduce/classify the legacy MCP Hybrid subagent Hook failure;
- migrate exact-owned legacy TabBeacon MCP Hybrid integrations to the already
  admitted command-v1 Hook transport;
- prove preservation of third-party Hooks/MCP/config and manual Hook trust;
- run real Codex subagent qualification;
- harden and publicly release v0.7.2 after all gates pass.

This exception does not admit unrelated runtime/platform work.

## Promotion train is frozen, not discarded

The previously admitted Discoverability & Automated Demo train is retargeted to
**v0.7.3** and frozen until the hotfix closes.

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

## Final pause state

After successful public v0.7.2 closeout:

```text
CURRENT_PUBLIC_RELEASE=v0.7.2
CURRENT_PUBLIC_TARGET=v0.7.2
ACTIVE_FEATURE_DEVELOPMENT=PAUSED
PROMOTION_TARGET_RELEASE=v0.7.3
V073_IMPLEMENTATION=FROZEN
V08_OPTIONS_STATUS=NON_AUTHORITATIVE
ROADMAP_V08_CREATED=false
NEXT_RECOMMENDED_GOAL=DOGFOOD_OR_EXPLICIT_V073_RESUME
```

Future v0.7.3 implementation still requires a new explicit Owner admission
against then-current `main`. No automatic continuation from hotfix release into
promotion work is allowed.

```text
NEW_PROVIDER_ADDED=false
CLAUDE_PROVIDER=DEFERRED
OPENCODE_PROVIDER=DEFERRED
CODEX_APP_SERVER=DEFERRED
NATIVE_TAB_ICON_DISPOSITION=NO_GO
```
