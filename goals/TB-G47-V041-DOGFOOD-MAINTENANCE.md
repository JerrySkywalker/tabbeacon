# TB-G47 — v0.4.1 Dogfood Maintenance & Release

## Status

PLANNED as the first production step after v0.4.0 + post-release G45X.

## Purpose

Turn real v0.4 dogfood findings into one narrow public patch release before v0.5 changes architecture. G47 should leave the public baseline clean enough that v0.5 can focus on new product capability rather than carrying known UX defects.

## Inputs

Known current facts:

- crates.io/GitHub `v0.4.0` does not contain post-release `tabbeacon sessions`;
- issue #45: Control Center Up/Down navigation consumes repeated key events too aggressively;
- `detect_windows_terminal()` currently launches `wt.exe --version`, which can create an unwanted Windows Terminal window;
- normal Human Setup/Config output still exposes machine-style flags and internal feasibility/debug fields;
- Human CLI output has minimal semantic color;
- installation dogfood showed public build requires Rust 1.97.1 while a user default toolchain may still be older.

## Scope

### 1. Publish post-release Sessions

Ship the already-accepted read-only Sessions capability in the next public binary:

```text
tabbeacon sessions
tabbeacon sessions --json
tabbeacon sessions --plain
```

Preserve G45X privacy boundaries: no raw native session IDs, prompt/tool content, process control, or session control.

### 2. Key-repeat navigation fix

Use Crossterm key-event kinds intentionally.

Default policy for page/field/value navigation:

```text
Press   -> one action
Repeat  -> ignored
Release -> ignored
```

`Ctrl+C`, quit confirmation, Apply/Revert, and feature-gated terminal smoke paths must remain correct. One physical press must advance exactly one screen/value. Long-list repeat is deferred to v0.5 where it can be rate-limited deliberately.

### 3. Remove Windows Terminal launch probe

Setup detection must not spawn the Windows Terminal GUI launcher merely to detect availability.

Required behavior:

- `WT_SESSION` proves current Windows Terminal session;
- any non-current-session availability detection must be static/non-launching (PATH/App Execution Alias/package/path inspection or a simpler truthful `not current session` classification);
- running `tabbeacon setup`, `setup --quick`, or `setup --full` must not create a new Terminal window as a detection side effect.

### 4. Human output cleanup

Default Human flows must stop interleaving machine receipts such as:

```text
SETUP=PASS
OWNER_ACTION=none
CONFIG_PATH=...
TITLE_SPINNER_FEASIBILITY=PRODUCTION
```

Normal Human output should show a compact final summary and actionable follow-up. Existing machine-oriented fields belong to JSON/plain/explicit diagnostic channels. Do not remove machine evidence required by tests; route it to the correct output mode.

At minimum clean:

```text
setup / setup --quick / setup --full
config show / set / preset / reset where human output is used
uninstall human result where applicable
```

### 5. Minimal semantic color

Add restrained Human-only semantic color where practical:

```text
heading/accent
success
attention/warning
failure
dim explanation
```

Admit `auto / always / never` as the target color policy only if it can be done without prematurely introducing the full v0.5 Interface preference architecture. If persistence would broaden G47 too much, implement safe `auto` rendering and leave persistent color preference to G50.

Color must never be the sole state signal and must disappear automatically when output is redirected unless explicitly forced by an admitted flag.

### 6. Installation guidance

Update installation/troubleshooting docs so the minimum Rust 1.97.1 requirement and process-scoped `rustup run 1.97.1 cargo install ...` recovery are clear. Do not mutate the user's default or machine Rust toolchain automatically.

## Risk vector

```text
CODE_CHANGED=true
PRESENTATION_CHANGED=true   # TUI navigation/human visible output
PROVIDER_CHANGED=false
USER_PERSISTENT_CONFIG_CHANGED=false unless color persistence is explicitly admitted
SECURITY_OR_PRIVACY_CHANGED=false
RELEASE_BOUNDARY=true
```

Use one representative real Windows Terminal Control Center smoke for the input/lifecycle changes plus ordinary hosted exact-head code CI. Do not rerun provider L4 when Hook/profile/trust source is unchanged.

## Acceptance

```text
SESSIONS_PUBLIC_BINARY=PASS
SESSIONS_PRIVACY=PASS

KEY_PRESS_ONE_STEP=true
KEY_REPEAT_IGNORED_FOR_PAGE_NAV=true
KEY_RELEASE_IGNORED_FOR_PAGE_NAV=true
ISSUE_45_FIXED=true

SETUP_WT_POPUP=false
WT_DETECTION_NON_LAUNCHING=true

HUMAN_MACHINE_FLAGS_SEPARATED=true
HUMAN_SETUP_SUMMARY=PASS
HUMAN_CONFIG_SUMMARY=PASS

HUMAN_COLOR_MONOCHROME_SAFE=true
REDIRECTED_OUTPUT_NO_UNWANTED_ANSI=true

RUST_1_97_1_GUIDANCE=PASS

CODE_CI=PASS
WINDOWS_TERMINAL_SMOKE=PASS
V0_4_1_RELEASE=PASS
```

## Release closure

Release `0.4.1` / `v0.4.1` from one accepted immutable release source. Required fresh work:

- locked code/static/build CI;
- focused TUI input regression;
- no-popup Setup smoke;
- Human/JSON/plain contract regression;
- package dry-run/content inspection;
- Windows x64 ZIP + SHA-256;
- crates.io publication/verification;
- GitHub tag/Release/assets;
- clean public consumer verification.

## Non-goals

Do not start the v0.5 i18n architecture, Naming Engine v2, import/export, live-refresh TUI, new provider, daemon, wrapper, automatic trust, or repository-local config in this Goal.

## Estimated effort

**6–10 effective engineering hours.**

## Next

`TB-G50 — Unified Human Presentation & i18n Foundation`.
