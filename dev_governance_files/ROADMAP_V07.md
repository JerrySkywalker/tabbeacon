# TabBeacon v0.7 execution roadmap

## Status

ADMITTED from the public v0.6.1 production baseline and its post-release
closeout on `main` at `0dc3c79e32088d4ecc5a975e3ca7ddf9363679dd`.

Public v0.6.1 remains the production baseline until G90 explicitly authorizes
and completes a v0.7.0 release transaction.

This file is the compact execution index for v0.7. The authoritative product
intent, safety boundaries, Native Tab Icon disposition rules, and open-source
presentation requirements live in
[`../goals/TB-V07-NATIVE-TAB-ICON-OSS-POLISH.md`](../goals/TB-V07-NATIVE-TAB-ICON-OSS-POLISH.md).
Each numbered Goal below owns its implementation and exit gates.

## Product theme

**v0.7 — Native Tab Icon Feasibility & Open Source Polish**

v0.7 deliberately combines two independent tracks:

1. **Native Tab Icon Feasibility** — determine, with isolated real Windows
   Terminal evidence, whether stock Windows Terminal can support safe,
   exact-tab, reversible native tab-icon mutation through an officially
   documented instrumentation surface. Production native-icon support is not a
   required outcome.
2. **Open Source Polish** — establish a coherent TabBeacon brand, a mature
   English-first bilingual README, a small project-health badge surface,
   scalable coding-agent support documentation, and a navigable user/design/
   development documentation system.

The release is successful if the Native Tab Icon track reaches a truthful final
engineering disposition (`GO_EXPERIMENTAL`, `GO_PRODUCTION_CANDIDATE`, or
`NO_GO`) and the open-source/documentation track completes. v0.7 must not be
held hostage by a risky Windows Terminal hack.

## Frozen pre-v0.7 source truth

The v0.6.1 source investigation already established:

```text
WT_INTERNAL_NATIVE_PIPELINE=SOURCE_CONFIRMED
STOCK_PUBLIC_ICON_BRIDGE=false
XAML_FEASIBILITY_STARTED=false
```

Stock Windows Terminal contains an internal `Tab::UpdateIcon` path that updates
both tab model state and `TabViewItem.IconSource`. Existing public terminal
control routes do not expose that path: OSC 0/1/2/21 are title controls; the
inspected stock OSC 9001 action route is not a tab-icon transport; the action
model has no public `SetTabIcon`; and `WT_SESSION` is a useful session binding
but not an icon mutation handle.

The prior research identified Microsoft XAML Diagnostics as the only currently
admitted stock-Windows-Terminal feasibility route worth exploring. It is
process instrumentation, not a normal terminal protocol. No attachment occurred
in v0.6.1.

Canonical prior evidence:
[`../docs/research/WT_NATIVE_TAB_ICON_SOURCE_TRUTH_2026-08.md`](../docs/research/WT_NATIVE_TAB_ICON_SOURCE_TRUTH_2026-08.md).

G83 must revalidate current upstream source before any instrumentation work. If
Windows Terminal has gained an official supported tab-icon API, the XAML route
must stop and the official API becomes the preferred research direction.

## Dependency sequence

```text
PUBLIC v0.6.1 + post-release closeout
        ↓
TB-G83   v0.7 Admission, Source Revalidation & Inventory
        ↓
TB-G84   Isolated XAML Diagnostics Harness
        ↓
TB-G85   Exact-Tab Correlation & Native Icon Mutation/Restore
        ↓
TB-G86   Native Icon Reliability Matrix & Final Disposition
        ↓
TB-G87   Brand System & README v2 (English + Simplified Chinese)
        ↓
TB-G88   Documentation Information Architecture & Guides
        ↓
TB-G89   Contributor Experience & Documentation QA/CI
        ↓
TB-G90   v0.7 Hardening & Release
```

G84–G86 are sequential safety gates. G87–G89 may begin only after G83 is
accepted and may overlap the technical research only when there is no shared
writer or visual-review conflict. For unattended development, sequential
execution remains the default.

## Goal index

| Goal | Scope | Estimated effort |
| --- | --- | ---: |
| G83 | freeze v0.7 scope; revalidate current Windows Terminal/native-icon source truth; audit README/docs/brand baseline; establish experiment boundaries | 3–5 h |
| G84 | disposable Windows Terminal launcher; XAML Diagnostics attach/enumerate/detach proof; no icon mutation | 5–8 h |
| G85 | unique-marker exact-tab correlation; snapshot native `IconSource`; mutate one tab; visual/machine verification; exact restore | 6–10 h |
| G86 | multi-tab/window/pane/reload/close/crash matrix; zero-wrong-tab gate; final `GO_EXPERIMENTAL` / `GO_PRODUCTION_CANDIDATE` / `NO_GO` ADR | 5–8 h |
| G87 | TabBeacon SVG brand system; README English canonical + Simplified Chinese counterpart; two-badge policy; coding-agent support section; real product screenshot | 7–10 h |
| G88 | docs portal; getting started; configuration; coding-agent support; troubleshooting; FAQ; design and development guides | 7–11 h |
| G89 | CONTRIBUTING v2; SECURITY/documentation polish; internal-link/SVG/stale-version docs checks; Windows CI integration | 4–6 h |
| G90 | v0.7 release candidate; tests/docs/visual review; v0.6.1→v0.7.0 upgrade; package/public consumers; public release | 4–6 h |
| **Total** | **v0.7.0** | **41–64 h** |

## Track A — Native Tab Icon Feasibility

### Research objective

Answer one engineering question conclusively:

> Can TabBeacon identify exactly one intended tab in a disposable stock Windows
> Terminal instance, mutate its native `IconSource`, verify the visible result,
> restore the exact prior state, and fail open under lifecycle races without
> unsafe/private implementation techniques?

The track does **not** require production native-icon support in v0.7.

### Required isolation model

Before a final GO/NO-GO disposition, all instrumentation work must use a
purpose-created disposable Windows Terminal process. In particular:

```text
ACTIVE_OWNER_WT_TARGETED=false
WT_HOSTING_ACTIVE_CODEX_GOAL_TARGETED=false
PRODUCTION_TABBEACON_MUTATED=false
OWNER_CODEX_CONFIG_MUTATED=false
WT_SETTINGS_MUTATED=false
WT_PACKAGE_MUTATED=false
```

No experiment may attach to a Windows Terminal instance that is hosting the
Codex process executing that experiment.

### Preferred exact-tab correlation

The initial admitted correlation strategy is a content-minimal temporary unique
title marker emitted into the target disposable tab, followed by XAML tree
enumeration.

Mutation is admitted only when:

```text
MATCH_COUNT=1
```

`MATCH_COUNT=0` or `MATCH_COUNT>1` must produce fail-open zero-mutation behavior.
Tab index, active-tab assumptions, process order, window order, or guessed
visual-tree positions are not sufficient ownership proof.

### Mutation / restore proof

G85 must separately prove:

```text
ICON_SNAPSHOT=PASS
ICON_MUTATION=PASS
VISUAL_ICON_CHANGED=true
TARGET_TAB_ONLY=true
ICON_RESTORE=PASS
```

Prefer preserving/restoring the original `IconSource` object/reference or an
exact equivalent representation rather than guessing a replacement path.

The first mechanism proof should use a deterministic probe icon rather than the
final TabBeacon brand mark. Branding is not allowed to obscure mechanism
validation.

### Hard safety gates

The following are non-negotiable:

```text
WRONG_TAB_MUTATION=0
WT_CRASH=0
RESTORE_FAILURE=0
ELEVATION_REQUIRED=false
WT_SETTINGS_MUTATED=false
WT_PACKAGE_MUTATED=false
PRIVATE_ABI_REQUIRED=false
SIGNATURE_SCANNING_REQUIRED=false
MEMORY_PATCHING_REQUIRED=false
ACTIVE_OWNER_WT_TARGETED=false
```

Any observed wrong-tab mutation is an immediate `NO_GO` for the studied
approach. Do not retry until a lucky run and then discard the failure.

### Reliability matrix

G86 must cover representative real cases including:

- one tab;
- multiple tabs in one window;
- multiple Windows Terminal windows;
- split panes;
- pre-existing custom/native icon;
- no pre-existing icon;
- tab rename during/around correlation;
- target tab closing mid-operation;
- target window closing mid-operation;
- settings reload;
- helper failure/crash;
- attach failure;
- duplicate/no marker match;
- TabBeacon title/activity updates occurring in the disposable target;
- Windows Terminal restart/cleanup;
- more than one currently available stock Windows Terminal build/channel when
  practical and safely isolated.

### Final disposition

G86 must produce exactly one of:

#### `GO_EXPERIMENTAL`

The mechanism is repeatably safe in the admitted matrix but is not sufficiently
stable across Windows Terminal implementation/version boundaries for product
admission. Experimental evidence/code may remain isolated and non-installed.

#### `GO_PRODUCTION_CANDIDATE`

The mechanism satisfies the full safety matrix with adequate implementation
stability to justify a **future** productionization train. v0.7 still does not
silently promote it into the normal runtime. A later release must explicitly
accept the new instrumentation trust boundary.

#### `NO_GO`

Exact-tab correlation, restore, stability, or safety is not reliable enough.
Publish a durable ADR and keep the current title-mark backend as production
truth.

All three dispositions count as a successfully completed feasibility track.

## Track B — Open Source Polish

### README language policy

```text
README.md       = canonical English
README.zh-CN.md = full Simplified Chinese counterpart
```

Both files require reciprocal language navigation. v0.7 does not require a
fully duplicated bilingual `docs/` tree; technical/design documentation remains
English canonical unless a later plan explicitly admits broader translation.

### Badge policy

The README hero intentionally separates **project health** from **coding-agent
compatibility**.

The default badge surface contains exactly two project-health badges:

```text
README_BADGE_COUNT=2
README_BADGE_RUST=true
README_BADGE_WINDOWS_CI=true
README_AGENT_BADGES=false
```

The Rust badge represents the repository MSRV/policy (currently Rust 1.97.1 or
newer as declared by the package), not a provider version. The Windows CI badge
must reflect the real repository Windows CI workflow.

Do not add Codex, Agy, Claude, OpenCode, crates.io, downloads, release, license,
or terminal badges to the hero badge row. Release/crates.io/license navigation
may appear as ordinary compact links.

### Coding-agent support surface

README owns a concise **Supported Coding Agents** table. Detailed capability
truth moves to `docs/coding-agent-support.md`.

Production rows may include current admitted providers such as Codex and Agy,
but capability/version semantics must remain truthful:

- Codex compatibility is capability-based; a version string is diagnostic, not
  mutation authority.
- Agy remains bounded by its exact admitted production profile unless later
  evidence changes that policy.
- unsupported/unavailable capabilities must not be inferred merely to claim
  parity.

Claude and OpenCode remain explicitly **DEFERRED**, outside the production
support table and outside v0.7 implementation scope.

### Brand system

Required v0.7 brand assets:

```text
docs/assets/brand/tabbeacon-mark.svg
docs/assets/brand/tabbeacon-logo.svg
docs/assets/brand/tabbeacon-mark-monochrome.svg
docs/assets/brand/tabbeacon-state-strip.svg
```

The visual concept should derive from **Tab + Beacon**, using a terminal/tab
shape plus a compact beacon/status signal language rather than a generic AI
brain/robot. It must not use or imitate third-party provider trademarks.

The mark should remain identifiable at small tab-icon candidate sizes such as
16×16, 24×24, and 32×32 even though production native-icon support is not a
v0.7 requirement.

SVG requirements:

```text
SCRIPT=false
EXTERNAL_URL=false
EMBEDDED_RASTER=false
FONT_DEPENDENCY=false
VIEWBOX=true
TRANSPARENT_BACKGROUND=true
```

A real, privacy-safe Windows Terminal product screenshot is also required for
README presentation; do not use a fabricated UI mockup as the primary product
proof.

### README v2 information order

Recommended canonical order:

1. Logo / project name / concise tagline
2. English ↔ 简体中文 switch
3. Rust + Windows CI badges
4. Releases / crates.io / Documentation / MIT navigation links
5. Why TabBeacon?
6. What It Looks Like
7. Features
8. Supported Coding Agents
9. Quick Start
10. Compatibility
11. How It Works
12. Safety & Privacy
13. Configuration
14. Documentation
15. Contributing
16. License

Code fences must use proper language tags (`powershell`, `toml`, `json`,
`rust`, etc.) so GitHub native syntax highlighting provides color. Do not rely
on unsupported HTML color hacks. GitHub Alerts and Mermaid may be used where
they genuinely improve clarity.

### Documentation information architecture

Add a navigation layer rather than mass-moving historical source-of-truth files.
Existing links/ADRs/research paths should remain stable unless a migration is
independently justified.

Required additions:

```text
docs/README.md
docs/getting-started.md
docs/configuration.md
docs/coding-agent-support.md
docs/troubleshooting.md
docs/faq.md

docs/design/product-principles.md
docs/design/visual-language.md
docs/design/native-tab-icon.md
docs/design/branding.md

docs/development/build-and-test.md
docs/development/release-process.md
```

Existing technical files such as `architecture.md`, `codex-hooks.md`,
`agy-setup.md`, compatibility docs, ADRs, and research remain canonical and are
linked from the portal.

### Troubleshooting scope

The troubleshooting guide should prioritize real observed failure modes,
including:

- Hook trust not granted;
- Hook timeout / shell startup symptoms;
- `upgrade-preflight` blocked state;
- ambiguous package-image processes;
- owned MCP executable lock and ownership-safe drain semantics;
- capability unproven/incompatible diagnostics;
- title fallback/ownership conflicts;
- workspace identity/root-anchor confusion;
- exact Agy profile mismatch.

No troubleshooting procedure may weaken ownership, trust, or fail-open safety
merely for convenience.

### Contributor experience

`CONTRIBUTING.md` must become useful to a normal external contributor without
requiring them to understand the entire internal autonomous-agent governance
system before making a small change. It should cover prerequisites, build/test,
Windows-specific validation, architecture/provider boundaries, documentation
changes, visual changes, PR expectations, and security/privacy constraints.

`AGENTS.md` and governance files remain authoritative for autonomous/internal
execution but are not the only contributor on-ramp.

### Documentation QA / CI

Prefer a small repository-owned PowerShell check rather than introducing a new
large Node toolchain solely for Markdown.

At minimum validate:

```text
INTERNAL_MARKDOWN_LINKS_VALID=true
README_LANGUAGE_LINKS_RECIPROCAL=true
REQUIRED_BRAND_ASSETS_EXIST=true
SVG_WELL_FORMED=true
SVG_ACTIVE_CONTENT=false
STALE_CURRENT_RELEASE_MARKERS=0
DOCS_PORTAL_LINKS_VALID=true
```

English/Chinese natural-language equivalence is review-owned, not a brittle
machine translation-diff requirement. Machine checks should compare critical
commands/invariants/support-table facts where practical.

## Global invariants

v0.7 preserves the production philosophy already established by v0.6.1:

```text
DAILY_COMMAND_CODEX=codex
DAILY_COMMAND_AGY=agy
FAIL_OPEN=true
NO_WRAPPER=true
NO_PATH_SHADOW=true
NO_PTY_HOST=true
GLOBAL_DAEMON_ADDED=false
SELF_UPDATE=false
HOOK_TRUST_BYPASS=false
AUTO_HOOK_TRUST=false
PROJECT_LOCAL_CONFIG=false
PROJECT_FILES_MUTATED_FOR_PREFERENCES=false
RAW_PROMPT_CONTENT_PERSISTED=false
RAW_ASSISTANT_CONTENT_PERSISTED=false
RAW_TOOL_CONTENT_PERSISTED=false
RAW_NATIVE_SESSION_IDS_HUMAN_EXPOSED=false
PROCESS_SESSION_CONTROL=false
REMOTE_CONTROL=false
PROVIDER_CAPABILITY_MUST_BE_PROVEN=true
```

Native-icon experimentation must not weaken these invariants.

## Explicitly deferred from v0.7

Do not fold these into v0.7 merely because time remains:

- Claude provider (`TB-G20` / future successor planning);
- OpenCode provider (`TB-G30` / future successor planning);
- new coding-agent provider implementation of any kind;
- Codex App Server production backend;
- production native-tab-icon integration unless separately admitted after G86
  by a later roadmap/release decision;
- web/remote dashboard;
- process/session/tab control;
- cloud sync;
- self-update;
- automatic Hook trust;
- repository-local TabBeacon settings;
- user-editable naming score weights;
- Hook reliability/history analytics (belongs to Hookstat, not TabBeacon).

## Suggested unattended train partitions

```text
Train A (~10–12 h)
  G83
  G84
  begin G85

Train B (~10–12 h)
  finish G85
  G86

Train C (~10–12 h)
  G87

Train D (~10–12 h)
  G88
  begin G89

Train E (~8–12 h)
  finish G89
  G90
```

Ordinary correctable implementation/test/harness failures should consume the
existing admitted correction budget rather than creating unnecessary Owner
stops. Safety gates, visual acceptance, destructive state boundaries, public
release boundaries, and any ambiguous native-icon instrumentation behavior
remain legitimate stop points.

## v0.7 release definition

```text
TABBEACON_V07_THEME=Native Tab Icon Feasibility & Open Source Polish

NATIVE_TAB_ICON_FEASIBILITY=true
NATIVE_TAB_ICON_PRODUCTION_REQUIRED=false
NATIVE_TAB_ICON_DISPOSITION_REQUIRED=true

README_DEFAULT_LANGUAGE=en-US
README_ZH_CN=true
README_BADGE_COUNT=2
README_BADGE_RUST=true
README_BADGE_WINDOWS_CI=true
README_AGENT_BADGES=false
CODING_AGENT_SUPPORT_TABLE=true
CODING_AGENT_SUPPORT_DETAILS=true

PROJECT_LOGO_SVG=true
BRAND_GUIDE=true
REAL_PRODUCT_SCREENSHOT=true

DOCS_PORTAL=true
GETTING_STARTED=true
CONFIGURATION_GUIDE=true
CODING_AGENT_SUPPORT_GUIDE=true
TROUBLESHOOTING=true
FAQ=true
PRODUCT_PRINCIPLES_DOC=true
VISUAL_LANGUAGE_DOC=true
NATIVE_ICON_DESIGN_DOC=true
CONTRIBUTING_V2=true
DOCS_CI=true

CLAUDE_PROVIDER=DEFERRED
OPENCODE_PROVIDER=DEFERRED
CODEX_APP_SERVER=DEFERRED
```

G90 alone owns the version bump and public-release transaction. Earlier Goals
must not claim `CURRENT_PUBLIC_RELEASE=v0.7.0`.