# ADR 0009 — Generalize Presentation Identity from Repository to Workspace

## Status

Accepted for the Codex-first v0.2 track as a planning decision; implementation is assigned to `TB-G10A`.

## Context

TabBeacon originally modeled the stable right-hand title identity as a Git repository alias. That was appropriate for the v0.1 development scope, where G04 explicitly solved ordinary repositories, originless repositories, unborn repositories, linked worktrees, moves, reclones, collision handling, and offline persistence.

Dogfood showed a broader real usage case: Codex is also launched from ordinary directories that are not Git repositories. In the current runtime, repository discovery failure produces `DegradedRepositoryIdentity`; because title ownership may already have moved from Codex to TabBeacon, the user can lose both the native Codex title and the TabBeacon identity/status.

The issue is architectural rather than cosmetic. Production G11 animation would otherwise encode a repository-only assumption into its worker contract.

## Decision

Introduce **workspace identity** as the presentation-facing abstraction.

A workspace may be:

1. a Git-backed workspace, resolved by the existing repository identity machinery; or
2. an ordinary-directory workspace, resolved locally from the directory path.

Repository identity remains a first-class specialized mechanism. Existing repository canonical keys, alias assignments, collision history, worktree semantics, and offline guarantees are preserved.

Presentation and future animation should consume `workspace_alias` semantics instead of assuming every session has a Git repository alias.

## Ordinary-directory identity

For a non-Git directory, the implementation should derive an opaque versioned identity from a normalized absolute path, for example:

```text
dir-v1:<sha256(normalized-absolute-path)>
```

The raw absolute path is not the durable canonical identity value. A separate sanitized display hint may use the basename or a deterministic special label such as `HOME`.

Directory moves create a new identity because no repository/content anchor exists.

## Alias registry

Git-backed and directory-backed workspace identities share one visible alias namespace. Existing Git aliases are stable and are never renamed simply because directory workspaces are introduced.

## Security and privacy

- no network access;
- no GitHub/provider lookup;
- no repository-local marker files;
- no raw path in terminal control sequences beyond an explicitly sanitized display hint;
- no raw path required in durable canonical identity state;
- no prompt, tool, or model content participates in workspace identity.

## Consequences

Positive:

- Codex launched from a non-Git directory keeps meaningful TabBeacon status;
- G11 worker identity becomes correct before production animation is implemented;
- repository logic remains reusable and backward compatible;
- the product model better matches the real concept being presented: the user's current work context, not necessarily a Git repository.

Costs:

- presentation/core-adjacent naming must be generalized carefully;
- tests must prove Git compatibility and directory collision behavior;
- ordinary directories do not retain identity across moves by design.

## Rejected alternatives

### Hard-code a generic `CODEX` alias outside Git

Rejected because it collapses unrelated non-Git tabs and defeats the identity purpose of TabBeacon.

### Keep separate repository and directory alias registries

Rejected because independent namespaces can create visible collisions.

### Use network/project discovery

Rejected because offline-first operation is a product invariant.

### Defer to G14 as a small bug

Rejected because G11 production worker state should not be built on an already-known repository-only identity contract.
