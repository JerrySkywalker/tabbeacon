# Changelog

All notable changes to TabBeacon will be documented here.

## [0.5.2] - 2026-08-23

### Added

- Added the high-performance Codex 0.149 runtime: a Root Workspace Fast
  Anchor Path keeps normal post-anchor events at zero Git subprocesses, with
  a release-mode one-second regression gate.
- Added the Codex 0.149 hybrid transport: ten normal lifecycle Hooks use the
  session-scoped TabBeacon stdio MCP server, which alone receives
  `WT_SESSION`; only `SessionEnd` remains a command Hook.
- Added authoritative `SessionEnd` cleanup for activity, Windows Terminal
  progress, and owned frame/tab color. EOF cleanup remains fallback-only.

### Changed

- Normal events keep real spinner animation and result-ready transitions
  without starting a command shell; the synchronous `SessionEnd` cleanup Hook
  starts one command process per session.
- The source-audited Codex 0.147 command fallback remains supported. The
  direct Windows declaration and generic hostile-path declaration both retain
  synchronous one-second bounds.
- Strengthened owned-Hook repair and runtime continuity: exact owned groups
  are repaired only when proven absent, while third-party Hooks and MCP
  servers remain preserved. `/hooks` trust remains manual-only; existing MCP
  trust hashes are preserved and the new `SessionEnd` definition requires
  Owner review.
- Agy has no admitted production provider in this release.

## [0.5.1] - 2026-08-21

### Added

- Added read-only upgrade preflight diagnostics for runtime worker images and
  bounded owned-worker drain decisions.
- Added Hook Inspector ownership/currentness/trust diagnostics, Adaptive
  Naming score explanations, safe title provenance, and the localized
  `Why this title?` Control Center overlay.
- Added provider-neutral Integrations and provider-aware Sessions projections.
  Codex remains the only admitted production provider.

### Changed

- Admitted the source-audited Codex 0.149.0 Hook profile while preserving the
  manual trust boundary and fail-closed treatment of future unknown versions.
- Codex 0.149 now delivers ten content-minimal lifecycle events through a
  session-scoped stdio `mcp_tool` server with EOF cleanup, while preserving
  third-party MCP configuration and the Codex 0.147 command fallback.
- Long-lived activity workers now run from hash-verified immutable user-local
  runtime images, leaving the package-installed CLI replaceable on Windows.
- Kept provider badges compact by default, retained fail-open one-shot Hooks,
  and added no Agy provider, wrapper, PATH interception, daemon, self-update,
  project-local configuration, or remote-control surface.

## [0.5.0] - 2026-08-20

### Added

- Added bilingual Human presentation (`en-US` and `zh-CN`) while preserving
  stable JSON and plain automation contracts.
- Added deterministic Adaptive Naming v2, device-local Workspace alias
  preferences, and privacy-bounded top-level `export` / `import` portability.
- Added the daemonless live Control Center Workspace, Sessions, Interface,
  Help, and guided safe-repair surfaces.

### Changed

- Control Center refreshes only bounded local observations, preserves dirty
  drafts, refuses concurrent drift, and keeps all writes explicit.
- Kept Hook trust manual-only, added no provider, daemon, wrapper, PATH
  interception, project-local configuration, remote control, or self-update.

## [0.4.1] - 2026-08-20

### Added

- Published the privacy-preserving read-only `tabbeacon sessions` command that
  was accepted after the immutable v0.4.0 release.

### Changed

- Made Control Center page, field, and value navigation edge-triggered: Press
  advances once while Repeat and Release events are ignored.
- Removed Setup's `wt.exe --version` launcher probe. `WT_SESSION` proves the
  current session; other contexts are reported truthfully as not current.
- Made default Setup, Config, and Uninstall results concise Human summaries.
  Legacy machine receipts remain available only through explicit `--plain`.
- Added restrained automatic Human CLI color that is disabled for redirected
  output or `NO_COLOR`.
- Documented Rust 1.97.1 and the process-scoped `rustup run 1.97.1` install
  recovery path.

## [0.4.0] - 2026-08-19

### Added

- Added a typed command-line surface with shell completions, human-first
  `status` and `doctor` views, retained JSON contracts, and explicit legacy
  `--plain` output for automation compatibility.
- Added a unified read-only management projection and guided `setup --quick`
  presets that preview one atomic typed change before the existing
  ownership-aware Apply path.
- Added the keyboard-only Control Center with Overview, Appearance, Codex
  Integration, Diagnostics, and live Preview screens. Appearance edits remain
  staged until Apply and can be reverted without touching user state.

### Changed

- Hardened full-screen terminal cleanup across normal exit, Ctrl+C, setup
  errors, cleanup errors, and unwind paths; narrow terminals now render a
  bounded minimum-size message and non-TTY use remains non-fullscreen.
- Kept daily agent launch literally `codex`, retained manual Hook trust, and
  added no provider, wrapper, PATH interception, daemon, self-update, session
  control, or remote-control surface.

## [0.3.0] - 2026-08-18

### Changed

- Added visible title-authority observation and an explicit owned-title probe,
  with Windows Terminal title-policy diagnosis and ownership-safe remediation.
- Made the default working title animation a balanced 100 ms braille sequence
  while retaining a stable offline workspace alias.
- Hardened session convergence, generation isolation, recovery, and terminal
  close behavior without changing the daily `codex` command.
- Replaced the single exact-profile lookup with an offline typed Codex
  compatibility registry. Only `codex-hooks-rust-v0.147.0` is admitted;
  newer versions remain explicitly unadmitted or unknown and fail open.

## [0.2.0] - 2026-08-16

### Changed

- Fixed the native/off title lifecycle when an initially absent Codex
  `config.toml` is intentionally preserved: repeat setup, title reconciliation,
  doctor, and ownership-safe uninstall now continue to work without creating a
  competing title configuration.
- Added `tabbeacon status`, `tabbeacon status --json`, and `tabbeacon doctor
  --json`, backed by a versioned typed diagnostics model with structured
  warnings/failures, read-only state inspection, and content-minimal privacy
  boundaries.
- Added `tabbeacon setup`, a draft-first guided setup and reconfiguration flow
  that reports bounded environment/integration state, previews typed settings
  before Apply, preserves Cancel, and delegates Hook ownership/trust behavior
  to the existing `setup codex` implementation.
- Configured `title-spinner` and `both` activity modes now use a real
  session/turn/terminal-scoped ephemeral worker instead of the v0.1 frame-zero
  fallback; `title-indicator` remains the static option.
- Worker ownership uses content-minimal atomic leases, G10 generation plus
  event ordering, bounded predecessor handoff, terminal isolation, stale
  expiry, and path-plus-binary upgrade identity. Worker failure remains
  decoration-only and daily launch remains `codex`.
- Non-Git directories now resolve to opaque `dir-v1` workspace identities in
  the same stable alias registry as Git workspaces.
- Default TabBeacon-owned titles now use the compact status-first grammar
  `<status-slot> <repository-alias>`.
- Repository identity remains stable on the right while ready, working,
  result-ready, approval, question, and reset semantics update only the left
  slot; redundant lifecycle-word suffixes are no longer emitted.
- Codex Hooks now use the exact source-audited `0.147.0` compatibility profile,
  reject stale prior-turn events through content-minimal durable generations,
  isolate thread-spawn subagents, and classify compact lifecycle explicitly.
- Ownership-safe setup covers all eleven admitted Hook events while preserving
  unrelated notifiers, plugins, same-event groups, unknown events, and the
  official `/hooks` trust boundary.

## [0.1.1] - 2026-08-15

### Changed

- First crates.io distribution for TabBeacon.
- Cargo package metadata and package-content hygiene for ordinary Cargo users.
- `cargo install tabbeacon` installs only the user-facing CLI; internal visual
  test tooling remains opt-in for repository validation.
- No intended product runtime behavior changes versus the corrected v0.1.0
  release.

## [0.1.0] - 2026-08-15

### Added

- Persistent user-global presentation configuration with typed title, tab-color,
  activity, spinner, and theme choices.
- Comfortable `muted-dark` palette, retained `classic` compatibility palette,
  static title activity fallback, `preview`, and compact config CLI/wizard.
- Ownership-aware transition between TabBeacon and Codex native title output.
- First production provider using owned Codex user-global hooks.
- Fail-open one-shot hook normalization through the existing core, repository
  identity, and Windows Terminal presentation layers.
- Idempotent setup, read-only doctor, and ownership-safe uninstall with atomic
  configuration writes and exact local backups.
- Initial repository governance and Rust bootstrap skeleton.
