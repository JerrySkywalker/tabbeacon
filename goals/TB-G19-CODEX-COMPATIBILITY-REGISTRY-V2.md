# TB-G19 — Codex Compatibility Registry v2

## Status

PLANNED. Codex-only maintenance Goal after presentation reliability is stable
in the reconciled post-release v0.3 sequence.

## Purpose

Make Codex compatibility easier to audit and update without weakening the current exact-profile safety model.

v0.2 intentionally binds production behavior to a source-audited Codex profile. v0.3 keeps that principle and adds a maintainable registry plus development-side diff tooling.

## Compatibility model

Introduce a typed registry conceptually similar to:

```text
CodexCompatibilityRegistry
  version/profile
  hook event set
  required identity/order fields
  turn awareness
  subagent awareness
  compact awareness
  timeout semantics
  terminal-title ownership semantics
```

A newer Codex version MUST NOT inherit support merely because its version number is greater.

## Current baseline

At v0.2 planning time the admitted production profile is:

```text
codex-hooks-rust-v0.147.0
```

v0.148 prerelease builds exist upstream and require explicit audit before admission. Prerelease availability alone does not justify production support.

## Development-side audit tooling

Add a repository development tool/script that accepts an old admitted Codex source tag and a candidate source tag and produces a bounded compatibility report.

At minimum compare relevant source surfaces for:

```text
Hook event declarations
Hook payload identity/order fields
session/turn/subagent metadata
compact lifecycle
hook timeout behavior
terminal-title configuration/ownership behavior
```

The report should classify the candidate as something equivalent to:

```text
SAFE_COMPATIBLE
REQUIRES_REVIEW
BREAKING_OR_UNPROVEN
```

This is developer tooling; TabBeacon runtime remains offline and does not fetch Codex source or releases.

## Profile admission

A new Codex version/profile is admitted only after:

1. source comparison;
2. deterministic fixture updates/tests;
3. isolated real-Codex smoke where required;
4. title-ownership compatibility check;
5. explicit documentation of supported semantics.

Unknown/new events remain fail-open.

## Diagnostics integration

`status`/`doctor` should report the exact detected/admitted profile and distinguish:

```text
supported
known-but-unadmitted
unknown/unavailable
```

without online lookups.

## Non-goals

- automatic support for all future Codex versions;
- App Server production backend;
- runtime network compatibility checks;
- Claude/OpenCode provider work.

## Validation

- registry unit tests;
- audit-tool fixture tests;
- old/current profile regression;
- candidate-version classification fixture;
- one final hosted code CI;
- L4 only when a new real Codex profile is actually admitted.

## Exit gate

```text
COMPATIBILITY_REGISTRY=PASS
EXACT_PROFILE_POLICY_RETAINED=true
SOURCE_DIFF_TOOL=PASS
UNKNOWN_VERSION_FAIL_OPEN=PASS
RUNTIME_NETWORK_ACCESS=false
PROFILE_DIAGNOSTICS=PASS
```

## Exit receipt

```text
GOAL_ID=TB-G19
DISPOSITION=<PASS|FAIL|BLOCKED|UNPROVEN>
STARTING_MAIN=<sha>
EXPECTED_HEAD=<sha>
ADMITTED_PROFILES=<list>
SOURCE_DIFF_TOOL=<...>
UNKNOWN_VERSION=<...>
L4=<...|N/A>
CI=<...>
OWNER_ACTION=<none-or-specific>
```
