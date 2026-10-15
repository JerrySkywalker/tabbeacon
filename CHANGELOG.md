# Changelog

All notable changes to TabBeacon will be documented here.

## [0.9.0] - Unreleased shared TUI development candidate

- Owner-approved [bounded support contract v1](docs/v0.9.0-support-contract.md):
  original Preview binding, observed drift refusal, cooperating writers and limited
  recovery. Arbitrary external-writer CAS and all-or-nothing cross-file crash
  commits are explicitly unsupported, not test passes. Human en-US/zh-CN risk
  notices precede Repair/Restore and Alias writes; unsafe recovery retains evidence.
  JSON/plain keep their machine transport without localized stderr notices.
  The bounded engineering baseline and frozen shared API are Owner-accepted;
  new material bytes, publication and human/optical qualification remain gates.

- Absorb the never-released v0.8.2 Hook Names/Doctor Matrix candidate in Draft
  PR #123; preserve Issue #122 history and track the new train in Issue #124.
- Consume the public crates.io packages Core `terminal-ui-contract =0.2.0`,
  Runtime `terminal-ui-runtime =0.1.0` and Adapter `terminal-ui-ratatui =0.1.0`;
  Cargo.lock pins their registry checksums. Frozen API source is
  `eb16cd91f924c145180b5befcf9f2cacc7a46d84`, distinct from package source
  `206325e2367cda90f8cbe5d418dc7761da5f81bb`. Historical Git/unpublished status
  is superseded; TabBeacon publication remains independently unauthorized.
- Share one provider observation per refresh, collect off the UI thread and retain
  immutable snapshots, last-good/error state, single-flight scheduling and epochs.
- Use shared Standard/Vim/Custom input, pane focus, mouse hit geometry and bounded
  viewports; F2 Smart/Raw shares the same revision and F5 only requests refresh.
- Preserve staged editing, CAS and manual Hook trust. Source/synthetic evidence
  does not qualify real input latency or authorize publication/install/trust.
  See [Owner review notes](docs/v0.9.0-owner-review.md).

### Release materials / 发行材料

- Include both READMEs in the normal Cargo package; refresh current dependency,
  API-freeze and version facts and provide the official bilingual
  [v0.8.1 → v0.9.0 upgrade/rollback guide](docs/v0.9.0-upgrade.md).
- Configure only the normal Windows MSVC Release binary link with packaged
  `build.rs` and `/Brepro`.
  Two independent clean normal Release builds must match their entire raw SHA256;
  a minimal probe or binary metadata normalization is not acceptance. Exact
  artifacts and current result belong to the candidate's evidence receipt.
- Preserve the original24-byte metadata FAIL and both old binaries. Train D
  remains PARTIAL (O01/O02 Owner PASS,18 SKIP); optical P95, actual terminal
  recovery, DEC, current real Provider and strict OS-stall exit remain UNPROVEN.
  The historical700ms failure stays visible, with unresolved cause/stability.
- Source/package/license/MSRV/CI evidence never grants Ready, merge, tag,
  publication, installation, Hook trust or real Provider permission.

### 简体中文 v0.9.0

- 接入公开 Registry 的精确 Core0.2.0、Runtime0.1.0、Adapter0.1.0；冻结 API 与包源码
  身份分开记录，旧 Git/未发布依赖状态仅为历史。TabBeacon0.9.0 仍是未发布候选。
- 共享异步观察、快照、Standard/Vim/Custom 输入、焦点/鼠标几何及有界视口；保留
  分阶段编辑、所有权、合作写者条件提交、已观察漂移拒绝和有限恢复，不新增功能。
- 双语 README 纳入正常包；更新 Owner Review、发行流程及官方081→090升级回滚文档。
  Windows MSVC `/Brepro` 的完整产物资格以两次独立干净 Release 构建原始哈希一致为准。
- 原24字节重建 FAIL、700ms 历史失败和真人2 PASS/18 SKIP永久保留。光学 P95、
  实际终端恢复、DEC、真实 Provider、严格 OS stall仍 UNPROVEN，不把材料通过当作发布批准。
- 任意外部写者强 CAS、跨文件崩溃原子性为本版明确不支持的非门禁；手工 Hook trust
  和其他安全/资格门禁保持独立。未安装、未发布、未自动授予信任。

## [0.8.2] - Unreleased scoped candidate

- Name the eleven existing Codex Hooks `TabBeacon · <Event>` with handler-level
  `statusMessage`. Preserve commands, events, matcher and timeout semantics;
  exact complete historical ownership is required for migration. Changed trust
  hashes require fresh manual native `/hooks` review; trust is never granted by setup.
- Add read-only `doctor --matrix` with provider and execution-mode filters and
  shared Human/Plain/JSON evidence. Static declaration/trust health, registered
  capabilities, current-view evidence and historical no-daemon proof stay separate.
- Integrate mode evidence and readable owned names into the existing Integrations,
  Diagnostics and Hooks pages. Native shared-daemon terminal routing remains
  blocked on upstream capability; no daemon support or publication is claimed.

## [0.8.1] - Official compatibility hotfix (publication recorded externally)

- Recover the originating native Codex console for Windows command Hooks launched
  with `CREATE_NO_WINDOW`, using bounded ancestry, installed-image identity,
  inherited terminal binding and existing console membership. Preserve redirected
  protocol handles; detach only the Hook after rendering and worker launch.
- Separate Doctor's isolated command-ingress proof from unproven real provider
  terminal delivery. After valid-route tail measurements, give Windows command
  Hooks a two-second deadline; retain non-Windows/MCP contracts and manual trust.
- Add an owned console regression and a content-minimal old/new launcher fixture.
  Require known complete declarations before setup/uninstall configuration writes,
  including rejection of jointly edited manifest and live commands.
  Historical candidate requirement: real Codex 0.159.2 TUI acceptance and
  publication were separate Owner gates. Official v0.8.1 publication and its
  historical exact-version qualification are recorded in the public release;
  they do not qualify the current v0.9.0 Provider or physical-terminal claims.

## [0.8.0] - Release contents (publication date recorded externally)

- Added partial per-provider presentation overrides, explicit preview/Apply,
  portable v2 configuration transfer, and ownership-aware transition handling.
- Added isolated Cursor structured-Hook routing and strict color-only output
  while preserving its native title; the original daily command remains `agent`.
- Added a capability-gated Codex structured `Interrupt` path and bilingual
  product terminology. Warning, failed-turn, and the original HTTP 404 class
  in the audited Codex 0.157.1 original-command Hook path are Owner-approved
  v0.8.0 known limitations: these errors may leave tab state unchanged, their
  detection is not PASS, and Issue #116 remains open.
- Admitted exactly Agy 1.2.7 alongside 1.1.19 for title-only and
  preserve-native configuration. Its real mixed Working title is observed;
  real Ready visibility and attended preserve-native switching remain unproven.
- Accepted mixed Codex/Cursor working/completion and Agy Working/noninterference
  observations; same-tab handoff and Agy native switching were not executed
  at the Owner's direction. Cursor remains strict color-only with native title.

## [0.7.3] - 2026-09-01

### Added

- Added a privacy-safe deterministic promotional demo captured from one
  exact-owned real Windows Terminal window using TabBeacon's production
  renderer and synthetic `API`, `WEB`, and `DOCS` workspaces.
- Added validated social-preview SVG/PNG assets and refreshed public repository
  discovery metadata around the Codex-led "Codex and more" position.

### Changed

- Moved the exact Codex/Agy support truth and real demo higher in the English
  and Simplified Chinese READMEs, using immutable media links that render from
  crates.io.
- Simplified the normal install path to `cargo install tabbeacon`, followed by
  `tabbeacon setup` and the unchanged daily `codex` command. Version-pinned
  `--locked` installation remains a release-verification path only.
- Tightened the package allowlist so promo media, social-preview assets,
  evidence, and FFmpeg remain outside the crate while all packaged test source
  dependencies remain complete.

### Maintained

- Changed no runtime or provider behavior. Codex remains capability-based,
  Agy remains the exact admitted 1.1.19 profile, and Claude Code/OpenCode remain
  deferred. Daily commands remain literal `codex` and `agy`; no wrapper, PATH
  shadow, PTY host, global daemon, trust bypass, or production configuration
  mutation is introduced.

## [0.7.2] - 2026-08-31

### Fixed

- Prevented Codex child/subagent Hook activity from replacing root-session tab
  state, while retaining synchronous fail-open command Hooks and their
  one-second timeout.
- Kept Stop-derived ResultReady presentation alive through its bounded static
  activity-worker lease after the one-shot Stop Hook exits.
- Moved active qualification and visual Windows Terminal windows onto
  exact-owned RUN_ID, anchor TabItem, and HWND lifecycle cleanup without broad
  process or window termination.

### Changed

- Migrated only exact-owned historical Codex MCP Hybrid integrations to the
  current command-v1 Hook transport. The transaction preserves unrelated
  Hooks, MCP servers, and configuration, and requires fresh manual `/hooks`
  trust review when its managed Hook declarations change.
- Removed avoidable Git subprocess work from conventional Codex Hook workspace
  identity resolution, retaining conservative fallback for uncommon or
  ambiguous repository layouts.

### Maintained

- Added no provider or presentation feature. Daily launch remains literal
  `codex`; no wrapper, PATH shadow, PTY host, global daemon, trust bypass, or
  production configuration mutation is introduced.

## [0.7.1] - 2026-08-28

### Changed

- Repaired the TabBeacon SVG wordmark with a deterministic bounded glyph grid:
  cap height and baseline are uniform, glyph cells do not overlap, and the
  primary mark remains unchanged.
- Reconciled current-facing Agy and architecture documentation with the
  admitted Agy 1.1.19 production profile, while preserving historical records.
- Corrected the current public-release pointer in the governance roadmap.
- Entered a deliberate post-v0.7.1 dogfood pause. Future feature work requires
  a new explicit Owner admission; v0.8 options remain non-authoritative.

### Maintained

- No runtime, provider, configuration-ownership, Hook-trust, terminal
  presentation, or native-tab-icon behavior changed in this maintenance
  release.

## [0.7.0] - 2026-08-28

### Added

- Added TabBeacon's original SVG identity, English-canonical README, full
  Simplified Chinese README, privacy-safe real Windows Terminal fixture
  screenshot, documentation portal, user/design/development guides,
  CONTRIBUTING v2, and offline documentation CI checks.
- Added a scalable supported-coding-agent guide: Codex remains
  capability-based, while Agy remains production-supported only for the exact
  admitted 1.1.19 profile. Claude Code and OpenCode remain deferred.

### Changed

- Closed Native Windows Terminal Tab Icon feasibility as `NO_GO`: stock
  Windows Terminal still has no supported public tab-icon bridge, and the
  documented XAML Diagnostics route could not meet the required isolated-target
  safety bar. No attachment or mutation was attempted; `TitleMarkBackend`
  remains the production presentation path.

## [0.6.1] - 2026-08-27

### Added

- Added capability-based Codex Hook admission: local explicit Hook capability
  and schema evidence select the conservative `codex-hooks-command-v1` profile
  without treating a Codex version string as mutation authority.
- Added Provider Visual Identity v2 propagation through activity workers, with
  independent provider, runtime, and workspace title slots.
- Added OS-level Windows regression coverage for ambient inherited pipe handles
  and descendant redirected-stream EOF closure.

### Changed

- Switched persistent Windows title output to Unicode-wide VT transport and
  kept `SetConsoleTitleW` outside the normal title path.
- Preserved the originating Hook's first complete title frame for a newly
  published worker; already-active workers render without replacing that title.
- Restricted Windows worker and observer handle inheritance with
  `PROC_THREAD_ATTRIBUTE_HANDLE_LIST`. Explicit NUL-backed standard streams
  remain, while unrelated inheritable Codex redirected-pipe handles are not
  passed to descendants.
- Retained synchronous, fail-open command Hooks with a one-second timeout and
  manual Codex `/hooks` trust. Daily launch remains literal `codex`.

## [0.6.0] - 2026-08-25

### Added

- Added the exact Agy 1.1.19 production provider through its supported
  user-global structured title callback while keeping daily launch literal
  `agy`.
- Added content-minimal Agy Ready/Working lifecycle projection, provider-aware
  Sessions and title explanation, compact `A` provider badges, and a
  provider-namespaced Root Workspace Anchor.

### Changed

- Agy setup now persists title activation, migrates the earlier owned quoted
  command declaration, preserves unrelated settings, and refuses callback
  executable paths that require unsafe shell quoting.
- Integrations now exposes Agy's admitted callback capabilities and explicitly
  reports unavailable approval/health/background state plus unsupported direct
  Windows Terminal presentation.

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
