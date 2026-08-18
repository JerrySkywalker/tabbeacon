# TB-G45X — Sessions & Live Overview

## Status

PROMOTED / POST-v0.4 IMPLEMENTATION CANDIDATE. The released v0.4.0 source and
tag remain unchanged; this goal uses only the bounded read-only CLI interface.

## Purpose

Expose read-only live TabBeacon runtime observability for users running many concurrent Codex tabs, without turning TabBeacon into a session manager.

## Candidate interfaces

```text
tabbeacon sessions
```

The Control Center screen remains deferred; the CLI proves the complete bounded
observability contract without adding a second presentation surface.

## Allowed data

```text
workspace alias
semantic state
age/recency
worker health
```

## Forbidden data

```text
raw native session ID
raw turn ID
prompt text
assistant response
tool input/output
credentials
canonical private workspace identity
```

## Non-goals

- killing processes;
- switching/focusing terminal tabs as a control plane;
- remote control;
- session resume/orchestration;
- persistent activity history/logging.

## Validation

- read-only invariant;
- privacy contract tests;
- concurrent sessions remain isolated;
- stale/invalid leases are represented truthfully;
- no release blocker if deferred.

## Exit gate if promoted

```text
SESSIONS_VIEW=PASS
READ_ONLY=true
RAW_NATIVE_SESSION_IDS=false
PROMPT_CONTENT=false
REMOTE_CONTROL=false
```

Estimated optional effort: **4–7 h**.

## Candidate outcome

`tabbeacon sessions` supports human, JSON, and plain output from the existing
ephemeral worker lease store. It preserves one row per inspectable concurrent
lease, reports current leases as `recently_authorized` rather than claiming
process liveness, represents expired leases as `stale_lease`, and counts invalid
leases without echoing their contents. The inspection creates no root, lock,
history, or mutation path.
