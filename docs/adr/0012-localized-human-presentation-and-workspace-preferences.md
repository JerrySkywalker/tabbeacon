# ADR 0012 — Localized Human Presentation, Adaptive Naming, and Local Workspace Preferences

## Status

Accepted for the v0.5 planning track when this ADR lands on `main`.

## Context

TabBeacon v0.4 introduced Human-first status/doctor output, guided Setup, a Ratatui Control Center, and post-release read-only Sessions. Real dogfood showed that the next product stage needs more than incremental text edits:

- Human output still contains some machine-style receipt/debug flags;
- the UI is English-only;
- user-visible color policy is not centralized;
- the Control Center is snapshot-based rather than live-refreshing;
- current repository abbreviation is deterministic but shortest-initialism-first and under-serves single-token, Unicode, and mixed-style names;
- explicit alias preference must be possible without writing personal UI state into project repositories;
- settings/preferences need an explicit backup/migration surface.

The existing product invariants remain: daily command `codex`, fail-open behavior, offline identity, manual Hook trust, no wrapper/PATH shadow/PTY host/global daemon baseline, and no prompt/tool/model content persistence.

## Decision 1 — Human meaning is separated from Human rendering

Business/domain code should produce typed semantic data rather than owning localized prose and terminal styling directly.

Admit one shared Human presentation model, conceptually:

```text
HumanDocument
  title
  status
  sections
  fields/messages/actions
```

Human CLI and TUI renderers consume this model. Machine JSON/plain contracts remain separate.

Consequences:

- default Human surfaces can become bilingual and styled without duplicating product logic;
- tests can assert semantic documents separately from terminal strings;
- stray machine flags can be removed from Human output without deleting machine evidence channels.

## Decision 2 — v0.5 localization is Human-only

Supported locales:

```text
auto
en-US
zh-CN
```

Localize Human labels, prose, headings, actions, help, and TUI text.

Never localize:

```text
CLI command/subcommand names
JSON keys
plain output keys
persisted enum spellings
schema IDs/versions
diagnostic/error IDs
provider/profile IDs
```

OS locale may resolve `auto`; unsupported locale falls back to English.

## Decision 3 — Interface preferences are user-local state

Language/color/reduced-motion preferences belong to the user's TabBeacon state root, not a repository.

No v0.5 implementation may require or create project-local files such as:

```text
.tabbeacon
.tabbeacon.toml
tabbeacon.toml
```

Existing Presentation settings should not be needlessly reformatted/migrated solely to add Interface preferences. A separate user-local store is preferred if it reduces migration risk.

## Decision 4 — Adaptive Naming v2 is deterministic scoring, not AI

The default naming engine remains:

```text
offline
deterministic
bounded
explainable
collision-safe
```

It may use Unicode normalization/segmentation/display-width helpers, but no model/network lookup, repository-language analysis, remote metadata, or user-history learning is required for default naming.

Pipeline:

```text
display name
 -> normalize
 -> tokenize
 -> generate bounded candidates
 -> integer score
 -> deterministic tie-break
 -> collision handling
 -> stable alias
```

Candidate scoring rewards information coverage/acronym preservation/readability and penalizes severe loss, awkward one-character collapse, overlong width, and collisions.

Existing assigned aliases remain stable history and do not change automatically when the policy changes.

## Decision 5 — Generated alias and user preference are separate layers

Identity allocation and user choice are not the same state.

Preferred model:

```text
StableAliasRegistry
  -> generated_alias + policy_version

WorkspacePreferenceStore
  -> optional override_alias

EffectiveAlias = override_alias ?? generated_alias
```

Workspace override is bound to canonical workspace identity, not a raw visible path.

An explicit override collision is rejected. TabBeacon does not silently rename another workspace, swap aliases, or suffix the user's requested override.

Different devices may intentionally keep different local overrides.

## Decision 6 — Import/export is top-level user configuration portability

Admit:

```text
tabbeacon export
tabbeacon import
```

These aggregate user-configurable TabBeacon state rather than only workspace preferences.

Eligible state includes Presentation settings, Interface preferences, and Workspace alias preferences.

Explicitly excluded:

```text
Hook trust
runtime sessions/leases
credentials/tokens/cookies
raw native session/turn IDs
Windows Terminal machine/profile state
PowerShell profile state
absolute private workspace paths
arbitrary runtime logs/diagnostics
```

Git workspace matching may use a stable digest of canonical identity. Ordinary directory identity is path-derived/device-local and must be represented truthfully.

Import is preview-first and validates a typed plan before mutation.

## Decision 7 — Control Center live refresh remains local and daemonless

The v0.5 Control Center may refresh local read-only state at a bounded cadence inside the existing event loop.

Do not add Tokio, a network service, or a global resident daemon solely to make the TUI live.

Refresh never overwrites dirty drafts. Concurrent baseline change becomes an explicit conflict.

## Decision 8 — Input/accessibility behavior is explicit

Page/field/value navigation is edge-triggered by default:

```text
Press -> one action
Repeat -> ignored
Release -> ignored
```

Long lists may later use deliberate bounded repeat with an initial delay and repeat cadence.

Color is never the sole semantic signal. Human rendering must remain usable in monochrome. CJK width/grapheme behavior is first-class.

## Decision 9 — Safety-class remediation remains authoritative

The Control Center may surface actions according to existing safety classes. Hook trust remains manual. Previewable safe repair requires Preview -> explicit Apply and preserves unrelated state.

No arbitrary repair scripts or implicit ownership expansion are introduced by v0.5.

## Consequences

Positive:

- one product-wide bilingual architecture instead of string-by-string branching;
- deterministic naming can become substantially more readable without losing offline stability;
- users can override names without polluting repositories;
- export/import provides explicit portability without a cloud sync service;
- Live Control Center becomes a useful daily local dashboard while staying daemonless;
- machine automation contracts remain stable.

Costs:

- new persistent Interface and Workspace preference stores require migration/ownership testing;
- Unicode display width and translation increase layout test coverage;
- alias registry migration must preserve existing assignments exactly;
- import spans multiple local stores and needs transaction/rollback discipline;
- TUI live refresh must keep observational state separate from dirty drafts.

## Rejected alternatives

### Project-local `.tabbeacon` configuration

Rejected for v0.5 because TabBeacon preferences are personal terminal presentation state. Project-local files would create git-dirty/team-policy/worktree inheritance problems without a product requirement.

### Cloud/background preference sync

Rejected for v0.5. Explicit export/import is sufficient portability without adding identity, auth, merge-conflict, privacy, or service dependencies.

### AI/LLM abbreviation generation

Rejected for the default engine because it would violate offline determinism, increase latency/cost, and make stable alias reproduction harder to test.

### Store only the final alias string

Rejected because it loses whether a value was generated or explicitly selected, making reset, algorithm migration, explanation, and future policy-version reasoning ambiguous.

### Translate machine schemas

Rejected because locale-dependent JSON/plain/config values would break automation and cross-machine compatibility.
