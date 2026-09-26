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
the exact project installation and inherited terminal context against a
separately captured expected Windows Terminal digest. The latter is presently
available only in the Owner's isolated qualification tab; without it, route
admission is refused. Its stdout is
only the Hook protocol object `{}`. It never reads prompt, response, tool
output or transcript text for state. Unknown, malformed and unbound events are
decoration-only failures.

The current candidate persists generation-safe session routing but has no
production color write. Terminal identity inheritance observed in the isolated
real Cursor probe is distinct from a verified safe output channel. Real
color-only display, native-title preservation, release on exit, two concurrent
Cursor sessions, and mixed-provider Visual/L4 acceptance remain unproven.
Do not interpret a successful install or synthetic test as production support.
