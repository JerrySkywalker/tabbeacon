# ADR 0006 — Offline Repository Identity

- Status: Accepted
- Date: 2026-08-13

## Decision

Repository identity and abbreviation are computed from local Git/filesystem metadata and machine-local history. Normal operation does not query GitHub or another network service.

The implementation keeps four one-way layers:

```text
RepositoryDiscovery
        ↓
CanonicalRepositoryIdentity
        ↓
AbbreviationPolicy
        ↓
StableAliasRegistry
```

Discovery runs a closed set of local `git rev-parse`, `git config --local`, and
`git rev-list` operations. It records the worktree Git directory separately
from the common Git directory so linked worktrees converge.

Canonicalization prefers a usable local `origin`, then deterministically
ordered other remotes. Common HTTPS, SSH URL, and SCP-like SSH forms converge on
a scheme- and user-neutral host/path key. With no usable remote, sorted local
root commits provide a move-stable content fallback. Only an unborn repository,
which has no content identity, falls back to a digest of its common-dir path.

Abbreviation is deterministic and presentation-safe. It tokenizes hyphen,
underscore, whitespace, and reasonable camel-case boundaries; readable
expansion precedes a stable hash suffix. Previously allocated aliases are
preserved. A new collision expands only the newcomer.

The registry uses an operating-system file lock for multiprocess first-use
serialization. Every change is written to a new digest-named immutable JSON
generation, flushed, and atomically renamed into place. Abandoned temporary
files and a corrupt newer generation do not hide the latest valid generation;
if no valid published generation exists, resolution fails closed instead of
overwriting evidence.

## Consequences

Generated identity history lives outside dotfiles and project repositories: in
`%LOCALAPPDATA%\TabBeacon\repository-identity` on Windows and the per-user XDG
state location on other platforms. Worktrees of one canonical repository share
repository identity while task/branch presentation may differ later.

Normal resolution does not fetch, pull, perform DNS, call a provider API, or
write repository files. Repository identity remains separate from
`AgentSessionKey`, provider state, reconciliation, and presentation policy.

The implementation uses the Rust standard library plus dependencies already
present in TabBeacon (`serde`, `serde_json`, and `sha2`); G04 adds no dependency
or license surface.
