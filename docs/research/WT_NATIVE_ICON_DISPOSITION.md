# Windows Terminal Native Icon Disposition

## TB-G86 decision — 2026-08-28

```text
GOAL_ID=TB-V07-24H-NATIVE-ICON-FEASIBILITY-TRAIN-AB-001
NATIVE_TAB_ICON_DISPOSITION=NO_GO
PRODUCTION_RUNTIME_NATIVE_ICON=false
NORMAL_CLI_NATIVE_ICON=false
PRODUCTION_NATIVE_ICON_INTEGRATION=false
TITLE_MARK_BACKEND_REMAINS_PRODUCTION=true
```

This is a successful feasibility conclusion, not a feature failure. TabBeacon
will continue to use the stable `TitleMarkBackend` for terminal identity and
state decoration.

## Deciding safety boundary

G83 revalidated that stock Windows Terminal retains an internal icon pipeline
but has no supported public tab-icon bridge. The only remaining documented
route is `InitializeXamlDiagnosticsEx`, whose public contract loads an
`IObjectWithSite` diagnostic component into the *target process*.

On the available stock Windows Terminal 1.24.11911.0, two Windows Terminal
processes were already present before G84. A purpose-created, nonce-named
`wt -w <fresh-name>` observation then produced:

```text
PREEXISTING_WT_PROCESS_COUNT=2
POST_LAUNCH_NEW_WT_PROCESS_COUNT=0
TARGET_ADMISSION=REFUSED_NO_UNAMBIGUOUS_NEW_MARKER_WINDOW
INITIALIZE_XAML_DIAGNOSTICS_EX=NOT_CALLED
```

The harness did not infer ownership from the fresh window name, active tab,
PID image, window order, or a title prefix. Because no newly created Terminal
process could be proven, an XAML Diagnostics attach would have been
process-scoped instrumentation of an existing host that might contain the
active Codex or Owner terminal. That violates the feasibility isolation
contract, so the harness refused before the diagnostics API call.

## Consequences

- No XAML Diagnostics attachment, visual-tree enumeration, `IconSource`
  snapshot, icon mutation, icon restore, or native-tab visual change occurred.
- `WRONG_TAB_MUTATION=0`, `WT_CRASH=0`, and `RESTORE_FAILURE=0` describe the
  zero-authorized-mutation observation; they are not positive mutation proof.
- No elevation, Windows Terminal settings/package change, private ABI,
  signature scanning, memory patching, persistent helper, service, or Owner
  TabBeacon mutation was used.
- The temporary helper/source was intentionally not retained in the repository:
  without a safe target substrate it would be a generic process-instrumentation
  artifact rather than useful reproducible product research.

## Reconsideration rule

Do not retry this route merely because time remains. Re-open Native Tab Icon
research only under a new admitted Goal after at least one material condition
changes: stock Windows Terminal exposes a supported icon API, or an explicit
Owner-approved isolated Terminal process can be proven without touching an
active/development/production host. A new G83 source and live-isolation
revalidation is required before any future attachment.
