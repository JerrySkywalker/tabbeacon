# Native Windows Terminal tab icon

## Disposition

```text
NATIVE_TAB_ICON_DISPOSITION=NO_GO
PRODUCTION_NATIVE_ICON_INTEGRATION=false
TITLE_MARK_BACKEND_REMAINS_PRODUCTION=true
```

Native Windows Terminal tab icon is not a v0.7 feature, an experimental
production integration, or a deferred implementation promise. It is a closed
feasibility conclusion under the accepted current-host evidence.

## Why the route stopped

Stock Windows Terminal retains an internal icon pipeline but exposes no
supported public application-controlled tab-icon protocol, action, or API. The
remaining documented XAML Diagnostics mechanism is process-scoped
instrumentation. On the qualified host, a purpose-created window could not be
proven to be isolated from pre-existing Windows Terminal processes. Any attach
could therefore have touched an Owner or development terminal, so the route
stopped before attachment.

No XAML Diagnostics attach, visual-tree enumeration, `IconSource` mutation,
snapshot, restore, memory patching, private ABI use, or settings/package change
occurred.

## Required boundary for any future admission

This `NO_GO` is not an incomplete production proof. In a separately admitted
future feasibility route, a mutation would require a content-minimal unique
title marker and `MATCH_COUNT == 1` before any write. A zero or multiple match
must fail open with no icon mutation. Before a write, that route would need the
strongest reversible `IconSource`/state snapshot the public API exposes,
neighboring-tab evidence for wrong-tab detection, a target-only visual change,
and successful restore. A single wrong-tab mutation would stop broadening.

None of those correlation, snapshot, mutation, or restore conditions was
reached or proven here. No experimental helper was retained in the repository:
without a safely isolated target it would be unsafe general
process-instrumentation tooling rather than reproducible research.

## Production behavior

`TitleMarkBackend` remains the production visual backend. It uses the typed
title, activity, palette, and Windows Terminal progress presentation model and
fails open when a decoration is unavailable. Provider identity cannot select an
unproven icon backend.

## Future reconsideration

Only a separately admitted Goal may reconsider this conclusion after a material
change: either Windows Terminal documents a supported native-icon bridge, or
an explicit Owner-approved, demonstrably isolated disposable terminal process
is available. A new source and isolation admission is required before any
instrumentation; a fresh window name or newer version number is insufficient.

Even a future `GO_PRODUCTION_CANDIDATE` feasibility result would not authorize
production native-icon integration in v0.7. Production integration requires a
separate, explicitly authorized release goal.

See the [research disposition](../research/WT_NATIVE_ICON_DISPOSITION.md) and
[ADR 0018](../adr/0018-native-tab-icon-feasibility-no-go.md) for exact evidence.
