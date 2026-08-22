# TB-V052 — v0.5.2 High-Performance Codex Runtime

## Status

CANDIDATE. Local functionality and performance proof are complete; exact-head
hosted CI, independent reliability review, and the Owner-only dogfood/re-enable
transaction remain. This maintenance Goal is admitted after public v0.5.1 and
before the G64 Agy hard stop. It does not resume or otherwise alter G64.

## Harness admission

```text
HARNESS_MODEL_ID=JERRY_HARNESS_MODEL_V1
HARNESS_BASELINE_ID=JERRY_AUTONOMY_CI_PARAMS_V1
HARNESS_PROFILE=COMPRESSED_TRAIN_V1
AUTHORITY_CLASS=A3
ELASTICITY_GRADE=B3
INITIAL_PROGRESS_STATE=P_INIT
CURRENT_LAYER=L1
MAX_ADMITTED_LAYER=L3
NEXT_PROOF_VECTOR=V1/E1/F1/G1
ALLOWED_REPOSITORIES=JerrySkywalker/tabbeacon
ALLOWED_PATHS=dev_governance_files/ROADMAP_V06.md; goals/TB-V052-HIGH-PERFORMANCE-CODEX-RUNTIME.md; src/providers/codex/**; src/repo/**; src/** only when required by the Hook path; tests/**; scripts/**; docs/codex-hooks.md; Cargo.toml; Cargo.lock
ALLOWED_SERVICES=GitHub repository, pull requests, and Actions for JerrySkywalker/tabbeacon only
PROTECTED_BOUNDARIES=Owner ~/.codex configuration; Hook trust; G64/Agy admission; public release and package publication
OWNER_ONLY_BOUNDARIES=real Codex dogfood/re-enable; Hook review/trust; public v0.5.2 release decision
BUDGET_OVERRIDES=NONE
BUDGET_STATE_REF=V:\build\tabbeacon\TB-V052-HIGH-PERFORMANCE-CODEX-RUNTIME-011
LAST_ACCEPTED_CHECKPOINT=START_MAIN 22eba064100a5a3919ebaaf81f9217c6c6374355
CONTEXT_MODULES=TabBeacon AGENTS.md; QUALITY_GATES.md; ROADMAP_V06.md; G59; G63; Harness Goal/Receipt Contract
STOP_CONDITIONS=Owner-only boundary; trust/config mutation; G64 work; destructive external mutation; unproven production claim; conflict with advanced main that cannot be safely rebased
```

## Objective

Make the admitted synchronous Codex Hook path genuinely sub-second under
Windows multi-Codex load without increasing its one-second timeout. Preserve
the trusted declaration command, timeout, and synchronous semantics whenever
possible.

## Scope and constraints

- Measure the current production-equivalent path with phase attribution for
  cold/warm and concurrency 1/4/8.
- SessionStart and deterministic missing-anchor fallback may perform complete
  workspace discovery. After a valid Root Workspace Anchor exists, ordinary
  Hook events must execute with zero Git subprocesses.
- Preserve Root Workspace authority, linked-worktree correctness,
  generation/session isolation, bounded stale-anchor cleanup, and
  privacy-safe workspace-mismatch observability.
- Preserve the PR69 Codex integration continuity and PR71 proven third-party /
  MCP Hook repair guarantees.
- If Fast Anchor Path cannot satisfy the SLA, optimize the Windows shell and
  cold-start path first. Only then may a session-scoped, fail-open IPC worker
  be introduced. A machine-global daemon is forbidden.

## Acceptance

```text
BASELINE_PHASE_ATTRIBUTION=RECORDED
FAST_ANCHOR_PATH=PASS
NORMAL_HOOK_GIT_SUBPROCESS_COUNT=0
WORKSPACE_MISMATCH_SEMANTICS=ANCHORED_AND_BOUNDED
HOOK_DECLARATION_CHANGED=true
HOOK_TIMEOUT_SECONDS=1
HOOK_ASYNC=false
WARM_P99_LT_500MS=PASS
CONCURRENCY_8_P99_LT_750MS=PASS
MAX_LT_900MS=PASS
PRODUCTION_TIMEOUT_FAILURES=0
PERFORMANCE_REGRESSION_GATES=PASS
CODEX_0147_REGRESSION=PASS
CODEX_0149_REGRESSION=PASS
PR69_CONTINUITY_REGRESSION=PASS
PR71_REPAIR_REGRESSION=PASS
CODE_CI=PENDING_HOSTED_EXACT_HEAD
INDEPENDENT_RELIABILITY_REVIEW=PENDING
V052_RELEASE_CANDIDATE=PENDING_EXACT_HEAD
OWNER_CODEX_CONFIG_MUTATED=false
OWNER_HOOK_TRUST_MUTATED=false
PUBLIC_RELEASE=false
```

## Risk vector and gates

```text
CODE_CHANGED=true
PRESENTATION_CHANGED=true
PROVIDER_CHANGED=true
USER_PERSISTENT_CONFIG_CHANGED=true
SECURITY_OR_PRIVACY_CHANGED=true
RELEASE_BOUNDARY=true
```

Run focused deterministic performance, anchor, compatibility, Hook-repair, and
runtime-state safety tests while iterating. At the settled candidate, require
one exact-head hosted code CI, one representative owned Windows presentation
proof, one focused independent reliability/safety review, and a bounded Owner
dogfood/re-enable transaction that makes no unattended Owner configuration or
trust mutation.

## Explicit non-goals

- G64 / Agy admission or any real Agy environment action;
- changing Codex Hook trust or Owner `~/.codex` configuration;
- increasing the production Hook timeout, making it asynchronous, introducing
  a wrapper, PATH shadow, PTY host, self-update, or machine-global daemon;
- public package publication, tagging, or GitHub Release creation;
- unrelated workspace-identity, Hook-repair, or runtime-image redesign.

## Next

`TB-G64-AGY-ADMISSION-REAL-ENVIRONMENT-SPIKE` remains Owner-gated and is not
resumed by this Goal.
