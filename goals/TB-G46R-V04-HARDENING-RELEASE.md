# TB-G46R — v0.4 Hardening and Release

## Status

COMPLETE. v0.4.0 was released from
`0e0c81279b4f90c9d67f9b841405401d532e7c24`; post-release G45X did not change
that immutable release source or tag.

## Purpose

Publish one accepted v0.4 source proving both fresh-install and v0.3-upgrade human workflows while preserving direct Codex launch, structured automation, ownership safety, and terminal recovery.

## Mandatory real-world upgrade path

```text
v0.3.0 installed/configured
  ↓
upgrade to v0.4.0
  ↓
existing settings/integration preserved or intentionally migrated
  ↓
tabbeacon setup --quick
  ↓
human status/doctor
  ↓
Control Center
  ↓
daily codex unchanged
```

Also cover a clean/fresh first-run installation path.

## Release gates

Use Fast Lane v2 during iteration, then one release train with:

- full locked Rust/static/build CI;
- CLI compatibility regression;
- JSON compatibility regression;
- human status/doctor dogfood;
- guided setup dogfood including atomic preset behavior;
- one real Windows Terminal Control Center enter/exit smoke;
- ownership/trust safety review;
- package/dry-run/content inspection;
- Windows x64 artifact/checksum;
- crates.io publication/verification;
- Git tag/GitHub Release/assets;
- clean public-consumer install verification.

Reuse unchanged v0.3 presentation/worker/provider evidence when relevant risk paths did not change. Do not rerun the G18 convergence matrix merely because v0.4 changed management UX.

## Release limitations

Do not silently add:

- new provider;
- App Server production backend;
- wrapper or PATH interception;
- global daemon;
- self-update;
- automatic Hook trust;
- session control.

## Completion definition

```text
TB_G40=COMPLETE
TB_G41=COMPLETE
TB_G42=COMPLETE
TB_G43=COMPLETE
TB_G44=COMPLETE
TB_G46=COMPLETE
TB_G45X=COMPLETE_POST_RELEASE

GUIDED_SETUP=PASS
GUIDED_SETUP_FULLSCREEN=false
GUIDED_SETUP_ENUM_TYPING_REQUIRED=false
PRESET_SELECTION_ATOMIC=true

STATUS_DEFAULT=HUMAN_FIRST
DOCTOR_DEFAULT=HUMAN_FIRST
STATUS_JSON_COMPATIBLE=true
DOCTOR_JSON_COMPATIBLE=true

CONTROL_CENTER=PASS
CONTROL_CENTER_STAGED_APPLY=true
LIVE_PREVIEW=true
TUI_EXIT_RESTORES_TERMINAL=true

DAILY_COMMAND=codex
HOOK_TRUST_BYPASS=false
PROVIDER_ADDED=false
GLOBAL_DAEMON_ADDED=false

V0_4_RELEASE=PASS
```

Estimated release effort: **3–5 h** after predecessor goals are accepted.

## Release outcome

- G46 real Windows Terminal lifecycle, terminal restoration, and same-shell
  sentinel: PASS.
- Hosted exact-head release CI, package dry-run, and package-content review:
  PASS.
- Fresh install and isolated v0.3-to-v0.4 upgrade with preserved settings and
  integration: PASS.
- crates.io `tabbeacon 0.4.0`, immutable `v0.4.0` tag, and GitHub Release:
  PASS.
- Windows x64 ZIP SHA-256:
  `c8e5c20f7a1c0b944ec86a3515e3d529284a87763031110a38e36794b80e5359`.
- Clean crates.io and GitHub-asset public consumers: PASS.
- Owner settings, Hook trust, Windows Terminal, and PowerShell mutations: none.
