# ADR 0015 — Agy 1.1.19 production profile

## Status

Accepted from the Owner-present G64 admission on 2026-08-24 and narrowed by
the real production activation smoke on 2026-08-25.

## Decision

TabBeacon admits exactly Agy 1.1.19. Daily launch remains literal `agy`. The
sole production backend is Agy's supported user-global structured title
callback. Hooks, wrappers, PATH interception, PTY hosting, and resident daemons
are not part of the profile.

The callback accepts bounded, duplicate-free JSON and retains no prompt, tool,
model, transcript, account, error, raw session, or raw path content. Production
authority is intentionally limited to:

- a present conversation/session identity, hashed before persistence;
- equal `workspace.current_dir` and `workspace.project_dir`, resolved to the
  provider-neutral Root Workspace Anchor and safe local alias;
- exact `agent_state=initializing|working` as lifecycle evidence for `Working`;
- exact `agent_state=idle` as lifecycle evidence for `Ready`; and
- plain title output through the existing `AgentEvidence`, `SessionReconciler`,
  Root Workspace Anchor, and `PresentationPolicy` layers.

Result, stop/end, approval, health, background task count, subagents, Hooks,
and direct Windows Terminal control remain unavailable or unsupported. The
callback returns a plain title only; it does not emit VT color, progress, or
animation bytes.
Their absence is never converted into semantic state. Unsupported versions
do not inherit admission from version ordering.

Production setup modifies only the official user-global
`~/.gemini/antigravity-cli/settings.json` title member, including the supported
persistent `enabled=true` activation. It records an exact
pre-write backup and content-free ownership manifest, refuses foreign title
owners and unrelated drift, applies atomically after a byte recheck, tolerates
Agy-only JSON formatting rewrites, and restores the original bytes on owned
uninstall. If the TabBeacon executable is absent or a callback fails, the Agy
command remains native and fail-open; TabBeacon installs no launcher.

Agy 1.1.19's Windows command runner rejects a leading quoted executable even
when the path itself contains no spaces. Setup therefore writes an unquoted,
shell-safe executable path and refuses paths that would require quoting or
shell interpretation. The v1 quoted declaration migrates only after exact
owned-state verification.

## Evidence boundary

G64 proved an authenticated literal Agy environment, stable resume identity,
stable root workspace evidence, and byte-exact Owner configuration restoration.
The G65 production activation smoke then proved the literal callback command,
persistent activation, repeated content-minimal `idle`/`working` state delivery,
and direct `agy` usability in the separately trusted workspace.

## Consequences

Integrations, doctor/status, Sessions, and the provider registry distinguish
supported configured, supported not configured, known unadmitted, unsupported
version, and owned configuration drift. Sessions stores only a bounded hashed
provider observation and never launches a worker merely to make Agy visible.
Agy root anchors live in a provider-specific namespace, retain only hashes and
the safe shared alias, and record later workspace mismatch without rebinding.
