# TB-G05 — First Provider: Codex Hooks

## 1. Goal identity

```text
GOAL_ID=TB-G05-CODEX-HOOKS
STARTING_MAIN=70f85d9bf0742965e94b81c59ec3ec02e1b93425
BRANCH=tb-g05-codex-hooks
REPOSITORY=JerrySkywalker/tabbeacon
```

This goal implements the first production provider backend by consuming the
supported Codex command-hook interface. It does not introduce a launcher or a
second agent runtime.

## 2. Upstream admission evidence

The contract was frozen on 2026-08-14 after reading the current OpenAI Codex
hooks and configuration documentation, the locally installed CLI, and current
upstream source.

```text
LOCAL_CODEX_VERSION=0.147.0
LOCAL_CODEX_HOOKS_FEATURE=stable/enabled
UPSTREAM_RELEASE=rust-v0.147.0
UPSTREAM_SOURCE_HEAD=4eff3b788ba629acc944ed6db6502c362fc08e0a
UPSTREAM_SOURCE_DATE=2026-08-14T08:16:23Z
```

Admitted upstream behavior:

- command hooks are discovered from the user layer at
  `~/.codex/hooks.json` and from inline `~/.codex/config.toml` declarations;
- project hook sources are trust-gated and are not needed for this global
  integration;
- current supported events include `SessionStart`, `UserPromptSubmit`,
  `PreToolUse`, `PermissionRequest`, `PostToolUse`, `Stop`, and `SessionEnd`;
- Windows command handlers use `commandWindows` in JSON
  (`command_windows` in TOML);
- only command handlers currently execute;
- unmanaged hook definitions run only after the user trusts the current
  normalized definition hash; a changed definition is inactive until reviewed
  again;
- asynchronous command hooks are informational and do not participate in the
  operation's control decision;
- `SessionEnd` is always run synchronously, defaults to a one-second timeout,
  is capped at three seconds, and cannot keep the thread open;
- Codex owns the terminal title by default; the supported disablement is
  `[tui].terminal_title = []`.

Generated schemas on upstream `main` can lead the installed release. Release
documentation and behavior from the installed `0.147.0` binary govern the real
smoke; the pinned source head is implementation evidence, not permission to
depend on unreleased fields.

## 3. Primary invariant

```text
BEFORE=codex
AFTER_TABBEACON=codex
```

Allowed one-time management commands are:

```text
tabbeacon setup codex
tabbeacon doctor
tabbeacon uninstall codex
```

`tabbeacon hook codex` is an internal hook ingress command, not a daily
launcher.

## 4. Allowed implementation scope

- a Codex hook normalizer below `src/providers/`;
- an internal one-shot hook ingress command;
- provider-neutral `AgentEvidence` construction and use of the existing G01
  reconciler;
- G04 repository identity lookup for safe short titles;
- use of the existing G02 presentation policy and Windows Terminal renderer;
- ownership-aware user-global Codex hook configuration;
- supported Codex terminal-title configuration management;
- `setup`, `doctor`, and `uninstall` CLI surfaces;
- local generated integration state and backups below the per-user TabBeacon
  application-state directory;
- focused tests, this goal contract, one provider ADR, and directly relevant
  architecture/user documentation.

## 5. Explicit non-goals

- no `tabbeacon codex` daily launcher;
- no fake or shadow `codex.exe`, PATH change, shell function, PTY wrapper, or
  launch daemon;
- no Codex app-server provider backend;
- no TUI scraping;
- no provider-specific event type in `src/core`;
- no change to G01 reconciliation semantics;
- no new authoritative warning, interruption, stalled, or failure inference;
- no setup/uninstall for Claude, OpenCode, or later providers;
- no G06X or G07 implementation;
- no mutation of a project repository by normal hook handling;
- no mutation of the owner's real Codex configuration by automated tests.

## 6. Provider boundary

```text
Codex Hook JSON
      ↓
CodexHookNormalizer
      ↓
AgentEvidence
      ↓
G01 SessionReconciler
      ↓
G04 RepositoryIdentityResolver
      ↓
G02 PresentationPolicy / WindowsTerminalRenderer
      ↓
owned Windows console output
```

The backend declares lifecycle authority for `Phase` and `Attention` and no
authority for `Health`. Raw prompt text, tool payloads, assistant messages, and
provider event names do not enter core state or presentation strings.

One hook invocation is sufficient to produce a complete state transition for
the admitted events. A compact start produces no update, which preserves the
already displayed state without introducing a cross-process liveness guess.

## 7. Evidence mapping

| Codex evidence | Provider-neutral patch | Notes |
| --- | --- | --- |
| `SessionStart` source `startup`, `resume`, or `clear` | `Ready`, clear attention | lifecycle evidence |
| `SessionStart` source `compact` | no evidence / no presentation write | compaction must preserve current state |
| `UserPromptSubmit` | `Working`, clear attention | primary start-of-work evidence |
| `PreToolUse` or `PostToolUse` | reinforce `Working`, clear attention | corroborating only; not a sole liveness model |
| `PermissionRequest` | `WaitingUser + Approval` | hook returns no approval decision |
| `Stop` | `WaitingUser + ResultReady` | a hook stop point, not an authoritative app-server completion verdict |
| `SessionEnd` | `Ended`, clear attention | triggers presentation reset |
| unsupported/unrecognized event | no evidence / no presentation write | forward compatible and fail open |

The provider never derives `Health::Failed` from a shell exit, a
`PostToolUse` payload, missing events, elapsed time, or a timeout. It never
derives `Warning`, `Interrupted`, or stalled state from the current hook
surface.

## 8. Hook execution and fail-open contract

- configured lifecycle hooks are asynchronous and informational wherever the
  Codex interface permits it;
- `SessionEnd` is synchronous only because Codex requires it and uses the
  one-second bounded timeout after lifecycle end;
- the hook command writes no control JSON and makes no allow/deny/block
  decision;
- malformed or unsupported input, repository discovery failure, corrupt local
  registry state, terminal capability loss, and presentation output failure
  are contained inside the hook command;
- the hook ingress process exits successfully after such a degradation so
  TabBeacon cannot block Codex progression;
- missing TabBeacon executables remain a Codex hook-launch failure only; the
  asynchronous declarations prevent that failure from becoming a provider
  operation decision;
- hook input is size-bounded and no raw prompt/tool content is logged or
  persisted;
- no network access is used by hook normalization, repository identity, or
  presentation.

## 9. Global configuration ownership

`tabbeacon setup codex` targets the supported user-global layer and must:

- preserve every unrelated hook event/group/handler;
- preserve unrelated Codex TOML including MCP servers, profiles,
  notifications, permissions, and feature settings;
- use a format-preserving TOML edit for the one owned terminal-title key;
- record exact owned hook declarations and the prior terminal-title state in a
  machine-local ownership manifest;
- create exact pre-mutation backups before the first external-config write;
- use a process lock and atomic durable replacement for every managed file;
- be idempotent without rewriting already-correct files;
- refuse to claim a matching pre-existing declaration that it did not create;
- leave new definitions untrusted and direct the owner to Codex's supported
  `/hooks` review UI.

`tabbeacon uninstall codex` preflights all managed elements before mutation. It
removes only exact owned hook groups and restores only the terminal-title value
that setup replaced. A modified owned element causes a safe refusal; unrelated
changes made after setup remain untouched.

The integration state lives under:

```text
%LOCALAPPDATA%\TabBeacon\codex-integration
```

Tests inject isolated roots instead of changing the owner's files.

## 10. Doctor contract

`tabbeacon doctor` reports independently:

- Codex executable/version compatibility;
- user-global hooks file presence and parseability;
- every expected TabBeacon hook declaration;
- missing or modified owned declarations;
- trusted, untrusted, or modified/inactive hook state for the admitted Codex
  compatibility shape;
- supported terminal-title disablement or a title-ownership conflict;
- ownership-manifest consistency;
- TabBeacon hook executable presence.

Doctor never marks hooks active from declaration presence alone and never
writes trust state.

## 11. Dependency decision

Two new direct dependencies are allowed:

- `toml_edit`, with default parse/display features, solely for comment- and
  formatting-preserving mutation of the one supported Codex TOML setting. The
  admitted current crate is pure Rust, `0.25.13+spec-1.1.0`, requires Rust 1.85
  or newer, and is dual licensed `MIT OR Apache-2.0`.
- `atomic-write-file` `0.3.1`, with no optional features, solely to durably
  replace existing external configuration files with platform-correct atomic
  semantics. It requires Rust 1.85 or newer and is BSD-3-Clause licensed.

Both are compatible with the pinned Rust 1.97.1 toolchain. Together they avoid
unsafe hand-editing and the non-atomic remove/rename gap on Windows. No
SQLite/C or network dependency is introduced.

## 12. Acceptance tests

At minimum automated tests prove:

- raw hook payload normalization;
- startup, resume, clear, and compact distinction;
- prompt submit to working;
- permission request to approval;
- stop to result-ready without a completed verdict claim;
- session end reset;
- pre/post tool use only reinforce working;
- unsupported event, malformed JSON, and missing required fields;
- deterministic duplicate-event evidence and multi-session separation;
- repository/worktree identity and collision alias integration;
- setup idempotence and byte stability on a second setup;
- unrelated hooks and unrelated TOML preservation;
- uninstall ownership and prior title restoration;
- safe refusal for a modified owned hook or title key;
- absent hook binary and hook runtime failures remain fail open;
- current Codex JSON/TOML configuration shape;
- terminal-title ownership conflict detection;
- no daily launcher surface;
- isolated roots for all destructive configuration tests.

## 13. Validation and evidence

Required local gates:

```text
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build --locked --all-targets
pwsh -NoProfile -File ./scripts/ci/run-local-ci.ps1 -ExpectedHead <candidate>
```

A controlled real local smoke may back up and mutate the real user Codex
configuration only after isolated tests pass. It must preserve direct `codex`
launch and stop for explicit owner trust review rather than bypassing it.

G05 does not change G02 palette, VT encoding, renderer behavior, or G03 visual
infrastructure. Hosted visual CI is therefore `N/A` for this goal. Provider-to-
terminal autonomous visual E2E remains TB-G07, while the real G05 smoke must
exercise the hook/config integration and representative state rendering where
the interactive/trust preconditions permit.

Final acceptance requires one draft G05 PR and hosted code CI bound to the
exact candidate SHA. G05 is not merged during this goal unless separately
authorized.
