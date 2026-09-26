# Cursor project Hook integration candidate (v0.8.0)

The v0.8.0 candidate keeps the original `agent` command. Hook installation is
explicitly scoped to one workspace and does not select an ambient Cursor profile.
Use a disposable or Owner-approved workspace during qualification:

```powershell
tabbeacon cursor check --workspace C:\path\to\workspace --json
tabbeacon setup --plain cursor --workspace C:\path\to\workspace
tabbeacon cursor uninstall --workspace C:\path\to\workspace --json
```

`setup` adds four exact-owned commands to `workspace\.cursor\hooks.json`:
`sessionStart`, `beforeSubmitPrompt`, `stop`, and `sessionEnd`. It preserves
foreign Hook entries and unrelated JSON fields. Repeating setup is idempotent;
a partial exact installation is reconciled. Modified or ambiguous owned
declarations fail closed instead of being overwritten. Uninstall removes only
the exact commands installed by the current executable; a changed executable
path needs explicit ownership review before cleanup. No real user workspace
should be changed by unattended qualification.

The hidden `__cursor-hook-v1` command reads bounded structured Hook JSON,
normalizes only event, session, generation and supported outcome, and checks
the exact project installation. The product Hook uses TabBeacon's own
`LOCALAPPDATA` state root; it does not require the temporary probe's
`CURSOR_DATA_DIR` or `TABBEACON_CURSOR_EXPECTED_WT_SHA256` variables. On
Windows it requires the Hook child and its operating-system parent to appear
on the same console process list, then binds the inherited `WT_SESSION` hash
to the persisted session route. Failure suppresses decoration. Its stdout is
only the Hook protocol object `{}`. It never reads prompt, response, tool
output or transcript text for state.

The current candidate can write strict color-only OSC through the owned
`CONOUT$` handle after the route and configuration checks. Its cross-process
lock orders the final write with generation admission. A per-terminal lease
records confirmed writes; preserve-native and exact-session end release only
confirmed owned color. Interrupted or partial writes remain unknown and grant
no later reset authority. This path does not emit title or progress bytes.
Terminal identity inheritance observed in the isolated real Cursor probe is
distinct from product output and visibility proof. One isolated original-command
product run at candidate `cfbd484` delivered admitted Hook state: the Owner saw
working/completed color changes without visible native-title interference, and
the later session left a matching end tombstone and unowned color lease. An
earlier start-only route on another terminal remained without an end event.
That run did not retain per-Hook flush results or completed-session generation
counts. A newer candidate writes a separate, bounded, best-effort receipt with
lower bounds for prompt/stop output attempts and confirmed color/release flushes,
plus start/end observations. The receipt never gates presentation or cleanup;
missing receipts and old sessions have unknown history. The v1 color lease stays
unchanged for rollback compatibility. These counts do not prove visible rendering.
Two concurrent Cursor sessions and mixed-provider Visual/L4 acceptance remain
unproven.
The read-only integration list therefore shows Cursor as an unadmitted
candidate. An exact project declaration can show `installed_unproven` while
terminal presentation remains unavailable.
Do not interpret a successful install or synthetic test as production support.
