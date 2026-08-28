# TB-G83 — v0.7 Admission, Source Revalidation & Inventory

## Status

PLANNED after public v0.6.1 and its post-release closeout.

## Purpose

Open the v0.7 train without prematurely starting instrumentation or changing
production behavior. Revalidate the Native Tab Icon source truth against current
stock Windows Terminal, freeze the exact v0.7 scope/safety gates, inventory the
public README/docs/brand baseline, and prepare deterministic disposable
experiment boundaries for G84–G86.

Master plan:
[`TB-V07-NATIVE-TAB-ICON-OSS-POLISH.md`](TB-V07-NATIVE-TAB-ICON-OSS-POLISH.md).

Execution index:
[`../dev_governance_files/ROADMAP_V07.md`](../dev_governance_files/ROADMAP_V07.md).

## Entry baseline

Expected planning baseline:

```text
CURRENT_PUBLIC_RELEASE=v0.6.1
RELEASE_SHA=b3c1ee91036683bee9ebd1e15020364cb556c2a4
POST_RELEASE_MAIN=0dc3c79e32088d4ecc5a975e3ca7ddf9363679dd
XAML_FEASIBILITY_STARTED=false
NATIVE_ICON_WORK_STARTED=false
```

If remote `main` moved, inspect the delta before admitting implementation. A
normal descendant metadata/dogfood change may be accepted; an overlapping
native-icon/docs train requires reconciliation before continuing.

## Workstream A — source revalidation

Re-check the latest available **stock** Windows Terminal source and relevant
public Microsoft documentation before relying on v0.6.1 research.

At minimum revisit:

- internal `Tab::UpdateIcon` / icon model and `TabViewItem.IconSource` path;
- OSC 0/1/2/21 title dispatch;
- OSC 9001 / WT action dispatch;
- action argument model for any newly added `SetTabIcon` or equivalent;
- `WT_SESSION` semantics;
- Windows Terminal public issues/documentation for terminal-controlled tab icons;
- current Microsoft XAML Diagnostics public API/documentation and support
  boundary.

Record exact source refs/versions/dates.

### Official-API supersession rule

If a supported stock Windows Terminal API now provides safe terminal/tab icon
mutation, stop the XAML-specific implementation plan and return a disposition
that the official API should replace the instrumentation route. Do not continue
XAML merely because it was previously planned.

Required classification:

```text
WT_INTERNAL_NATIVE_PIPELINE=<CONFIRMED|CHANGED|UNPROVEN>
STOCK_PUBLIC_ICON_BRIDGE=<true|false|unproven>
XAML_ROUTE_STILL_RELEVANT=<true|false>
```

## Workstream B — public repository inventory

Audit current public-facing repository material without rewriting it yet.

Inventory:

- `README.md` information hierarchy and stale release/version wording;
- absence/presence of `README.zh-CN.md`;
- current badge policy;
- current coding-agent support presentation;
- `CONTRIBUTING.md` external contributor usability;
- `SECURITY.md` public clarity;
- `docs/` files and current stable technical source-of-truth paths;
- current architecture/ADR/research navigation;
- brand/logo assets;
- README screenshots/visual assets;
- existing CI surfaces suitable for lightweight docs checks.

Produce a content-minimal planning receipt or repository planning note listing
what G87–G89 must add/fix. Do not move historical docs in G83.

## Workstream C — brand requirements freeze

Freeze the design brief only; do not spend G83 polishing the logo.

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

## Workstream D — experiment containment design

Define the G84/G85 disposable test root and process policy.

The planned harness must be able to prove:

```text
DISPOSABLE_WT_ONLY=true
ACTIVE_OWNER_WT_TARGETED=false
WT_HOSTING_ACTIVE_CODEX_GOAL_TARGETED=false
WT_SETTINGS_MUTATED=false
WT_PACKAGE_MUTATED=false
PRODUCTION_TABBEACON_MUTATED=false
```

The experiment must use a separately launched disposable Windows Terminal
instance/window with a unique correlation namespace. It must never attach to the
terminal hosting the development Codex process.

Specify evidence paths, cleanup ownership, and how process/window identity will
be proven without dumping private terminal content.

## Workstream E — Goal train admission

Revalidate and, if necessary, tighten the G84–G90 files before starting G84.
Do not broaden v0.7 into provider work.

Explicit deferred state:

```text
CLAUDE_PROVIDER=DEFERRED
OPENCODE_PROVIDER=DEFERRED
NEW_PROVIDER_ADDED=false
CODEX_APP_SERVER_PRODUCTION=DEFERRED
```

## Validation

G83 is primarily planning/research. Required validation is source/document
review and consistency, not broad runtime retesting.

Run repository docs/link checks if already available and appropriate. Do not
create an artificial real-Codex or XAML L4 gate when no runtime behavior changed.

## Risk vector

```text
CODE_CHANGED=false
PRESENTATION_CHANGED=false
PROVIDER_CHANGED=false
USER_PERSISTENT_CONFIG_CHANGED=false
SECURITY_OR_PRIVACY_CHANGED=false
EXPERIMENTAL_INSTRUMENTATION_STARTED=false
RELEASE_BOUNDARY=false
```

If implementation/instrumentation begins, G83 scope has been exceeded.

## Acceptance

```text
V07_SCOPE_FROZEN=true
WT_SOURCE_REVALIDATED=true
WT_SOURCE_REFS_RECORDED=true
STOCK_PUBLIC_ICON_BRIDGE=<true|false|unproven>
XAML_ROUTE_STILL_RELEVANT=<true|false>
README_DOCS_INVENTORY=COMPLETE
BRAND_BRIEF_FROZEN=true
DISPOSABLE_WT_POLICY_FROZEN=true
ACTIVE_OWNER_WT_TARGETED=false
XAML_DIAGNOSTICS_STARTED=false
NATIVE_ICON_MUTATION_STARTED=false
CLAUDE_PROVIDER=DEFERRED
OPENCODE_PROVIDER=DEFERRED
CODE_CI=<PASS|N/A>
```

## Stop conditions

Stop before G84 when:

- source truth is ambiguous enough that the planned XAML route cannot be
  justified;
- a new official API changes the architecture and requires roadmap amendment;
- a conflicting v0.7/native-icon writer exists;
- disposable WT containment cannot be designed without targeting active Owner
  terminals.

## Estimated effort

**3–5 effective engineering hours.**

## Next

If XAML remains the admitted feasibility route:
`TB-G84 — Isolated XAML Diagnostics Harness`.

If an official API supersedes it, amend G84/G85 first rather than executing
stale instructions.