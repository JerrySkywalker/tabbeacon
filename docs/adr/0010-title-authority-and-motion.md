# ADR 0010 — Title Authority and Motion

## Status

ACCEPTED for v0.3 planning and sequencing. The public v0.2.0 release remains
frozen at `0b1d5136833a05bf94b7d32c414a21da2f5ac78e`; v0.3 implementation
starts from the later post-release `main` after the planning admission is
merged. This ADR does not authorize TB-G16 before accepted TB-G15 evidence.

## Context

v0.2 can correctly compute semantic presentation and can write Windows Terminal OSC title/color/progress output. It also has a production session/turn/terminal-scoped worker. Dogfood nevertheless exposed a visible failure mode where color changes correctly but the title remains or reverts to `PowerShell` / `Administrator: PowerShell` after a brief TabBeacon title.

The v0.2 ownership model primarily arbitrates Codex versus TabBeacon by configuring Codex terminal-title output. It does not yet model all title authorities between TabBeacon and the visible tab: Windows Terminal profile policy, shell/profile title writers, or other application writers.

Separately, v0.2 retains pre-G11 defaults: static `title-indicator` and a 180 ms worker frame interval. The production animator exists, but new/default users do not automatically experience it. Post-release recovery also added an owned live-tab UIA reader and terminal-close worker cleanup; v0.3 builds from those capabilities instead of replacing them.

## Decision

### 1. Visible-title health is end-to-end

A successful OSC write is not sufficient proof of title health. v0.3 models the title channel as an observable authority surface with at least:

```text
healthy
suppressed
contended
unavailable
unverified
```

The diagnostic classification must distinguish "title was never admitted" from "title appeared and was overwritten later" where the platform can prove that distinction.

### 1a. Passive diagnostics and active probing are separate

`status`, `doctor`, and their JSON forms remain read-only operational
diagnostics. Active visible-title observation is an explicit opt-in action that
uses an owned fixture and the same typed title-authority model. A not-requested
probe is `unverified`, not a failed presentation claim; unavailable and
classified probe outcomes are represented distinctly. The active probe never
rewrites persistent user configuration.

### 2. Default working presentation uses motion

The v0.3 new-install/default balanced profile uses:

```text
activity=title-spinner
spinner=braille
frame_interval=100 ms
```

The 100 ms target is normative for the built-in default and is not initially exposed as an arbitrary user-configurable duration.

### 3. Animation scheduling is deadline-based

The worker should schedule frames against monotonic deadlines rather than accumulating `sleep(interval) + work` drift. The intended sequence is conceptually:

```text
next = start + 100 ms
render
sleep_until(next)
next += 100 ms
```

Overrun must skip/catch up safely rather than busy-loop.

### 4. Only the mutable status slot animates

The title grammar remains:

```text
<status-slot> <workspace-alias>
```

The right-side workspace alias is stable throughout one activity sequence.

### 5. Static-state convergence may use a bounded settle burst

Startup/result/attention/static states may reassert the same title for a short bounded window to converge through startup races. This is not a daemon and must terminate automatically. Persistent contention must be diagnosed rather than fought indefinitely.

### 6. Remediation is ownership-safe

Windows Terminal or shell remediation is allowed only when the exact target and prior value can be proven and restored. No broad profile rewrite, PowerShell-profile editing, Developer Mode change, or security-policy change is implied by this ADR.

### 7. Codex compatibility remains explicit

Future Codex versions do not inherit the v0.147.0 profile automatically. v0.3 introduces a versioned compatibility registry and development-side source-diff tooling while keeping runtime behavior offline and fail-open.

## Rejected alternatives

### Faster blind title fighting

Writing OSC titles continuously at a very high rate without diagnosing contention is rejected. It wastes work, can flicker, and converts an ownership problem into a race.

### Global resident title service

Rejected as the default architecture. The session/turn-scoped worker remains the production baseline.

### Silent migration of existing user preferences

Rejected. Existing configured users retain their explicit choices; guided setup may offer the new balanced profile.

## Consequences

- title diagnostics require a visible-title observation seam for trusted Windows Terminal tests;
- the default v0.3 user experience becomes visibly animated;
- 1/4/8-active-tab performance becomes a release concern;
- title contention becomes a classified operational state rather than an unexplained presentation failure;
- visual CI must prove both animation cadence/variation and stable identity placement.
