# Contributing

TabBeacon is early-stage and its public contracts are still being established.

Before changing code or governance, read:

1. `AGENTS.md`;
2. `dev_governance_files/PROJECT_CHARTER.md`;
3. `dev_governance_files/ROADMAP.md`;
4. `dev_governance_files/QUALITY_GATES.md`;
5. the relevant ADRs under `docs/adr/`.

## Pull requests

After the bootstrap commit, changes should be made on a focused branch and submitted by pull request. A PR is not mergeable merely because a similarly named CI run is green: the required evidence must bind to the exact PR head SHA.

Keep provider-specific behavior inside `src/providers/`. Do not put Codex-, Claude-, or OpenCode-specific event names into the provider-neutral core.
