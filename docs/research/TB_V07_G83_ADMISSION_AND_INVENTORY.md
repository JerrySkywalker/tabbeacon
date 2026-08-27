# TB-G83 v0.7 Admission, Source Revalidation & Inventory

- Goal: `TB-G83-V07-ADMISSION-SOURCE-REVALIDATION`
- Train: `TB-V07-24H-NATIVE-ICON-FEASIBILITY-TRAIN-AB-001`
- Repository baseline: `6a2cb7d1984b0dccd06875e057ca6d90d2986134`
- Recorded: 2026-08-28

## Scope freeze

```text
CURRENT_PUBLIC_RELEASE=v0.6.1
NATIVE_TAB_ICON_FEASIBILITY=true
NATIVE_TAB_ICON_PRODUCTION_REQUIRED=false
NATIVE_TAB_ICON_DISPOSITION_REQUIRED=true
PRODUCTION_NATIVE_ICON_INTEGRATION=false
CLAUDE_PROVIDER=DEFERRED
OPENCODE_PROVIDER=DEFERRED
CODEX_APP_SERVER=DEFERRED
NEW_PROVIDER_ADDED=false
```

The first v0.7 train is limited to `TB-G83` through `TB-G86`. G87 branding,
README, documentation-portal, and contributor implementation are explicitly
out of scope; this record is an inventory for those later Goals only.

## Revalidated route decision

The current source record is in
[`WT_NATIVE_TAB_ICON_SOURCE_TRUTH_2026-08.md`](WT_NATIVE_TAB_ICON_SOURCE_TRUTH_2026-08.md).
It freezes the upstream stock source ref and the Microsoft XAML Diagnostics
documentation used by this train.

```text
WT_SOURCE_REVALIDATED=true
WT_SOURCE_REFS_RECORDED=true
WT_INTERNAL_NATIVE_PIPELINE=CONFIRMED
OFFICIAL_NATIVE_ICON_BRIDGE=false
STOCK_PUBLIC_ICON_BRIDGE=false
XAML_ROUTE_STILL_RELEVANT=true
XAML_DIAGNOSTICS_STARTED=false
NATIVE_ICON_MUTATION_STARTED=false
```

No supported stock Windows Terminal tab-icon protocol, command-line option,
or action was found. `--title` and `--tabColor` remain supported public
surfaces, but neither is a native-tab-icon setter. XAML Diagnostics therefore
remains admitted for G84 only under the containment rules below.

## Public repository inventory for G87-G89

Observed at the admitted baseline:

| Surface | Current state | Later Goal action |
| --- | --- | --- |
| `README.md` | Long v0.6.x product/readme structure; no bilingual counterpart or v0.7 information hierarchy | G87 replaces it with the English canonical README and precise current release wording. |
| `README.zh-CN.md` | Absent | G87 adds the Simplified Chinese counterpart and reciprocal language links. |
| Hero project-health badges | No two-badge v0.7 hero policy | G87 adds exactly Rust and real Windows CI badges. |
| Coding-agent presentation | README contains implementation-oriented v0.6 text; deferred providers are mentioned as architecture points | G87/G88 provide a concise, truthful support table and detailed capability guide. |
| `CONTRIBUTING.md` | Present but terse and primarily governance-oriented | G89 creates an external-contributor on-ramp without weakening governance. |
| `SECURITY.md` | Present and concise | G89 reviews public reporting and posture clarity. |
| Documentation navigation | Stable technical docs, ADR index, and research docs exist; `docs/README.md` portal is absent | G88 adds navigation without moving canonical sources. |
| Brand and visual assets | No `docs/assets/brand` directory or product screenshot asset | G87 adds original TabBeacon assets and a privacy-safe real product screenshot. |
| CI | Existing `ci.yml`, `visual-ci.yml`, and release dangling-symlink workflow | G89 evaluates a lightweight repository-owned docs check in the existing Windows CI surface. |

## Brand brief freeze

```text
BRAND_CONCEPT=Tab + Beacon
GENERIC_AI_BRAIN_ROBOT=false
THIRD_PARTY_PROVIDER_MARKS=false
SMALL_MARK_TARGETS=16x16,24x24,32x32
SVG_SCRIPT=false
SVG_EXTERNAL_URL=false
SVG_EMBEDDED_RASTER=false
SVG_FONT_DEPENDENCY=false
```

No brand asset was created or changed by G83.

## G84-G86 disposable-target containment

The experiment root is the exact-owned, noncanonical directory:

```text
EXPERIMENT_ROOT=V:\build\tabbeacon\TB-V07-NATIVE-ICON-FEASIBILITY-001
REPOSITORY_WORKTREE=V:\build\tabbeacon\TB-V07-NATIVE-ICON-FEASIBILITY-001\worktree
```

G84 must create a new, purpose-specific Windows Terminal window with a fresh,
content-minimal `TB-ICON-PROBE-<nonce>` title. A target PID may be admitted
only when all of the following are recorded locally:

1. the process/window began during the launch attempt;
2. its executable is the installed stock Windows Terminal package;
3. exactly one owned top-level window has the freshly generated marker; and
4. its PID is not any Windows Terminal process present before the launch.

If any identity fact is missing, ambiguous, or changes before attach, the
harness must refuse to attach. It must never attach by process image, PID,
active tab, window order, tab index, or title prefix alone. It may use a normal
`WM_CLOSE` request only for a still-proven owned disposable window; it must not
terminate or modify pre-existing Windows Terminal processes.

Evidence is limited to the nonce, sanitized process/window identity facts,
known XAML type/count summaries, HRESULTs, helper lifecycle, and cleanup state.
It excludes terminal text, environment dumps, command history, credentials,
and unrelated window content.

```text
DISPOSABLE_WT_ONLY=true
ACTIVE_OWNER_WT_TARGETED=false
WT_HOSTING_ACTIVE_CODEX_GOAL_TARGETED=false
PRODUCTION_WT_TARGETED=false
WT_SETTINGS_MUTATED=false
WT_PACKAGE_MUTATED=false
PRODUCTION_TABBEACON_MUTATED=false
PRIVATE_ABI_REQUIRED=false
SIGNATURE_SCANNING_REQUIRED=false
MEMORY_PATCHING_REQUIRED=false
ELEVATION_REQUIRED=false
```

## G83 exit

```text
V07_SCOPE_FROZEN=true
README_DOCS_INVENTORY=COMPLETE
BRAND_BRIEF_FROZEN=true
DISPOSABLE_WT_POLICY_FROZEN=true
CODE_CI=N/A
G84_ADMITTED=true
```

The next admitted action is G84 attach/enumerate-only harness work. G85 icon
mutation remains prohibited until G84 proves all of its required substrate and
receives the required focused safety review.
