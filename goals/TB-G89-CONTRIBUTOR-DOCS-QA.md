# TB-G89 — Contributor Experience & Documentation QA/CI

## Status

PLANNED after accepted G88.

## Purpose

Make the public repository easier to contribute to and prevent the new v0.7
README/docs/brand system from drifting immediately after release. Rewrite
`CONTRIBUTING.md` as a practical external contributor on-ramp, polish public
security/documentation guidance where needed, and add lightweight
repository-owned documentation validation to the real Windows CI path.

## Product/runtime boundary

G89 is documentation/tooling/CI work. It must not change provider runtime
semantics or user configuration behavior.

Required:

```text
RUNTIME_BEHAVIOR_CHANGED=false
PROVIDER_BEHAVIOR_CHANGED=false
RELEASE_BOUNDARY=false
```

## A. CONTRIBUTING v2

The current contributor guide is intentionally compact and points directly into
internal governance. v0.7 should preserve those deeper references while making
ordinary contribution possible without first reverse-engineering the entire
agent-governance system.

Recommended structure:

1. Project scope / what TabBeacon is
2. Prerequisites
3. Clone and build
4. Run focused tests
5. Full quality gates
6. Windows-specific behavior/tests
7. Repository architecture overview
8. Provider-specific boundary
9. Documentation changes
10. Visual/brand changes
11. Native-icon experiment boundary
12. Making a pull request
13. Exact-head CI expectations
14. Security/privacy expectations
15. When deeper governance/ADR reading is required

### Contributor tone

Keep it concise, public, and actionable. Avoid requiring a typo/documentation
contributor to understand every internal receipt identifier before submitting a
small PR.

At the same time, do not weaken real boundaries:

- provider-specific behavior stays under provider adapters;
- provider-neutral core does not absorb vendor event names;
- Hook trust is never bypassed;
- prompt/assistant/tool content is not persisted merely for diagnostics;
- high-risk Windows instrumentation requires the dedicated safety process.

## B. SECURITY.md polish

Audit current `SECURITY.md` for public clarity.

Where necessary clarify:

- supported release/security-reporting scope;
- what kinds of issues are security-sensitive for TabBeacon (Hook/config
  ownership, process targeting, terminal instrumentation, content/privacy
  leakage, path/command handling, etc.);
- safe reporting path already used by the repository;
- no secrets should be pasted into public issues.

Do not invent unsupported contact channels or response SLAs.

If `SECURITY.md` is already sufficient, record `NO_CHANGE_REQUIRED` rather than
rewriting for churn.

## C. Documentation check script

Prefer a repository-owned PowerShell script integrated with existing Windows
CI, for example:

```text
scripts/ci/check-docs.ps1
```

Exact placement/name may follow current script conventions.

Avoid adding a large Node/JavaScript toolchain solely to lint Markdown unless a
clear benefit is demonstrated and accepted.

## D. Minimum machine checks

### Internal Markdown links

Check repository-local Markdown links in the v0.7 public docs surface.

Required:

```text
INTERNAL_MARKDOWN_LINKS_VALID=true
```

The checker should understand at least ordinary relative file links and anchors
as needed by the actual docs. Do not create a flaky web crawler for external
URLs as a mandatory release gate.

### README language reciprocity

Verify:

```text
README.md exists
README.zh-CN.md exists
README.md -> README.zh-CN.md
README.zh-CN.md -> README.md
```

Required:

```text
README_LANGUAGE_LINKS_RECIPROCAL=true
```

### Badge policy

Machine-check the deliberate v0.7 hero policy where practical:

```text
README_BADGE_COUNT=2
README_BADGE_RUST=true
README_BADGE_WINDOWS_CI=true
README_AGENT_BADGES=false
```

Do not count arbitrary images deeper in the README as hero badges. Implement the
check around an explicit stable marker/section if necessary rather than fragile
regex guessing.

### Critical EN/ZH invariant parity

Do not attempt machine translation equivalence.

Instead verify a bounded set of critical facts where practical, for example:

- installation command exists in both;
- `tabbeacon setup` exists in both;
- daily `codex` command invariant appears in both;
- daily `agy` command invariant appears where current support docs require;
- supported coding-agent table has the same production providers;
- Claude/OpenCode remain deferred;
- manual trust/fail-open safety language is not accidentally absent from one
  README.

The checker may use explicit structured comments/markers if needed to avoid
brittle prose parsing.

### Brand asset presence / SVG safety

Verify required assets exist:

```text
docs/assets/brand/tabbeacon-mark.svg
docs/assets/brand/tabbeacon-logo.svg
docs/assets/brand/tabbeacon-mark-monochrome.svg
docs/assets/brand/tabbeacon-state-strip.svg
```

Validate XML/SVG parseability and reject obvious active/external content:

```text
SVG_WELL_FORMED=true
SVG_SCRIPT=false
SVG_EXTERNAL_URL=false
SVG_EMBEDDED_RASTER=false
```

Do not build a false sense of full SVG sandbox security from a simplistic regex;
state exactly what the checker proves. Use a real XML parser where practical.

### Stale current-release truth

Add a narrow current-facing stale-version check. It should not reject historical
release notes/changelog/receipts merely because they mention older versions.

Focus on files/sections that claim current installation/public release state,
for example README, docs portal, getting started, current roadmap pointer, and
possibly package/release navigation.

Required at release candidate:

```text
STALE_CURRENT_RELEASE_MARKERS=0
```

Before G90 bumps/publishes v0.7, checks must distinguish "current public release
is v0.6.1" from "target next release v0.7.0" rather than forcing premature
public claims.

### Docs portal

Verify required guide links exist from `docs/README.md`.

## E. Code fence hygiene

Audit public guides/README examples. Where a fenced block contains a known
language, use the correct language tag so GitHub native syntax highlighting
works.

Do not require a language tag for blocks intentionally representing generic
plain output/ASCII diagrams (`text` is acceptable).

A warning-only check may be preferable to an overbroad hard fail if historical
technical docs contain legitimate untyped fences.

## F. CI integration

Integrate the docs check into the current real Windows CI workflow or local CI
entrypoint according to repository conventions.

Requirements:

- exact-head CI still proves the checked commit;
- docs-only PRs exercise documentation gates;
- normal code PRs also catch README/docs drift;
- no network dependency is required for core docs checks;
- execution time remains small relative to the main Rust gates.

Do not replace current code tests/clippy with docs checks.

## G. Optional OSS repository polish (P1, non-blocking)

If time remains after required acceptance, inspect and improve where useful:

- GitHub issue templates / config;
- PR template clarity for docs/visual changes;
- repository description/topics recommendations;
- social preview asset installation instructions if repository settings require
  Owner UI action;
- README metadata/navigation consistency.

Do not block G89/G90 if an Owner-only GitHub repository setting cannot be
changed autonomously. Return an exact optional Owner action instead.

## H. Documentation review

Perform focused review for:

- unsafe troubleshooting instructions;
- claims that provider support is broader than source evidence;
- accidental provider badges;
- English/Chinese command mismatch;
- stale current release/install instructions;
- broken brand links/assets;
- accidental external-script SVG content;
- contributor guidance contradicting `AGENTS.md`/quality gates on high-risk work.

## Risk vector

```text
CODE_CHANGED=CI_or_docs_helper_only
PRESENTATION_CHANGED=repository_public_surface
PROVIDER_CHANGED=false
USER_PERSISTENT_CONFIG_CHANGED=false
SECURITY_OR_PRIVACY_CHANGED=docs_and_SVG_validation
RELEASE_BOUNDARY=false
```

A full runtime L4 is not required solely because documentation CI changed.
Run exact-head hosted CI because workflow/check behavior changes.

## Acceptance

```text
CONTRIBUTING_V2=PASS
SECURITY_DOC=<PASS|NO_CHANGE_REQUIRED>
DOCS_CHECK_SCRIPT=PASS
INTERNAL_MARKDOWN_LINKS_VALID=true
README_LANGUAGE_LINKS_RECIPROCAL=true
README_BADGE_COUNT=2
README_AGENT_BADGES=false
CRITICAL_EN_ZH_INVARIANTS=PASS
REQUIRED_BRAND_ASSETS_EXIST=true
SVG_WELL_FORMED=true
SVG_ACTIVE_CONTENT=false
DOCS_PORTAL_LINKS_VALID=true
STALE_CURRENT_RELEASE_MARKERS=0_for_current_public_truth
DOCS_CI_INTEGRATED=true
DOCS_CI=PASS
RUNTIME_BEHAVIOR_CHANGED=false
HOSTED_EXACT_HEAD_CI=PASS
DOCS_REVIEW_FINDINGS=0
```

## Estimated effort

**4–6 effective engineering hours.**

## Next

`TB-G90 — v0.7 Hardening & Release`.