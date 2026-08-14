# G05 static security review

This is a bounded code and adversarial-input review, not a formal proof.

| Risk | Evidence and disposition |
| --- | --- |
| Command injection | Absolute executable is double-quoted; `"`, `%`, CR, and LF are rejected. Real `cmd.exe` probes for representable metacharacters invoked only the copied binary. PASS. |
| Hook output/terminal injection | Hook ingress is silent; errors do not echo raw payloads; titles flow through typed presentation sanitization and owned console output. PASS. |
| Unbounded input/allocation | CLI reads at most 1 MiB plus one byte and drops larger input fail-open. Deep/large/future payload fuzz did not panic or hang. PASS. |
| Path traversal/deletion | Managed paths are fixed children of explicit roots. Uninstall removes only exact owned groups after manifest validation. Direct symlink targets are refused. PASS with P3 ancestor-reparse limitation. |
| Ownership/TOCTOU | Process lock serializes TabBeacon writers; exact declarations and target paths are revalidated. Same-user external races are surfaced by atomic errors/doctor but are not claimed eliminated. WARN/P3. |
| Backup exposure | Backups can contain the owner's prior Codex config and inherit the per-user local-app-data access boundary. Contents are never printed; retention and explicit ACL strengthening are future policy items. PASS with limitation. |
| Corruption/recovery | Corrupt config/hook/manifest, backup collision, locked file, and abandoned temp cases do not trigger guessed ownership or silent overwrite. PASS. |
| Fail open | Missing/renamed/non-executable binary, invalid input, repository/registry/output failures, and nonzero hook exit leave Codex usable; one-second declaration bounds synchronous execution. PASS. |

The release-blocking review result is G05R-001: a disabled but trusted owned
hook was falsely reported active. The hardening branch corrects it and adds a
regression test. No other P0/P1 issue was found. Overall on the hardening head:
`SECURITY_REVIEW=WARN` because the production PR does not yet contain that fix.
