# Windows Terminal Native Tab Icon Surface Matrix

Source baseline: stock Windows Terminal
`86d15aef08e500be34497ff4e3a6f0d099ffb067`; comparison fork:
intelligent-terminal `fa707a7ff9604ac1f7399e8a810ef01dcb30db72`.

| Surface | Verified behavior | Can stock TabBeacon safely set a native tab icon? | Disposition |
| --- | --- | --- | --- |
| `Tab::UpdateIcon` / `TabViewItem.IconSource` | Internal UI-thread pipeline with hidden/restore state | No public entry point | Internal only |
| OSC 0 / 1 / 2 / 21 | All dispatch to title handling | No; title only | Use `TitleMarkBackend` |
| OSC 9001 | Stock dispatch supports `CmdNotFound` only | No icon action | Not a bridge |
| `ActionAndArgs` | Tab title/color actions exist; no tab-icon action | No | Not a bridge |
| `WT_SESSION` | Child environment session identity | No public tab object or icon setter | Binding only |
| `microsoft/intelligent-terminal` | Adds fork-specific shell metadata | No portable stock contract | Do not depend on it |
| Windows XAML Diagnostics | Official instrumentation endpoint with injected site object | Unproven; needs isolated attach, tab correlation, restore proof | Bounded future spike only |

No column in this matrix licenses private ABI invocation, signature scanning,
hard-coded addresses, Store-package replacement, a settings hack, or a default
terminal change.
