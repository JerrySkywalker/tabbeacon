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

## Fresh phase admission

Immediately before G105, record the exact accepted G104 candidate rather than
assuming a planning-era SHA remains current:

```text
REPOSITORY=JerrySkywalker/tabbeacon
EXPECTED_START_HEAD=<exact accepted G104 candidate head>
CHECKED_OUT_HEAD=EXPECTED_START_HEAD
EXPECTED_REMOTE_MAIN=<freshly fetched origin/main>
WORKTREE=<one clean owned implementation worktree>
QUALIFICATION_ROOT=<one exact-owned disposable Codex/config/workspace root>
```

The admitted source boundary is G105-focused tests, fixtures, and only a
correction required by failed qualification in the G104 Codex transport scope;
any such correction requires a new G104/G105 admission and gates. The
qualification root must never be an Owner production configuration, session, or
trust store. No package/release metadata, PR #100, provider expansion, or
presentation feature is admitted by this phase.

## A. Qualification environment

Prefer a disposable isolated Codex HOME/config/state root that does not mutate
Owner production Hook declarations or trust.

The fixture must install/reconcile TabBeacon through the same supported
ownership path used by users, but against disposable configuration.

Before the real scenario, complete the ordinary manual Codex Hook trust review
for the changed command declarations in that disposable environment. Record
only the resulting trusted/deliverable state and content-minimal event counts;
do not copy trust state, hashes, or sessions from an Owner configuration.

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
DISPOSABLE_COMMAND_HOOK_TRUST_REVIEW=PASS
TABBEACON_COMMAND_HOOK_DELIVERY_OBSERVED=true
CHILD_PRETOOLUSE_COMMAND_DELIVERY_COUNT>=1
CHILD_POSTTOOLUSE_COMMAND_DELIVERY_COUNT>=1
CHILD_PRETOOLUSE_AGENT_IDENTITY_OBSERVED=true
CHILD_POSTTOOLUSE_AGENT_IDENTITY_OBSERVED=true
CHILD_PRETOOLUSE_NORMALIZATION=IgnoreSubagent
CHILD_POSTTOOLUSE_NORMALIZATION=IgnoreSubagent
```

Do not infer this only from source; inspect the disposable effective config and
runtime observations. A session with untrusted or skipped Hook declarations
cannot satisfy this gate. For each generic child event independently, prove that
both `agent_id` and `agent_type` were observed and that normalization returned
`IgnoreSubagent`; lifecycle-event identity cannot satisfy this generic-event
proof. Record only counts, identity presence, and normalization outcomes; never
retain a raw child payload or tool content.

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
FINAL_OWNED_PARENT_UIA_VISUAL=PASS
```

At the same time prove ordinary parent/root presentation still functions:

```text
PARENT_WORKING_PRESENTATION=PASS
PARENT_RESULT_READY_PRESENTATION=PASS_or_equivalent_current_semantics
```

Use content-minimal receipts and existing presentation evidence methods; do not
persist model/tool bodies.

## F. Fail-open behavior

Exercise a bounded malformed-input or runtime-failure fixture only after the
verified TabBeacon command Hook handler has started, and prove Codex progression
is not blocked. A missing/unavailable binary is diagnostic-only because it does
not reach TabBeacon's fail-open handler; it cannot satisfy this gate.

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
DISPOSABLE_COMMAND_HOOK_TRUST_REVIEW=PASS
TABBEACON_COMMAND_HOOK_DELIVERY_OBSERVED=true
CHILD_PRETOOLUSE_COMMAND_DELIVERY_COUNT>=1
CHILD_POSTTOOLUSE_COMMAND_DELIVERY_COUNT>=1
CHILD_PRETOOLUSE_AGENT_IDENTITY_OBSERVED=true
CHILD_POSTTOOLUSE_AGENT_IDENTITY_OBSERVED=true
CHILD_PRETOOLUSE_NORMALIZATION=IgnoreSubagent
CHILD_POSTTOOLUSE_NORMALIZATION=IgnoreSubagent
SUBAGENT_TOOL_CALLS_SUCCEED=true
PRETOOLUSE_HOOK_FAILED_COUNT=0
POSTTOOLUSE_HOOK_FAILED_COUNT=0
ROOT_PRESENTATION_MUTATED_BY_CHILD=false
PARENT_PRESENTATION=PASS
FINAL_OWNED_PARENT_UIA_VISUAL=PASS
FAIL_OPEN=PASS
OWNER_PRODUCTION_CONFIG_MUTATED=false_expected
OWNER_HOOK_TRUST_MUTATED=false_expected
```

Next: `TB-G106-V072-HOTFIX-HARDENING-RELEASE.md`.
