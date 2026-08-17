# Release Criteria

A public v0.3 candidate is releasable only when all applicable conditions below are proved at one exact candidate SHA.

## Product

- daily launch remains `codex` after setup;
- fail-open behavior is demonstrated;
- repository identity is offline-first and stable across tested moves/worktrees/collisions;
- title, progress, and tab color behave according to the visual state contract;
- setup is idempotent and uninstall is ownership-safe;
- unrelated user Codex configuration/hooks survive setup and uninstall.

## Architecture

- provider-specific event types remain below the normalizer boundary;
- presentation code consumes typed visual state, not raw provider strings;
- Codex hooks remain the production v0.3 backend unless a later ADR explicitly promotes another backend;
- experimental app-server work is not a hidden runtime dependency of the production path.

## Verification

- one final locked exact-head code CI PASS;
- focused current release smoke PASS;
- presentation, provider, performance, and configuration-safety evidence is
  fresh only when its relevant risk surface changed, otherwise explicitly
  REUSED with an empty bounded risk diff;
- exact-head equality holds for every fresh evidence lane.

## Packaging

- release artifacts are reproducible from the tagged source within the documented toolchain;
- license and notices are present;
- release notes describe limitations and the Windows Terminal/Codex scope;
- crates.io publication is intentional, not an accidental consequence of CI.
