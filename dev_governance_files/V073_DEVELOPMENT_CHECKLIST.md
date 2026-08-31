# TabBeacon v0.7.3 development checklist

This checklist is the execution companion to [`ROADMAP_V073.md`](ROADMAP_V073.md).
It converts the four stable Goals into concrete implementation and acceptance
items while preserving the post-v0.7.2 runtime/provider freeze.

## Baseline

```text
ADMISSION_BASE_MAIN=63a76bceb6f1710b87aa687bef1f56abf3bf3268
CURRENT_PUBLIC_RELEASE=v0.7.2
TARGET_PUBLIC_RELEASE=v0.7.3
THEME=Discoverability_Demo_Distribution_Polish
POSITIONING_PRIMARY=CODEX_AND_MORE
RUNTIME_BEHAVIOR_CHANGED=false_expected
PROVIDER_BEHAVIOR_CHANGED=false
ROADMAP_V08_CREATED=false
```

## Positioning rules

- Lead discovery copy with **Codex and more** where it improves user acquisition.
- Keep exact support truth adjacent on durable surfaces: Codex production
  capability-based; Agy production exact 1.1.19; Claude/OpenCode deferred.
- Never use `and more` as evidence of generic provider support.
- Preserve literal daily commands `codex` and `agy`.
- Preserve no-wrapper/no-PATH-shadow/no-global-daemon/fail-open invariants.

## V073-R0 — Re-admission and source reconciliation

- [ ] Freshly fetch current `origin/main` and require the admitted baseline or
      perform a bounded re-admission if it moved.
- [ ] Validate/merge the v0.7.3 planning PR before implementation writes.
- [ ] Inspect PR #100 remote head and full diff.
- [ ] Check whether local recovery commit
      `31c076d4458a4c0606e494c1dea452946a92fb15` still exists.
- [ ] Classify each retained PR #100/local-recovery change as G99, G100, stale,
      or unrelated.
- [ ] Reuse scope-clean work only.
- [ ] Do not force-push merely to retain PR #100.
- [ ] If clean reconciliation is unsafe or needlessly complex, preserve PR #100
      as historical evidence and create a fresh Train-A branch/PR from current
      main.
- [ ] Reconcile retained G99-G102 target-version wording to v0.7.3 before each
      affected Goal is accepted.
- [ ] Use canonical writer-lease tooling for every repository mutation phase.

## TB-G99 — GitHub Discovery Surface

### Repository description

- [ ] Re-check current GitHub description and length/display behavior.
- [ ] Prefer Codex-led candidate wording:
      `Live status for Codex and more in Windows Terminal — no launcher required.`
- [ ] Confirm compactness and truthfulness.
- [ ] Confirm `and more` does not imply Claude/OpenCode support.
- [ ] Apply through supported GitHub API/CLI only.

### Topics

- [ ] Revalidate current GitHub Topic usage.
- [ ] Choose 6-10 accurate topics.
- [ ] Re-check historical candidates:
      `coding-agents`, `codex-cli`, `ai-coding`, `windows-terminal`, `terminal`,
      `cli`, `rust`, `windows`.
- [ ] Reject keyword stuffing and deferred-provider topics.
- [ ] Apply through supported GitHub API/CLI only.

### Social preview

- [ ] Reconcile the existing social-preview source from PR #100 if still valid.
- [ ] Use repaired TabBeacon-owned brand assets only.
- [ ] Use Codex-led value copy while keeping the artwork provider-neutral enough
      to remain truthful for Agy.
- [ ] Generate deterministic SVG.
- [ ] Generate deterministic PNG.
- [ ] Require exact 1280x640 dimensions.
- [ ] No external fonts.
- [ ] No remote images or external SVG dependencies.
- [ ] No active content/scripts.
- [ ] No private repository/path/user content.
- [ ] If GitHub still exposes only a Settings UI upload, prepare the final PNG and
      record `SOCIAL_PREVIEW_UPLOAD=WAITING_OWNER_UI`; do not automate browser
      cookies/session UI.

### G99 acceptance

```text
DESCRIPTION_TRUTHFUL=true
DESCRIPTION_CODEX_LED=true
SUPPORTED_PROVIDER_TRUTH_VISIBLE=true
GITHUB_TOPICS_COUNT=6..10
GITHUB_TOPICS_RELEVANT=true
SOCIAL_PREVIEW_SVG=PASS
SOCIAL_PREVIEW_PNG=PASS
SOCIAL_PREVIEW_DIMENSIONS=1280x640
RUNTIME_BEHAVIOR_CHANGED=false
PROVIDER_BEHAVIOR_CHANGED=false
```

## TB-G100 — Automated Real-Windows-Terminal Promo Demo

### Architecture

- [ ] Reuse the v0.7.2 exact-owned Windows Terminal lifecycle.
- [ ] Use unique RUN_ID.
- [ ] Use a fixed exact anchor TabItem.
- [ ] Derive the ancestor Window/HWND from that TabItem.
- [ ] Do not search a dynamic top-level `Window.Name`.
- [ ] Never use desktop-wide capture or coordinate scraping.
- [ ] Never use broad Windows Terminal process/window termination.

### Showcase fixture

- [ ] Reconcile the feature-gated showcase fixture from PR #100 if scope-clean.
- [ ] Use production presentation policy/renderer, not painted/fake state labels.
- [ ] Use synthetic aliases such as `API`, `WEB`, `DOCS`.
- [ ] Demonstrate a compact progression through Ready, Working, ResultReady, and
      one truthful attention state.
- [ ] Do not run a real Codex/Agy model session.
- [ ] Do not expose a new normal production CLI surface solely for marketing.

### Capture and cleanup

- [ ] Create one controlled stock Windows Terminal window per capture run where
      possible.
- [ ] Capture only the exact-owned window.
- [ ] Cleanup on PASS.
- [ ] Cleanup on FAIL.
- [ ] Cleanup on BLOCKED.
- [ ] Cleanup on timeout/exception/cancellation where supported.
- [ ] Require exact ownership revalidation before close.
- [ ] Refuse ambiguous stale recovery.

Required cleanup receipt:

```text
TEMP_WINDOWS_CREATED=<n>
TEMP_WINDOWS_CLOSED=<same n>
OWNED_TEMP_WT_REMAINING=0
OWNER_WINDOWS_CLOSED=0
BROAD_WINDOWS_TERMINAL_KILL=false
```

### Media generation

- [ ] Resolve existing FFmpeg first.
- [ ] If absent, install only `Gyan.FFmpeg` through Microsoft Winget.
- [ ] Do not vendor or redistribute FFmpeg.
- [ ] Capture deterministic PNG frame sequence.
- [ ] Encode using palettegen/paletteuse workflow.
- [ ] Target 10 fps.
- [ ] Target 8-12 seconds.
- [ ] Prefer 960-1100 px width.
- [ ] Prefer <=4 MiB; hard limit <=6 MiB.
- [ ] Infinite loop.

### Committed media

- [ ] `docs/assets/demo/tabbeacon-demo.gif`
- [ ] `docs/assets/demo/tabbeacon-demo-poster.png`
- [ ] Do not commit source frames.
- [ ] Do not commit palette scratch.
- [ ] Do not commit FFmpeg binaries.
- [ ] Do not commit Owner-specific evidence roots.

### Privacy/truth gate

```text
PROMO_REAL_WINDOWS_TERMINAL=true
PROMO_REAL_TABBEACON_RENDERER=true
PROMO_REAL_MODEL_SESSION=false
PRIVATE_CONTENT_VISIBLE=false
OWNER_USERNAME_VISIBLE=false
PRIVATE_PATH_VISIBLE=false
PRIVATE_REPOSITORY_VISIBLE=false
UNRELATED_WINDOW_VISIBLE=false
```

## TB-G101 — README & crates.io Distribution Polish

### README product entry

- [ ] Preserve repaired logo/brand system.
- [ ] Preserve English canonical README + zh-CN parity.
- [ ] Keep exactly the intended compact badge set.
- [ ] Move the real-WT promo high enough to communicate value quickly.
- [ ] Use a Codex-led hero such as:
      `Live identity and status for Codex—and more—across Windows Terminal tabs,
      without changing how you launch them.`
- [ ] Keep exact Supported Coding Agents table near enough to prevent overclaim.

### Primary install path

Normal user path:

```powershell
cargo install tabbeacon
tabbeacon setup
codex
```

- [ ] Remove `--locked` from the primary README install command.
- [ ] Do not require a version pin in normal Quick Start.
- [ ] Keep `--locked` only for advanced/release verification.
- [ ] Preserve separate Agy setup/daily command truth.

### crates.io rendering

- [ ] Check README rendering from crates.io context.
- [ ] Ensure important links/media resolve or degrade gracefully.
- [ ] Use stable absolute GitHub media references when required.
- [ ] Do not make crates.io depend on repository-relative marketing paths that
      cannot resolve.

### Cargo package audit

- [ ] Audit name/version/rust-version/license/repository/readme/keywords/categories.
- [ ] Run `cargo package --locked`.
- [ ] Inspect package file list/archive.
- [ ] Confirm `Cargo.lock` release behavior remains correct.
- [ ] Confirm promo GIF is excluded from `.crate`.
- [ ] Confirm social preview is excluded from `.crate`.
- [ ] Confirm promo scratch/evidence is excluded from `.crate`.
- [ ] Confirm FFmpeg binaries are absent.

Required:

```text
README_PRIMARY_INSTALL_COMMAND=cargo install tabbeacon
README_EN_ZH_PARITY=PASS
CARGO_PACKAGE=PASS
CARGO_PACKAGE_CONTENT_AUDIT=PASS
PROMO_GIF_IN_CRATE=false
SOCIAL_PREVIEW_IN_CRATE=false
PROMO_BUILD_EVIDENCE_IN_CRATE=false
RUNTIME_BEHAVIOR_CHANGED=false
```

## TB-G102 — v0.7.3 Hardening & Public Release

### Release preparation

- [ ] Require G99 accepted.
- [ ] Require G100 accepted.
- [ ] Require G101 accepted.
- [ ] Confirm no runtime/provider semantic changes entered the train.
- [ ] Bump `0.7.2 -> 0.7.3` only in G102.
- [ ] Update Cargo metadata/lockfile/changelog/release notes/current-facing docs.
- [ ] Release notes describe only discovery, demo, and distribution polish.

### Final gates

- [ ] `cargo fmt --check`
- [ ] `cargo test --all-targets --locked`
- [ ] `cargo clippy --all-targets --all-features --locked -- -D warnings`
- [ ] `cargo package --locked`
- [ ] docs checks
- [ ] media dimension/file-size/privacy checks
- [ ] package-content audit
- [ ] Windows x64 artifact build
- [ ] SHA256 sidecar
- [ ] exact-head hosted CI
- [ ] independent review
- [ ] zero high-risk findings

### Public transaction

Only after explicit Owner release authorization:

- [ ] merge exact accepted release head;
- [ ] publish crates.io `tabbeacon 0.7.3`;
- [ ] create immutable `v0.7.3` tag at release SHA;
- [ ] create GitHub Release `v0.7.3`;
- [ ] upload Windows ZIP + sidecar;
- [ ] verify public metadata/assets.

### Fresh consumers

- [ ] normal `cargo install tabbeacon` resolves `0.7.3`;
- [ ] exact `cargo install tabbeacon --version 0.7.3 --locked` passes;
- [ ] fresh GitHub ZIP consumer verifies sidecar hash, extracts, and reports
      `tabbeacon 0.7.3`.

### Closeout

- [ ] metadata-only closeout if required;
- [ ] `CURRENT_PUBLIC_RELEASE=v0.7.3`;
- [ ] `ACTIVE_FEATURE_DEVELOPMENT=PAUSED`;
- [ ] PR #100/successor is closed/merged or retained with truthful historical
      disposition;
- [ ] `ROADMAP_V08_CREATED=false`;
- [ ] `NEXT_RECOMMENDED_GOAL=LONG_TERM_DOGFOOD_NO_ACTIVE_DEVELOPMENT`.

## Long-term dogfood checklist

After v0.7.3 release, no v0.8 feature train should begin automatically.
Recommended observation period: 4 weeks minimum, 6-8 weeks preferred.

Observe:

- [ ] multiple Codex CLI upgrades;
- [ ] multi-subagent long turns;
- [ ] ResultReady and SessionEnd stability;
- [ ] Ctrl+C/abnormal exits;
- [ ] multi-tab, multi-repo, worktree behavior;
- [ ] TabBeacon binary upgrade/replaceability;
- [ ] worker/process/lease cleanliness;
- [ ] temporary WT cleanup remains zero-residue;
- [ ] doctor/hooks/sessions diagnostics remain explanatory;
- [ ] Agy coexistence remains stable.

Suggested v0.8 admission threshold:

```text
DOGFOOD_WEEKS>=4
P0_COUNT=0
P1_COUNT=0
REPEATED_UNKNOWN_HOOK_FAILURE=false
HIGH_FREQUENCY_MANUAL_RECOVERY=false
TEMP_WT_RESIDUE=false
CODEX_UPGRADES_SURVIVED>=2_preferred
```

## Effort budget

```text
READMISSION_AND_RECONCILIATION=1..2 h
G99=1..2 h
G100=3..5 h
G101=1.5..2.5 h
G102=3..4 h
LIKELY_TOTAL=11..15 h
```
