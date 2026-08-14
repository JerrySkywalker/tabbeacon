# Lab incidents and environment classification

- This host inherited `RUSTUP_HOME`/`CARGO_HOME` values under an inaccessible
  CI service directory and forced Rust 1.96. Commands were corrected locally to
  the already-installed user Rust 1.97.1 toolchain. No repository workaround or
  toolchain mutation was required. Class: `WINDOWS_ENVIRONMENT`.
- One initial compound npm acquisition timed out after the packages had
  completed. No package process remained; subsequent probes used the isolated
  package cache. Class: `EXTERNAL_SERVICE`/tool timeout, no product effect.
- An early PowerShell lab variable used the case-insensitive reserved name
  `$home`; assignment failed and one setup attempt inherited `CODEX_HOME` as
  `C:\Users\jerry`. TabBeacon refused setup because the existing ownership
  manifest targeted a different Codex home. A read-only audit found no new
  `config.toml`, `hooks.json`, or TabBeacon directory at that path and unchanged
  timestamps for the real owner Codex/configuration and integration state.
  All later probes isolate both `CODEX_HOME` and `LOCALAPPDATA`. Class:
  `TEST_ORACLE_DEFECT`, owner state untouched.
- Newly copied binaries were initially delayed by Windows scanning (up to about
  3.5 seconds). Repeat runs of the exact binary were 28–243 ms. The hook timeout
  remains the meaningful bound. Class: `WINDOWS_ENVIRONMENT`.
