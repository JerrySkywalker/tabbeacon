# Isolated Codex compatibility matrix

All homes and package locations are isolated beneath ignored lab output. No
global package change and no owner trust-state mutation occurred.

| Version | Role | Obtainable | Config | Discovery | commandWindows | Title config | Trust state | Lifecycle | Disposition |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0.147.0 | frozen/current/latest stable | yes | PASS | PASS | PASS | PASS | PASS | PASS | PASS |
| 0.146.0 | previous stable | yes | parses | discovers | parses | parses | shape compatible | preservation passes | OUTSIDE_DECLARED_SUPPORT |
| 0.145.0 | previous stable | yes | parses | discovers | parses | parses | shape compatible | preservation passes | OUTSIDE_DECLARED_SUPPORT |

Each version ran from a temporary package location with isolated `CODEX_HOME`
and `LOCALAPPDATA`. The official app-server `hooks/list` surface found the
seven owned declarations plus one unrelated fixture. The lab used the official
configuration write API to simulate trust only in those homes, then proved
doctor, uninstall, preservation, and reinstall behavior. All seven current
hashes were identical across the three binaries.

`MINIMUM_CODEX_VERSION=0.147.0` is deliberate. Successful parsing by an older
binary is compatibility information, not admission; doctor correctly fails
0.145.0 and 0.146.0. Overall matrix disposition: `PASS` for the declared
support contract.
