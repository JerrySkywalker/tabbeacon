# TB-G45X — Sessions & Live Overview

## Status

OPTIONAL / EXPERIMENTAL / NON-BLOCKING for v0.4. May begin after accepted G41 and land before v0.4 only if scope remains bounded.

## Purpose

Expose read-only live TabBeacon runtime observability for users running many concurrent Codex tabs, without turning TabBeacon into a session manager.

## Candidate interfaces

```text
tabbeacon sessions
```

and/or a Control Center Sessions screen.

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
