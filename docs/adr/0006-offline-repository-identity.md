# ADR 0006 — Offline Repository Identity

- Status: Accepted
- Date: 2026-08-13

## Decision

Repository identity and abbreviation are computed from local Git/filesystem metadata and machine-local history. Normal operation does not query GitHub or another network service.

Previously allocated short keys are preserved when possible; new collisions expand the new key before disturbing an existing assignment.

## Consequences

Generated identity history lives outside dotfiles and outside project repositories. Worktrees of one canonical repository share repository identity while task/branch presentation may differ later.
