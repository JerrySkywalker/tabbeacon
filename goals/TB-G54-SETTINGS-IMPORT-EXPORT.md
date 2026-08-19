# TB-G54 — Settings Export / Import

## Status

PLANNED after accepted G53.

## Purpose

Add top-level, versioned backup/migration commands for user-configurable TabBeacon state without exporting machine security state, runtime state, or private workspace paths.

Preferred commands:

```text
tabbeacon export
tabbeacon import <path>
```

The exact optional flags may evolve, but `export` and `import` are top-level product commands rather than a narrow `preferences export/import` namespace.

## Export scope

Export only user-owned configuration intended to survive reinstall or move to another machine where semantics are portable.

Include, when present:

```text
Presentation settings
Interface preferences
Workspace alias preferences
schema/version metadata
safe policy/version metadata needed to interpret those preferences
```

Do not export:

```text
Hook trust state
trusted Hook hashes
raw Hook/provider runtime payloads
sessions / worker leases
raw native session IDs
raw turn/agent IDs
credentials, tokens, cookies
Windows Terminal profile/settings state
PowerShell profile state
absolute private workspace paths
arbitrary diagnostics/logs
process IDs/window handles/runtime binding
```

The export document is a user configuration artifact, not a system image.

## Portable identity model

Git workspace preferences need a portable matching key without exposing a raw canonical identity. Prefer a stable digest derived from the canonical identity, conceptually:

```text
workspace_key = sha256(canonical_identity)
```

The export may include a safe display hint if needed for user review, but matching authority belongs to the digest, not the mutable visible name.

Ordinary directory workspace identity is derived from normalized local absolute path and is therefore device-local. v0.5 must not pretend it is cross-device portable.

Default export policy for ordinary-directory overrides should be one of:

- omit from portable export and report count/omission; or
- include explicitly marked `device_local` entries that import refuses to auto-bind on another device.

Choose one policy and freeze it with fixtures before implementation completion.

## Export format

Use one versioned canonical document with an explicit schema identifier such as:

```text
tabbeacon-export-v1
```

TOML or JSON is an implementation decision to be settled early in G54; the format must be deterministic, bounded, human-inspectable enough for backup, and safe to parse. Do not support arbitrary executable content.

Suggested UX:

```text
tabbeacon export
  -> Human summary + canonical export to stdout when explicitly requested, or a predictable file path

tabbeacon export --output <path>
  -> atomic create/replace according to explicit semantics
```

Do not overwrite an existing export file silently unless the user explicitly requests replacement.

## Import model

Import is validation-first and preview-first.

Pipeline:

```text
read bounded document
  ↓
validate schema/version/size/types
  ↓
resolve portable workspace keys where safe
  ↓
build typed ImportPlan
  ↓
render Human diff
  ↓
Apply or Cancel
  ↓
atomic/compensated writes
```

Interactive TTY import shows a staged summary and requires explicit Apply.

Non-interactive import must not prompt indefinitely. Mutation requires an explicit flag such as `--apply`; otherwise it is dry-run/preview only.

A future format version newer than the implementation must fail safely or ignore only fields explicitly declared forward-compatible; never guess security-sensitive semantics.

## ImportPlan

Create one typed plan before any mutation. Conceptually include:

```text
presentation_changes
interface_changes
workspace_preference_changes
portable_matches
unmatched_entries
conflicts
preserved_state
```

The plan must make conflicts visible before Apply.

## Transaction / rollback behavior

Multiple local stores may be affected. Import must not leave silent partial configuration.

Before mutation:

- capture typed snapshots/receipts for every store to be changed;
- validate all writable destinations;
- refuse stale/concurrent drift.

During Apply:

- write only user-local TabBeacon stores;
- use atomic replace/snapshot semantics;
- if a later store fails, restore prior stores where safe;
- if complete rollback cannot be verified, return a hard failure with truthful partial-state reporting rather than declaring success.

Import never applies Hook trust or Windows Terminal external remediation.

## Conflict policy

Workspace alias conflicts are explicit. If an imported override collides with an existing effective alias, show the conflict and require resolution; do not silently suffix or rename another workspace.

Unknown portable workspace digests may be retained as pending preferences only if the storage model can do so safely without leaking raw identities; otherwise report them as unmatched and leave state unchanged.

## Round-trip contract

Required property:

```text
configured state
  -> export
  -> fresh isolated TabBeacon state
  -> import/apply
  -> export
```

The user-configurable semantics should be equivalent after canonical normalization. Machine/runtime/security state is intentionally not equivalent because it is excluded.

## CLI / localization

Human export/import summaries use G50/G51 localization and semantic color. Canonical export document keys are locale-independent.

`--json` may report an operation receipt if useful, but do not create two incompatible export file formats solely to localize text.

## Testing

Required families:

- empty/default export;
- configured Presentation + Interface export;
- workspace override export;
- excluded sensitive/runtime fields assertion;
- deterministic schema/serialization;
- bounded malformed/oversize/unknown-version import rejection;
- dry-run is non-mutating;
- Apply/Cancel semantics;
- multi-store atomic/compensated failure;
- concurrent-drift refusal;
- Git portable identity digest match;
- ordinary-directory portability policy;
- alias conflict refusal;
- en-US/zh-CN Human summary with identical machine document;
- full export/import/export round-trip.

## Risk vector

```text
CODE_CHANGED=true
PRESENTATION_CHANGED=false
PROVIDER_CHANGED=false
USER_PERSISTENT_CONFIG_CHANGED=true
SECURITY_OR_PRIVACY_CHANGED=true
RELEASE_BOUNDARY=false
```

Use ordinary hosted exact-head CI, focused persistent-state transaction proof, and one independent privacy/ownership review. No provider L4 and no title Visual matrix.

## Acceptance

```text
EXPORT=PASS
IMPORT=PASS
EXPORT_SCHEMA_VERSIONED=true
EXPORT_LOCALE_INDEPENDENT=true
IMPORT_PREVIEW_FIRST=true
NON_TTY_IMPORT_NO_HANG=true
NON_TTY_MUTATION_REQUIRES_EXPLICIT_APPLY=true
IMPORT_CANCEL_LOSSLESS=true
IMPORT_TRANSACTION_SAFETY=PASS
IMPORT_CONCURRENT_DRIFT_REFUSED=true
EXPORT_IMPORT_ROUND_TRIP=PASS
PORTABLE_GIT_IDENTITY_DIGEST=PASS
DIRECTORY_PORTABILITY_TRUTHFUL=true
ALIAS_IMPORT_CONFLICT_REFUSED=true
HOOK_TRUST_EXPORTED=false
HOOK_TRUST_IMPORTED=false
RUNTIME_SESSIONS_EXPORTED=false
CREDENTIALS_EXPORTED=false
PRIVATE_ABSOLUTE_PATHS_EXPORTED=false
PRIVACY_REVIEW=PASS
PERSISTENT_CONFIG_SAFETY=PASS
CODE_CI=PASS
```

## Non-goals

No cloud sync, repository-local config, session transfer/resume, Hook-trust migration, Windows Terminal profile migration, provider expansion, or self-update.

## Estimated effort

**7–10 effective engineering hours.**

## Next

`TB-G55 — Live Control Center`.
