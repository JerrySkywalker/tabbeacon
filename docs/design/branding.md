# TabBeacon branding

TabBeacon's identity combines a terminal-tab silhouette with a beacon point.
The tab keeps the mark grounded in the product's real presentation surface;
the point and two signal arcs describe visible, evidence-driven status without
claiming control over the terminal or a coding agent.

## Assets

| Asset | Use |
| --- | --- |
| [`tabbeacon-mark.svg`](../assets/brand/tabbeacon-mark.svg) | Compact product mark for documentation and small product-adjacent contexts. |
| [`tabbeacon-logo.svg`](../assets/brand/tabbeacon-logo.svg) | Primary horizontal lockup for README-scale use. |
| [`tabbeacon-mark-monochrome.svg`](../assets/brand/tabbeacon-mark-monochrome.svg) | Single-color use where the semantic palette is unavailable. |
| [`tabbeacon-state-strip.svg`](../assets/brand/tabbeacon-state-strip.svg) | Reference strip for the product's semantic state language. |

## Construction and sizing

The mark has three deliberate layers: a tab-shaped frame, a restrained terminal
cue, and a beacon point with two outward arcs. Keep clear space equal to the
beacon-point diameter on every side. Use the full-color mark at 24 px or larger;
use the monochrome mark only when one ink color is required. Do not render the
horizontal logo below 160 px wide; use the mark instead.

The primary palette is ink `#14213D`, deep tab blue `#20345A`, beacon cyan
`#39D9F2`, and signal blue `#80C7FF`. The state strip additionally documents
neutral, working, ready, attention, and question colors. These colors support
presentation comprehension; they do not grant trust, compatibility, or
configuration authority.

## Light, dark, and monochrome use

The full-color mark has a transparent background and remains readable on the
reviewed light and dark surfaces. The monochrome asset uses TabBeacon ink and
is the approved one-color fallback on a light or muted-light panel. Preserve
the transparent background; do not add a gradient, shadow, outline, or
third-party logo lockup.

## Prohibited uses

- Do not redraw, stretch, rotate, crop, or recolor the mark outside this guide.
- Do not replace the beacon with an AI brain, robot, provider glyph, or Windows
  Terminal lookalike.
- Do not combine the mark with Codex, Agy, OpenAI, Windows Terminal, or other
  third-party marks as if TabBeacon were endorsed by them.
- Do not use the mark to imply native Windows Terminal tab-icon support. The
  accepted current-host disposition is
  [NO_GO](../research/WT_NATIVE_ICON_DISPOSITION.md).

Third-party provider names are used only to describe compatible integrations;
their trademarks remain separate from TabBeacon's product identity.

## Review record

The required SVG assets were rendered and inspected at 16, 24, 32, 64, and
128 px, plus horizontal README-logo scale, on light and dark review surfaces.
The tab silhouette, beacon point, and signal arcs remain identifiable at the
small sizes; no clipped geometry or small text exists in the marks.

```text
SMALL_MARK_REVIEW=PASS
LIGHT_MODE_REVIEW=PASS
DARK_MODE_REVIEW=PASS
NO_CLIPPED_GEOMETRY=true
NO_UNREADABLE_SMALL_TEXT=true
```

[`tabbeacon-overview.png`](../assets/screenshots/tabbeacon-overview.png) is a
privacy-safe capture of a real Windows Terminal window rendered by TabBeacon's
deterministic presentation fixture. It is intentionally not presented as a
live Codex or Agy model conversation. The capture contains only the controlled
fixture identity and terminal chrome; no private prompt, assistant, tool,
repository, path, token, or authentication content is present.
