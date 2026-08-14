# G05 dependency and supply-chain review

## Direct additions

| Crate | Locked version | License | Features | Assessment |
| --- | --- | --- | --- | --- |
| `atomic-write-file` | 0.3.1 | BSD-3-Clause | defaults only; no optional unnamed-temp/unstable features | Current crates.io version, Rust 1.85 floor, active unarchived repository, Windows generic backend tested. PASS. |
| `toml_edit` | 0.25.13+spec-1.1.0 | MIT OR Apache-2.0 | default parse/display only; no serde/debug/unbounded | Current crates.io version, Rust 1.85 floor, active unarchived repository, required for format-preserving edits. PASS. |

`atomic-write-file` brings `rand`/`getrandom` for unpredictable same-directory
temporary names. Its Windows/generic path uses create-new, file sync, and rename.
The one audited unsafe block converts generated ASCII alphanumeric bytes to
UTF-8 and includes a local safety justification. `toml_edit` itself contained
no `unsafe` occurrence in the locked crate source. TabBeacon retains
`unsafe_code = "forbid"` for product code.

The Windows backend cannot make hooks, config, and manifest one transaction;
the product therefore uses phase-marked ownership state and doctor rather than
claiming cross-file atomicity. No dependency change or feature reduction is
justified by the evidence. Overall: `DEPENDENCY_REVIEW=PASS`.
