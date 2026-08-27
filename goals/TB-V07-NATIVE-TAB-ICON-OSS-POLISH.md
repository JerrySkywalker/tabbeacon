# TabBeacon v0.7 Master Goal — Native Tab Icon Feasibility & Open Source Polish

## Status

ADMITTED after public v0.6.1 and the v0.6.1 post-release closeout.

Planning baseline:

```text
PUBLIC_RELEASE=v0.6.1
RELEASE_SHA=b3c1ee91036683bee9ebd1e15020364cb556c2a4
POST_RELEASE_MAIN=0dc3c79e32088d4ecc5a975e3ca7ddf9363679dd
```

Execution index:
[`../dev_governance_files/ROADMAP_V07.md`](../dev_governance_files/ROADMAP_V07.md).

## Objective

Deliver v0.7 as a bounded research-and-polish release with two independent
outcomes:

1. reach a conclusive, evidence-backed engineering disposition on safe native
   Windows Terminal tab-icon mutation without requiring that feature to enter
   production; and
2. make the public repository read, look, and navigate like a mature open-source
   project through a TabBeacon-specific brand, English-first bilingual README,
   scalable coding-agent compatibility presentation, contributor onboarding,
   and coherent user/design/development documentation.

Claude and OpenCode provider adaptation remain deferred. v0.7 adds no new
coding-agent provider.

## Why v0.7 exists

The v0.6.x line established reliable daily behavior, provider-neutral
presentation, Codex capability-based compatibility, Agy production admission,
upgrade-safe runtime behavior, and Provider Visual Identity. The largest
remaining Windows Terminal visual question is whether TabBeacon can control a
**native tab icon** rather than only the provider/title mark and tab color.

At the same time, the repository has accumulated strong technical and governance
documentation but its public presentation is still closer to an engineering
workspace than a polished OSS project. v0.7 treats repository presentation and
information architecture as product work rather than incidental cleanup.

## Track A — Native Tab Icon Feasibility

### Existing source truth

Prior source research established:

```text
WT_INTERNAL_NATIVE_PIPELINE=SOURCE_CONFIRMED
STOCK_PUBLIC_ICON_BRIDGE=false
```

Stock Windows Terminal internally updates tab icon model/UI state through
`Tab::UpdateIcon` / `TabViewItem.IconSource`, while the inspected public
terminal/action routes do not expose a supported native-icon setter. `WT_SESSION`
remains a session-binding value, not an icon handle.

Microsoft XAML Diagnostics is the only currently admitted stock-Windows-Terminal
instrumentation route for feasibility research. It must be treated as an
explicit trust boundary and never confused with a supported terminal protocol.

G83 must revalidate current upstream source before any XAML work. An official
supported tab-icon API, if now available, supersedes the instrumentation plan.

### Feasibility is the product requirement

v0.7 does **not** require native icon production support.

Required terminal state:

```text
NATIVE_TAB_ICON_FEASIBILITY=true
NATIVE_TAB_ICON_PRODUCTION_REQUIRED=false
NATIVE_TAB_ICON_DISPOSITION_REQUIRED=true
```

G86 must finish with exactly one:

```text
GO_EXPERIMENTAL
GO_PRODUCTION_CANDIDATE
NO_GO
```

No indefinite `UNPROVEN` loop is acceptable after the admitted matrix has been
executed unless a hard external platform limitation genuinely prevents the
matrix from running; such a limitation must be recorded explicitly and narrowed
before release planning continues.

### Experiment isolation

All attachment/mutation work before final disposition uses only disposable
Windows Terminal instances created for the experiment.

Forbidden targets:

- any Windows Terminal hosting the Codex process performing development;
- any normal Owner production Windows Terminal window;
- another user's process;
- Windows Terminal package/settings state;
- global machine services.

Required invariants:

```text
ACTIVE_OWNER_WT_TARGETED=false
WT_HOSTING_ACTIVE_CODEX_GOAL_TARGETED=false
WT_SETTINGS_MUTATED=false
WT_PACKAGE_MUTATED=false
PRODUCTION_TABBEACON_MUTATED=false
```

### Exact-tab correlation contract

The initial allowed correlation design is:

```text
disposable target tab
  -> temporary unique content-minimal title marker
  -> XAML visual/model enumeration
  -> exactly one matching TabViewItem
  -> icon mutation admitted
```

Only `MATCH_COUNT=1` grants mutation authority.

These are insufficient by themselves:

- active tab;
- tab ordinal/index;
- window order;
- process enumeration order;
- guessed XAML child offsets;
- title prefix without a unique nonce;
- matching process image alone.

Zero or multiple matches must preserve all icon state.

### Mutation/restore contract

The mechanism proof must separately establish:

```text
ICON_SNAPSHOT=PASS
ICON_MUTATION=PASS
VISUAL_ICON_CHANGED=true
TARGET_TAB_ONLY=true
ICON_RESTORE=PASS
```

Restore means the prior tab icon state is restored accurately, including the
case where no explicit icon existed. The experiment should retain the original
object/semantic representation where the API permits; it must not fake restore
by applying a generic default icon.

Use a deterministic probe icon for mechanism qualification. Only after the
mechanism is understood may the brand mark be used for visual exploration.

### Hard rejection conditions

Any of these rejects the studied approach for production candidacy:

```text
WRONG_TAB_MUTATION>0
WT_CRASH>0 caused by admitted interaction
RESTORE_FAILURE>0
ELEVATION_REQUIRED=true
PRIVATE_ABI_REQUIRED=true
SIGNATURE_SCANNING_REQUIRED=true
MEMORY_PATCHING_REQUIRED=true
WT_PACKAGE_MUTATION_REQUIRED=true
```

A wrong-tab event must be preserved as evidence; do not erase it through reruns
or reinterpretation.

### Reliability matrix

G86 covers at least:

- one tab;
- multiple tabs;
- multiple windows;
- split panes;
- an existing native/custom icon;
- no prior icon;
- tab rename;
- tab close race;
- window close race;
- settings reload;
- attach failure;
- helper crash/failure;
- no marker match;
- duplicate marker match;
- concurrent TabBeacon title/activity changes in the disposable session;
- WT cleanup/restart;
- more than one current stock Windows Terminal build/channel where practical.

### Disposition semantics

#### GO_EXPERIMENTAL

The proof works safely inside the admitted environment but remains too tied to
implementation details/version behavior for production. Experimental code may
remain under a clearly non-installed/non-packaged experiment path.

#### GO_PRODUCTION_CANDIDATE

The proof is robust enough to justify a **future** productization plan. This
status does not itself permit v0.7 runtime integration. Productization changes
the trust boundary and belongs to a separately admitted later train/version.

#### NO_GO

The route is not reliable/safe enough. Record an ADR explaining the observed
failure boundary and keep the stable TitleMarkBackend as production truth.

All three are successful completion states for v0.7 feasibility.

## Track B — Open Source Polish

### Public README language model

```text
README.md       = English canonical
README.zh-CN.md = Simplified Chinese counterpart
```

Each file links to the other near the hero. README is the bilingual public
surface; the entire technical documentation tree is not duplicated in v0.7.

### Hero badge policy

Project-health badges and provider compatibility are separate concepts.

The default README hero contains exactly two badges:

```text
README_BADGE_COUNT=2
README_BADGE_RUST=true
README_BADGE_WINDOWS_CI=true
README_AGENT_BADGES=false
```

The Rust badge reflects the package/repository MSRV policy. The Windows CI badge
reflects the real hosted Windows CI workflow.

Do not add coding-agent, release, downloads, crates.io, license, or Windows
Terminal badges to the hero row. Ordinary compact links may expose Releases,
crates.io, Documentation, and MIT License.

### Coding-agent compatibility is a first-class section

README contains a concise **Supported Coding Agents** table, not provider badges.
Detailed provider/capability truth is in `docs/coding-agent-support.md`.

The presentation must scale to additional providers later without redesigning
the hero. Each provider row/report must distinguish policy from proof:

- Codex: compatibility is capability-based; version strings are diagnostic only.
- Agy: report the exact currently admitted profile/capability envelope.
- Claude: deferred, not production-supported in v0.7.
- OpenCode: deferred, not production-supported in v0.7.

Never place deferred providers in the production-support table in a way that
implies current support.

### Brand requirements

The TabBeacon brand is based on **Tab + Beacon**, not on a generic AI robot or
brain.

Required assets:

```text
docs/assets/brand/tabbeacon-mark.svg
docs/assets/brand/tabbeacon-logo.svg
docs/assets/brand/tabbeacon-mark-monochrome.svg
docs/assets/brand/tabbeacon-state-strip.svg
```

Desired visual language:

- terminal/tab geometry;
- compact beacon/status point and signal motif;
- semantic relation to the product's existing status palette;
- distinguishable small mark suitable for possible future 16×16/24×24/32×32
  tab-icon use;
- no copied or derivative provider trademarks.

SVG safety/portability:

```text
SCRIPT=false
EXTERNAL_URL=false
EMBEDDED_RASTER=false
FONT_DEPENDENCY=false
VIEWBOX=true
TRANSPARENT_BACKGROUND=true
```

README should include one real privacy-safe Windows Terminal product screenshot,
not a mock primary product proof.

### README v2 content contract

Canonical order should remain concise and newcomer-oriented:

1. hero/logo/tagline/language links;
2. Rust + Windows CI badges;
3. compact Releases/crates.io/Docs/MIT links;
4. Why TabBeacon?;
5. What It Looks Like;
6. Features;
7. Supported Coding Agents;
8. Quick Start;
9. Compatibility;
10. How It Works;
11. Safety & Privacy;
12. Configuration;
13. Documentation;
14. Contributing;
15. License.

Examples should use correct Markdown language fences so GitHub native syntax
highlighting supplies code color. Do not depend on HTML color tricks that GitHub
may sanitize. GitHub Alerts/Mermaid are allowed when they clarify structure.

### Documentation portal

v0.7 adds a navigation layer without gratuitously moving historical technical
source-of-truth paths.

Required new user documents:

```text
docs/README.md
docs/getting-started.md
docs/configuration.md
docs/coding-agent-support.md
docs/troubleshooting.md
docs/faq.md
```

Required design documents:

```text
docs/design/product-principles.md
docs/design/visual-language.md
docs/design/native-tab-icon.md
docs/design/branding.md
```

Required development documents:

```text
docs/development/build-and-test.md
docs/development/release-process.md
```

Existing architecture, Codex/Agy integration, capability, ADR, and research
documents remain at stable paths and are indexed from `docs/README.md`.

### Product-principles document

Must make the project's distinctive invariants understandable outside internal
governance prose:

```text
literal daily provider command
fail-open behavior
manual trust
no wrapper
no PATH shadow
no PTY host
no global daemon baseline
offline-first workspace identity
provider-neutral core
provider capability must be proven
privacy/content minimization
```

### Troubleshooting document

Prioritize observed operational cases rather than generic advice:

- untrusted Hooks;
- Hook timeout/shell latency;
- `upgrade-preflight` blocked;
- ambiguous process preservation;
- owned MCP package lock and safe drain;
- compatibility unproven/incompatible;
- title ownership/fallback behavior;
- workspace/root-anchor identity confusion;
- exact Agy-profile mismatch.

No troubleshooting step may recommend bypassing ownership or trust gates.

### CONTRIBUTING v2

The current internal governance remains authoritative, but the public
`CONTRIBUTING.md` must become a practical external on-ramp:

- prerequisites;
- clone/build/run;
- tests;
- Windows-specific tests;
- architecture/provider boundary;
- docs changes;
- visual changes;
- pull-request expectations;
- security/privacy constraints;
- links to deeper governance for contributors doing high-risk work.

### Documentation CI

Prefer repository-owned lightweight checks, likely PowerShell integrated with
existing Windows CI, rather than adding a large toolchain solely for Markdown.

Minimum machine gates:

```text
INTERNAL_MARKDOWN_LINKS_VALID=true
README_LANGUAGE_LINKS_RECIPROCAL=true
REQUIRED_BRAND_ASSETS_EXIST=true
SVG_WELL_FORMED=true
SVG_ACTIVE_CONTENT=false
STALE_CURRENT_RELEASE_MARKERS=0
DOCS_PORTAL_LINKS_VALID=true
```

Critical EN/ZH commands/invariants/support-table facts should be checked where
practical. Natural-language translation quality remains a human review concern.

## Goal dependency graph

```text
G83
 ├─> G84 -> G85 -> G86
 └─> G87 -> G88 -> G89
                  \   /
                   G90
```

For unattended single-writer execution, the recommended order remains strictly
G83 → G84 → G85 → G86 → G87 → G88 → G89 → G90. The graph only documents
logical independence; it is not permission to create conflicting writers.

## Goal summaries

### TB-G83 — v0.7 Admission, Source Revalidation & Inventory

Freeze scope, re-check current Windows Terminal/public APIs, inventory README/
docs/brand debt, establish disposable experiment roots, and prevent stale source
assumptions from driving XAML work.

Estimated: **3–5 h**.

### TB-G84 — Isolated XAML Diagnostics Harness

Prove disposable WT launch plus documented XAML diagnostics attach, visual/model
tree enumeration, and clean detach. **No icon mutation in this Goal.**

Estimated: **5–8 h**.

### TB-G85 — Exact-Tab Correlation & Native Icon Mutation/Restore

Use a unique marker to identify exactly one TabViewItem, snapshot icon state,
mutate a deterministic probe icon, verify the intended tab only, and restore the
original state.

Estimated: **6–10 h**.

### TB-G86 — Native Icon Reliability Matrix & Final Disposition

Run lifecycle/concurrency/version matrix, preserve every wrong-tab/restore/crash
finding, and publish the final GO/NO-GO ADR.

Estimated: **5–8 h**.

### TB-G87 — Brand System & README v2

Create SVG brand assets, real product screenshot, English canonical README,
Simplified Chinese README, two-badge hero, supported-agent section, and modern
GitHub-native presentation.

Estimated: **7–10 h**.

### TB-G88 — Documentation Information Architecture & Guides

Add docs portal plus user/design/development guide set while preserving existing
technical source paths.

Estimated: **7–11 h**.

### TB-G89 — Contributor Experience & Documentation QA/CI

Rewrite CONTRIBUTING for external contributors, polish security/docs metadata,
and add repository-owned documentation checks to Windows CI.

Estimated: **4–6 h**.

### TB-G90 — v0.7 Hardening & Release

Only here bump to 0.7.0, perform full release gates, validate docs/brand
rendering, test fresh install and v0.6.1→v0.7.0 upgrade, then publish only after
explicit release admission/authorization required by current governance.

Estimated: **4–6 h**.

## Long-train guidance

Recommended partitions:

```text
Train A (~10–12h): G83 + G84 + begin G85
Train B (~10–12h): finish G85 + G86
Train C (~10–12h): G87
Train D (~10–12h): G88 + begin G89
Train E (~8–12h): finish G89 + G90
```

For long unattended execution:

- do not idle to consume a time budget;
- continue useful admitted work when predecessor gates pass;
- do not fabricate visual or real-environment PASS;
- ordinary correctable test/build/harness defects should be fixed within the
  normal correction budget;
- stop on destructive/ambiguous instrumentation, wrong-tab mutation, Owner-only
  visual acceptance, security/privacy uncertainty, or public release boundary;
- keep durable receipts/branches so a later process can resume without repeating
  expensive evidence unnecessarily.

## Risk boundaries

Native-icon experimentation changes the risk model even when it does not change
production code. Treat XAML attachment and any injected diagnostics component as
a high-risk experimental boundary requiring focused independent review before a
GO disposition.

Brand/docs work does not justify relaxing runtime safety gates. Conversely,
Native Icon failure must not block README/docs completion or v0.7 release when a
truthful final `NO_GO` ADR exists.

## Global product invariants

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

## Explicit non-goals

v0.7 does not include:

- Claude provider implementation;
- OpenCode provider implementation;
- any third new coding-agent provider;
- Codex App Server production backend;
- production native-icon runtime integration unless a later separately admitted
  plan promotes G86 evidence;
- web dashboard or remote control;
- session kill/resume/tab focus control;
- automatic Hook trust;
- cloud sync;
- self-update;
- repository-local settings;
- editable naming-score weights;
- Hook reliability/history analytics.

Claude/OpenCode stay deferred even if time remains after the documentation
track.

## Completion

v0.7 planning/research implementation is complete when G83–G89 are accepted and
G86 has one final disposition.

v0.7.0 is complete only when G90 publishes/validates the public release under an
explicit release transaction and the public release truth is reconciled.

A `NO_GO` native-icon ADR is compatible with a successful v0.7.0 release.