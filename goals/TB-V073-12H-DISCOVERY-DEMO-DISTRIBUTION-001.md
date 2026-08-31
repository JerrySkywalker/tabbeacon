# TB-V073 — 12h Discoverability, Demo & Distribution Train

```text
GOAL_ID=TB-V073-12H-DISCOVERY-DEMO-DISTRIBUTION-001
REPOSITORY=V:\src\tabbeacon
ADMISSION_BASE_MAIN=63a76bceb6f1710b87aa687bef1f56abf3bf3268
CURRENT_PUBLIC_RELEASE=v0.7.2
TARGET_PUBLIC_RELEASE=v0.7.3
PLANNING_BRANCH=agent/tb-v073-planning-admission-001
PLANNING_PR=resolve_from_branch_at_start
USEFUL_WORK_BUDGET=UP_TO_12_HOURS
EXECUTION_MODE=UNATTENDED_BOUNDED_IMPLEMENTATION
PUBLIC_RELEASE_AUTHORIZATION=false
```

## Purpose

Execute the first major v0.7.3 implementation train after the planning admission
is accepted. The intended path is:

```text
planning admission
  -> PR #100 / local recovery reconciliation
  -> G99 GitHub discovery surface
  -> G100 deterministic real-WT promo
  -> G101 README/crates.io/package polish
  -> optional G102 release-candidate preparation
```

v0.7.3 is the final bounded public-surface maintenance train before an extended
Owner dogfood period. It is not a runtime/provider feature train.

## Owner positioning decision

Public discovery should be **Codex-led**. The preferred shorthand is:

> **Codex and more**

This is user-acquisition copy, not a compatibility wildcard. Exact production
support truth must remain visible:

```text
CODEX_SUPPORT=production_capability_based
AGY_SUPPORT=production_exact_1.1.19
CLAUDE_SUPPORT=deferred
OPENCODE_SUPPORT=deferred
```

Candidate copy to validate rather than blindly hard-code:

```text
GITHUB_DESCRIPTION=Live status for Codex and more in Windows Terminal — no launcher required.
README_HERO=Live identity and status for Codex—and more—across Windows Terminal tabs, without changing how you launch them.
```

## Non-negotiable product boundaries

```text
DAILY_COMMAND_CODEX=codex
DAILY_COMMAND_AGY=agy
FAIL_OPEN=true
NO_WRAPPER=true
NO_PATH_SHADOW=true
NO_PTY_HOST=true
NO_GLOBAL_DAEMON=true
RUNTIME_BEHAVIOR_CHANGED=false_expected
PROVIDER_BEHAVIOR_CHANGED=false
NEW_PROVIDER_ADDED=false
HOOK_TIMEOUT_CHANGED=false
HOOK_TRUST_BOUNDARY_CHANGED=false
NATIVE_TAB_ICON_DISPOSITION=NO_GO
CLAUDE_PROVIDER=DEFERRED
OPENCODE_PROVIDER=DEFERRED
CODEX_APP_SERVER=DEFERRED
ROADMAP_V08_CREATED=false
```

If implementation unexpectedly requires runtime/provider/Hook semantic changes,
stop and report the exact reason. Do not smuggle a product hotfix into this
promotion train.

## Public-mutation boundary

This Goal authorizes reversible v0.7.3 implementation and GitHub discovery
metadata changes that belong to G99, including repository description/topics.
It does **not** authorize the irreversible G102 public release transaction.

Do not:

- publish crates.io 0.7.3;
- create/push the public v0.7.3 tag;
- create the public GitHub v0.7.3 Release;
- upload public release binaries as the v0.7.3 release transaction.

If all implementation gates complete and only publication remains, stop at:

```text
WAITING_FOR_OWNER_V073_RELEASE_AUTHORIZATION
```

A GitHub social-preview Settings UI upload may also remain an Owner UI action;
it is non-blocking when the exact final PNG is validated and no supported API
exists.

## 0 — Read authoritative planning

Before mutation, read completely:

```text
dev_governance_files/ROADMAP_V073.md
dev_governance_files/V073_DEVELOPMENT_CHECKLIST.md
dev_governance_files/DEVELOPMENT_PAUSE.md
goals/TB-G99-V072-DISCOVERY-SURFACE.md
goals/TB-G100-V072-AUTOMATED-PROMO-DEMO.md
goals/TB-G101-V072-CARGO-DISTRIBUTION-POLISH.md
goals/TB-G102-V072-HARDENING-RELEASE.md
```

The stable G99-G102 IDs remain valid; their old target-version wording must be
reconciled to v0.7.3 before each affected Goal is accepted.

## 1 — Validate and merge the planning PR

Freshly resolve the PR whose head branch is:

```text
agent/tb-v073-planning-admission-001
```

Require:

```text
BASE=main
DRAFT=false_or_mark_ready_after_validation
DOCS_CHECKS=PASS
GIT_DIFF_CHECK=PASS
INDEPENDENT_PLANNING_REVIEW_FINDINGS=0
HIGH_RISK_FINDINGS=0
```

Merge only its exact accepted head. Fetch new `origin/main` and record:

```text
POST_PLANNING_MAIN=<sha>
```

If the planning PR has already been safely merged before this Goal starts,
verify the merged content and continue from fresh current main.

## 2 — Writer lease and worktree discipline

Use the canonical writer-lease primitive:

```text
tools/governance/Invoke-TabBeaconWriterLease.ps1
```

Use one active Implementer writer per mutation train. Read-only subagents may
review/research in parallel, but they must not mutate overlapping source.

Prefer the repository's Fast Lane conventions: focused branch/worktree, focused
local gates, one final hosted exact-head CI per accepted train, and additional
review only where risk requires it.

For noncanonical worktrees, preserve the existing shared Cargo target policy.

## 3 — Reconcile PR #100 and retained recovery evidence

Freshly inspect:

```text
PR=100
EXPECTED_HISTORICAL_REMOTE_HEAD=4731a3ffbca643a4e3d3afcd3b61f1d849eaa434
LOCAL_RECOVERY_CONTEXT=31c076d4458a4c0606e494c1dea452946a92fb15_if_present
```

Classify every retained change as:

```text
G99
G100
STALE_PRE_V072_ASSUMPTION
UNRELATED
```

Preserve scope-clean G99/G100 source/evidence. Explicitly retire the old
`TOPLEVEL_WINDOW_NAME == dynamic tab title` assumption.

Use the v0.7.2 proven ownership path:

```text
unique RUN_ID
  -> exact fixed anchor TabItem
  -> ancestor Window/HWND
  -> exact-owned lifecycle cleanup
```

Do not force-push merely to retain PR #100. Decision:

```text
IF clean current-main integration is safe and history remains comprehensible
THEN continue PR #100
ELSE preserve PR #100 unchanged as historical evidence and create a clean
     successor Train-A PR from current main
```

Record:

```text
TRAIN_A_PR=<number>
PR100_DISPOSITION=<RECONCILED|PRESERVED_HISTORICAL>
FORCE_PUSH_USED=false
```

## 4 — G99: GitHub Discovery Surface

Implement and accept G99.

### Description

Revalidate the Codex-led candidate against current README truth and GitHub
length/display behavior. Prefer:

```text
Live status for Codex and more in Windows Terminal — no launcher required.
```

or a demonstrably better compact equivalent preserving the same positioning.

Require:

```text
DESCRIPTION_TRUTHFUL=true
DESCRIPTION_CODEX_LED=true
DESCRIPTION_PROVIDER_OVERCLAIM=false
```

Apply through supported GitHub API/CLI.

### Topics

Revalidate and choose 6-10 truthful topics. Historical candidates:

```text
coding-agents
codex-cli
ai-coding
windows-terminal
terminal
cli
rust
windows
```

No deferred-provider topics and no keyword stuffing.

### Social preview

Reconcile or regenerate exact TabBeacon-owned:

```text
docs/assets/social/tabbeacon-social-preview.svg
docs/assets/social/tabbeacon-social-preview.png
```

Require:

```text
WIDTH=1280
HEIGHT=640
EXTERNAL_FONT_DEPENDENCY=false
REMOTE_IMAGE_DEPENDENCY=false
ACTIVE_CONTENT=false
PRIVATE_CONTENT_VISIBLE=false
```

Use Codex-led copy if it remains readable at card scale, but keep the visual
truthful for the existing multi-provider product.

If GitHub still lacks a supported social-preview write API:

```text
SOCIAL_PREVIEW_UPLOAD=WAITING_OWNER_UI
```

and continue; do not automate browser cookies/UI.

## 5 — G100: deterministic real-WT promotional demo

Implement and accept G100 using the post-v0.7.2 exact-owned WT lifecycle.

Normative pipeline:

```text
controlled typed showcase fixture
  -> production presentation policy/renderer
  -> one stock Windows Terminal window
  -> exact TabItem -> ancestor HWND
  -> exact-window PNG frames
  -> FFmpeg palettegen/paletteuse
  -> GIF + poster
```

Use synthetic workspace aliases such as:

```text
API
WEB
DOCS
```

The media should show a compact truthful progression through Ready, Working,
ResultReady, and one attention state. Do not hand-paint lifecycle state into
captured frames.

### Tool boundary

Only the previously admitted encoder is allowed:

```text
FFmpeg
Winget package: Gyan.FFmpeg
```

Resolve existing installation first; install through Winget only if absent.
Never vendor/redistribute it.

### Media gates

```text
PROMO_REAL_WINDOWS_TERMINAL=true
PROMO_REAL_TABBEACON_RENDERER=true
PROMO_REAL_MODEL_SESSION=false
NO_DESKTOP_CAPTURE=true
FPS=10
DURATION_SECONDS=8..12
TARGET_WIDTH=960..1100_preferred
GIF_TARGET_SIZE_MIB<=4_preferred
GIF_HARD_LIMIT_MIB<=6
GIF_LOOP=true
PRIVATE_CONTENT_VISIBLE=false
OWNER_USERNAME_VISIBLE=false
PRIVATE_PATH_VISIBLE=false
PRIVATE_REPOSITORY_VISIBLE=false
UNRELATED_WINDOW_VISIBLE=false
```

Commit only:

```text
docs/assets/demo/tabbeacon-demo.gif
docs/assets/demo/tabbeacon-demo-poster.png
```

Do not commit frames, palettes, capture scratch, FFmpeg binaries, or Owner
specific evidence.

### Temporary WT hard gate

Every promo/visual run must finish with:

```text
TEMP_WINDOWS_CLOSED=TEMP_WINDOWS_CREATED
OWNED_TEMP_WT_REMAINING=0
OWNER_WINDOWS_CLOSED=0
BROAD_WINDOWS_TERMINAL_KILL=false
```

Cleanup failure is a real qualification-infrastructure failure and must not be
silently ignored.

## 6 — Accept Train A

Train A contains only reconciled G99/G100 work plus directly necessary
docs/scripts/fixtures.

Run focused local validation, docs/media/privacy gates, then one final hosted
exact-head CI and independent review.

Require:

```text
G99=COMPLETE
G100=COMPLETE
TRAIN_A_SCOPE_CLEAN=true
HOSTED_EXACT_HEAD_CI=PASS
INDEPENDENT_REVIEW_FINDINGS=0
HIGH_RISK_FINDINGS=0
```

Merge exact accepted Train-A head and fetch new main.

## 7 — G101: README & crates.io Distribution Polish

Create a focused Train-B branch/worktree from post-Train-A main.

### README

Use Codex-led hero positioning but preserve exact provider truth.

Normal user Quick Start must become:

```powershell
cargo install tabbeacon
tabbeacon setup
codex
```

The primary path must not require:

```text
--locked
--version
installer
TabBeacon Winget/Scoop
```

Keep exact release verification separately documented as:

```powershell
cargo install tabbeacon --version 0.7.3 --locked
```

Maintain English/zh-CN parity and keep the real-WT GIF high enough to explain
the product quickly.

### crates.io/package audit

Run:

```text
cargo package --locked
```

Inspect package file list/archive. Require:

```text
README_PRIMARY_INSTALL_COMMAND=cargo install tabbeacon
README_EN_ZH_PARITY=PASS
CRATES_IO_RENDER=PASS
CARGO_PACKAGE=PASS
CARGO_PACKAGE_CONTENT_AUDIT=PASS
PROMO_GIF_IN_CRATE=false
SOCIAL_PREVIEW_IN_CRATE=false
PROMO_BUILD_EVIDENCE_IN_CRATE=false
FFMPEG_BINARY_IN_CRATE=false
```

No runtime/provider changes.

## 8 — Accept Train B

Run focused local validation and one final hosted exact-head CI/review.

Require:

```text
G101=COMPLETE
TRAIN_B_SCOPE_CLEAN=true
HOSTED_EXACT_HEAD_CI=PASS
INDEPENDENT_REVIEW_FINDINGS=0
HIGH_RISK_FINDINGS=0
```

Merge exact accepted Train-B head and fetch new main.

## 9 — Optional G102 release-candidate preparation

Only if useful-work budget remains after G99-G101 are durably accepted.

Reconcile the retained G102 planning text to v0.7.3 and prepare a focused
release-candidate branch. It may include the normal repository release-prep
changes, including version `0.7.2 -> 0.7.3`, changelog/release notes, package
checks, Windows artifact preparation, and a draft release PR.

Do not merge/publish the public release transaction under this Goal.

Require before stopping:

```text
RELEASE_CANDIDATE_PREP=<PASS|NOT_STARTED>
PUBLIC_RELEASE_MUTATION=false
```

If all pre-publication gates are green, stop at:

```text
WAITING_FOR_OWNER_V073_RELEASE_AUTHORIZATION
```

## 10 — No v0.8 work

Do not create `ROADMAP_V08.md` or implement any v0.8 option. Do not add Claude,
OpenCode, Codex App Server, native-tab-icon work, installer/package-manager
surfaces, or unrelated reliability/platform changes.

## 11 — 12-hour continuation policy

Use the full useful-work budget. Do not stop after every intermediate PR.
Continue automatically across the admitted chain while truthfully respecting
single-writer and exact-head gates.

Stop early only for:

1. a newly proven P0/P1 runtime/product defect;
2. unsafe repository/PR/window ownership ambiguity;
3. a required Owner Settings UI action that genuinely blocks further work
   (social-preview upload alone does not block);
4. the public v0.7.3 release boundary;
5. useful-work budget exhaustion.

At budget exhaustion, leave all completed work in durable commits/merged PRs and
provide one exact continuation Goal.

## Final receipt

```text
DISPOSITION=<COMPLETE|PARTIAL_PROGRESS|BLOCKED|FAIL|WAITING_FOR_OWNER_V073_RELEASE_AUTHORIZATION>
GOAL_ID=TB-V073-12H-DISCOVERY-DEMO-DISTRIBUTION-001

START_MAIN=<sha>
POST_PLANNING_MAIN=<sha>
FINAL_MAIN=<sha>

PLANNING_PR=<number>
PLANNING_PR_MERGED=<true|false>

PR100_DISPOSITION=<RECONCILED|PRESERVED_HISTORICAL|N/A>
TRAIN_A_PR=<number|N/A>
TRAIN_A_MERGED=<true|false>
TRAIN_B_PR=<number|N/A>
TRAIN_B_MERGED=<true|false>

POSITIONING_PRIMARY=CODEX_AND_MORE
GITHUB_DESCRIPTION=<text|N/A>
GITHUB_TOPICS_COUNT=<count|N/A>
SOCIAL_PREVIEW=<PASS|WAITING_OWNER_UI|FAIL|N/A>

G99=<COMPLETE|PARTIAL|BLOCKED|N/A>
G100=<COMPLETE|PARTIAL|BLOCKED|N/A>
G101=<COMPLETE|PARTIAL|BLOCKED|N/A>
G102_RELEASE_CANDIDATE_PREP=<PASS|NOT_STARTED|BLOCKED>

PROMO_GIF=<path|N/A>
PROMO_GIF_BYTES=<bytes|N/A>
PROMO_GIF_SHA256=<sha|N/A>
PROMO_POSTER=<path|N/A>
PROMO_PRIVACY_REVIEW=<PASS|FAIL|N/A>

TEMP_WINDOWS_CREATED=<count>
TEMP_WINDOWS_CLOSED=<count>
OWNED_TEMP_WT_REMAINING=<count>
OWNER_WINDOWS_CLOSED=0
BROAD_WINDOWS_TERMINAL_KILL=false

README_PRIMARY_INSTALL_COMMAND=<command|N/A>
README_EN_ZH_PARITY=<PASS|FAIL|N/A>
CARGO_PACKAGE=<PASS|FAIL|N/A>
PROMO_GIF_IN_CRATE=<true|false|N/A>
SOCIAL_PREVIEW_IN_CRATE=<true|false|N/A>

RUNTIME_BEHAVIOR_CHANGED=false
PROVIDER_BEHAVIOR_CHANGED=false
NEW_PROVIDER_ADDED=false
ROADMAP_V08_CREATED=false
PUBLIC_RELEASE_MUTATION=false

ACTIVE_HOLDERLESS_LEASE_FINAL=false

NEXT_RECOMMENDED_GOAL=<WAITING_FOR_OWNER_V073_RELEASE_AUTHORIZATION|TB-V073-RELEASE-CLOSEOUT-002|exact blocker>
```
