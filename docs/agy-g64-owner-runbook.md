# Agy G64 Owner fast runbook

This runbook collects minimized qualification facts only. The Owner-present
2026-08-24 run admitted exact Agy 1.1.19 through the structured title callback;
ADR 0015 records the frozen production profile. Re-running this qualification
workflow does not itself admit or enable another version. Normal daily launch
remains literally `agy`.

```powershell
$tabbeacon = '<exact candidate tabbeacon.exe>'
$q = Join-Path $env:LOCALAPPDATA 'TabBeacon\qualification\agy'

& $tabbeacon agy qualification status --root $q
& $tabbeacon agy qualification plan
& $tabbeacon agy qualification init --root $q
& $tabbeacon agy qualification probe --root $q
```

The title callback command prepared for an Owner-approved temporary Agy
configuration is:

```text
<exact-tabbeacon.exe> agy qualification __title-callback-v1 --root <qualification-root>
```

Hook samples can be streamed into the matching minimized recorder:

```text
<provider Hook command> | <exact-tabbeacon.exe> agy qualification record-hook <pre-tool-use|post-tool-use|pre-invocation|post-invocation|stop> --root <qualification-root>
```

For an Owner-approved temporary Hook declaration, the fail-open callback form
is `<exact-tabbeacon.exe> agy qualification __hook-callback-v1 <event-name>
--root <qualification-root>`. Unknown event names return success without
parsing or retaining the raw payload.

Before applying either temporary integration, stop and verify the exact Agy
version, current official configuration schema/location, Owner presence, real
authenticated environment, byte-exact snapshot, ownership scope, and restore
plan. Do not guess the configuration path or shape and do not use a
workspace-local configuration.

After explicit Owner approval, exercise representative ready, working,
approval, result/stop, workspace, resume/fork, and background-task cases. Then:

```powershell
& $tabbeacon agy qualification inspect --root $q
& $tabbeacon agy qualification profile --root $q --json
& $tabbeacon agy qualification review --root $q --json
```

Restore the exact original Agy document, verify its byte-level digest and the
absence of TabBeacon-owned temporary declarations, then review the generated
packet. Only a separately implemented, versioned admitted-profile path may
accept it. After acceptance or rejection:

```powershell
& $tabbeacon agy qualification clean --root $q --confirm
```

Cleanup removes only a positively identified managed qualification workspace;
it never changes Agy configuration.
