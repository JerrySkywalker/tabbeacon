# TB-G57 — Dogfood Maintenance & Upgrade Preflight

## Status

PLANNED after public v0.5.0.

## Purpose

Turn the first real v0.5 dogfood findings into bounded maintenance behavior before larger architectural changes.

## Confirmed dogfood defect: installed executable lock

Observed on Windows during `cargo install tabbeacon --version 0.5.0 --locked --force`:

```text
compile PASS
Replacing %USERPROFILE%\.cargo\bin\tabbeacon.exe
Access denied / os error 5
```

Two live TabBeacon processes were proven to execute from the installed binary. After stopping only those exact-path TabBeacon processes, the executable became exclusively openable and the same install completed.

G57 must preserve this as a regression fixture and add a product-visible preflight.

## Upgrade preflight

Add a read-only command conceptually:

```text
tabbeacon upgrade-preflight
```

It should report:

- current executable path/version;
- whether the current executable is replaceable;
- live TabBeacon processes/workers attributable to the installed binary;
- whether each process can be proven to belong to TabBeacon runtime ownership;
- safe next action.

No mutation in default mode.

An explicit drain operation may be admitted only if it is ownership-scoped and Owner-explicit, e.g.:

```text
tabbeacon upgrade-preflight --drain
```

It must never kill Codex, Windows Terminal, PowerShell, Cargo, arbitrary `tabbeacon.exe` from another path, or an unproven process.

G63 remains the permanent fix; G57 is diagnosis/operational safety.

## Hook trust diagnostic precision

Refine Hook status so users can distinguish at least:

```text
DECLARATION_EXACT
DECLARATION_MODIFIED
CURRENTNESS_STALE
TRUST_HASH_STALE_OR_CHANGED
TRUST_REVIEW_REQUIRED
HOOK_DISABLED
HOOK_UNOWNED_OR_AMBIGUOUS
```

Do not describe exact/current Hook declarations as "definitions modified" merely because the Codex `trusted_hash` differs.

The real v0.5 dogfood case to preserve:

```text
hooks.declarations=PASS
hooks.currentness=PASS
hooks.trust=FAIL because 11 trusted_hash values differ
```

After manual Codex `/hooks` review, `hooks.trust` becomes PASS. Automatic trust remains forbidden.

## Issue #45 governance closeout

GitHub Issue #45 describes pre-v0.4.1 repeated-key navigation. Current production code filters non-`Press` key kinds. Verify deterministic regression coverage and close the issue as completed with the fixing release/behavior recorded. No reopening of the old implementation is required.

## Testing

Required families:

- active installed-binary worker detection;
- exact-path ownership filtering;
- no-worker replaceable state;
- read-only preflight causes no process/config mutation;
- explicit drain refuses ambiguous/unowned processes;
- explicit drain stops only proved TabBeacon worker processes;
- preflight after drain proves replaceable when OS permits it;
- trust-hash mismatch wording differs from declaration drift;
- declaration/currentness/trust combinations remain machine-stable;
- Issue #45 Press/Repeat/Release regression remains green.

## Risk vector

```text
CODE_CHANGED=true
PRESENTATION_CHANGED=true   # Human diagnostics/preflight output
PROVIDER_CHANGED=false
USER_PERSISTENT_CONFIG_CHANGED=false
SECURITY_OR_PRIVACY_CHANGED=true  # process/Hook ownership exposure
RELEASE_BOUNDARY=false
```

Use one focused ownership/safety review. No provider L4 or full Visual matrix.

## Acceptance

```text
UPGRADE_PREFLIGHT=PASS
UPGRADE_PREFLIGHT_DEFAULT_READ_ONLY=true
INSTALLED_BINARY_LOCK_DETECTED=true
OWNED_WORKER_FILTER=PASS
UNOWNED_PROCESS_NOT_KILLED=true
EXPLICIT_DRAIN_ONLY=true
HOOK_TRUST_DIAGNOSTICS_PRECISE=true
AUTO_HOOK_TRUST=false
ISSUE_45_REGRESSION=PASS
ISSUE_45_CLOSEOUT=true
CODE_CI=PASS
```

## Estimated effort

**4–6 effective engineering hours.**

## Next

`TB-G58 — Presentation Channel Cleanup`.