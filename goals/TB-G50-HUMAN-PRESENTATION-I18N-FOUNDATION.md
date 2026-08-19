# TB-G50 — Unified Human Presentation & i18n Foundation

## Status

PLANNED after accepted/public v0.4.1.

## Purpose

Create one locale-neutral domain-to-human presentation boundary so later bilingual CLI/TUI work does not duplicate business logic or localize machine contracts.

## Architectural outcome

Introduce a shared typed Human presentation model conceptually equivalent to:

```text
HumanDocument
  title
  status/summary
  sections[]
    fields[]
    messages[]
    actions[]
```

The exact Rust names may differ, but business/management code should emit typed meaning and renderers should own human wording, localization, width handling, and semantic style.

Normal human surfaces must stop depending on scattered `println!()` calls for product semantics.

## Locale contract

v0.5 supports exactly:

```text
auto
en-US
zh-CN
```

Resolution order:

```text
explicit admitted CLI override
  ↓
TABBEACON_LANG if admitted
  ↓
local Interface preference
  ↓
OS locale
  ↓
en-US
```

Use BCP-47-style identifiers. Unsupported locales fall back safely to English rather than partially localizing output.

Preferred implementation may use Fluent-compatible message catalogs plus a small locale resolver. The implementation must not embed a growing forest of `if chinese { ... }` branches across product code.

## Machine-contract boundary

Never localize:

```text
command/subcommand names
JSON field names
--plain keys
stable machine enum values
error/diagnostic IDs
Hook/profile identifiers
schema names/versions
```

Localization applies to Human labels, explanations, headings, action prose, and TUI help only.

Machine outputs must remain byte/semantic-equivalent across `en-US` and `zh-CN` except for explicitly documented non-contract whitespace if any.

## Interface preference foundation

Create user-local Interface preferences without restructuring existing Presentation settings unnecessarily.

Target concepts:

```text
InterfacePreferences {
  language: Auto | EnUs | ZhCn,
  color: Auto | Always | Never,
  reduced_motion: bool  # may remain default-only until G51/G56
}
```

Preferred location is user-local TabBeacon state, not repository-local state. Existing v0.4 Presentation settings must load unchanged.

All preference writes require atomic/ownership-safe semantics appropriate to a per-user file and must preserve unknown/future fields according to the chosen schema policy.

## Human color policy

Formalize:

```text
auto
always
never
```

Human renderers may style headings, success, warning/attention, failure, and dim explanatory text. Color is decorative; glyph/text still communicates state in monochrome.

Redirected/non-TTY output in `auto` mode must not emit unwanted ANSI escapes.

## Migration strategy

Existing v0.4.1 users have no Interface preference store. Absence means:

```text
language=auto
color=auto
reduced_motion=false
```

Reading defaults must not create state. First explicit preference Apply may create it atomically.

## Initial integration scope

G50 should prove the architecture on a representative set rather than translating every screen yet. Migrate at minimum:

```text
status human renderer
doctor human renderer
shared status/health labels
one Setup summary path
one Control Center screen/header/footer path
```

G51 completes product-wide bilingual integration.

## Testing

Required deterministic families:

- locale resolution precedence;
- unsupported locale fallback;
- en-US and zh-CN representative HumanDocument rendering;
- no ANSI in color=never and redirected auto;
- color does not change semantic text fields;
- JSON/plain outputs remain locale-independent;
- absent Interface store read is non-mutating;
- atomic preference write/restore and concurrent-drift refusal as applicable;
- CJK display-width primitives established for later TUI use.

## Risk vector

```text
CODE_CHANGED=true
PRESENTATION_CHANGED=false   # no tab-title/progress/VT presentation semantics
PROVIDER_CHANGED=false
USER_PERSISTENT_CONFIG_CHANGED=true
SECURITY_OR_PRIVACY_CHANGED=false
RELEASE_BOUNDARY=false
```

Persistent preference changes require one focused ownership/restore/drift safety family. No provider L4 and no old title-animation Visual matrix.

## Acceptance

```text
HUMAN_DOCUMENT_MODEL=PASS
HUMAN_RENDERER_SHARED=true
LOCALE_RESOLVER=PASS
LOCALES=en-US,zh-CN
LANGUAGE_AUTO=true
INTERFACE_PREFERENCES_LOCAL_ONLY=true
PROJECT_LOCAL_CONFIG=false
COLOR_POLICY=auto|always|never
COLOR_NOT_SOLE_SIGNAL=true
JSON_LOCALE_INDEPENDENT=true
PLAIN_LOCALE_INDEPENDENT=true
ABSENT_PREFS_READ_NON_MUTATING=true
PERSISTENT_CONFIG_SAFETY=PASS
CODE_CI=PASS
```

## Non-goals

Do not implement Adaptive Naming, alias override, import/export, live refresh, provider additions, automatic Hook trust, repository-local config, daemon, or session control here.

## Estimated effort

**7–10 effective engineering hours.**

## Next

`TB-G51 — Localized Guided Setup & Interface Preferences`.
