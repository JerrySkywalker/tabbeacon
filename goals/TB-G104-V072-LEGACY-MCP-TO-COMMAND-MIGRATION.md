# TB-G104 — v0.7.2 Legacy MCP Hybrid -> Command v1 Migration

## Purpose

Make the current desired Codex Hook transport independent from the transport
recorded by an older TabBeacon ownership manifest. Migrate only exact-owned
legacy MCP Hybrid integrations to `codex-hooks-command-v1` while preserving all
unrelated user state.

## Changed-risk vector and required gates

```text
CODE_CHANGED=true
PRESENTATION_CHANGED=false
PROVIDER_CHANGED=true
USER_PERSISTENT_CONFIG_CHANGED=true
SECURITY_OR_PRIVACY_CHANGED=true
RELEASE_BOUNDARY=false
```

This Goal requires one focused migration/ownership-safety family proving exact
ownership, minimal mutation, third-party preservation, concurrent-drift
refusal, idempotence, and uninstall restoration. Because it changes a real
Codex provider/profile and command-Hook trust boundary, it also requires the
focused real-provider qualification in G105. The settled implementation
candidate additionally requires one fresh hosted exact-head code CI and a
focused ownership/trust/migration safety review. Presentation and public-release
gates are N/A for G104 itself; G106 selects the release boundary.

## Fresh phase admission

Immediately before any G104 source or disposable-fixture mutation, record a
fresh admission. This document is not itself a mutation authority for an
arbitrary later head.

```text
REPOSITORY=JerrySkywalker/tabbeacon
EXPECTED_START_HEAD=<exact accepted G103 predecessor head>
CHECKED_OUT_HEAD=EXPECTED_START_HEAD
EXPECTED_REMOTE_MAIN=<freshly fetched origin/main>
WORKTREE=<one clean owned implementation worktree>
```

The allowed source boundary is the Codex transport/ownership implementation
(`src/providers/codex/{profile,config,mcp,mod,runtime,capability}.rs`), its
focused tests/fixtures, and a minimal current diagnostic document if needed.
The allowed external target is one exact-owned disposable Codex configuration
root named in the admission receipt. The phase must not touch an Owner
configuration, package/release metadata, PR #100, another provider, or any
source outside the admitted migration boundary. Re-admit after every candidate
head change and immediately before a configuration apply.

## Design correction

The implementation must separate two concepts that are currently coupled:

```text
EXISTING_OWNED_TRANSPORT
DESIRED_CURRENT_TRANSPORT
```

An existing manifest containing a TabBeacon MCP server proves historical owned
state. It must not automatically select `mcp_hybrid_v1()` as the desired current
profile forever.

For a compatible current Codex installation, desired state is:

```text
DESIRED_CURRENT_TRANSPORT=command_v1
```

## A. Legacy recognition

Retain a bounded recognizer for exact legacy states needed to migrate or safely
inspect:

- first MCP Hybrid form;
- Hybrid + independent SessionEnd command form;
- any previously admitted exact owned transition already recorded by tests.

Recognition does not grant permission to create new MCP Hybrid declarations.

```text
LEGACY_MCP_HYBRID_NEW_ADMISSION=false
```

## B. Migration plan

For an exact-owned legacy target, a preview must show only the intended owned
delta:

1. remove/replace TabBeacon-owned `mcp_tool` Hook handlers;
2. remove the exact TabBeacon-owned MCP server declaration if no longer desired;
3. create the current command-v1 TabBeacon Hook declarations;
4. preserve current supported title delegation/ownership semantics;
5. preserve all unrelated Hook groups/MCP servers/config.

Do not mutate on ambiguous ownership or concurrent target drift.

## C. Trust

Changed command Hook definitions require manual review under Codex's existing
trust model.

Required:

```text
HOOK_TRUST_BYPASS=false
TRUST_REVIEW_REQUIRED_AFTER_MIGRATION=true
AUTO_TRUST_GRANTED=false
```

Do not attempt to copy a legacy MCP trust/hash into a different command Hook
shape as if that were equivalent user trust.

## D. Legacy runtime compatibility

Do not delete the MCP runtime code merely to make the source look cleaner.
Already-running Codex sessions launched before migration may still own an MCP
stdio child.

v0.7.2 may retain:

```text
__mcp-hook-stdio-v1
McpHookSession
legacy manifest readers
```

provided new setup/repair no longer admits a fresh MCP Hybrid target after
successful migration.

## E. Idempotence

After migration:

```text
first setup/repair -> migration + trust review required
second setup/repair -> AlreadyInstalled/AlreadyExact equivalent
MCP_SERVER_RECREATED=false
```

A future command-v1 compatible Codex discovery must not be overridden back to
MCP Hybrid solely because a stale historical manifest field survived. The
manifest/current owned state must be reconciled atomically and truthfully.

## F. Required tests

At minimum cover:

1. exact legacy MCP Hybrid fixture migrates to command v1;
2. Hybrid + SessionEnd command fixture migrates;
3. third-party same-event Hook preserved;
4. third-party MCP server preserved byte/semantic-equivalent;
5. unrelated Codex config preserved;
6. ambiguous TabBeacon-like MCP server blocks mutation;
7. concurrent target drift blocks apply;
8. missing/invalid ownership manifest blocks destructive migration;
9. post-migration second run is idempotent;
10. a fresh compatible installation remains command v1;
11. migration never grants Hook trust.
12. migration followed by ownership-safe uninstall removes only TabBeacon
    declarations while preserving third-party Hooks, MCP servers, and unrelated
    Codex configuration.

## G. Acceptance

Required:

```text
LEGACY_MCP_MANIFEST_DETECTED=true_tested
LEGACY_MCP_TO_COMMAND_MIGRATION=PASS
DESIRED_CODEX_TRANSPORT=command_v1
LEGACY_MCP_HYBRID_NEW_ADMISSION=false
OWNED_TABBEACON_MCP_SERVER_REMOVED=true_when_exact
THIRD_PARTY_MCP_SERVERS_PRESERVED=true
THIRD_PARTY_HOOKS_PRESERVED=true
UNRELATED_CODEX_CONFIG_PRESERVED=true
HOOK_TRUST_BYPASS=false
MIGRATION_IDEMPOTENT=true
MIGRATION_UNINSTALL_RESTORE=PASS
```

Next: `TB-G105-V072-REAL-SUBAGENT-QUALIFICATION.md`.
