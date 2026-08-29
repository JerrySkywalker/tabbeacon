# TB-G105 — v0.7.2 Real Codex Subagent Qualification

## Purpose

Prove that the migrated command-v1 transport removes the visible subagent Hook
failure in a real Codex multi-agent workflow and that child activity cannot
mutate the parent/root TabBeacon presentation.

Unit tests are necessary but insufficient for this Goal.

## Preconditions

Required:

```text
G103=COMPLETE
G104=COMPLETE
DESIRED_CODEX_TRANSPORT=command_v1
LEGACY_MCP_HYBRID_NEW_ADMISSION=false
```

Use the exact candidate head that contains the accepted migration logic.

## A. Qualification environment

Prefer a disposable isolated Codex HOME/config/state root that does not mutate
Owner production Hook declarations or trust.

The fixture must install/reconcile TabBeacon through the same supported
ownership path used by users, but against disposable configuration.

If real Codex authentication/session prerequisites cannot be safely isolated,
use the minimum Owner-present boundary required by repository policy and record
it. Do not silently substitute a pure unit test for real qualification.

## B. Parent/subagent scenario

Run a bounded scenario with:

- one real parent Codex session;
- at least one thread-spawned subagent;
- multiple subagent tool invocations that exercise both `PreToolUse` and
  `PostToolUse`;
- normal parent activity before/after the child;
- no deliberately destructive user task.

The qualification instruction should avoid private repository/model content and
may use a disposable trivial workspace.

## C. Hook transport evidence

Prove the real session uses command-v1 declarations rather than TabBeacon MCP
Hook delivery:

```text
TABBEACON_MCP_TOOL_HOOKS_ACTIVE=false
TABBEACON_COMMAND_HOOKS_ACTIVE=true
TABBEACON_MCP_SERVER_REQUIRED_FOR_HOOK_DELIVERY=false
```

Do not infer this only from source; inspect the disposable effective config and
runtime observations.

## D. Subagent failure gate

Required:

```text
SUBAGENT_TOOL_CALLS_SUCCEED=true
PRETOOLUSE_HOOK_FAILED_COUNT=0
POSTTOOLUSE_HOOK_FAILED_COUNT=0
TABBEACON_HOOK_TOOL_CALL_FAILED_COUNT=0
```

Do not hide or filter error lines merely to produce zero counts.

## E. Root presentation isolation

The normalizer/runtime evidence must show child-context events are ignored for
root-state mutation:

```text
SUBAGENT_PRETOOLUSE=IgnoreSubagent
SUBAGENT_POSTTOOLUSE=IgnoreSubagent
SUBAGENT_START=IgnoreSubagent
SUBAGENT_STOP=IgnoreSubagent
ROOT_PRESENTATION_MUTATED_BY_CHILD=false
```

At the same time prove ordinary parent/root presentation still functions:

```text
PARENT_WORKING_PRESENTATION=PASS
PARENT_RESULT_READY_PRESENTATION=PASS_or_equivalent_current_semantics
```

Use content-minimal receipts and existing presentation evidence methods; do not
persist model/tool bodies.

## F. Fail-open behavior

Exercise a missing/unavailable TabBeacon command Hook binary or bounded failure
fixture according to current test conventions and prove Codex progression is not
blocked.

```text
FAIL_OPEN=PASS
```

Do not weaken current Hook timeout or trust policy to achieve this.

## G. Regression matrix

At minimum include:

1. parent-only tool call;
2. one subagent with multiple tools;
3. multiple subagents sequentially if supported by the current real fixture;
4. subagent stop followed by parent tool call;
5. session end/cleanup;
6. command-v1 fresh install and migrated legacy install parity where practical.

## Acceptance

```text
REAL_CODEX_SUBAGENT_QUALIFICATION=PASS
SUBAGENT_TOOL_CALLS_SUCCEED=true
PRETOOLUSE_HOOK_FAILED_COUNT=0
POSTTOOLUSE_HOOK_FAILED_COUNT=0
ROOT_PRESENTATION_MUTATED_BY_CHILD=false
PARENT_PRESENTATION=PASS
FAIL_OPEN=PASS
OWNER_PRODUCTION_CONFIG_MUTATED=false_expected
OWNER_HOOK_TRUST_MUTATED=false_expected
```

Next: `TB-G106-V072-HOTFIX-HARDENING-RELEASE.md`.
