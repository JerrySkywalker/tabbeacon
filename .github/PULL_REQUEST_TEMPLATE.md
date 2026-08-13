## Goal / scope

- Goal ID:
- Intended scope:
- Explicit non-goals:

## Exact head

- EXPECTED_HEAD:
- CODE_HEAD:
- VISUAL_HEAD: N/A / `<sha>`

## Validation

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] `cargo test --all-targets --all-features`
- [ ] `cargo build --locked --all-targets`
- [ ] Relevant integration checks
- [ ] Visual CI (when presentation-affecting and G03 is available)

## Safety / drift

- [ ] No unrelated drift was modified.
- [ ] External configuration changes are ownership-safe and reversible, if applicable.
- [ ] Fail-open behavior is preserved, if applicable.

## Evidence disposition

`PASS` / `FAIL` / `BLOCKED` / `UNPROVEN`
