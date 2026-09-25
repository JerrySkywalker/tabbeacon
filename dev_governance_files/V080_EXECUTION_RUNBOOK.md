# v0.8.0 execution runbook — ZenBook Duo

Parent: [ROADMAP_V08.md](ROADMAP_V08.md)

Acceptance: [V080_ACCEPTANCE_MATRIX.md](V080_ACCEPTANCE_MATRIX.md)

```text
GOAL_ID=TB-V080-MULTICLI-TRUSTED-STATE-TRAIN-001
HOST_TARGET=zenbookduo
TARGET_RELEASE=v0.8.0
INITIAL_NEXT_CAR=G00
```

This is a resumable development train, not an instruction to stop after planning
or after each car. Read current repository instructions first, verify the merged
admission, then execute G00-G06 continuously as prerequisites permit. G07 remains
an explicit public-release boundary. Do not infer completion from this runbook.

## 1. Storage and launch

ZenBook Duo uses these Owner-approved roots:

```text
DEV_ROOT=C:\Dev
DEV_SRC=C:\Dev\src
DEV_WORKTREES=C:\Dev\worktrees
DEV_BUILD=C:\Dev\build
DEV_CACHE=C:\Dev\cache
DEV_ARTIFACTS=C:\Dev\artifacts
CANONICAL_REPO=C:\Dev\src\tabbeacon
TRAIN_WORKTREE_PARENT=C:\Dev\worktrees\tabbeacon
SHARED_CARGO_TARGET=C:\Dev\build\tabbeacon\codex-target
TRAIN_ARTIFACT_PARENT=C:\Dev\artifacts\tabbeacon\TB-V080-MULTICLI-TRUSTED-STATE-TRAIN-001
```

Do not fall back to V drive or create directories directly under `C:\`. These are
ZenBook Duo roots, not an instruction to relocate other machines. Validate inherited
`DEV_*`, `CARGO_TARGET_DIR`, repository path, and worktree topology before running
builds. Do not rewrite global environment/profile settings. Use a session-local
Cargo target override for this train after validating it; retain shared artifacts
rather than creating a full Rust target per car. Any technically necessary isolated
target needs REASON, EXPECTED_STORAGE_GB, TARGET_PATH, and RETENTION_AFTER_GOAL.

Owner launch in PowerShell:

```powershell
Set-Location 'C:\Dev\src\tabbeacon'
codex --yolo
```

Model and reasoning selection remain in the existing CLI UI; this runbook does not
hard-code them into the invocation. `--yolo` does not extend Goal authority or replace
ownership checks. The agent must use an isolated owned worktree for writes and must
not mutate real production configuration just because command approval is bypassed.

The checkout may not yet contain this documentation merge. G00 therefore starts with
read-only local inspection, validates the origin as `JerrySkywalker/tabbeacon`, then
fetches `origin` and reads these files from the exact fetched `origin/main` if needed.
Fetching is permitted; blind `git pull`, branch switches, reset/clean, or stashing
foreign work are not. Confirm that the docs admission is actually merged and that
current main descends from it before implementation. If remote main advanced, inspect
the intervening bounded diff and admit the new exact start head; do not overwrite it.

## 2. G00 preflight and worktree admission

Record local HEAD/branch/status, `git worktree list --porcelain`, origin, exact remote
main, and relevant open PRs. Preserve all unrelated or uncommitted work. An occupied
canonical checkout is not a reason to clean it: create a clean isolated worktree
from admitted remote main. Reuse an existing train worktree only after proving goal,
branch, base, clean state, and absence of a competing writer.

Resolve the actually executed TabBeacon binary and installed source/version; inspect
Codex, Agy, Cursor CLI identity and relevant integration health through bounded,
read-only commands. Do not run setup/reconcile/install/upgrade merely to inventory.
Do not assume the three CLIs are installed or that Agy is still the admitted profile.
Read the existing toolchain/CI configuration rather than upgrading the machine.
Classify unavailable capabilities as prerequisites, not as permission to install or
change another project.

Choose one active Implementer per worktree/branch. Suggested branch shape:
`feat/v080-g02-provider-policy`; use focused branches for independently revertible
changes, not an obligatory branch per task ID. Branch creation, worktree creation,
source edits, focused local tests, pushes, PR creation, reviews, and non-release
merges within this approved scope are permitted after this runbook is invoked by
the Owner, subject to the existing gates. No force push, protection bypass, or
unrelated repository mutation is admitted.

Write one compact initial receipt and proceed into G01. A healthy preflight is not
the final deliverable. Do not ask the Owner to re-approve an already authorized next
car simply because G00 finished.

## 3. Execution discipline

For each car:

1. Read the roadmap, acceptance rows, current code, predecessor receipt, and open PRs.
2. Record exact start head, owned branch/worktree, resolved paths, risk vector, and
   intended proof families before editing.
3. Implement the bounded scope using existing abstractions; add focused tests.
4. Settle one candidate, review the actual diff, run only applicable final gates,
   and record reused evidence with empty risk-diff justification.
5. Open/update a focused PR, ensure exact-head checks and applicable reviews pass,
   merge intentionally, and verify remote main. Do not bypass protections.
6. Update the single train receipt/resume pointer and continue to the next admitted
   car rather than producing another planning-only response.

Preserve the Fast Lane policy. A changed persistent config/privacy/high-risk ownership
boundary receives the focused independent review required by policy; routine docs do
not spawn auditor chains. Read-only reviewers do not become competing writers. If an
independent review required by policy is unavailable, record that gate honestly.

Current hosted CI infrastructure stays unchanged. A quota, billing, runner, or access
failure is an external blocker, not an invitation to deploy a self-hosted runner or
edit protections. Latch once; perform useful independent local work only while
leaving dependent merges/release blocked. Do not repeatedly retry an unchanged lane.

Maintain a single resume receipt under the train artifact parent, with small per-car
proof references where useful. Use the PR conversation for durable public summaries.
Do not create commits solely to restate already accepted evidence; do not commit local
runtime state or invent a second machine-readable governance platform.

## 4. Product and external-write boundaries

Use isolated homes/configuration directories, synthetic content, owned test terminals,
and content-minimal evidence wherever feasible. Prove test redirection/isolation
before calling a candidate setup/uninstall path. Tests must never silently fall back
to the real user's home or authentication/configuration files.

Preserve native CLI launch, fail-open behavior, provider-neutral state, and exact
ownership. No screen/text/ANSI scraping; no model-output parsing; no terminal control
sequences in a provider's JSON stdout. No approval/context/continuation side effects.
Keep v0.7.3 daily binaries and real Codex/Agy/Cursor configuration unchanged unless a
separate explicit Owner action admits a specific mutation.

Any manual Hook trust review remains manual. Do not obtain credentials from unrelated
files or configure another account. If the required real-provider test cannot be
isolated or needs Owner trust, first prepare everything safe, then give one bounded
Owner action rather than modifying production unattended. Missing UIA/L4 proof is
BLOCKED/UNPROVEN, never synthetic evidence mislabeled as a real session.

## 5. Blockers, continuation, and completion

Distinguish a code defect that can be fixed in scope from an external or authority
boundary. Fix the former and continue. For the latter, record:

```text
DISPOSITION=BLOCKED
BLOCKER_LATCHED=true
BLOCKER_FINGERPRINT=<stable-content-minimal-identifier>
BLOCKED_CAR=<Gxx>
BLOCKED_REQUIREMENT=<task-or-family>
LAST_ACCEPTED_HEAD=<sha>
REOPEN_TRIGGER=<specific-change-needed>
OWNER_ACTION=<one-specific-action-or-none>
NEXT_SAFE_WORK=<independent-work-or-none>
```

Do not convert blocked Cursor or an unresolved original #116 class into completed
scope. Continue independent work only where it does not pretend a dependency passed.
If there is no safe useful work, stop with one actionable receipt; do not repeat an
unchanged audit. On a later continuation, re-read source/evidence and resume from the
last accepted boundary rather than repeating completed cars.

Use existing evidence dispositions only: PASS, FAIL, BLOCKED, UNPROVEN, REUSED, N/A.
Implementation progress and release authorization are separate fields. A G06 family
can PASS while G07 is BLOCKED on publication authorization; the whole train is not
complete until its required closure is genuinely satisfied.

Recommended compact receipt (omit irrelevant fields; never invent results):

```text
DISPOSITION=<PASS|FAIL|BLOCKED|UNPROVEN>
GOAL_ID=TB-V080-MULTICLI-TRUSTED-STATE-TRAIN-001
HOST=<observed>
WORKTREE=<owned-path>
REMOTE_MAIN=<sha>
CURRENT_CAR=<Gxx>
LAST_COMPLETED_CAR=<Gxx-or-none>
EXPECTED_HEAD=<candidate-sha>
P_STATUS=<disposition>
C_STATUS=<disposition>
E_STATUS=<disposition>
T_STATUS=<disposition>
CODE_CI=<PASS|FAIL|BLOCKED|N/A>
VISUAL=<PASS|REUSED|BLOCKED|UNPROVEN|N/A>
L4=<PASS|REUSED|BLOCKED|UNPROVEN|N/A>
PERSISTENT_CONFIG_SAFETY=<PASS|REUSED|BLOCKED|UNPROVEN|N/A>
ISSUE_116_ORIGINAL_404=<OBSERVABLE|PARTIAL|UNAVAILABLE|UNPROVEN>
PRS=<numbers-and-exact-heads>
REUSED_EVIDENCE=<source-head-and-risk-diff-or-none>
BLOCKER_LATCHED=<true|false>
PUBLIC_RELEASE_AUTHORIZED=false
PUBLIC_RELEASED=false
OWNER_PRODUCTION_CHANGED=false
RELEASE_RECOMMENDATION=<READY_FOR_OWNER_AUTHORIZATION|NOT_READY>
OWNER_ACTION=<none-or-specific>
NEXT_CAR=<Gxx-or-none>
```

## 6. G06 to G07 handoff

Finish every safe admitted implementation, integration, documentation, and preparation
step before requesting public-release authority. Give the Owner the exact candidate,
package/checksum references, required gate results/reuse, current limitations, and
an explicit proposed publication/adoption transaction. Never set READY while required
provider, visual, safety, or scope evidence is missing.

Only then may external Owner authorization admit G07. Revalidate the candidate and
relevant source/evidence freshness before the one stable v0.8.0 release. Follow
RELEASE_CRITERIA without reusing historical release exceptions. Public release,
public-consumer verification, and Owner production convergence remain separate facts.
If authorization or convergence cannot be satisfied, record that boundary rather
than publishing automatically or falsely claiming whole-train completion.

## 7. Copyable Owner start prompt

```text
执行 TB-V080-MULTICLI-TRUSTED-STATE-TRAIN-001，目标为完整 v0.8.0，不要停留在规划或仅做审计。
先只读核对当前仓库、origin、工作区和 worktree；安全 fetch origin，确认文档准入 PR 已合并。
从精确 origin/main 读取 AGENTS.md、QUALITY_GATES.md、ROADMAP_V08.md、
V080_ACCEPTANCE_MATRIX.md 和 V080_EXECUTION_RUNBOOK.md；本地文件落后时先读远端版本。
使用 ZenBook Duo 的 C:\Dev 根目录，在独立 owned worktree 中作为唯一 Implementer 写入。
按 G00→G06 连续推进 P/C/E/T 全部工作包，分阶段开发、测试、PR、按门禁合并并验证远端 main。
不要每完成一站就重新索取继续许可；真实外部阻塞按 runbook 锁存，并继续安全的独立工作。
不做 self-hosted runner、组织迁移、Fleet、其他 Provider 或无关重构，不削减 Cursor 或术语范围。
不绕过 Hook 信任、配置/进程所有权、必需 CI 或发布门禁，不自动替换日用二进制和真实配置。
最终完成整版候选验收与发布准备；公开发布、生产采用按 G07 单独核准精确候选和动作。
回复只汇报真实结果、已合并 PR、精确 SHA、证据、阻塞和下一步；不要把未执行项目写成 PASS。
```
