# Windows Terminal Native Icon Disposition

## Decision for this goal

```text
WT_INTERNAL_NATIVE_PIPELINE=SOURCE_CONFIRMED
STOCK_SAFE_NATIVE_ICON_BRIDGE=UNPROVEN
XAML_DIAGNOSTICS_SPIKE=N/A
NATIVE_ICON_DISPOSITION=UNPROVEN
```

Stock Windows Terminal has a real internal icon pipeline, but its public
terminal protocols and tab action model do not expose it. Therefore no
production or experimental TabBeacon native-icon backend is enabled.
`TitleMarkBackend` remains the complete production fallback.

## Why no XAML diagnostics spike ran

The only official diagnostics route found requires process instrumentation via
an injected `IObjectWithSite` implementation. This goal did not yet establish
an owned C++/WinRT diagnostic-site binary, a safe isolated stock-WT attach
workflow, exact `TabViewItem` correlation, mutation restoration, or recovery
after a helper failure. Attaching such a shim to the terminal hosting the goal
is prohibited. Building one merely to create a partial proof would expand the
P1 research scope beyond the completed P0/PVI train.

`XAML_SPIKE=N/A` is a bounded-budget outcome, not evidence that diagnostics is
safe or unsafe. A future opt-in-only spike must use an isolated Windows
Terminal process and stop immediately on wrong-tab mutation or a crash.

## Release consequence

Native icon failure does not affect provider identity, runtime state, workspace
identity, literal `codex`/`agy` launches, Hook behavior, or the v0.6.1 release
boundary. The next safe research goal is a disposable XAML Diagnostics
feasibility harness that proves only attach, enumerate, exact-tab correlate,
restore, and exit cleanup before considering any TabBeacon integration.
