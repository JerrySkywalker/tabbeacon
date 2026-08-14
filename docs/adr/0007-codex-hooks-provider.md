# ADR 0007 — Codex User-Global Hook Provider

- Status: Accepted
- Date: 2026-08-14
- Goal: TB-G05-CODEX-HOOKS

## Context

TabBeacon's first provider must preserve literal daily `codex` launch, remain
fail open, and feed the existing provider-neutral evidence model without
scraping the Codex TUI.

Codex `0.147.0` supports user-global command hooks in
`~/.codex/hooks.json`, hash-based hook review/trust, and supported terminal
title configuration. It accepts Windows-specific commands through
`commandWindows`. Although current upstream source implements asynchronous
command hooks, the admitted installed release parses but skips non-end hooks
configured with `async = true`.

## Decision

Install seven user-global command declarations for `SessionStart`,
`UserPromptSubmit`, `PreToolUse`, `PermissionRequest`, `PostToolUse`, `Stop`,
and `SessionEnd`. Every declaration invokes the internal
`tabbeacon hook codex` ingress, is synchronous for `0.147.0` compatibility,
and uses the minimum one-second timeout.

The Windows command ends with `|| exit /b 0`. The internal command is silent
and always exits successfully. Together these rules prevent a missing or
failed TabBeacon binary from selecting Codex's special exit-code-2 blocking
contract. A Codex-enforced timeout is recorded as a failed hook run but does
not set the provider operation's block/stop decision.

The adapter maps only lifecycle evidence:

- startup/resume/clear to ready;
- compact to no write, preserving current state;
- prompt and tool activity to working;
- permission request to approval attention;
- stop to result-ready attention, explicitly not an authoritative completion
  verdict;
- session end to lifecycle end/reset.

Hooks declare lifecycle authority for phase and attention and no health
authority. Raw event types and payloads do not enter the core.

Setup writes only exact owned groups in the supported global `hooks.json` and
sets `[tui].terminal_title = []` through a format-preserving TOML edit.
Machine-local ownership state and exact pre-mutation backups live under the
TabBeacon local application-data root. Setup never writes hook trust; the user
reviews the definitions through Codex `/hooks`. Uninstall removes or restores
only exact owned fields and refuses drift.

## Consequences

- Daily use remains exactly `codex`; no launcher, PATH shadow, PTY, or daemon is
  introduced.
- Hook fidelity is intentionally lower than an app-server backend: hook
  failures, tool exit status, absent events, and elapsed time cannot establish
  warning, interruption, failure, or stalled state.
- The one-shot process uses the existing reconciler for each complete admitted
  transition. Compact is the sole preservation event and deliberately performs
  no write.
- Windows Terminal presentation is written through the owned console handle,
  because hook stdout is captured and interpreted by Codex.
- `doctor` reimplements the admitted `0.147.0` normalized hook hash only for
  read-only active/inactive diagnosis. A Codex version outside the admitted
  compatibility floor fails that proof rather than guessing.
- TB-G06X remains the separate experimental higher-fidelity app-server track;
  TB-G07 remains responsible for autonomous provider-to-terminal E2E.
