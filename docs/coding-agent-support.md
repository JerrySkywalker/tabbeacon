# Supported coding agents

TabBeacon normalizes admitted provider evidence into provider-neutral state. Capability, configuration, Hook trust and visible application are separate facts; see the [English/Chinese terminology](terminology.md).

| Coding agent | Status | Daily command | Exact boundary |
| --- | --- | --- | --- |
| Codex CLI | Production | `codex` | Capability-based command-Hook admission; version ordering is not a support gate. |
| Agy CLI | Production | `agy` | Exact 1.1.19 and 1.2.7 title-callback profiles only; 1.2.13 is not admitted. |
| Cursor Agent | Production | `agent` | Exact-owned project Hooks; strict color-only or preserve-native, Cursor-native title. |
| Claude Code | Deferred | N/A | No integration. |
| OpenCode | Deferred | N/A | No integration. |

## Codex evidence boundary

The current observed Codex 0.159.0 command-Hook profile has 11 baseline events. It does not inherit the older 0.157.1 real Interrupt proof. That real Interrupt, recovery, completion and normal exit were accepted under their original version/profile and source/binary pins. In the audited 0.157.1 original-command Hook path, Warning, Failed and the original HTTP 404 class have no authoritative structured signal. They may leave stale tab state; detection is **not PASS**, and [Issue #116](https://github.com/JerrySkywalker/tabbeacon/issues/116) stays **OPEN**. TabBeacon does not infer failure from output text, ANSI or transcripts. Missing or incompatible required capabilities fail safely; a newer version number alone does not deny support.

## Agy title boundary

Only exact 1.1.19 and 1.2.7 title callbacks are admitted. `initializing`/`working` map to Working and an actual `idle` callback maps to Ready. Result-ready, approval, failure/health, tab color, progress and animation are unavailable. In a real mixed 1.2.7 session the Working title was `Agy⭕AGYWORKS` and Agy did not interfere with Codex/Cursor colors. After the observed response the title remained `Agy⭕AGYWORKS`; a real Ready/idle visible transition is **UNPROVEN**. Deterministic tests of `idle` do not supply a missing real callback. Daily Agy 1.2.13 remains unsupported and should not receive automated setup or downgrade. See [Agy setup](agy-setup.md) and [ADR 0015](adr/0015-agy-1-1-19-production-profile.md).

The Owner stopped the additional attended Agy preserve-native-to-title-only switch. Its real L4 is **NOT EXECUTED**, while isolated configuration, ownership/restore and owned Visual tests retain their specific proofs. Setup owns only the admitted `title` member and preserves unrelated settings.

## Cursor and mixed-provider boundary

The project-owned Cursor Hook route delivers strict color-only working/completion output without changing the Cursor-native title. The Owner observed separate Codex, Cursor and Agy tabs: Codex/Cursor became green while working and blue after completion; Agy kept default color, showed its native title and did not disturb either completed tab. All three exited normally without cross-tab disturbance. An earlier two-Cursor acceptance is separately pinned. Individual Hook history and exact per-event terminal writes were not logged. Same-tab Codex→Cursor→Agy handoff was **NOT EXECUTED** at the Owner's direction; deterministic generation/ownership and Visual proofs must not be relabeled as that real L4.

## Setup and trust

Use `tabbeacon setup codex` for an existing managed Codex integration and review any new definition through normal native `/hooks`. Setup does not grant trust or copy authentication. `tabbeacon setup agy` applies only to an actual admitted 1.1.19/1.2.7 installation. Cursor setup is per-owned project, never an automatic rollout to unrelated workspaces. Continue to launch daily commands literally as `codex`, `agy` and `agent`; no TabBeacon wrapper or PATH shadow is installed.

The production terminal backend is `TitleMarkBackend`. Native Windows Terminal tab icons remain [NO_GO](design/native-tab-icon.md) under accepted host safety evidence. Provider data cannot select an unadmitted backend.