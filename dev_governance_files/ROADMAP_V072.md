# TabBeacon v0.7.2 execution roadmap

## Status

**OWNER-ADMITTED ROADMAP WITH A NARROW FIRST IMPLEMENTATION SLICE** from public
`v0.7.1` and post-release closeout on `main` at
`21181ecd3a3d3dc2f3de57548677d4a667f64be7`.

The Owner's broader feature-development pause remains in force. v0.7.2 is a
narrow roadmap exception for discoverability, deterministic promotional
evidence, and Rust/crates.io distribution polish. The active implementation
scope of this first slice is **TB-G99 and TB-G100 only**. TB-G101 and TB-G102
remain planning-only until a separate explicit Owner admission. This does
**not** admit v0.8 feature work.

```text
CURRENT_PUBLIC_RELEASE=v0.7.1
TARGET_PUBLIC_RELEASE=v0.7.2
ACTIVE_FEATURE_DEVELOPMENT=PAUSED_EXCEPT_V072_MAINTENANCE
ACTIVE_IMPLEMENTATION_SCOPE=TB-G99_TB-G100_ONLY
G101_G102_EXECUTION=SEPARATE_OWNER_ADMISSION_REQUIRED
ROADMAP_V08_CREATED=false
NEW_PROVIDER_ADDED=false
RUNTIME_BEHAVIOR_CHANGED=false_expected
PROVIDER_BEHAVIOR_CHANGED=false_expected
```

## Product theme

**v0.7.2 — Discoverability & Automated Demo**

The release addresses three product-distribution gaps only:

1. make the repository easier to discover and understand through GitHub-native
   metadata and a coherent social-preview asset;
2. generate a short, privacy-safe, deterministic animated demo using a real
   Windows Terminal and TabBeacon's real presentation renderer; and
3. make the public Rust installation contract unambiguous and prove both the
   normal latest-version install and the exact release consumer.

The user-facing primary install command is permanently simple:

```powershell
cargo install tabbeacon
```

The exact release-engineering verification command for this train is separate:

```powershell
cargo install tabbeacon --version 0.7.2 --locked
```

`--version` and `--locked` are release/reproducibility controls, not requirements
for the README's primary user path.

## Explicit non-goals

v0.7.2 does not add or admit:

- a Windows installer or installation PowerShell script;
- a Winget or Scoop package for TabBeacon;
- PATH mutation, auto-update, or system-wide installation behavior;
- a new coding-agent provider;
- Operational Reliability v2 or Provider Platform v2;
- new production terminal/runtime semantics;
- Native Tab Icon implementation or XAML Diagnostics;
- Codex App Server;
- Claude Code or OpenCode production support;
- real-model traffic for promotional evidence.

Existing GitHub Windows ZIP release artifacts may continue under the established
release process, but v0.7.2 does not build a new installation system around
them.

## External promotional tooling

Only one new external promotional tool is admitted:

```text
TOOL=FFmpeg
INSTALL_SOURCE=Microsoft Winget
PACKAGE_ID=Gyan.FFmpeg
PURPOSE=encode exact-owned PNG frame sequences into optimized GIF assets
```

The generator must reuse a working `ffmpeg` already on `PATH`. If unavailable,
the admitted Windows development flow may install `Gyan.FFmpeg` through Winget
with package/source agreements accepted non-interactively. No FFmpeg binary is
vendored, linked, or redistributed by TabBeacon.

The GIF pipeline must be:

```text
controlled typed showcase fixture
  -> real exact-owned Windows Terminal window
  -> UIA/exact-window correlation
  -> TabBeacon-owned Windows window capture to PNG sequence
  -> crop/scale
  -> FFmpeg palettegen
  -> FFmpeg paletteuse
  -> looping GIF
```

Desktop-wide `gdigrab` or equivalent capture is not admitted. FFmpeg is an
encoder, not the authority for which window is captured.

## Demo truth boundary

The promotional demo is not a fake live Codex session. It is deterministic
product evidence using the real renderer and a controlled fixture.

```text
PROMO_REAL_WINDOWS_TERMINAL=true
PROMO_REAL_TABBEACON_RENDERER=true
PROMO_REAL_MODEL_SESSION=false
PROMO_REAL_CODEX_PROCESS=false
PROMO_CONTROLLED_FIXTURE_ONLY=true
PROMO_SEMANTICS_SUBSET_OF_PRODUCTION=true
```

The primary demo should use only Codex-compatible presentation semantics unless
an independently proven reason requires another admitted provider. It must not
invent Agy or deferred-provider states for visual effect.

Recommended deterministic aliases are synthetic values such as `API`, `WEB`,
and `DOCS`; no private repository path or Owner content belongs in the demo.

## Dependency sequence

```text
PUBLIC v0.7.1 + DOGFOOD PAUSE
        |
        v
TB-G99  GitHub Discovery Surface
        |
        v
TB-G100 Automated Real-WT Promo Demo
        |
        v
TB-G101 README & crates.io Distribution Polish
        |
        v
TB-G102 v0.7.2 Hardening & Release
        |
        v
PUBLIC v0.7.2
        |
        v
DOGFOOD PAUSE RESUMES
```

## Goal index

| Goal | Scope | Estimated effective effort |
| --- | --- | ---: |
| G99 | repository description/topics audit and mutation; deterministic 1280x640 social-preview source/render | 2–4 h |
| G100 | typed showcase fixture; exact-owned real WT orchestration/capture; FFmpeg GIF; poster; privacy/visual evidence | 6–10 h |
| G101 | README English/Chinese demo placement; simple Cargo install contract; crates.io package surface audit and marketing-asset separation | 3–5 h |
| G102 | full maintenance gates; public `0.7.2`; default and exact crates.io consumers; post-release truth; resume dogfood pause | 3–5 h |
| **Total** | **v0.7.2** | **14–24 h** |

## G99 acceptance summary

Required outcomes:

```text
GITHUB_DESCRIPTION=PASS
GITHUB_TOPICS_COUNT=6..10
GITHUB_TOPICS_RELEVANT=true
SOCIAL_PREVIEW_SVG=PASS
SOCIAL_PREVIEW_PNG=PASS
SOCIAL_PREVIEW_DIMENSIONS=1280x640
```

Topic selection must be based on current GitHub usage and semantic accuracy,
not keyword stuffing. Candidate families may include coding agents, Codex CLI,
Windows Terminal, terminal/developer tools, Rust, and Windows, but the exact set
is an implementation-time evidence decision.

The social-preview source must use only TabBeacon-owned visual identity, fixed
text/state examples, and no provider trademark imitation or external fonts.
Use the existing local Edge/headless rendering approach when practical.

If GitHub still provides no supported API for social-preview upload, generation
of the final asset is sufficient for G99 code acceptance. The repository UI
upload is an optional Owner action and must not be automated through browser
session/cookie hacks.

## G100 acceptance summary

The demo generator must create at least:

```text
docs/assets/demo/tabbeacon-demo.gif
docs/assets/demo/tabbeacon-demo-poster.png
```

Recommended media target:

```text
DURATION=8..12 seconds
FPS=10
LOOP=infinite
WIDTH=960..1100 px
GIF_TARGET_SIZE<=4 MiB
GIF_HARD_LIMIT<=6 MiB
```

Temporary PNG frames and FFmpeg palette intermediates remain build evidence and
must not be committed.

Required safety/truth gates:

```text
NO_DESKTOP_CAPTURE=true
TARGET_WINDOW_MATCH_COUNT=1
PROMO_PRIVACY_REVIEW=PASS
PRIVATE_CONTENT_VISIBLE=false
REAL_MODEL_REQUEST=false
PRODUCTION_CONFIG_MUTATION=false
HOOK_TRUST_MUTATION=false
NATIVE_ICON_RESEARCH=false
```

Normal CI validates the committed assets and feature-gated promo code; it does
not regenerate the real-WT GIF on every PR. Regeneration is an interactive
visual-evidence operation when promo tooling/timeline/presentation materially
changes or during an explicit release refresh.

## G101 acceptance summary

README English and Chinese primary Quick Start must lead with:

```powershell
cargo install tabbeacon
tabbeacon setup
```

No primary-install version pin or `--locked` flag.

The release engineering path separately proves:

```powershell
cargo install tabbeacon --version 0.7.2 --locked
```

Marketing media should remain GitHub-hosted presentation assets rather than
inflating the crates.io runtime package.

Required package policy:

```text
PROMO_GIF_IN_CRATE=false
SOCIAL_PREVIEW_IN_CRATE=false
PROMO_BUILD_EVIDENCE_IN_CRATE=false
CARGO_PACKAGE=PASS
```

The README may use an appropriate stable GitHub-hosted absolute asset reference
so both GitHub and crates.io render the demo without embedding the GIF in the
crate archive.

## G102 acceptance summary

Normal release discipline remains mandatory:

```text
TESTS=PASS
CLIPPY=PASS
CARGO_PACKAGE=PASS
DOCS_CI=PASS
HOSTED_EXACT_HEAD_CI=PASS
RELEASE_REVIEW_FINDINGS=0
HIGH_RISK_FINDINGS=0
```

After publication, prove two separate clean public consumers:

```text
DEFAULT_CRATES_IO_INSTALL=PASS
  command: cargo install tabbeacon

EXACT_CRATES_IO_INSTALL=PASS
  command: cargo install tabbeacon --version 0.7.2 --locked
  installed version: 0.7.2
```

Public release surfaces remain the established TabBeacon set: crates.io,
immutable `v0.7.2` tag, GitHub Release, Windows x64 ZIP, and SHA-256 sidecar.

## Product invariants

Throughout the train:

```text
DAILY_COMMAND_CODEX=codex
DAILY_COMMAND_AGY=agy
FAIL_OPEN=true
NO_WRAPPER=true
NO_PATH_SHADOW=true
NO_PTY_HOST=true
GLOBAL_DAEMON_ADDED=false
NEW_PROVIDER_ADDED=false
RUNTIME_BEHAVIOR_CHANGED=false_expected
PROVIDER_BEHAVIOR_CHANGED=false_expected
CLAUDE_PROVIDER=DEFERRED
OPENCODE_PROVIDER=DEFERRED
CODEX_APP_SERVER=DEFERRED
NATIVE_TAB_ICON_DISPOSITION=NO_GO
ROADMAP_V08_CREATED=false
```

A feature-gated visual/showcase helper is development/release tooling and must
not become a normal installed runtime surface.

## Final state

After successful public v0.7.2 closeout:

```text
CURRENT_PUBLIC_RELEASE=v0.7.2
ACTIVE_FEATURE_DEVELOPMENT=PAUSED
V08_OPTIONS_STATUS=NON_AUTHORITATIVE
ROADMAP_V08_CREATED=false
NEXT_RECOMMENDED_GOAL=DOGFOOD_ONLY_NO_ACTIVE_DEVELOPMENT
```
