# TB-G103 — v0.7.2 Hotfix Admission & Exact Reproduction

## Purpose

Reproduce and classify the subagent Hook failure against current source before
changing transport/migration logic. Establish whether the failure occurs before
TabBeacon's MCP server receives a call, inside TabBeacon MCP dispatch, or in a
later reducer/presentation stage.

## Fresh phase admission

Before G103 source inspection or disposable-fixture work, record an exact,
read-only admission. This phase does not authorize later migration writes.

```text
REPOSITORY=JerrySkywalker/tabbeacon
EXPECTED_START_HEAD=<exact post-planning-merge origin/main head>
CHECKED_OUT_HEAD=EXPECTED_START_HEAD
EXPECTED_REMOTE_MAIN=EXPECTED_START_HEAD
WORKTREE=<one clean owned hotfix worktree>
CODE_CHANGED=false_expected
PRESENTATION_CHANGED=false
PROVIDER_CHANGED=true_diagnostic_only
USER_PERSISTENT_CONFIG_CHANGED=true_disposable_fixture_only
SECURITY_OR_PRIVACY_CHANGED=false
RELEASE_BOUNDARY=false
```

The source boundary is read-only Codex transport/normalizer/capability audit
plus focused tests. The only configuration target is one exact-owned disposable
fixture root recorded in the receipt; no Owner configuration, trust store,
release metadata, PR #100 content, new provider, or presentation feature is
admitted. Any G103 code change or source-head drift requires a fresh admission
and reclassification before it can be used by G104.

## Required starting truth

```text
CURRENT_PUBLIC_RELEASE=v0.7.1
TARGET_PUBLIC_RELEASE=v0.7.2
CURRENT_DESIRED_NEW_INSTALL_PROFILE=codex-hooks-command-v1
LEGACY_UPGRADED_PROFILE=codex-hooks-mcp-hybrid-v1
PR100_MERGE_ALLOWED=false
```

## A. Source audit

Re-read exact current:

- `src/providers/codex/profile.rs`;
- `src/providers/codex/config.rs`;
- `src/providers/codex/mcp.rs`;
- `src/providers/codex/mod.rs`;
- capability/source admission code;
- current Codex Hook docs/source used by the repository.

Confirm the distinction between:

1. the profile discovered/desired for a compatible current installation; and
2. the profile retained merely because an existing ownership manifest records a
   TabBeacon MCP server.

## B. Disposable legacy fixture

Construct a disposable exact-owned legacy integration representative of a real
upgraded installation:

```text
10 TabBeacon mcp_tool Hook declarations
TabBeacon mcp_server declaration
SessionEnd TabBeacon command Hook
valid ownership manifest
at least one unrelated third-party Hook
at least one unrelated third-party MCP server
```

Do not use Owner production config as the primary reproduction fixture.

## C. Real/subagent reproduction

Where current Codex can be exercised safely in a disposable HOME/config root,
run a bounded parent + subagent scenario with multiple child tool calls.

Capture only content-minimal operational facts:

```text
PRETOOLUSE_TOTAL
PRETOOLUSE_FAILED
POSTTOOLUSE_TOTAL
POSTTOOLUSE_FAILED
SUBAGENT_START_OBSERVED
SUBAGENT_STOP_OBSERVED
TABBEACON_MCP_TOOL_CALL_REACHED_SERVER=<true|false|unproven>
```

Do not persist prompt/tool/model bodies.

## D. Semantic control

Independently prove the current normalizer behavior when a valid command-Hook
payload contains `agent_id` / `agent_type`:

```text
PreToolUse + agent identity -> IgnoreSubagent
PostToolUse + agent identity -> IgnoreSubagent
SubagentStart -> IgnoreSubagent
SubagentStop -> IgnoreSubagent
ROOT_PRESENTATION_MUTATED=false
```

This establishes that the intended reducer behavior is not the defect.

## E. Root-cause classification

Accept one or more evidence-backed classes:

```text
MCP_TOOL_UNAVAILABLE_IN_SUBAGENT_CONTEXT
MCP_TEMPLATE_DROPS_SUBAGENT_IDENTITY
MCP_SERVER_DISPATCH_DEFECT
OTHER_PROVEN_CAUSE
```

Do not repair by increasing retries/timeouts unless the evidence specifically
proves a timing budget is the root cause.

## Exit

G103 completes when the production symptom is reproduced or boundedly proven,
the normalizer semantic control passes, and the desired fix direction remains
`legacy MCP Hybrid -> command_v1` unless new evidence disproves that plan.

Required receipt:

```text
G103=COMPLETE
ROOT_CAUSE=<classification>
NORMALIZER_SUBAGENT_CONTROL=PASS
LEGACY_TRANSPORT_CONVERGENCE_JUSTIFIED=true
OWNER_PRODUCTION_CONFIG_MUTATED=false
```

Next: `TB-G104-V072-LEGACY-MCP-TO-COMMAND-MIGRATION.md`.
