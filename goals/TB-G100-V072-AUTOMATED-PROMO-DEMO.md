# TB-G100 — v0.7.2 Automated Real-WT Promo Demo

## Purpose

Generate a short, deterministic promotional GIF that demonstrates TabBeacon's
actual Windows Terminal presentation behavior without running a real coding
agent or capturing Owner content.

The demo must be real product rendering, not a drawn terminal mockup:

```text
controlled typed fixture
  -> production TabBeacon presentation renderer
  -> real exact-owned Windows Terminal
  -> exact-window capture
  -> PNG frames
  -> FFmpeg optimized GIF
```

## Preconditions

G99 should be accepted or its implementation branch reconciled without a shared
writer conflict. Re-read the current visual fixture, UIA/capture machinery, and
presentation capability truth before extending anything.

```text
PROMO_REAL_WINDOWS_TERMINAL=true
PROMO_REAL_TABBEACON_RENDERER=true
PROMO_REAL_MODEL_SESSION=false
RUNTIME_BEHAVIOR_CHANGED=false_expected
```

## A. External tool policy

The only newly admitted external media tool is FFmpeg.

```text
TOOL=FFmpeg
WINGET_PACKAGE_ID=Gyan.FFmpeg
PURPOSE=GIF encoding only
```

Generator behavior:

1. resolve an existing `ffmpeg` on `PATH` and record its version;
2. if missing on the admitted Windows development host, install only through
   Microsoft Winget:

```powershell
winget install --id Gyan.FFmpeg -e --source winget --accept-package-agreements --accept-source-agreements
```

3. re-resolve and verify `ffmpeg` / `ffprobe` after install;
4. never vendor, copy, link, or redistribute FFmpeg in TabBeacon.

Do not add OBS, ScreenToGif, ImageMagick, Gifski, or another recording stack.

## B. Capture authority

FFmpeg must **not** record the desktop. Explicitly prohibit desktop-wide
`gdigrab` or coordinate-based screen scraping.

The authority for what is captured remains the existing TabBeacon-owned Windows
window-capture path after exact UI Automation/process/window correlation.

Required:

```text
NO_DESKTOP_CAPTURE=true
TARGET_WINDOW_MATCH_COUNT=1
EXACT_OWNED_WT_WINDOW=true
UNRELATED_OWNER_WINDOW_VISIBLE=false
```

If exact ownership cannot be proven, stop the capture rather than weakening the
selector.

## C. Showcase fixture boundary

Extend the existing feature-gated visual fixture rather than adding a normal
production CLI command or daemon.

A good implementation may add a bounded showcase subcommand/scenario under the
existing visual-fixture feature. It must accept only typed/product-owned values
needed by the demo.

Preferred synthetic workspace aliases:

```text
API
WEB
DOCS
```

The primary public demo should use Codex-compatible presentation semantics only
unless current production evidence establishes an equally truthful alternative.
Do not invent deferred-provider support or Agy attention/result semantics for
visual effect.

```text
PROMO_SEMANTICS_SUBSET_OF_PRODUCTION=true
DEFERRED_PROVIDER_RENDERING=false
```

## D. Recommended timeline

Target approximately 8–12 seconds at 10 fps. The exact animation may be refined
for readability, but the semantic progression should be comparable to:

```text
scene 1: neutral/ready tabs
scene 2: multiple working tabs with real activity frames/color/progress
scene 3: one result-ready, one still working, one attention/question state
scene 4: stable glanceable end-state, then loop
```

Every visible state must be produced by the same production presentation policy
used by TabBeacon, not hand-painted into the captured image.

No real Codex process or model request is needed:

```text
REAL_CODEX_SESSION=false
REAL_MODEL_REQUEST=false
PRIVATE_PROMPT_CAPTURE=false
```

## E. Orchestration

Add one focused automation entrypoint, preferably:

```text
scripts/generate-promo-assets.ps1
```

It should, as far as the existing architecture safely permits:

1. preflight repository/source identity and required tools;
2. build the feature-gated visual fixture;
3. create a unique bounded run ID;
4. launch one controlled Windows Terminal window with the intended synthetic
   demo tabs;
5. use `pwsh -NoProfile` for controlled fixture shells;
6. synchronize the deterministic timeline;
7. identify exactly one owned target WT window;
8. capture a PNG sequence through the existing exact-window capture path;
9. terminate/restore only exact-owned fixture resources;
10. encode the GIF and produce a poster;
11. publish a content-minimal receipt.

The generator must not modify production Codex/Agy configuration or Hook trust.

## F. PNG frames

Use a temporary exact-owned evidence root such as:

```text
V:\build\tabbeacon\TB-V072-PROMO-<RUN_ID>\frames\
```

Recommended source cadence:

```text
FPS=10
DURATION=8..12 seconds
FRAME_COUNT≈80..120
```

Frames, palettes, and scratch captures are evidence only and must not be
committed.

Capture the entire exact-owned WT window first when that is the safest ownership
primitive. Crop/scale during media generation so the final demo emphasizes the
tab bar and a small amount of controlled terminal surface rather than an entire
desktop-sized frame.

## G. FFmpeg encoding

Use a two-pass GIF palette workflow. Exact scale/crop/dither parameters may be
optimized against the generated source, but the pipeline is normative:

```text
PNG sequence
  -> fps/crop/scale
  -> palettegen
  -> paletteuse
  -> infinite-loop GIF
```

A representative implementation is:

```powershell
ffmpeg -framerate 10 -i 'frame-%04d.png' `
  -vf 'fps=10,scale=960:-1:flags=lanczos,palettegen=stats_mode=diff' `
  -y palette.png

ffmpeg -framerate 10 -i 'frame-%04d.png' -i palette.png `
  -lavfi 'fps=10,scale=960:-1:flags=lanczos[x];[x][1:v]paletteuse=dither=bayer:bayer_scale=3:diff_mode=rectangle' `
  -loop 0 -y tabbeacon-demo.gif
```

Do not treat the example numeric parameters as immutable if a safer/crocker
crop or smaller file is proven better.

## H. Committed outputs

Required:

```text
docs/assets/demo/tabbeacon-demo.gif
docs/assets/demo/tabbeacon-demo-poster.png
```

Optionally add a small declarative timeline file only if it improves review and
does not create an unsafe arbitrary-render surface.

Do not commit:

```text
frames/
palette.png
capture scratch
FFmpeg binaries
Owner-specific evidence paths
```

## I. Media quality gates

Target:

```text
PROMO_GIF_DURATION=8..12s
PROMO_GIF_FPS=10
PROMO_GIF_LOOP=true
PROMO_GIF_WIDTH=960..1100px preferred
PROMO_GIF_TARGET_SIZE<=4MiB
PROMO_GIF_HARD_LIMIT<=6MiB
```

If the hard size limit is exceeded, prefer crop/height/palette/resolution
optimization before degrading activity motion to an unreadably low frame rate.

## J. Privacy and truth review

Required final review:

```text
PROMO_PRIVACY_REVIEW=PASS
PRIVATE_CONTENT_VISIBLE=false
OWNER_USERNAME_VISIBLE=false_expected
PRIVATE_PATH_VISIBLE=false
PRIVATE_REPOSITORY_VISIBLE=false
UNRELATED_WINDOW_VISIBLE=false
PROMO_REAL_WINDOWS_TERMINAL=true
PROMO_REAL_TABBEACON_RENDERER=true
PROMO_REAL_MODEL_SESSION=false
```

Caption/README wording must explicitly describe the asset as a deterministic
showcase fixture rendered through the real Windows Terminal presentation stack,
not a live Codex conversation.

## K. Receipt

Produce a content-minimal evidence receipt recording at least:

```text
SOURCE_SHA
WINDOWS_TERMINAL_VERSION
FFMPEG_VERSION
FRAME_COUNT
FPS
DURATION_MS
OUTPUT_DIMENSIONS
GIF_BYTES
GIF_SHA256
TARGET_WINDOW_MATCH_COUNT
PROMO_REAL_WINDOWS_TERMINAL=true
PROMO_REAL_MODEL_SESSION=false
PRIVATE_CONTENT_VISIBLE=false
```

Do not retain raw provider/model content, session IDs, Owner user name, or
private repository paths.

## L. CI policy

Normal hosted CI must not launch an interactive Windows Terminal and regenerate
100+ frames on every PR.

Normal CI should validate:

- feature-gated showcase code compiles/tests;
- committed GIF and poster exist;
- dimensions/file-size ceilings are respected;
- README/assets references are valid;
- generator/source metadata is coherent.

Interactive regeneration is required only when promo tooling/timeline or
material presentation semantics change, or when explicitly refreshed for a
release.

## Risk vector

```text
CODE_CHANGED=feature_gated_visual_fixture_and_promo_scripts
PRESENTATION_CHANGED=promotional_evidence_only
PROVIDER_CHANGED=false
USER_PERSISTENT_CONFIG_CHANGED=false
SECURITY_OR_PRIVACY_CHANGED=exact_window_capture_and_public_media_review
RELEASE_BOUNDARY=false
```

## Exit

G100 completes only when a clean real-WT GIF and poster are reproducibly
generated, privacy/truth gates pass, and the normal TabBeacon runtime/provider
behavior remains unchanged.

Next: `TB-G101-V072-CARGO-DISTRIBUTION-POLISH.md`.
