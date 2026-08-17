# TB-G19 — Codex Compatibility Registry v2

## Status

PLANNED after accepted TB-G18. Fast Lane v2 deliberately keeps this v0.3 Goal small.
A new Codex release/profile is **not** required for v0.3 completion.

## Purpose

Make the existing exact-profile safety model maintainable without turning compatibility
maintenance into a second source-analysis product.

## Current baseline

The existing code already has a single-profile compatibility prototype:

```text
CodexHookProfile::RUST_V0_147_0
codex-hooks-rust-v0.147.0
exact for_version() admission
unknown event fail-open
```

G19 should generalize this into a typed registry rather than replace working provider
logic.

## Minimal compatibility registry

Introduce a bounded `CodexCompatibilityRegistry` (name may differ) containing explicit
entries with the already-proven compatibility facts, conceptually:

```text
version/profile
hook event set
required identity/order fields
turn awareness
subagent awareness
compact awareness
timeout semantics
terminal-title ownership semantics
```

For v0.3 it is sufficient for the registry to contain exactly the already admitted
`0.147.0` production profile.

A newer version MUST NOT inherit support because its number is greater.

## Diagnostics states

`status`/`doctor` should distinguish the equivalent of:

```text
supported
known_unadmitted
unknown_or_unavailable
```

Runtime remains offline. No release lookup occurs during normal TabBeacon execution.

## Lightweight development-side source diff

Add a repository development script/tool that accepts an admitted source tag and a
candidate source tag/source checkout and emits one bounded report over only the relevant
Codex surfaces:

```text
Hook event declarations
payload identity/order fields
session/turn/subagent metadata
compact lifecycle
Hook timeout behavior
terminal-title configuration/ownership behavior
```

A simple bounded `git diff`/source extraction report is preferred over a complex semantic
diff engine.

Classification may be:

```text
SAFE_COMPATIBLE
REQUIRES_REVIEW
BREAKING_OR_UNPROVEN
```

The tool informs a human/agent admission decision; it does not automatically admit a
profile.

## New profile admission

New profile admission is optional in G19.

If no new profile is admitted:

```text
REAL_CODEX_L4=N/A_NO_NEW_PROFILE
```

If a new profile is deliberately admitted, then source review, focused fixtures,
title-ownership compatibility, and one isolated real-Codex L4 become required for that
new profile.

Do not let prerelease availability silently expand v0.3 scope.

## Validation

Fast Lane v2:

- focused registry unit tests;
- focused source-diff fixture tests;
- current admitted-profile regression;
- known-unadmitted/unknown diagnostics regression;
- one final hosted code CI;
- Visual=N/A unless visible diagnostics/presentation semantics materially change;
- L4=N/A unless a new real profile is admitted.

No generic auditor is required unless provider wire/admission semantics are changed in a
way not already covered by the exact-profile tests.

## Exit gate

```text
COMPATIBILITY_REGISTRY=PASS
ADMITTED_PROFILES=codex-hooks-rust-v0.147.0
EXACT_PROFILE_POLICY_RETAINED=true
SOURCE_DIFF_TOOL=PASS
KNOWN_UNADMITTED_STATE=PASS
UNKNOWN_VERSION_FAIL_OPEN=PASS
RUNTIME_NETWORK_ACCESS=false
PROFILE_DIAGNOSTICS=PASS
CODE_CI=PASS
L4=N/A_NO_NEW_PROFILE
```

## Exit receipt

```text
GOAL_ID=TB-G19
DISPOSITION=<PASS|FAIL|BLOCKED|UNPROVEN>
STARTING_MAIN=<sha>
EXPECTED_HEAD=<sha>
ADMITTED_PROFILES=<list>
SOURCE_DIFF_TOOL=<...>
KNOWN_UNADMITTED=<...>
UNKNOWN_VERSION=<...>
CODE_CI=<...>
L4=<PASS|N/A_NO_NEW_PROFILE>
OWNER_ACTION=<none-or-specific>
```

Estimated effective engineering effort: **2–4 h** without a new profile admission.
