# ADR 0014 — Agy pre-admission qualification boundary

## Status

Accepted for pre-admission infrastructure only. This ADR does not admit Agy as
a production provider and does not complete `TB-G64`.

## Context

The Agy CLI publicly documents title/status callbacks and Hooks, but those
payloads contain transcript locations, paths, identifiers, account fields,
tool arguments, and error content alongside potentially useful lifecycle and
workspace facts. Documentation and a local version probe cannot establish
real authenticated behavior, ownership semantics, or direct-launch fail-open
behavior.

## Decision

TabBeacon reserves the checked provider ID `agy` and implements only
disposable, content-minimal qualification primitives before G64:

- an immutable `unadmitted` capability profile with no enabled capabilities;
- version drift diagnostics that never infer support from version ordering;
- strict title/status and Hook recorders that whitelist safe facts and drop
  all content-capable fields, reject duplicate/deep JSON, and redact internal
  comparison tokens from debug output;
- candidate-only normalization that cannot produce core `AgentEvidence`;
- Root Workspace Anchor, task-count, and title/WT fixtures that record no
  raw paths, native IDs, or title content;
- an ownership plan that refuses mutation until a fresh Owner-approved G64
  transaction proves backup, exact ownership, drift refusal, and restore; and
- an explicit test-only provider-registry projection with unavailable
  capabilities and no title-badge participation.

The ordinary product registry and guided Setup remain Codex-only. No Agy
configuration, Hook, title command, wrapper, PATH interception, PTY host, or
daemon is installed. `tabbeacon agy` is an observational qualification CLI,
not an Agy launcher; normal Agy launch remains literally `agy`.

Any later runner invocation pins direct native executable identities by absolute
path and SHA-256, rejects reparse/out-of-root disposable captures, and applies
bounded input, output, and timeout handling. These are qualification safeguards,
not evidence that a particular Agy binary or profile is admitted.

## Consequences

The later G64 Owner-present spike can use the same recorders and fixtures to
collect minimized observations, but must still independently prove the actual
profile, real lifecycle authority, title/WT feasibility, Hook behavior, and
ownership-safe restore. G65 may not reuse documentation-only or fixture-only
results as production evidence.

## Rejected alternatives

### Implement an Agy adapter from public documentation

Rejected because official text and version discovery do not prove local,
authenticated, direct-launch behavior or ownership semantics.

### Store raw callback/Hook samples for future debugging

Rejected because the payloads include transcript paths, workspace paths,
identifiers, account data, tool arguments, and errors. Agy qualification must
remain content-minimal by construction.

### Add an unadmitted Agy row to ordinary Setup

Rejected because it would make an unavailable candidate look installable.
Only explicit qualification tests may project the unadmitted row.
