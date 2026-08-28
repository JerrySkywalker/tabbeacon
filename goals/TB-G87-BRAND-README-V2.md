# TB-G87 — Brand System & README v2

## Status

PLANNED after accepted G83. Recommended unattended order places this after G86
so the Native Icon research has a settled disposition before public-facing
visual/design language is finalized.

## Purpose

Give TabBeacon a coherent project identity and rebuild the repository homepage
for a mature open-source audience without changing runtime behavior.

Deliver:

- TabBeacon-owned SVG brand assets;
- English canonical `README.md`;
- full Simplified Chinese `README.zh-CN.md` counterpart;
- exactly two hero badges (Rust + Windows CI);
- a scalable Supported Coding Agents section that is separate from badges;
- GitHub-native syntax highlighting and alerts;
- one real privacy-safe Windows Terminal product screenshot;
- design documentation for brand and visual language.

## Product / runtime boundary

G87 is public presentation work.

Do not:

- change provider runtime semantics;
- add Claude/OpenCode support;
- integrate experimental native-icon code into production;
- change Hook trust behavior;
- bump package version to 0.7.0;
- publish a release.

Required:

```text
RUNTIME_BEHAVIOR_CHANGED=false
PROVIDER_BEHAVIOR_CHANGED=false
RELEASE_BOUNDARY=false
```

## A. Brand concept

Freeze and implement a visual identity derived from **Tab + Beacon**.

Preferred conceptual ingredients:

- a terminal/tab silhouette or tab-shaped frame;
- a compact beacon/status point;
- restrained outward signal/pulse geometry;
- optional subtle terminal/cursor language;
- relation to TabBeacon's real semantic status palette;
- no generic AI brain/robot cliché;
- no imitation of OpenAI/Codex/Agy/other provider trademarks.

The mark should remain useful if a future release adopts a native tab icon, but
v0.7 does not require that feature.

## B. Required brand assets

Create:

```text
docs/assets/brand/tabbeacon-mark.svg
docs/assets/brand/tabbeacon-logo.svg
docs/assets/brand/tabbeacon-mark-monochrome.svg
docs/assets/brand/tabbeacon-state-strip.svg
```

Optional if practical and visually reviewed:

```text
docs/assets/brand/social-preview.png
```

### Mark requirements

`tabbeacon-mark.svg` should remain legible/recognizable at candidate icon sizes:

```text
16x16
24x24
32x32
64x64
```

Do not solve small-size legibility by embedding unreadable text.

### SVG portability / safety

All committed SVG brand assets must satisfy:

```text
SCRIPT=false
EXTERNAL_URL=false
EMBEDDED_RASTER=false
FONT_DEPENDENCY=false
VIEWBOX=true
TRANSPARENT_BACKGROUND=true
```

Avoid CSS/feature complexity that renders inconsistently in GitHub light/dark
contexts. If a wordmark requires text, prefer Markdown text next to the mark or
self-contained vector paths rather than a system-font dependency.

## C. README language model

Required:

```text
README.md=CANONICAL_ENGLISH
README.zh-CN.md=FULL_SIMPLIFIED_CHINESE_COUNTERPART
```

Near the hero:

- English README links to `README.zh-CN.md` as `简体中文`;
- Chinese README links back to `README.md` as `English`;
- navigation is reciprocal and obvious.

Do not create an entire duplicated Chinese technical docs tree in G87.

## D. Hero layout

The README hero should visually resemble a clean OSS project rather than an
internal engineering note.

Recommended hierarchy:

```text
TabBeacon mark/logo
TabBeacon
concise tagline
English | 简体中文
Rust badge | Windows CI badge
Releases · crates.io · Documentation · MIT License
```

Suggested English positioning:

> Live identity and status for coding-agent tabs, without changing how you
> launch them.

Suggested Chinese positioning may convey:

> 为 Coding Agent 终端标签提供实时身份与状态提示，同时保持原有启动方式不变。

Exact copy may be polished during visual review, but meaning must preserve the
literal-provider-command invariant.

## E. Strict badge policy

The hero contains exactly two project-health badges:

```text
README_BADGE_COUNT=2
README_BADGE_RUST=true
README_BADGE_WINDOWS_CI=true
README_AGENT_BADGES=false
```

### Rust badge

Represents repository/package MSRV policy, currently Rust 1.97.1 or newer if
unchanged by the repository at G87 execution time. Revalidate `Cargo.toml` and
repository toolchain truth rather than copying this number blindly.

The badge is not a "latest Rust" badge and is not tied to a coding agent.

### Windows CI badge

Must bind to the real current Windows CI workflow used by the repository. Do not
create a decorative always-passing badge.

### Explicit exclusions from hero badge row

Do not add:

- Codex badge;
- Agy badge;
- Claude/OpenCode badges;
- crates.io badge;
- downloads badge;
- release-version badge;
- license badge;
- Windows Terminal badge.

Releases, crates.io, docs, and license may be compact ordinary links under the
badges.

## F. Supported Coding Agents section

Provider compatibility is a separate first-class section, designed to scale.

README should contain a concise production support table similar to:

| Coding Agent | Status | Daily command | Compatibility policy |
| --- | --- | --- | --- |
| Codex CLI | Production | `codex` | Capability-based |
| Agy CLI | Production | `agy` | Exact admitted profile |

The exact current Agy admitted version/profile must be re-read from source/docs
at implementation time; do not hard-code stale data from this Goal file.

Then link to `docs/coding-agent-support.md` (created/finalized in G88) for the
capability matrix.

### Deferred providers

README may state separately:

```text
Claude Code — Deferred
OpenCode — Deferred
```

They must not appear in a way that implies production support.

No third provider is implemented in v0.7.

## G. README information order

Canonical structure:

1. Hero
2. Why TabBeacon?
3. What It Looks Like
4. Features
5. Supported Coding Agents
6. Quick Start
7. Compatibility
8. How It Works
9. Safety & Privacy
10. Configuration
11. Documentation
12. Contributing
13. License

Keep the README newcomer-oriented. Move detailed reference material into docs
rather than expanding the homepage indefinitely.

## H. What It Looks Like

Show concise examples using current production grammar, for example:

```text
Codex ⠋ OWH
Codex ✓ OWH
Agy   ○ JMG
```

Examples must reflect current semantics at the G87 head; update them if the
actual stable presentation differs.

Also add one real Windows Terminal screenshot at a stable docs asset path,
for example:

```text
docs/assets/screenshots/tabbeacon-overview.png
```

Screenshot requirements:

- real Windows Terminal, not a fabricated primary mockup;
- no secrets/auth tokens;
- no prompt/assistant content that should not be public;
- no private filesystem/user-identifying paths where avoidable;
- representative TabBeacon titles/status;
- visually readable on GitHub.

If a safe real screenshot cannot be produced autonomously, stop only that asset
at an Owner visual-capture boundary; do not fabricate one.

## I. GitHub-native color / formatting

Use proper fenced language identifiers:

```text
powershell
toml
json
rust
text
```

GitHub syntax highlighting is the supported way to color code examples. Do not
use sanitized/nonportable HTML style hacks to color arbitrary README text.

Use GitHub Alerts where useful, e.g. for:

- literal `codex`/`agy` launch note;
- manual Hook trust warning;
- fail-open/privacy guarantees.

Mermaid is allowed for a compact architecture flow if it genuinely improves
comprehension.

## J. Brand / visual design docs

Create or prepare for G88:

```text
docs/design/branding.md
docs/design/visual-language.md
```

`branding.md` should cover:

- concept/construction;
- mark vs logo usage;
- semantic palette relation;
- clear-space/minimum-size guidance;
- monochrome usage;
- light/dark considerations;
- prohibited uses;
- third-party trademark separation.

`visual-language.md` should explain the production presentation model:

```text
Provider | Runtime state | Workspace identity
```

and how title mark, animation, tab color, progress, and any future native icon
relate without conflating authority.

## K. README stale-truth cleanup

Audit and fix current-facing historical leftovers such as stale exact-version
install examples or statements that incorrectly describe v0.6.0/v0.6.1 as the
current state.

Do not rewrite historical changelog/release receipts that were truthful in their
own time.

## Validation

Required:

- Markdown links in both README files resolve;
- reciprocal language links;
- badge count exactly two;
- badge targets truthful;
- both READMEs present equivalent critical install/setup/safety/provider-support
  facts;
- SVGs well-formed and safe;
- visual review of logo in GitHub-like light/dark contexts;
- mark legibility review at small sizes;
- screenshot privacy review;
- no runtime code change.

This Goal requires an L3-style visual/design review because it materially changes
public presentation, even though runtime behavior is unchanged.

## Risk vector

```text
CODE_CHANGED=false_or_docs_only_helpers
PRESENTATION_CHANGED=repository_public_surface
PROVIDER_CHANGED=false
USER_PERSISTENT_CONFIG_CHANGED=false
SECURITY_OR_PRIVACY_CHANGED=screenshot_and_assets_review
RELEASE_BOUNDARY=false
```

## Acceptance

```text
TABBEACON_MARK_SVG=PASS
TABBEACON_LOGO_SVG=PASS
TABBEACON_MONOCHROME_SVG=PASS
TABBEACON_STATE_STRIP_SVG=PASS
SVG_ACTIVE_CONTENT=false
SMALL_MARK_REVIEW=PASS
README_CANONICAL_LANGUAGE=en-US
README_ZH_CN=true
README_LANGUAGE_LINKS_RECIPROCAL=true
README_BADGE_COUNT=2
README_BADGE_RUST=true
README_BADGE_WINDOWS_CI=true
README_AGENT_BADGES=false
SUPPORTED_CODING_AGENTS_SECTION=PASS
CLAUDE_PROVIDER=DEFERRED
OPENCODE_PROVIDER=DEFERRED
REAL_PRODUCT_SCREENSHOT=PASS_OR_OWNER_VISUAL_GATE
CODE_FENCES_LANGUAGE_TAGGED=true
README_STALE_CURRENT_TRUTH=0
BRANDING_DOC=PASS
VISUAL_LANGUAGE_DOC=PASS
RUNTIME_BEHAVIOR_CHANGED=false
VISUAL_REVIEW=PASS
```

## Estimated effort

**7–10 effective engineering hours.**

## Next

`TB-G88 — Documentation Information Architecture & Guides`.