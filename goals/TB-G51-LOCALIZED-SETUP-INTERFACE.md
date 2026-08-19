# TB-G51 — Localized Guided Setup & Interface Preferences

## Status

PLANNED after accepted G50.

## Purpose

Complete the English/Simplified-Chinese Human experience across guided Setup and the Control Center, and expose Interface preferences as a first-class user-facing surface.

## Guided Setup

Fresh Setup begins with a bilingual self-explaining language choice:

```text
Language / 语言

Auto / 自动
简体中文
English
```

After selection, all subsequent Human Setup content immediately uses the chosen locale. The language choice remains part of the in-memory draft until Apply; Cancel must not create or modify Interface preferences.

Returning users:

- resolve existing Interface language before rendering the first normal summary;
- expose Interface/Language as an explicit setup section;
- `setup --quick` should not force a language prompt when `auto` is a valid resolved default and no action is required;
- `setup --full` revisits language/interface choices with current values selected.

Presets remain atomic. No closed enum should require manual spelling.

## Human Setup output

Human output uses the G50 presentation layer. Default flows must show grouped, localized information and one final summary rather than machine flags interleaved throughout the wizard.

Required sections conceptually:

```text
Environment / 环境
Presentation / 外观呈现
Interface / 界面
Windows Terminal title policy / Windows Terminal 标题策略
Planned changes / 计划变更
Preview / 预览
Result / 结果
```

Machine receipts remain available through machine-oriented channels/tests without leaking into default Human rendering.

## Control Center Interface screen

Add `Interface / 界面` as a first-class screen.

Manage at least:

```text
Language     Auto / 简体中文 / English
Color        Auto / Always / Never
Reduced motion  default false; may be exposed now or completed in G56
```

Changing language/color in the TUI updates the in-memory draft and live Human rendering immediately, but persistence remains staged until Apply.

A dirty Interface draft follows the same Apply/Revert/quit-confirmation semantics as Appearance.

## Product-wide localization coverage

By G51 exit, localize default Human text for:

```text
status
doctor
setup / quick / full
config human summaries
sessions human output
Control Center existing screens
common errors/actions/help text used by those surfaces
```

Command names, JSON/plain fields, persisted enum values, profile IDs, and schema IDs remain English machine tokens.

## Width and Unicode behavior

Use terminal display width rather than byte/Unicode-scalar count for layout. Add deterministic CJK cases covering:

- headings and labels;
- narrow terminal truncation/wrapping;
- mixed Latin/CJK values;
- glyph + Chinese label combinations;
- no panic or broken border math from wide characters.

Do not truncate inside a grapheme cluster.

## Language switching behavior

Required semantics:

```text
TUI language draft changes
  -> current frame re-renders in target language
  -> machine/domain state unchanged
  -> Revert restores prior language
  -> Apply persists atomically
```

Setup language selection behaves similarly except Setup is scrollback-oriented rather than alternate-screen.

## Testing

Required:

- fresh Setup English path;
- fresh Setup Chinese path;
- auto-locale fallback path;
- Cancel before Apply leaves no Interface state;
- quick/full returning-user paths;
- TUI language live-switch + Revert + Apply;
- color auto/always/never render behavior;
- en-US/zh-CN snapshots or semantic line assertions for all mandatory Human surfaces;
- CJK width/narrow terminal buffer tests;
- JSON/plain equivalence across locales.

One bounded real Windows Terminal smoke should exercise at least one locale switch in the Control Center if TUI source changes materially.

## Risk vector

```text
CODE_CHANGED=true
PRESENTATION_CHANGED=true   # full-screen TUI visible interaction/layout
PROVIDER_CHANGED=false
USER_PERSISTENT_CONFIG_CHANGED=true
SECURITY_OR_PRIVACY_CHANGED=false
RELEASE_BOUNDARY=false
```

Use ordinary hosted exact-head CI, one representative real WT/TUI acceptance pack, and focused preference ownership safety. No provider L4.

## Acceptance

```text
SETUP_LANGUAGE_FIRST=true
SETUP_ZH_CN=PASS
SETUP_EN_US=PASS
SETUP_CANCEL_LOSSLESS=true
INTERFACE_SCREEN=PASS
TUI_LANGUAGE_LIVE_SWITCH=PASS
TUI_INTERFACE_STAGED_APPLY=true
STATUS_ZH_CN=PASS
DOCTOR_ZH_CN=PASS
SESSIONS_ZH_CN=PASS
CJK_WIDTH_TESTS=PASS
JSON_LOCALE_INDEPENDENT=true
PLAIN_LOCALE_INDEPENDENT=true
PROJECT_LOCAL_CONFIG=false
WINDOWS_TERMINAL_SMOKE=PASS
CODE_CI=PASS
```

## Non-goals

Do not add Naming Engine v2, alias overrides, export/import, live operational refresh, provider additions, automatic Hook trust, or remote/session control.

## Estimated effort

**5–8 effective engineering hours.**

## Next

`TB-G52 — Adaptive Workspace Naming Engine v2`.
