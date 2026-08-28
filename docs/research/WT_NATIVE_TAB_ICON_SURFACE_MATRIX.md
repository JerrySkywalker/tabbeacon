# Windows Terminal Native Tab Icon Surface Matrix

Source baseline: stock Windows Terminal
`8c0a234f056910776e56afa3f8a38d6ddc3db33c` (revalidated 2026-08-28);
comparison fork: intelligent-terminal `fa707a7ff9604ac1f7399e8a810ef01dcb30db72`.

| Surface | Verified behavior | Can stock TabBeacon safely set a native tab icon? | Disposition |
| --- | --- | --- | --- |
| `Tab::UpdateIcon` / `TabViewItem.IconSource` | Internal UI-thread pipeline with hidden/restore state | No public entry point | Internal only |
| OSC 0 / 1 / 2 / 21 | All dispatch to title handling | No; title only | Use `TitleMarkBackend` |
| OSC 9001 | Stock dispatch supports `CmdNotFound` only | No icon action | Not a bridge |
| `ActionAndArgs` | Tab title/color actions exist; no tab-icon action | No | Not a bridge |
| `WT_SESSION` | Child environment session identity | No public tab object or icon setter | Binding only |
| `microsoft/intelligent-terminal` | Adds fork-specific shell metadata | No portable stock contract | Do not depend on it |
| Windows XAML Diagnostics | Official instrumentation endpoint with injected site object | No safe attach substrate on the available stock WT: a fresh named window did not produce a new Terminal process | `NO_GO` on current host; re-admit only after a material isolation/public-API change |

No column in this matrix licenses private ABI invocation, signature scanning,
hard-coded addresses, Store-package replacement, a settings hack, a default
terminal change, or attachment to an existing Owner/development Terminal host.
