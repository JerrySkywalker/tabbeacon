# TB-G10A — Non-Git Workspace Identity Fallback

## Status

PLANNED. This goal is inserted between completed `TB-G10` and production `TB-G11`.

Starting authoritative main when this amendment was admitted:

```text
3c489a3528275aa624a26d4606d59bb833fa700b
```

## Why this goal exists

Dogfood exposed a product gap that was outside the original G04 contract: TabBeacon works when `cwd` resolves to a Git repository, but a Codex session started in an ordinary non-Git directory loses TabBeacon presentation.

The current runtime resolves repository identity before rendering. `RepositoryDiscovery` maps failed `git rev-parse` layout discovery to `NotRepository`, and `CodexHookRuntime` then returns `DegradedRepositoryIdentity` without writing a replacement TabBeacon title. Because TabBeacon setup may already own Codex terminal-title behavior, the visible result is worse than a simple no-op: the native Codex title may be absent while TabBeacon also emits no useful identity/status.

This is a scope gap, not a G11 regression. G04 intentionally defined Git repository identity, including originless and unborn repositories; it did not define ordinary directory identity.

## Architectural decision

Generalize the presentation-facing identity concept from **repository identity** to **workspace identity** while preserving the existing repository implementation as the Git-specialized path.

Conceptually:

```text
WorkspaceIdentityResolver
        |
        +-- Git repository
        |      -> existing RepositoryIdentityResolver
        |      -> existing canonical repository identity
        |      -> existing stable alias
        |
        +-- ordinary directory
               -> DirectoryWorkspaceIdentity
               -> local path-derived opaque identity
               -> same stable alias namespace
```

Git behavior is compatibility-sensitive and must remain unchanged.

## Workspace identity contract

### Git workspace

When `cwd` is inside a Git repository:

- preserve current remote/root-commit/unborn identity rules;
- preserve linked-worktree behavior;
- preserve move/reclone behavior;
- preserve all existing aliases and collision history;
- perform no network access.

### Ordinary directory workspace

When `cwd` is not in a Git repository:

- derive an opaque local identity from the normalized absolute directory path, versioned conceptually as `dir-v1`;
- do not persist the raw absolute path as the canonical durable key;
- derive a bounded presentation-safe display hint from the directory basename;
- treat moving the directory as a new workspace identity because no repository/content anchor exists;
- remain fully offline.

Recommended canonical shape:

```text
dir-v1:<sha256(normalized-absolute-path)>
```

The exact Rust type names may differ, but the privacy and stability semantics above are normative.

## Special local display hints

The implementation may provide explicit friendly hints for common roots where doing so is deterministic and local, for example:

```text
user home directory -> HOME
filesystem root     -> drive/root-safe name
other directory     -> sanitized basename
```

The identity key must remain distinct from the display hint.

## Alias namespace

Repository-backed and directory-backed workspaces MUST share one alias collision namespace.

Do not create independent registries that could assign the same visible alias to two simultaneously relevant identities.

Existing repository assignments must never be renamed merely because directory identities are introduced.

## Presentation boundary

Presentation should consume a `workspace_alias`-equivalent semantic input rather than assume that every Codex session owns a Git `repository_alias`.

The v0.2 title grammar remains unchanged:

```text
<status-slot> <workspace-alias>
```

Examples:

```text
○ OWH
⠋ OWH
✓ OWH

○ HOME
⠋ HOME
✓ HOME
```

Provider normalization must not learn path/display formatting rules.

## G11 dependency

Production G11 must build its worker contract on workspace identity, not repository-only identity.

Conceptually:

```text
WorkerPresentation
  workspace alias
  semantic active state
  spinner preset
```

This goal therefore precedes production G11 implementation even though G11 feasibility has already passed.

## Required behavior

- ordinary non-Git directory produces a stable local workspace identity;
- home directory produces a meaningful stable alias;
- filesystem root fails safely or produces a bounded stable alias;
- same-basename directories remain distinguishable through the shared collision registry;
- Unicode, long, and hostile directory names remain presentation-safe;
- no raw path leaks into terminal control bytes or durable canonical identity state;
- Git repository behavior remains byte/semantics compatible where not intentionally generalized at the type boundary;
- no network access is introduced;
- no repository-local marker file is introduced;
- missing/unusable workspace identity remains fail-open.

## Required regression

A real or controlled Windows Terminal/Codex scenario must cover:

```text
start Codex from a disposable non-Git directory
-> TabBeacon receives lifecycle Hook evidence
-> TabBeacon keeps a meaningful title identity
-> lifecycle status remains visible
-> direct command remains `codex`
```

## Validation profile

This goal uses the risk-based Fast Lane defined in `dev_governance_files/FAST_LANE.md`.

Minimum expected gates:

- focused unit/integration tests for directory workspace identity and Git compatibility;
- formatting/static checks sufficient for changed Rust scope;
- one final-head hosted code CI;
- L3 visual only if final title/presentation behavior changes in a way not already proven by existing typed renderer fixtures;
- one focused non-Git Codex/WT smoke because the bug is an integration gap;
- no repeated full audit pass after unchanged evidence.

## Non-goals

- production G11 worker implementation;
- G12 setup wizard;
- G13 JSON diagnostics;
- App Server, Claude, or OpenCode providers;
- global daemon;
- wrapper/PTY/PATH interception;
- network workspace discovery;
- generalized project detection beyond the current directory/Git model.

## Exit receipt

```text
GOAL_ID=TB-G10A
DISPOSITION=<PASS|FAIL|BLOCKED|UNPROVEN>
STARTING_MAIN=<sha>
EXPECTED_HEAD=<sha>
GIT_IDENTITY_COMPAT=<PASS|FAIL>
NON_GIT_DIRECTORY=<PASS|FAIL>
HOME_DIRECTORY=<PASS|FAIL>
SHARED_ALIAS_NAMESPACE=<PASS|FAIL>
RAW_PATH_PERSISTED=false
NO_NETWORK=true
NON_GIT_CODEX_SMOKE=<PASS|FAIL|BLOCKED>
CI=<PASS|FAIL|BLOCKED>
OWNER_ACTION=<none-or-specific-action>
```
