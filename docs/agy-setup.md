# Agy setup

TabBeacon 0.6.0 supports exactly Agy 1.1.19 on Windows through Agy's
user-global structured title callback. Install the provider integration once:

```powershell
tabbeacon setup agy
```

Daily launch remains the native command:

```powershell
agy
```

Setup owns only the `title` member in
`~/.gemini/antigravity-cli/settings.json`. It enables the supported callback,
preserves unrelated settings, refuses a foreign title owner or ownership
drift, and records an exact pre-install backup. Remove only TabBeacon's owned
declaration with:

```powershell
tabbeacon uninstall agy
```

When unrelated settings are unchanged, uninstall restores the original bytes.
When Agy has legitimately changed unrelated settings, uninstall preserves
those settings and removes only the exact owned title member. Reparse points,
unsupported versions, malformed ownership state, and unsafe executable paths
are refused.

## Admitted capabilities

Agy 1.1.19 provides a stable conversation identity, equal current/project
workspace roots, and the exact lifecycle subset `idle` (Ready) plus
`initializing`/`working` (Working). The callback returns a plain title.

Approval, result-ready, stop authority, health, background-task count,
failure, interruption, warning, direct tab color, Windows Terminal progress,
and animation are unavailable or unsupported. TabBeacon does not infer them
from arbitrary Agy output.

The callback stores only hashes, a safe workspace alias, bounded state tokens,
and timestamps. Prompt, assistant, tool, transcript, model, account, raw
session, and raw path content are not persisted. If TabBeacon is missing or a
callback fails, native Agy remains usable.

No wrapper, PATH shadow, PTY host, Hook configuration, or resident daemon is
installed.
