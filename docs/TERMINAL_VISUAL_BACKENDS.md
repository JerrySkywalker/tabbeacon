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

No native tab icon backend is enabled or implied by this model. A future
`WtNativeProtocolBackend` or `WtXamlDiagnosticsBackend` must implement the
same capability boundary and remain disabled until it proves exact-tab
correlation, restoration, upgrade tolerance, and fail-open fallback. Provider
metadata alone can never select a native icon path.

The current disposition therefore remains title-mark fallback first. Native
icon failure is decoration-only and cannot block the v0.6.1 release train.
