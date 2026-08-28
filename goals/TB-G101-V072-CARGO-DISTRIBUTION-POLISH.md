# TB-G101 — v0.7.2 README & crates.io Distribution Polish

## Purpose

Make the public Rust installation story simple and durable while keeping large
promotion assets out of the runtime crate package.

The primary user command is:

```powershell
cargo install tabbeacon
```

The exact release verification command is separate:

```powershell
cargo install tabbeacon --version 0.7.2 --locked
```

## Preconditions

This roadmap document does not itself authorize G101 implementation. A fresh
explicit Owner admission is required before any G101 repository write. A
separately authorized goal may perform bounded read-only G101 preparation.

G99 and G100 should be accepted or reconciled cleanly onto the current v0.7.2
candidate. Re-read current Cargo metadata, package include/exclude behavior,
README English/Chinese parity checks, and crates.io rendering behavior.

## A. README product entry

Preserve the current brand system, language switch, and exactly two hero badges.
Place the animated demo high enough that a new visitor can understand TabBeacon
quickly.

Required:

```text
README_CANONICAL_LANGUAGE=en-US
README_ZH_CN=true
README_BADGE_COUNT=2
README_AGENT_BADGES=false
PROMO_GIF_VISIBLE_FROM_README=true
```

The GIF caption must remain truthful: deterministic showcase fixture, real
Windows Terminal and production renderer, no live model session.

The prior static screenshot may remain as secondary real-product evidence if it
adds value without clutter.

## B. Primary install contract

README Quick Start in both languages must lead with:

```powershell
cargo install tabbeacon
tabbeacon setup
```

Then show the literal supported daily coding-agent launch command, e.g. `codex`.

Do not require a version pin or `--locked` in the primary user path.

```text
README_PRIMARY_INSTALL_COMMAND=cargo install tabbeacon
README_VERSION_PIN_REQUIRED=false
README_LOCKED_FLAG_REQUIRED=false
```

A dependency-locked installation may be documented as an advanced/reproducible
option in Getting Started or development/release documentation.

## C. Exact release engineering contract

Release validation must still prove the exact target version using:

```powershell
cargo install tabbeacon --version 0.7.2 --locked
```

This verifies the actual v0.7.2 crate and its shipped lockfile rather than
whichever release is latest at a later time.

## D. Cargo/crates.io metadata audit

Audit current:

```text
package name/version
rust-version
license
repository
readme
keywords
categories
publish target
Cargo.lock
include/exclude/package file list
```

Run:

```powershell
cargo package --locked
```

Inspect the generated package file list and archive contents.

## E. Promotion asset separation

Large GitHub marketing assets are not runtime package dependencies.

Required:

```text
PROMO_GIF_IN_CRATE=false
SOCIAL_PREVIEW_IN_CRATE=false
PROMO_BUILD_EVIDENCE_IN_CRATE=false
```

Prefer a GitHub-hosted absolute media reference in the README when needed so the
same README can render on GitHub and crates.io without embedding the GIF in the
`.crate` archive.

Do not accidentally remove small documentation/brand assets that the packaged
README or shipped docs genuinely require.

## F. crates.io rendering and links

Verify the README remains meaningful when rendered from crates.io context:

- logo/media references resolve or degrade acceptably;
- documentation links do not assume GitHub-relative paths that crates.io cannot
  resolve where the user needs them;
- install command remains version-independent;
- current provider/support claims remain truthful.

Do not add installer/Winget/Scoop instructions for TabBeacon.

## G. Validation

Required:

```text
README_PRIMARY_INSTALL_COMMAND=PASS
README_EN_ZH_PARITY=PASS
README_BADGE_COUNT=2
CARGO_PACKAGE=PASS
CARGO_PACKAGE_CONTENT_AUDIT=PASS
PROMO_GIF_IN_CRATE=false
SOCIAL_PREVIEW_IN_CRATE=false
RUNTIME_BEHAVIOR_CHANGED=false
PROVIDER_BEHAVIOR_CHANGED=false
```

Run current docs checks and relevant Rust/package gates. Hosted exact-head CI is
required according to repository policy before the G101 candidate is accepted.

## Risk vector

```text
CODE_CHANGED=docs_package_metadata_expected
PRESENTATION_CHANGED=README_public_surface
PROVIDER_CHANGED=false
USER_PERSISTENT_CONFIG_CHANGED=false
SECURITY_OR_PRIVACY_CHANGED=false_expected
RELEASE_BOUNDARY=false
```

## Exit

G101 completes when the README's primary installation path is the simple normal
Cargo command, package metadata is correct, marketing media does not bloat the
crate, and the exact release verification contract remains available for G102.

Next: `TB-G102-V072-HARDENING-RELEASE.md`.
