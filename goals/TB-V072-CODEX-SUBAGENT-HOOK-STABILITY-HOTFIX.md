# TabBeacon v0.7.2 — Codex Subagent Hook Stability Hotfix

```text
GOAL_ID=TB-V072-FULL-SUBAGENT-HOOK-HOTFIX-TO-PUBLIC-RELEASE-001
```

## Purpose

Ship a narrow production hotfix for legacy upgraded Codex installations that
remain on TabBeacon's historical session-scoped MCP Hybrid Hook transport and
can surface repeated subagent tool-Hook failures.

Observed dogfood symptom:

```text
PreToolUse hook (failed)
error: tool call failed for `tabbeacon-hook/tabbeacon_hook_event`
```

The hotfix converges exact-owned legacy MCP Hybrid integrations to the already
admitted conservative `codex-hooks-command-v1` transport. It does not attempt to
make child/subagent MCP runtimes inherit or reconnect TabBeacon's MCP tool.

## Authoritative design decision

```text
CURRENT_DESIRED_TRANSPORT=codex-hooks-command-v1
LEGACY_TRANSPORT=codex-hooks-mcp-hybrid-v1
LEGACY_MCP_HYBRID_NEW_ADMISSION=false
LEGACY_MCP_CODE_PHYSICAL_REMOVAL=false_for_v072
```

Existing MCP runtime code may remain for already-running/historical recognition
and safe upgrade handling. A successfully migrated installation must not create
new TabBeacon MCP Hook declarations on the next setup/repair.

## Root semantic contract

TabBeacon already has the correct semantic rule:

```text
subagent event or event carrying agent_id/agent_type
  -> IgnoreSubagent
  -> root-session presentation unchanged
```

The hotfix must preserve source-audited identity fields through the command
Hook path and prove that child tool activity does not alter the parent tab.

## Migration authority

Migration is allowed only when all TabBeacon-owned legacy declarations and MCP
server state are proven exact under the ownership manifest/current target.

Allowed mutation:

- replace exact-owned TabBeacon MCP Hook declarations with current command-v1
  declarations;
- remove only the exact-owned TabBeacon MCP server declaration no longer
  required by desired current state;
- retain/restitch exact owned title delegation according to current policy.

Forbidden mutation:

- third-party Hooks;
- third-party MCP servers;
- ambiguous TabBeacon-like declarations;
- unrelated Codex config;
- Hook trust state/grants;
- project-local config.

After migration, manual Codex `/hooks` trust review remains required for changed
Hook definitions.

## Goal sequence

```text
TB-G103  Hotfix Admission & Exact Reproduction
TB-G104  Legacy MCP Hybrid -> Command v1 Migration
TB-G105  Real Codex Subagent Qualification
TB-G106  v0.7.2 Hardening & Public Release
```

## Production invariants

```text
DAILY_COMMAND=codex
FAIL_OPEN=true
NO_WRAPPER=true
NO_PATH_SHADOW=true
NO_PTY_HOST=true
GLOBAL_DAEMON_ADDED=false
HOOK_TRUST_BYPASS=false
RAW_PROMPT_PERSISTED=false
RAW_TOOL_CONTENT_PERSISTED=false
ROOT_PRESENTATION_MUTATED_BY_SUBAGENT=false
```

Agy behavior is outside the hotfix scope and must remain unchanged.

## Release authorization

This Goal records, but does not create, Owner authorization for public `v0.7.2`
publication after all G103-G106 acceptance gates pass. Before the public
transaction, the executor must verify that the admitted execution Goal carries
this `GOAL_ID` and explicit Owner authorization for crates.io, immutable tag,
GitHub Release, Windows x64 ZIP/SHA-256 assets, public consumer verification,
and a focused post-release truth closeout if repository convention requires it.

If that explicit authorization is absent, stale, or does not cover the proposed
public mutation, stop with `OWNER_RELEASE_AUTHORIZATION=UNPROVEN`. When it is
verified and all applicable gates pass, no additional generic authorization wait
is required. A failed gate is not waived by this authorization.

## Deferred promotion work

The previously admitted Discoverability & Automated Demo train is frozen and
retargeted to v0.7.3. PR #100 must remain unmerged throughout this hotfix.

See:

- `dev_governance_files/ROADMAP_V072.md`
- `dev_governance_files/ROADMAP_V073.md`
