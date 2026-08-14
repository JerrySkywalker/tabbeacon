# Hardening test results

| Area | Result | Evidence |
| --- | --- | --- |
| Upstream/current stable | PASS | Installed, npm latest, and stable release are 0.147.0; upstream main observed at `23094236acac6fdc22f67a408ea8ccb8fac8e6e1`. |
| Compatibility matrix | PASS | Run `20260815-004718`; 0.147.0 supported, 0.145.0/0.146.0 explicitly outside support. |
| Trust forensics | PASS after P1 fix | Run `20260815-004504`; eight semantic mutation cases. |
| Windows quoting | PASS | Run `20260815-004240`; spaces, parentheses, ampersand, caret, bang, apostrophe, Unicode, long path, `%` rejection, and shell fail-open. |
| Hook fuzz | PASS | Run `20260815-004738`; 21 malformed/adversarial cases, no output, no timeout, maximum 243 ms. |
| Config chaos | PASS | Run `20260815-004738`; 11 preservation, corruption, locking, recovery, and race cases. |
| Atomicity/crash consistency | PASS within contract | Atomic per-file writes; content-addressed backups; `installing`/corrupt/missing manifests produce typed failure; no silent repair. |
| Multi-session/multi-repo | PASS | Three hardening integration tests, including 16 concurrent sessions and collision-family repositories. |
| Fail-open | PASS | One MiB input cap; silent hook CLI; missing Git/repository/output/state failures contained; command timeout/exit neutralization covered. |
| Lifecycle drills | PASS | setup/repeat/uninstall/reinstall, unrelated-hook preservation, modified-owned refusal, manifest damage, and isolated version upgrade. |
| Non-owner E2E | PASS | Hook JSON -> adapter -> evidence -> reconciler -> repository identity -> presentation -> VT bytes; not claimed as G07. |

The Windows quoting lab also used special-character `CODEX_HOME` and
`LOCALAPPDATA` paths. `|` and `"` are not representable in Windows file names;
a 316-character executable path was classified `WINDOWS_ENVIRONMENT`, not
product PASS. `%` in the executable path is rejected before config mutation.

No visual runner was started because neither the hardening test additions nor
the proposed doctor correction change presentation behavior.
