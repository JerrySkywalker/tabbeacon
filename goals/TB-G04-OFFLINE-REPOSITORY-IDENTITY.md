# TB-G04 — Offline Repository Identity

## Goal

Implement deterministic, offline repository discovery, canonical identity, and
stable human-short aliases without coupling repository metadata to agent
session identity, provider state, presentation policy, or project-local files.

## Starting point

- Repository: `JerrySkywalker/tabbeacon`
- `STARTING_MAIN=02e6c56845ed2ecd3872db0487ab5d412a2ebcd7`
- Previous governed goal: `TB-G03` (`PASS`)
- Feature branch: `tb-g04-offline-repository-identity`

## Authorized scope

- `src/repo/**` for repository discovery, canonical identity, abbreviation,
  stable alias history, and local persistence;
- focused repository-identity tests under `tests/`;
- `src/lib.rs` only if the existing `repo` export must be documented;
- `Cargo.toml` and `Cargo.lock` only for a narrowly justified, license-reviewed
  pure-Rust dependency that the standard library cannot safely replace;
- `docs/architecture.md` and ADR 0006 only for the implemented G04 contract;
- this goal contract.

No core reconciliation, provider, hook, presentation, VT, visual-CI, runner,
setup/uninstall, terminal, daemon, or unrelated cleanup change is authorized.

## Architecture contract

The implementation preserves these separate, one-way concerns:

```text
RepositoryDiscovery
        ↓
CanonicalRepositoryIdentity
        ↓
AbbreviationPolicy
        ↓
StableAliasRegistry
```

1. Repository discovery consumes a caller-supplied cwd and local Git/filesystem
   metadata. It supports ordinary repositories and linked worktrees and
   identifies the shared Git common directory when present.
2. Canonical identity normalizes local evidence into an opaque repository key.
   It is not `AgentSessionKey`, provider evidence, presentation state, or a raw
   display name.
3. Abbreviation policy is a pure deterministic function over safe repository
   names plus an expansion attempt. It does not read or write registry state.
4. The registry alone owns stable assignment, collision handling, concurrency,
   recovery, and durable local history.
5. Generated state lives below an appropriate TabBeacon per-user application
   data root supplied explicitly or derived by a platform adapter. It never
   writes `.terminal-name`, `.tabbeacon-local`, or any other file into a product
   repository or dotfile repository.

## Offline and discovery contract

Normal identity resolution must not call GitHub or another provider API,
perform DNS or network requests, fetch, pull, or modify repository files.
Tests require no real network service.

Discovery and canonicalization must:

- discover the repository from any cwd within its worktree;
- support ordinary `.git` directories and linked-worktree `.git` files;
- make worktrees sharing one Git common directory share repository identity;
- prefer `origin` when it has a usable URL;
- deterministically choose a suitable local remote when `origin` is absent;
- normalize common HTTPS, `ssh://`, and SCP-like SSH remote forms so equivalent
  host/path identities converge, including case/default-port and trailing
  slash or `.git` handling where semantically appropriate;
- use a deterministic local-only repository fingerprint when no usable remote
  exists, with explicit behavior for moved repositories, reclones, unborn
  repositories, and hostile or unavailable metadata;
- derive a safe human display name without trusting it as the canonical key;
- return typed errors or fallbacks rather than silently performing network I/O.

## Abbreviation and collision contract

Tokenization handles hyphens, underscores, whitespace, and reasonable
camel-case boundaries, including Unicode and hostile or very long names.
Representative base results include:

```text
jerry-dotfiles          -> JD
workstation-manager     -> WM
opencode-workspace-hub  -> OWH
jerry-proxy-control     -> JPC
```

Aliases are deterministic, bounded, and presentation-safe. On collision:

1. an existing assignment remains stable;
2. only the newcomer expands through deterministic readable candidates;
3. if readable expansion is exhausted, the newcomer uses a stable short hash;
4. a later collision never renames a previously assigned repository;
5. stability is more important than symmetric alias lengths.

## Persistence and concurrency contract

Registry state is local, generated, process-safe, and recoverable.

- A stable lock resource serializes first-use assignments across processes.
- The state read, collision decision, and commit occur under one exclusive lock.
- Durable commits use same-directory temporary data, flush/sync, and an atomic
  publication strategy that never exposes a partial registry as current.
- Recovery ignores abandoned temporary data, rejects corrupt or invalid state,
  and uses the newest fully valid durable generation or an explicit typed
  recovery result without losing stable prior assignments.
- Concurrent first registration of the same repository converges on one alias;
  concurrent colliding repositories receive unique stable aliases.
- Registry paths and serialized content safely handle Unicode and hostile names.

Avoid SQLite, C dependencies, background services, and repository-local state.
Prefer Rust standard-library file locking and versioned atomic snapshots. Any
new dependency requires a documented necessity, compatible license, locked
version, and proof that it preserves the distribution goals.

## Required behavior and acceptance criteria

1. The four concerns compile as independently testable typed boundaries.
2. HTTPS, `ssh://`, and SCP-like SSH spellings of one remote converge without
   network access; distinct hosts/paths remain distinct.
3. Ordinary repositories, linked worktrees, multiple worktrees of one
   repository, originless repositories, multiple remotes, moves, and reclones
   have deterministic documented identity behavior.
4. Abbreviation examples, tokenization, readable expansion, stable-old-alias
   collision behavior, and hash fallback pass deterministic tests.
5. Per-user state is outside project repositories and supports injected temp
   roots in tests.
6. Corrupt, partial, and abandoned state recovers without accepting fabricated
   assignments or overwriting a stable valid generation.
7. Multi-process concurrent first-use stress cannot corrupt state, duplicate an
   alias across distinct identities, or rename an existing identity.
8. Unicode, reserved/control characters, very long names and paths, missing
   metadata, and unborn repositories fail safely or produce bounded safe values.
9. A no-network test harness proves normal resolution invokes only admitted
   local Git operations and never provider/network commands.
10. L0/L1/L2 gates pass locally and hosted exact-head CI proves
    `EXPECTED_HEAD == CODE_HEAD`.

## Required test matrix

- SSH/HTTPS remote normalization;
- origin absent and multiple remotes;
- ordinary repository and linked worktree discovery;
- same identity through multiple worktrees;
- repository move and reclone with the same canonical remote;
- Unicode, hostile, and very long repository names;
- abbreviation collisions and stable old alias after newcomer collision;
- deterministic short-hash fallback;
- corrupt/partial local state and atomic publication recovery;
- concurrent same-repository and colliding first registration;
- no-network invariant.

Tests may create isolated temporary Git repositories and linked worktrees, but
must not require GitHub, DNS, fetch, pull, or modification of this checkout.

## Required validation and evidence

Before candidate creation run:

```text
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build --locked --all-targets
pwsh -NoProfile -File .\scripts\ci\run-local-ci.ps1
```

Create checkpoint commits during substantial implementation. For the final
candidate, rerun the local wrapper with `-ExpectedHead <candidate SHA>`, push
this one feature branch, and open one draft PR. Accept hosted code evidence only
when its checkout SHA equals the candidate. G04 has no visual change, so visual
CI is `N/A` unless the implementation actually crosses the prohibited
presentation boundary.

Completion evidence must state:

```text
GOAL_ID=TB-G04
STARTING_MAIN=02e6c56845ed2ecd3872db0487ab5d412a2ebcd7
EXPECTED_HEAD=<candidate-or-N/A>
CODE_HEAD=<candidate-or-N/A>
VISUAL_HEAD=N/A
LOCAL_VALIDATION=<PASS|FAIL|BLOCKED|UNPROVEN>
CI=<PASS|FAIL|BLOCKED|UNPROVEN>
VISUAL_CI=N/A
NO_NETWORK_INVARIANT=<PASS|FAIL|UNPROVEN>
CONCURRENT_REGISTRY=<PASS|FAIL|UNPROVEN>
UNRELATED_DRIFT_TOUCHED=false
OWNER_ACTION=<none-or-specific-action>
```

## Explicit non-goals

- `AgentSessionKey`, G01 reconciliation, provider evidence, or provider metadata;
- Codex hooks/App Server, Claude, OpenCode, or any provider integration;
- presentation titles, colors, progress, VT bytes, Windows Terminal behavior,
  visual fixtures, screenshots, UIA, or visual-runner changes;
- setup, uninstall, project/dotfile mutation, global configuration, daemon,
  telemetry, terminal/session management, or network discovery;
- GitHub API, DNS, fetch, pull, hosted identity lookup, or secret access;
- TB-G05 or any later roadmap implementation.

## Completion rule

TB-G04 completes only when one candidate has full local L0/L1/L2 PASS, the
required offline/concurrency/recovery matrix, and successful hosted exact-head
code CI. Do not start TB-G05 during this run.
