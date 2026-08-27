# ADR 0018 — Native Tab Icon Feasibility `NO_GO`

- Status: Accepted
- Date: 2026-08-28
- Goals: `TB-G83` through `TB-G86`
- Train: `TB-V07-24H-NATIVE-ICON-FEASIBILITY-TRAIN-AB-001`

## Context

TabBeacon's production `TitleMarkBackend` is a safe title/progress/palette
renderer. Native Windows Terminal tab icons are a decoration-only enhancement;
they must never weaken literal `codex`/`agy` use, fail-open behavior, provider
isolation, or the prohibition on instrumenting active Owner/development
Terminals.

TB-G83 revalidated the current stock source at
[`microsoft/terminal` `8c0a234f056910776e56afa3f8a38d6ddc3db33c`](https://github.com/microsoft/terminal/tree/8c0a234f056910776e56afa3f8a38d6ddc3db33c).
`Tab::UpdateIcon` still reaches `TabViewItem.IconSource`, while OSC title
controls, OSC 9001 action dispatch, action arguments, and `WT_SESSION` do not
provide a public application-controlled native-icon bridge. The current public
issue for terminal-controlled tab icons remains open.

The only remaining documented feasibility mechanism was Microsoft's
[`InitializeXamlDiagnosticsEx`](https://learn.microsoft.com/windows/win32/api/xamlom/nf-xamlom-initializexamldiagnosticsex).
Its documented contract takes the target PID and a DLL/CLSID for an
`IObjectWithSite` implementation created in that target process. It is process
instrumentation, not a terminal protocol.

## G84 isolation evidence

The available stock Windows Terminal was Stable `1.24.11911.0`. G84 used an
exact-owned build/evidence root, a random content-minimal marker namespace, and
a purpose-created named window request. It first tested the helper's negative
path: PID `0` was refused before any diagnostics call.

For the live containment observation, the harness recorded two pre-existing
Windows Terminal processes and then observed no newly created
`WindowsTerminal` PID after the fresh named-window launch:

```text
PREEXISTING_WT_PROCESS_COUNT=2
POST_LAUNCH_NEW_WT_PROCESS_COUNT=0
TARGET_ADMISSION=REFUSED_NO_UNAMBIGUOUS_NEW_MARKER_WINDOW
```

The exact sanitized receipt remains under the train-owned build root, not in
the repository. No unrelated tab title, terminal text, environment, command
history, or authentication data was recorded.

The process-level XAML Diagnostics API could not distinguish the fresh window
from the existing hosts. Attaching would therefore risk instrumenting the
Windows Terminal process that hosts this Codex goal or other Owner work. The
harness correctly stopped before `InitializeXamlDiagnosticsEx`.

## Decision

```text
NATIVE_TAB_ICON_DISPOSITION=NO_GO
G84=BLOCKED_BY_ISOLATION_SAFETY_BOUNDARY
G85=NOT_STARTED
G86=SAFETY_TERMINATED_NO_GO_CLOSEOUT
PRODUCTION_NATIVE_ICON_INTEGRATION=false
```

Do not add a Native Tab Icon backend, CLI command, setup behavior, hook,
service, daemon, or configuration mutation. Keep `TitleMarkBackend` as the
only production terminal identity path.

No `IconSource` write, `SetProperty` call, mutation probe, snapshot, restore,
or reliability matrix mutation was authorized. The resulting zero counts are
negative-safety evidence, not a successful native-icon mechanism:

```text
WRONG_TAB_MUTATION=0
WT_CRASH=0
RESTORE_FAILURE=0
RELIABILITY_SCENARIOS_RUN=0
```

The transient harness source and binaries are intentionally excluded from the
repository. They cannot serve as a safe general helper without a separately
proven isolated target process.

## Reconsideration gate

Only a new admitted Goal may reconsider this decision, and only after one of
these material changes is independently revalidated:

1. stock Windows Terminal documents a supported public native-tab-icon
   protocol/action/API; or
2. an explicit Owner authorization supplies a safely isolated disposable
   Windows Terminal process that is provably not hosting Owner, development,
   or Codex work.

Any future route must re-run G83 and the isolation admission before attachment.
Neither a new Terminal version number nor a fresh window name is enough.
