# Terminal Visual Backends

`TerminalVisualBackend` separates provider metadata from terminal capability.
Backends report only these decoration capabilities:

- `title_mark`;
- `native_tab_icon`;
- `progress`;
- `palette`.

Provider identity does not name or require a terminal implementation. A
backend decides independently whether native icon rendering is usable.

## Production backend

`TitleMarkBackend` is the v0.6.1 production backend. It wraps the existing
safe OSC title/progress/palette renderer and reports:

```text
title_mark=true
native_tab_icon=false
progress=true
palette=<Windows Terminal frame-color capability>
```

It always provides a stable textual provider fallback when the user's title
mode permits TabBeacon title output. Failure of a decoration path must not
affect literal `codex` or `agy` usability, Hook processing, provider
compatibility, or workspace identity.

## Native tab icon boundary

No native tab icon backend is enabled or implied by this model. Native Windows
Terminal tab icon is **NO_GO** under the accepted current-host safety evidence:
stock Windows Terminal has no supported public application-controlled bridge,
and the remaining process-scoped route could not be safely isolated. Provider
metadata alone can never select a native icon path.

`TitleMarkBackend` remains the production path. Reconsideration requires a
separately admitted Goal after a material public-API or safely isolated-target
change; it is not a postponed production feature.
