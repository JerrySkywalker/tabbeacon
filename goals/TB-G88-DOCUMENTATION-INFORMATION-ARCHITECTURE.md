# TB-G88 — Documentation Information Architecture & Guides

## Status

PLANNED after accepted G87.

## Purpose

Turn TabBeacon's accumulated technical documentation into a coherent public
information system without destroying stable historical/source-of-truth paths.
Add a documentation portal, concise user guides, scalable coding-agent support
documentation, design explanations, and development guides. Keep English as the
canonical docs language in v0.7.

## Core principle

**Add navigation and newcomer-oriented guides before moving historical files.**

Existing paths such as architecture, Codex/Agy integration guides, capability
documents, ADRs, release notes, and research may already be referenced by PRs,
issues, external links, and historical evidence. Do not reorganize them merely
for visual tidiness.

## Required new files

### Portal

```text
docs/README.md
```

The portal must categorize docs clearly for:

- new users;
- configuration/operations;
- supported coding agents;
- troubleshooting/FAQ;
- design/architecture;
- contributors/developers;
- ADR/research/history.

### User guides

```text
docs/getting-started.md
docs/configuration.md
docs/coding-agent-support.md
docs/troubleshooting.md
docs/faq.md
```

### Design guides

```text
docs/design/product-principles.md
docs/design/visual-language.md
docs/design/native-tab-icon.md
docs/design/branding.md
```

G87 may already create `branding.md` and `visual-language.md`; G88 owns their
final placement/navigation/content completeness.

### Development guides

```text
docs/development/build-and-test.md
docs/development/release-process.md
```

## A. Documentation portal

`docs/README.md` should be the stable human index rather than a dump of every
file alphabetically.

Suggested structure:

```text
Start Here
  - Getting Started
  - Configuration
  - Supported Coding Agents
  - Troubleshooting
  - FAQ

Design & Architecture
  - Product Principles
  - Architecture
  - Visual Language
  - Native Tab Icon research/disposition
  - Provider Visual Identity
  - Terminal Visual Backends
  - ADRs

Provider Guides
  - Codex Hooks
  - Agy Setup
  - capability/compatibility references

Development
  - Build & Test
  - Contributing
  - Release Process

Research & History
  - research/
  - release notes / upgrade notes as appropriate
```

Do not make governance files the only path to understand ordinary user
behavior.

## B. Getting Started

Target: a new Windows user can understand and perform the common installation
flow in about five minutes.

Required flow, revalidated against current CLI at G88 head:

```text
install
  -> tabbeacon setup
  -> manual provider trust/review when required
  -> literal coding-agent command
  -> expected TabBeacon result
```

For Codex, explain that daily launch remains literal `codex`, not a TabBeacon
wrapper. Mention `/hooks` manual review only when actually required by the
current integration contract.

For Agy, keep the exact currently admitted setup scope truthful.

Do not bury new users in upgrade-preflight internals unless they are upgrading
an existing installation.

## C. Configuration guide

Move detailed day-to-day reference away from the README while preserving
commands and closed typed choices.

Cover current user-facing configuration surfaces such as:

- title ownership/presentation mode;
- activity mode;
- spinner preset;
- tab color;
- theme;
- presets;
- guided setup;
- Control Center;
- alias/workspace preferences where relevant;
- export/import if current product still exposes them.

Every command/value must be verified against current source/help output. Do not
copy stale v0.5/v0.6 syntax blindly.

Clearly separate:

```text
user preference
provider integration state
Hook trust
runtime/session state
```

so readers do not infer that changing a visual setting grants provider trust.

## D. Coding Agent Support guide

This is the scalable home for provider compatibility, separate from README
badges.

Required concepts:

- production-supported providers;
- daily command;
- setup path;
- compatibility/admission policy;
- capability matrix;
- known unavailable/unsupported capabilities;
- trust/config ownership semantics;
- future/deferred providers.

The capability table should include only meaningful product capabilities and
must preserve truthfulness. Candidate dimensions include:

```text
provider identity
workspace identity
working state
result-ready
approval/question attention
tab color
WT progress
animation
sessions projection
integration diagnostics
compatibility policy
```

Use `Supported`, `Unsupported`, `Unavailable`, `Not proven`, or similarly clear
semantics. Never represent an unsupported capability as zero/false if that would
imply authoritative absence.

### Provider policy

Codex:

- capability-based compatibility;
- version diagnostic only;
- conservative fresh setup based on proven Hooks capability.

Agy:

- exact currently admitted production profile unless current source has changed;
- smaller capability set reported honestly.

Deferred:

```text
Claude Code=DEFERRED
OpenCode=DEFERRED
```

Do not implement or pre-announce support.

## E. Troubleshooting guide

Prioritize real dogfood/release incidents.

At minimum cover:

### Hook review/trust

Symptoms, read-only diagnostics, supported repair/setup path, and why TabBeacon
never auto-trusts provider Hooks.

### Hook timeout / shell startup

Explain the difference between TabBeacon work and host shell/process startup
latency, current fail-open behavior, and safe diagnostics. Do not advise raising
timeouts blindly unless the product contract actually changes.

### Upgrade preflight blocked

Explain relevant states such as:

```text
no_known_tabbeacon_lock / equivalent current spelling
blocked_by_owned_tabbeacon_mcp
blocked
unavailable
```

Use the exact current CLI spelling at implementation time.

### Ambiguous processes

Explain that ambiguous matching package-image processes are preserved because
ownership is not proven. The normal answer is to let relevant sessions exit
naturally or diagnose read-only; do not recommend taskkill/process-name killing.

### Ownership-safe drain

Explain that an explicit drain is permitted only for exact ownership-proven
TabBeacon worker/MCP processes under the current preflight contract and does not
mean "kill Codex".

### Compatibility state

Explain full/degraded/incompatible/unproven or the exact current model and why
provider version alone is not authority where capability-based policy applies.

### Title fallback / ownership

Explain supported terminal-title ownership, PowerShell/native title conflicts,
and what doctor/explain surfaces can prove.

### Workspace identity

Explain root workspace anchor, aliases, worktrees/nested CWD expectations, and
how to inspect naming/title provenance.

### Agy profile mismatch

Explain exact admitted profile behavior and safe unsupported-version handling.

## F. FAQ

Answer concise recurring questions, including:

- Does TabBeacon wrap Codex/Agy?
- Does it change the daily command?
- Does it read prompts, assistant text, or tool content?
- Does normal workspace identity require network access?
- Why is Hook trust manual?
- Why can upgrades sometimes require coding-agent sessions to exit?
- Why are ambiguous processes preserved?
- Why doesn't Codex support depend only on an exact version number?
- Why is Agy support narrower?
- Why is native Windows Terminal tab icon difficult?
- Does v0.7 ship production native icons if G86 is GO?
- Are Claude/OpenCode supported?

Keep answers linked to deeper canonical docs.

## G. Product Principles

`docs/design/product-principles.md` should translate internal invariants into a
clear public architecture philosophy:

1. keep literal provider commands;
2. fail open;
3. manual trust stays manual;
4. no PATH shadow/wrapper/PTy host for presentation;
5. no global daemon baseline;
6. offline-first workspace identity;
7. provider-neutral core;
8. capability claims require evidence;
9. content/privacy minimization;
10. explainability and ownership-safe mutation over convenience.

This is a design explanation, not a copy of `AGENTS.md`.

## H. Visual Language

Finalize G87's `docs/design/visual-language.md` around the current conceptual
slots:

```text
Provider | Runtime state | Workspace identity
```

Explain how:

- provider identity is fixed/product-owned;
- runtime state changes with evidence;
- workspace identity should remain stable;
- title mark, spinner/indicator, tab color, progress, and any future native icon
  are presentation channels rather than provider authority;
- provider visual identity cannot grant compatibility/trust/config mutation.

## I. Native Tab Icon design doc

`docs/design/native-tab-icon.md` is the public/design-facing interpretation of
G83–G86.

It must state:

- stock WT source truth;
- why no public icon bridge was available at research time;
- what XAML Diagnostics means as an instrumentation boundary;
- exact-tab correlation principle;
- restore/fail-open requirements;
- G86 final disposition;
- whether experimental code is retained;
- explicit statement that `GO_PRODUCTION_CANDIDATE` does not equal production
  integration in v0.7.

Link to detailed research/ADR evidence rather than duplicating all receipts.

## J. Branding guide

Finalize brand construction/use rules introduced by G87. Keep third-party
provider trademarks separate from TabBeacon identity.

## K. Build & Test guide

`docs/development/build-and-test.md` should cover external-developer essentials:

- Rust/MSRV/toolchain policy;
- build commands;
- focused tests;
- full test/clippy gates;
- Windows-only behavior;
- visual CI boundaries;
- optional/high-risk experiment boundaries;
- where evidence artifacts belong;
- how to avoid mutating Owner production state while developing.

Do not require every external contributor to run impossible Owner-only L4 gates
for unrelated docs/code changes; explain risk-based gates.

## L. Release Process guide

Document the public high-level release flow without leaking credentials:

```text
accepted exact head
  -> release candidate
  -> tests/clippy/package
  -> Windows artifacts + hash
  -> fresh/upgrade consumer smoke
  -> explicit release authorization
  -> crates.io/tag/GitHub Release
  -> public consistency audit
  -> post-release truth closeout
```

Clarify irreversible public boundaries and truthful partial-release handling.

## Link / path policy

Existing canonical paths should remain unless moving is independently necessary.
When a new guide supersedes README detail, link rather than delete important
technical source truth.

If a file must move, preserve redirects/stubs only if repository conventions make
that useful and avoid duplicate contradictory truths.

## Language policy

All new `docs/` files in G88 are English canonical.

`README.zh-CN.md` remains the required Chinese public entry point. Do not create
partial random Chinese mirrors of individual technical docs in this Goal.

## Validation

At minimum:

- every new portal link resolves;
- every command in getting-started/configuration is checked against current CLI;
- coding-agent capability table is checked against provider source/docs;
- troubleshooting does not recommend unsafe ownership/trust bypasses;
- native-icon doc exactly matches G86 disposition;
- docs do not claim v0.7 public before G90;
- no historical evidence is rewritten merely to remove old version strings.

## Risk vector

```text
CODE_CHANGED=false
PRESENTATION_CHANGED=public_docs
PROVIDER_CHANGED=false
USER_PERSISTENT_CONFIG_CHANGED=false
SECURITY_OR_PRIVACY_CHANGED=documentation_safety_review
RELEASE_BOUNDARY=false
```

## Acceptance

```text
DOCS_PORTAL=PASS
GETTING_STARTED=PASS
CONFIGURATION_GUIDE=PASS
CODING_AGENT_SUPPORT_GUIDE=PASS
TROUBLESHOOTING_GUIDE=PASS
FAQ=PASS
PRODUCT_PRINCIPLES_DOC=PASS
VISUAL_LANGUAGE_DOC=PASS
NATIVE_TAB_ICON_DESIGN_DOC=PASS
BRANDING_DOC=PASS
BUILD_AND_TEST_GUIDE=PASS
RELEASE_PROCESS_GUIDE=PASS
EXISTING_SOURCE_TRUTH_PATHS_PRESERVED=true_or_justified_changes
CLI_COMMANDS_CURRENT=true
CODING_AGENT_CAPABILITIES_TRUTHFUL=true
CLAUDE_PROVIDER=DEFERRED
OPENCODE_PROVIDER=DEFERRED
UNSAFE_TROUBLESHOOTING_WORKAROUNDS=0
PREMATURE_V07_PUBLIC_CLAIMS=0
DOC_LINKS=PASS
```

## Estimated effort

**7–11 effective engineering hours.**

## Next

`TB-G89 — Contributor Experience & Documentation QA/CI`.