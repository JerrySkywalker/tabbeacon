# TB-G105 — v0.7.2 Split Real Codex Qualification

## Purpose and adopted audit conclusion

G105 proves that the migrated command-v1 transport removes the visible
subagent Hook failure in a real Codex multi-agent workflow, that child activity
cannot mutate root TabBeacon presentation state, and that the exact candidate
has the required Windows Terminal presentation.

The independent local audit
`TB-V072-G105-INDEPENDENT-LOCAL-AUDIT-011` is adopted as advisory evidence. It
found G104 architecture `PASS` with zero blocking findings, preserved local
candidate `64adcc4` scope-clean, and Windows error 1223 as mixed qualification
infrastructure rather than a TabBeacon product failure. It also found that the
former one-second real `PostToolUse` timeout is
`LIKELY_RESOLVED_NEEDS_REAL_PROOF`; benchmark evidence alone cannot accept
G105.

The former monolithic G105 execution architecture is superseded. Its product
acceptance semantics are not weakened:

```text
G105A=REAL_CODEX_HOOK_SUBAGENT_SEMANTICS
G105B=WINDOWS_TERMINAL_PRESENTATION
G105_COMPLETE=(G105A=PASS AND G105B=PASS)
```

Unit tests, synthetic benchmarks, source inspection, or a fixture-only
presentation run cannot independently complete the other component.

## Changed-risk vector and required gates

```text
CODE_CHANGED=false_expected
PRESENTATION_CHANGED=true_qualification_proof
PROVIDER_CHANGED=true_command_hook_transport_qualification
USER_PERSISTENT_CONFIG_CHANGED=true_disposable_only
SECURITY_OR_PRIVACY_CHANGED=true_manual_trust_boundary
RELEASE_BOUNDARY=false
```

G105A is the real-provider qualification and fail-open gate. G105B is the
single final owned presentation gate. Both use one settled release candidate.
A failed qualification may authorize only a narrow G104 command-Hook transport
correction; that correction changes `CODE_CHANGED` to `true` and requires a
fresh G104/G105 admission and affected gates.

## Preconditions and fresh admission

Required:

```text
G103=COMPLETE
G104=COMPLETE
G104_MIGRATION_CANDIDATE=<exact reconciled PR102 candidate>
CURRENT_PUBLIC_RELEASE=v0.7.1
TARGET_PUBLIC_RELEASE=v0.7.2
DESIRED_CODEX_TRANSPORT=command_v1
LEGACY_MCP_HYBRID_NEW_ADMISSION=false
PR100_MERGED=false
```

Start from fresh `origin/main` and the reconciled PR102 candidate. Preserve
`64adcc4b69e08a95b98bf4df317967f531fc704c`; do not reset, rewrite, rebase, or
force-push its accepted product changes. The preferred reconciliation is a merge
of current `origin/main` into its local successor followed by a fast-forward
push to the existing PR #102 branch. Before that push,
`REMOTE_PR102_HEAD_IS_ANCESTOR_OF_FINAL=true` and `FORCE_PUSH_USED=false` are
required.

Record the exact candidate before either component starts:

```text
QUALIFICATION_SOURCE_SHA=<settled PR102 SHA>
QUALIFICATION_BINARY_PATH=<release candidate path>
QUALIFICATION_BINARY_SHA256=<sha256>
```

G105A and G105B must use that identical binary SHA-256. No package/release
metadata, PR #100, provider expansion, production Codex configuration, Owner
Hook trust, or Agy configuration is admitted by G105.

## Disposable trust and execution boundaries

Use a disposable, content-minimal workspace and isolated authenticated
`CODEX_HOME`; never copy credentials, trust, sessions, or configuration from an
Owner environment. Do not mutate Owner Hook declarations or trust.

Before G105A, compare the exact normalized command-v1 Hook definitions with
the already manually trusted isolated definitions. If they are byte/semantic
identical after normalization:

```text
HOOK_DEFINITION_CHANGED=false
ISOLATED_HOOK_TRUST_REUSABLE=true
```

If any trusted definition changed, do not bypass or synthesize trust. Prepare
the isolated candidate state and stop at
`WAITING_FOR_OWNER_ISOLATED_HOOK_TRUST`.

`--dangerously-bypass-hook-trust` is prohibited. No wrapper, PATH shadow, PTY
host, global daemon, restored MCP hybrid profile, production configuration
mutation, or production Agy mutation is permitted.

## G105A — real Codex Hook / subagent semantics

### Execution architecture

G105A uses real Codex 0.151.x `codex exec` against the isolated home and a
disposable workspace. It does not use Windows Terminal, UIA, a computer-use
helper, keyboard/mouse injection, or the prior elevated sandbox-setup route.

For this externally controlled disposable qualification workspace only,
`codex exec --yolo` is allowed solely to bypass approval/sandbox infrastructure
which otherwise initiated the canceled Windows setup helper. It does not
authorize Hook-trust bypass. Before accepting G105A, inspect the matching
Codex 0.151.x source and record that `--yolo` changes approval/sandbox policy
while Hook configuration, trust, and execution remain separately enforced; the
separate `--dangerously-bypass-hook-trust` flag must remain present. If that
source inspection contradicts this separation, stop.

Required execution facts:

```text
CODEX_VERSION=0.151.x
CODEX_EXEC_USED=true
CODEX_SANDBOX_BYPASS_USED_FOR_QUALIFICATION=true
CODEX_HOOK_TRUST_BYPASS_USED=false
REAL_CODEX_EXEC_SESSION=PASS
```

### Bounded real scenario

Run one content-minimal, harmless, read-only scenario that requires:

1. a parent tool invocation before creating a child;
2. one real subagent;
3. at least two child tool invocations;
4. normal child completion;
5. a parent tool invocation after the child; and
6. normal parent completion.

Do not mutate a repository, perform a network task, or retain raw prompt,
model, tool, or child-payload content. `codex exec --json` may be used for
structured count evidence only.

### Required real delivery and normalization evidence

```text
SUBAGENT_COUNT>=1
SUBAGENT_TOOL_CALL_COUNT>=2
PARENT_TOOL_CALL_BEFORE_CHILD>=1
PARENT_TOOL_CALL_AFTER_CHILD>=1
TABBEACON_MCP_TOOL_HOOKS_ACTIVE=false
TABBEACON_COMMAND_HOOKS_ACTIVE=true
TABBEACON_COMMAND_HOOK_DELIVERY_OBSERVED=true
CHILD_PRETOOLUSE_COMMAND_DELIVERY_COUNT>=1
CHILD_POSTTOOLUSE_COMMAND_DELIVERY_COUNT>=1
CHILD_AGENT_ID_OR_TYPE_OBSERVED=true
CHILD_PRETOOLUSE_NORMALIZATION=IgnoreSubagent
CHILD_POSTTOOLUSE_NORMALIZATION=IgnoreSubagent
```

For each generic child event independently, prove `agent_id` or `agent_type`
was observed and normalization returned `IgnoreSubagent`. Lifecycle-event
identity cannot substitute for the generic-event proof. The migrated exact
legacy ten-MCP profile (and SessionEnd when present) must be migrated through
the supported ownership path and then run the same real parent/subagent
scenario; a fresh command-v1 installation cannot substitute.

### Zero-failure, root-isolation, and fail-open gates

```text
SUBAGENT_TOOL_CALLS_SUCCEED=true
REAL_CODEX_SUBAGENT_QUALIFICATION=PASS
PRETOOLUSE_HOOK_FAILED_COUNT=0
POSTTOOLUSE_HOOK_FAILED_COUNT=0
STOP_HOOK_FAILED_COUNT=0
TABBEACON_HOOK_TIMEOUT_COUNT=0
TABBEACON_HOOK_TOOL_CALL_FAILED_COUNT=0
ROOT_PRESENTATION_MUTATED_BY_CHILD=false
CHILD_EVENT_CAUSED_ROOT_TRANSITION=false
ROOT_EVENTS_ADMITTED=true
REAL_MIGRATED_LEGACY_SUBAGENT_QUALIFICATION=PASS
FAIL_OPEN=PASS
```

Exercise fail-open only after the verified TabBeacon command Hook handler has
started. A missing/unavailable binary is diagnostic-only and cannot satisfy the
gate. Do not hide or filter error lines, weaken the declared Hook timeout, or
weaken trust policy to obtain zero counts. Any real TabBeacon Hook failure makes
`G105A=FAIL` and blocks release.

### Windows error 1223 classification

The observed Windows error 1223 occurred while Codex launched its elevated
sandbox setup helper. The child shell did not start and `PostToolUse` did not
begin. Therefore it is not evidence of a TabBeacon Hook failure and does not
prove or disprove the latency correction. Do not claim Codex is defective
beyond this observed qualification-environment interaction.

```text
WINDOWS_1223_CLASS=MIXED
WINDOWS_1223_PRODUCT_GATE=false
WINDOWS_1223_QUALIFICATION_INFRASTRUCTURE=true
```

## G105B — real Windows Terminal presentation / UIA

G105B runs only after `G105A=PASS` and uses the same
`QUALIFICATION_SOURCE_SHA` and `QUALIFICATION_BINARY_SHA256`. It uses a fresh,
stock Windows Terminal run and the exact candidate binary. No real model,
subagent, Codex Windows sandbox helper, keyboard injection, or foreground-focus
acceptance is required.

Establish ownership with one unique fixed anchor `TabItem`, derive its ancestor
top-level `Window`/HWND, and retain that exact HWND as the authority. Do not
search top-level `Window.Name` by a dynamic title and do not capture the
desktop.

Drive deterministic, content-minimal valid root Codex Hook payload fixtures
against the candidate binary. Exercise the minimum root lifecycle required for
baseline/Ready where applicable, Working, post-tool/result transition, Stop /
ResultReady, and final stable presentation. This fixture is presentation proof
only; it cannot satisfy G105A real Hook/subagent proof.

```text
ANCHOR_TAB_MATCH_COUNT=1
TARGET_WINDOW_MATCH_COUNT=1
TARGET_HWND_STABLE=true
ANCHOR_PRESENT_THROUGH_QUALIFICATION=true
WORKING_PRESENTATION=PASS
RESULT_READY_PRESENTATION=PASS
FINAL_OWNED_PARENT_UIA_VISUAL=PASS
DESKTOP_CAPTURE_USED=false
G105B=PASS
```

## Original G105 requirement coverage

Every original G105 product and release-acceptance property is retained exactly
once or deliberately bound to both components. “Both” means a common exact
candidate/binary binding, not duplicate execution of a product proof.

| Original requirement | Coverage owner | Required evidence |
| --- | --- | --- |
| Real parent Codex session, real subagent, child tools, and parent progress after child | G105A | real `codex exec` counts |
| Exact legacy MCP-to-command-v1 upgrade run | G105A | disposable migrated-legacy real scenario |
| Command-v1 delivery and no TabBeacon MCP Hook delivery | G105A | normalized real delivery facts |
| Child generic identity and `IgnoreSubagent` normalization | G105A | separate Pre/Post identity and normalization outcomes |
| Child/lifecycle events cannot mutate root state | G105A | root-isolation state evidence |
| Zero TabBeacon Hook failures/timeouts and successful child tools | G105A | complete failure/count receipt |
| Fail-open after the handler starts | G105A | bounded handler-reached failure fixture |
| Working and ResultReady/final root presentation | G105B | deterministic root lifecycle fixture |
| Exact-owned parent Windows Terminal UIA/visual proof | G105B | anchor-to-HWND ownership and UIA receipt |
| Disposable trust boundary with no trust bypass | G105A | normalized declaration comparison and manual-trust condition |
| One settled source/binary binds semantics and presentation | BOTH | identical source SHA and binary SHA-256 |
| No production configuration/trust/Agy mutation and no desktop capture | BOTH | component receipts and final boundary receipt |

```text
ORIGINAL_G105_REQUIREMENT_COVERAGE=100_PERCENT
UNCOVERED_ORIGINAL_G105_REQUIREMENTS=0
```

## Composite acceptance

```text
G105A=PASS
G105B=PASS
SAME_SOURCE_SHA=true
SAME_BINARY_SHA256=true
ORIGINAL_G105_REQUIREMENT_COVERAGE=100_PERCENT
G105=COMPLETE
PRODUCTION_CODEX_CONFIGURATION_MUTATED=false
PRODUCTION_HOOK_TRUST_MUTATED=false
PRODUCTION_AGY_CONFIGURATION_MUTATED=false
```

Only then may PR #102 be updated, accepted at its exact head, merged, and
handed to [`TB-G106-V072-HOTFIX-HARDENING-RELEASE.md`](TB-G106-V072-HOTFIX-HARDENING-RELEASE.md).
