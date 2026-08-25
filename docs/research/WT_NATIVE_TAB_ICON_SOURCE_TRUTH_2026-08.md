# Windows Terminal Native Tab Icon Source Truth — 2026-08

- Research goal: `TB-V061-12H-FORWARD-COMPAT-VISUAL-001`
- Stock source: [`microsoft/terminal` `86d15aef08e500be34497ff4e3a6f0d099ffb067`](https://github.com/microsoft/terminal/tree/86d15aef08e500be34497ff4e3a6f0d099ffb067)
- Experimental fork compared read-only: [`microsoft/intelligent-terminal` `fa707a7ff9604ac1f7399e8a810ef01dcb30db72`](https://github.com/microsoft/intelligent-terminal/tree/fa707a7ff9604ac1f7399e8a810ef01dcb30db72)
- Date: 2026-08-25

## Confirmed internal pipeline

Stock [`Tab::UpdateIcon`](https://github.com/microsoft/terminal/blob/86d15aef08e500be34497ff4e3a6f0d099ffb067/src/cascadia/TerminalApp/Tab.cpp)
stores `_lastIconPath` and icon style, honors hidden state, and mutates both
the tab model icon and `TabViewItem.IconSource`. Restoring visibility reapplies
the stored icon path. `TerminalPage` invokes its tab-icon update path on active
pane changes and settings refreshes. This proves an internal native pipeline,
not an external app protocol.

`WT_INTERNAL_NATIVE_PIPELINE=SOURCE_CONFIRMED`

## Stock terminal protocol truth

Stock [`OutputStateMachineEngine`](https://github.com/microsoft/terminal/blob/86d15aef08e500be34497ff4e3a6f0d099ffb067/src/terminal/parser/OutputStateMachineEngine.cpp)
maps OSC 0, 1, 2, and 21 to `SetWindowTitle`. They do not route to
`Tab::UpdateIcon`. OSC 9001 dispatches `DoWTAction`; stock
[`AdaptDispatch`](https://github.com/microsoft/terminal/blob/86d15aef08e500be34497ff4e3a6f0d099ffb067/src/terminal/adapter/adaptDispatch.cpp)
handles only the `CmdNotFound` action. It is not a tab-icon transport.

The stock action model exposes tab title and tab color (`NewTerminalArgs`,
`RenameTabArgs`, and `SetTabColorArgs`), but no `SetTabIcon` action in
[`ActionArgs.idl`](https://github.com/microsoft/terminal/blob/86d15aef08e500be34497ff4e3a6f0d099ffb067/src/cascadia/TerminalSettingsModel/ActionArgs.idl).
The public request for terminal-controlled tab icons, [issue #1868](https://github.com/microsoft/terminal/issues/1868), remains open.

`WT_SESSION` is injected into child environments by stock
[`ConptyConnection`](https://github.com/microsoft/terminal/blob/86d15aef08e500be34497ff4e3a6f0d099ffb067/src/cascadia/TerminalConnection/ConptyConnection.cpp).
It is useful for terminal session binding, but it is not a public handle for
mutating a tab's `IconSource`.

## Fork comparison

The inspected intelligent-terminal files retain the same `Tab::UpdateIcon`
and OSC action enumerations. Its `DoWTAction` additionally recognizes a
`ShellType` payload and its connection environment adds fork-specific values.
That is fork-specific metadata, not a stock Windows Terminal native-icon
bridge. No fork capability is used by TabBeacon.

## XAML diagnostics boundary

Microsoft documents [`InitializeXamlDiagnosticsEx`](https://learn.microsoft.com/en-us/windows/win32/api/xamlom/nf-xamlom-initializexamldiagnosticsex)
as the XAML diagnostics entry point; its documented endpoint accepts a CLSID
for an `IObjectWithSite` implementation to be injected into the target
process. This is official tooling, but it is explicit process instrumentation,
not a stock terminal protocol. It has not yet proven reliable exact-tab
correlation or restoration for an isolated Windows Terminal process.

No XAML diagnostics attachment occurred in this goal. In particular, no
diagnostics shim was attached to the terminal hosting the goal.
