# v0.7 Admission Pointer

## Status

ADMITTED after public TabBeacon v0.6.1 and post-release closeout.

Current public production truth remains:

```text
CURRENT_PUBLIC_RELEASE=v0.6.1
RELEASE_SHA=b3c1ee91036683bee9ebd1e15020364cb556c2a4
```

v0.7 is a **planned next release**, not a public version yet.

## Theme

**Native Tab Icon Feasibility & Open Source Polish**

## Authoritative planning files

Execution roadmap:
[`ROADMAP_V07.md`](ROADMAP_V07.md)

Master Goal:
[`../goals/TB-V07-NATIVE-TAB-ICON-OSS-POLISH.md`](../goals/TB-V07-NATIVE-TAB-ICON-OSS-POLISH.md)

Goal sequence:

```text
TB-G83 — v0.7 Admission, Source Revalidation & Inventory
TB-G84 — Isolated XAML Diagnostics Harness
TB-G85 — Exact-Tab Correlation & Native Icon Mutation/Restore
TB-G86 — Native Icon Reliability Matrix & Final Disposition
TB-G87 — Brand System & README v2
TB-G88 — Documentation Information Architecture & Guides
TB-G89 — Contributor Experience & Documentation QA/CI
TB-G90 — v0.7 Hardening & Release
```

## Frozen scope

```text
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
DOCS_PORTAL=true
CONTRIBUTING_V2=true
DOCS_CI=true

CLAUDE_PROVIDER=DEFERRED
OPENCODE_PROVIDER=DEFERRED
CODEX_APP_SERVER=DEFERRED
```

The Native Tab Icon track succeeds with any truthful final disposition:

```text
GO_EXPERIMENTAL
GO_PRODUCTION_CANDIDATE
NO_GO
```

A `NO_GO` result does not block v0.7 if the research is conclusively documented
and the open-source polish track completes.

## Execution rule

Long unattended development should start from the exact current remote `main`,
read `AGENTS.md`, current quality gates, `ROADMAP_V07.md`, the Master Goal, and
the active numbered Goal file. Do not infer authority from this pointer alone.

Provider expansion is deliberately out of scope. Claude and OpenCode remain
deferred until a later explicitly admitted roadmap.