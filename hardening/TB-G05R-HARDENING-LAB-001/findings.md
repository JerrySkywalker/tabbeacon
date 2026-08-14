# Finding register

| ID | Class | Severity | Status | Summary | Evidence |
| --- | --- | --- | --- | --- | --- |
| G05R-001 | PRODUCT_DEFECT / TRUST_BOUNDARY | P1 RELEASE-BLOCKING | FIXED_ON_HARDENING | Frozen doctor ignored `hooks.state.<key>.enabled=false` and falsely reported a trusted hook as active. The hardening patch treats absent enablement as the upstream default, false or invalid enablement as inactive, and returns typed `FAIL`. PR #6 was not changed. | Red/green `doctor_supports_current_codex_trust_shape_and_detects_inactive_or_conflicting_state`; trust run `20260815-004504`. |
| G05R-002 | FILESYSTEM / DOCUMENTATION | P3 FUTURE/HARDENING | OPEN | Per-file replacement is atomic, but multi-file setup is intentionally a manifest-guided protocol rather than one filesystem transaction. Interrupted `installing`, missing, and corrupt manifests are detected without guessing; recovery is manual. | Config-chaos run `20260815-004738`. |
| G05R-003 | TRUST_BOUNDARY / DOCUMENTATION | P3 FUTURE/HARDENING | OPEN | Uninstall removes hook declarations but leaves inert Codex trust-state keys, which Codex owns. An exact reinstall may therefore regain the prior trust hash even though setup conservatively prints review-required. | Compatibility and trust-forensics runs. |
| G05R-004 | SECURITY_BOUNDARY | P3 FUTURE/HARDENING | OPEN | Direct symlink targets are refused. Ancestor reparse points and external same-user races remain filesystem/environment boundaries; fixed user paths and the ownership manifest prevent relative traversal, but this is not a formal reparse-point proof. | Static review of `config.rs`; fixed-path and lock design. |

```text
P0_FINDINGS=0
P1_FINDINGS=1
P2_FINDINGS=0
P3_FINDINGS=3
PRODUCTION_DEFECT_FOUND=true
```

Informational observations: Windows cold scanning of a newly copied executable
can exceed one second, while repeat hook paths measured 28–243 ms in the final
fuzz run; Windows could not start the deliberately overlong 316-character test
path; and this host required command-local Rust 1.97.1 variables because its
process-global Rust environment pointed at an inaccessible CI toolchain.
