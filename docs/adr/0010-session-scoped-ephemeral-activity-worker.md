# ADR 0010: Session-scoped ephemeral activity worker

## Status

Accepted for the TB-G11 production implementation after the mandatory
feasibility proof passed.

## Decision

Active title animation uses a direct, ephemeral child of the admitted
`tabbeacon hook codex` process. There is no global resident daemon and no
wrapper around `codex`.

The provider adapter publishes a content-minimal atomic lease. Its identity is
the hash of the Codex session and originating Windows Terminal binding; the
lease separately records the hashed turn identity, G10 generation, monotonic
event sequence, activation revision, and executable owner. The executable
owner hashes both the canonical binary location and its bytes, so a relocated
or upgraded installation supersedes an obsolete worker.

The worker inherits the originating console, reopens `CONOUT$`, and emits only
title-frame bytes. The one-shot Hook remains responsible for the independently
configured Windows Terminal progress and color channels. A newer generation or
revision must publish first and receive the exact predecessor's bounded exit
receipt before it can present; otherwise decoration is suppressed fail-open.

The lease stores only:

- hashed session, turn, terminal, key, and executable-owner identity;
- generation, event sequence, revision, update, and expiry metadata;
- safe workspace alias, semantic `working` state, and a built-in spinner name.

It never stores prompt text, assistant content, tool input/output, credentials,
or raw Hook payloads. Workers self-expire after a bounded 24-hour lease, while
normal result, attention, session-end, generation, settings, upgrade, and
terminal-close paths stop them much earlier.

## Consequences

- Daily launch remains exactly `codex`.
- Worker spawn, crash, state, terminal, and cleanup failures affect decoration
  only and have no Hook-health authority.
- Same-workspace parallel sessions remain isolated by session and terminal
  binding rather than workspace identity.
- Title animation changes only the left status slot; workspace alias, progress,
  and semantic color remain independently typed.
- Atomic local files are the admitted production IPC baseline. A global daemon
  or a different IPC mechanism requires a new demonstrated need and decision.
