# TB-G53 — Local Workspace Preferences & Alias Control

## Status

PLANNED after accepted G52.

## Purpose

Adopt Adaptive Naming v2 into production assignment semantics while preserving all existing aliases, and add a separate device-local preference overlay for explicit user alias choices.

The user may accept the automatic result, select another generated candidate, or enter a custom alias. None of those preferences may modify repository files.

## State architecture

Keep identity allocation and user preference as distinct concepts.

Preferred model:

```text
StableAliasRegistry v2
  canonical identity -> {
    generated_alias,
    policy_version
  }

WorkspacePreferenceStore
  canonical identity -> {
    override_alias: Option<RepositoryAlias>
  }

EffectiveAlias = override_alias ?? generated_alias
```

The exact serialized schema may include migration metadata, generation numbers, integrity hashes, or other safety fields already used by TabBeacon, but the semantic separation is normative.

Preferred locations:

```text
%LOCALAPPDATA%\TabBeacon\repository-identity\
%LOCALAPPDATA%\TabBeacon\workspace-preferences\
```

Do not place preferences inside the repository/worktree.

## Registry v2 migration

Existing v0.4/v0.4.1 assignments are authoritative user-visible history.

Migration rule:

```text
existing identity + alias
  -> generated_alias = existing alias
  -> policy_version = legacy-preserved / equivalent version marker
  -> override_alias = none
```

Do not recompute and rename an existing workspace merely because Adaptive Naming v2 now prefers a different candidate.

New identities after G53 use Adaptive Naming v2 by default.

Migration must be atomic, process-safe, corruption-detecting, and restart-safe. If the old registry cannot be validated, fail safely rather than overwriting it with a new interpretation.

## User-facing alias commands

Admit a coherent top-level alias surface. Preferred grammar:

```text
tabbeacon alias
tabbeacon alias show
tabbeacon alias preview
tabbeacon alias explain
tabbeacon alias set <alias>
tabbeacon alias reset
```

`tabbeacon alias` may behave as `show` in Human mode.

### show

Human summary includes:

```text
Workspace
Automatic alias
Custom alias if any
Effective alias
Naming policy
```

Never print a raw canonical private identity/path by default.

### preview

Show the top bounded Adaptive v2 candidates for the current workspace without changing state.

### explain

Show safe tokenization/scoring rationale:

```text
Project/display hint
Tokens
Candidate strategy
Score/components
Automatic alias
Override state
Effective alias
```

The explain path is for understanding the deterministic policy, not for revealing registry internals or private paths.

### set

Persist an explicit local override only after validation and collision check.

Selecting the second/third generated suggestion is still an explicit user choice and is stored as an override; it must not rewrite the global scoring weights or make the engine learn from local history.

### reset

Remove only the explicit override and immediately return to the existing stable generated alias. It must not delete identity history or force a regeneration/migration.

## Alias validation

Preserve existing `RepositoryAlias` safety constraints unless G52 evidence justifies a compatible expansion for Unicode/CJK aliases. Any expansion must still forbid control characters, terminal escapes, unbounded content, unsafe punctuation, and title-breaking sequences.

If Unicode aliases are admitted, validation must be based on safe grapheme/display-width rules and normalization rather than ASCII-only assumptions.

## Collision policy

Effective aliases must remain unique in the local TabBeacon namespace.

If the user explicitly requests an alias already used by another workspace:

```text
REFUSE
explain the conflict generically
preserve both existing assignments
```

Do not:

- silently rename the other workspace;
- auto-swap assignments;
- silently add a hash/suffix to the user's explicit string;
- expose the other workspace's private canonical identity/path just to explain the collision.

The user chooses a different alias.

## TUI/domain integration boundary

G53 should expose typed APIs/domain state for the later Workspace screen, but does not need to build the final Live Control Center screen yet. A bounded existing-TUI integration may be added only if it reduces duplication without dragging G55 forward.

## Project-local prohibition

Normative invariant:

```text
REPOSITORY_LOCAL_CONFIG=false
PROJECT_FILE_MUTATION=false
```

TabBeacon v0.5 shall not create or require:

```text
.tabbeacon
.tabbeacon.toml
tabbeacon.toml
```

or equivalent per-project preference files.

## Device-local semantics

Workspace overrides are device-local in v0.5. Different machines may intentionally use different aliases for the same Git repository.

Cross-device backup/migration is provided by G54 export/import; no background synchronization service is introduced.

## Read-only / mutation behavior

`show`, `preview`, and `explain` are strictly read-only and should not create missing preference state merely by inspection.

`set` and `reset` are explicit user mutations. Use snapshot/conditional-write semantics so concurrent preference edits do not overwrite one another silently.

## Testing

Required families:

- old registry v1 -> v2 migration preserving every existing alias;
- new workspace assignment uses Adaptive v2;
- migration interruption/retry/idempotence;
- corrupt legacy registry fails safely;
- show/preview/explain are non-mutating;
- set/reset round-trip;
- override chosen from candidate list;
- custom override validation;
- collision refusal preserving both workspaces;
- concurrent write/drift refusal;
- Git linked worktrees share intended canonical preference semantics;
- ordinary directory identity remains local/path-derived;
- no repository file creation/modification;
- privacy: no raw canonical identity/path in normal Human output.

## Risk vector

```text
CODE_CHANGED=true
PRESENTATION_CHANGED=false
PROVIDER_CHANGED=false
USER_PERSISTENT_CONFIG_CHANGED=true
SECURITY_OR_PRIVACY_CHANGED=true
RELEASE_BOUNDARY=false
```

Use ordinary hosted exact-head CI plus one focused independent ownership/privacy review and one persistent-state migration/restore/drift family. No provider L4 or Windows title Visual unless visible title source code itself changes.

## Acceptance

```text
ALIAS_REGISTRY_V2=PASS
LEGACY_ALIAS_MIGRATION=PASS
EXISTING_ALIASES_PRESERVED=true
NEW_ASSIGNMENTS_USE_ADAPTIVE_V2=true
WORKSPACE_PREFERENCE_STORE=PASS
PREFERENCES_DEVICE_LOCAL=true
PROJECT_LOCAL_CONFIG=false
ALIAS_SHOW=PASS
ALIAS_PREVIEW=PASS
ALIAS_EXPLAIN=PASS
ALIAS_SET=PASS
ALIAS_RESET=PASS
READ_ONLY_ALIAS_COMMANDS_NON_MUTATING=true
OVERRIDE_COLLISION_REFUSED=true
CONCURRENT_DRIFT_REFUSED=true
LINKED_WORKTREE_SEMANTICS=PASS
PRIVACY_REVIEW=PASS
PERSISTENT_CONFIG_SAFETY=PASS
CODE_CI=PASS
```

## Non-goals

No import/export yet, no cloud/background sync, no repository-local config, no model/network naming, no provider changes, no process/session control.

## Estimated effort

**7–11 effective engineering hours.**

## Next

`TB-G54 — Settings Export / Import`.
