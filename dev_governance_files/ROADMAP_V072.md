# TabBeacon v0.7.2 execution roadmap

## Status

**OWNER-ADMITTED HOTFIX ROADMAP** from the public `v0.7.1` dogfood baseline.
The current repository baseline before this planning transaction is
`ef47127a1fb957db515aac818f4577e1d34a5b83`.

A real dogfood defect now takes priority over the previously admitted
Discoverability & Automated Demo maintenance train. The former v0.7.2
promotion scope is retargeted to **v0.7.3** and frozen while this hotfix is
implemented and released.

```text
CURRENT_PUBLIC_RELEASE=v0.7.1
TARGET_PUBLIC_RELEASE=v0.7.2
V072_THEME=CODEX_SUBAGENT_HOOK_STABILITY_HOTFIX
PROMOTION_TARGET_RELEASE=v0.7.3
PROMO_PR=100
PROMO_PR_STATE=FROZEN_DRAFT
ROADMAP_V08_CREATED=false
```

## Product theme

**v0.7.2 — Codex Subagent Hook Stability Hotfix**

The release fixes a production dogfood regression in upgraded installations
that still carry the legacy `codex-hooks-mcp-hybrid-v1` transport. When Codex
subagents execute tools, Hook delivery can fail before TabBeacon receives the
payload, producing visible errors such as:

```text
PreToolUse hook (failed)
error: tool call failed for `tabbeacon-hook/tabbeacon_hook_event`
```

The hotfix does not add a new Hook design. It converges exact-owned legacy MCP
Hybrid installations onto the conservative `codex-hooks-command-v1` transport
already used for new compatible installations since v0.6.1.

## Source-truth basis

Current product/source truth establishes:

1. `CodexHookNormalizer` already classifies any event carrying `agent_id` or
   `agent_type`, and explicit `SubagentStart`/`SubagentStop`, as
   `IgnoreSubagent`; subagents must not mutate root-session presentation.
2. `codex-hooks-command-v1` is the conservative current profile for newly
   discovered compatible Codex installations.
3. `profile_for_manifest()` currently selects `mcp_hybrid_v1()` whenever an
   existing ownership manifest contains a TabBeacon MCP server, keeping old
   upgraded installations on the legacy transport indefinitely.
4. The legacy MCP input template does not preserve subagent identity on generic
   `PreToolUse`/`PostToolUse` events, and subagent-side MCP availability is an
   external Codex runtime prerequisite that TabBeacon cannot safely guarantee.

The defect is therefore treated as a **legacy transport convergence problem**,
not as a reason to broaden the MCP transport.

## Hotfix strategy

The desired current state is:

```text
compatible Codex
  -> codex-hooks-command-v1
  -> one-shot fail-open command Hooks
  -> existing normalizer
  -> root event: normal presentation
  -> subagent event: IgnoreSubagent
```

Legacy exact-owned state is migration input only:

```text
10 TabBeacon mcp_tool Hooks
+ TabBeacon MCP server
+ SessionEnd command Hook
        |
        v
exact ownership/admission proof
        |
        v
command_v1 Hook declarations
+ no new TabBeacon MCP server declaration
+ unrelated Hooks/MCP/config preserved
+ manual Hook trust review required
```

The legacy MCP runtime code may remain temporarily for already-running sessions
and historical/upgrade recognition. v0.7.2 must not newly admit or recreate an
MCP Hybrid installation after successful migration.

## Dependency sequence

```text
PUBLIC v0.7.1
        |
        v
TB-G103  Hotfix Admission & Exact Reproduction
        |
        v
TB-G104  Legacy MCP Hybrid -> Command v1 Migration
        |
        v
TB-G105  Real Codex Subagent Qualification
        |
        v
TB-G106  v0.7.2 Hardening & Public Release
        |
        v
PUBLIC v0.7.2
        |
        v
DOGFOOD PAUSE

FROZEN IN PARALLEL:
TB-G99..TB-G102 / PR #100 -> retargeted to v0.7.3
```

## Goal index

| Goal | Scope | Estimated effective effort |
| --- | --- | ---: |
| G103 | reproduce/classify the real subagent Hook failure; freeze transport truth and migration safety boundary | 1–2 h |
| G104 | separate existing-vs-desired transport; migrate only exact-owned TabBeacon MCP declarations to command v1; preserve third-party state | 2–4 h |
| G105 | unit/migration/real Codex subagent regression proving zero Hook failures and no root-state mutation from child events | 2–4 h |
| G106 | full release gates, v0.7.2 publication, fresh consumers, post-release truth, resume dogfood pause | 2–4 h |
| **Total** | **v0.7.2 hotfix** | **7–14 h** |

## Migration invariants

Required:

```text
DESIRED_CODEX_TRANSPORT=command_v1
LEGACY_MCP_HYBRID_NEW_ADMISSION=false
LEGACY_MCP_RUNTIME_COMPATIBILITY=retained_if_needed

THIRD_PARTY_HOOKS_PRESERVED=true
THIRD_PARTY_MCP_SERVERS_PRESERVED=true
UNRELATED_CODEX_CONFIG_PRESERVED=true
HOOK_TRUST_BYPASS=false
TRUST_REVIEW_REQUIRED_AFTER_MIGRATION=true

DAILY_COMMAND_CODEX=codex
FAIL_OPEN=true
NO_WRAPPER=true
NO_PATH_SHADOW=true
NO_PTY_HOST=true
GLOBAL_DAEMON_ADDED=false
```

Do not remove or rewrite an MCP server or Hook group unless exact TabBeacon
ownership is proven by the current ownership manifest and target digest.
Ambiguous TabBeacon-like state remains fail-closed for mutation.

## Subagent semantic invariants

For real and synthetic subagent events:

```text
SUBAGENT_PRETOOLUSE=IgnoreSubagent
SUBAGENT_POSTTOOLUSE=IgnoreSubagent
SUBAGENT_START=IgnoreSubagent
SUBAGENT_STOP=IgnoreSubagent
ROOT_PRESENTATION_MUTATED_BY_SUBAGENT=false
```

The command transport must preserve the source-audited `agent_id` and
`agent_type` fields when Codex supplies them. Unknown/new fields remain ignored
under the existing bounded input policy; raw tool/prompt/model content remains
outside TabBeacon persistence and presentation.

## Real qualification requirement

The hotfix is not accepted from unit tests alone. A disposable real Codex
qualification must launch a parent session, create at least one subagent, and
exercise multiple child tool calls under the migrated command-Hook profile.

Required real evidence:

```text
REAL_CODEX_SUBAGENT_QUALIFICATION=PASS
SUBAGENT_TOOL_CALLS_SUCCEED=true
PRETOOLUSE_HOOK_FAILED_COUNT=0
POSTTOOLUSE_HOOK_FAILED_COUNT=0
ROOT_PRESENTATION_MUTATED_BY_CHILD=false
PARENT_PRESENTATION=PASS
```

Use disposable configuration/state where practical. Do not mutate Owner Hook
trust automatically.

## Release boundary

The Owner has authorized implementation and public release of this hotfix if all
applicable G103-G106 gates pass. The release transaction must remain exact-head
and forward-safe:

```text
PACKAGE_VERSION=0.7.2
CRATES_IO_VERSION=0.7.2
TAG=v0.7.2
GITHUB_RELEASE=v0.7.2
```

Public failure after any irreversible surface succeeds must be reported as a
truthful partial-public-release state; never move a public tag or overwrite a
crates.io version to simulate rollback.

## Promotion train retarget

The previously admitted Discoverability & Automated Demo work is not discarded.
It is frozen and retargeted to **v0.7.3**:

- PR #100 remains Draft and must not merge during the hotfix.
- Remote PR #100 head at hotfix admission was
  `4731a3ffbca643a4e3d3afcd3b61f1d849eaa434`.
- The Owner-reported local UIA recovery commit
  `31c076d4458a4c0606e494c1dea452946a92fb15` should be preserved if it still
  exists locally; it is not public truth until pushed/reconciled.
- The UIA diagnosis `TOPLEVEL_WINDOW_NAME_ASSUMPTION_INVALID` and the safer
  `EXACT_TABITEM_TO_ANCESTOR_WINDOW` direction remain useful v0.7.3 evidence.
- GitHub description/topics remain unapplied until the promotion train is later
  accepted.

See [`ROADMAP_V073.md`](ROADMAP_V073.md).

## Explicit non-goals

v0.7.2 hotfix does not add:

- promotional GIF/social-preview merge from PR #100;
- GitHub metadata changes;
- a third provider;
- Operational Reliability v2 or Provider Platform v2;
- Native Tab Icon or XAML Diagnostics;
- Codex App Server;
- installer/Winget/Scoop distribution for TabBeacon;
- new presentation semantics.

## Final state

After successful public v0.7.2 closeout:

```text
CURRENT_PUBLIC_RELEASE=v0.7.2
ACTIVE_FEATURE_DEVELOPMENT=PAUSED
PROMOTION_TARGET_RELEASE=v0.7.3
PROMO_PR=100
PROMO_PR_STATE=FROZEN_DRAFT
V08_OPTIONS_STATUS=NON_AUTHORITATIVE
ROADMAP_V08_CREATED=false
NEXT_RECOMMENDED_GOAL=DOGFOOD_OR_EXPLICIT_V073_RESUME
```
