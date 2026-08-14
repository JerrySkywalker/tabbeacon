# Owner return runbook

The lab found one P1 doctor defect. Do not close G05 from the frozen candidate
until the owner reviews and admits the minimal disabled-hook diagnostic patch.
The seven hook declarations themselves did not change.

1. Verify PR #6 still points to frozen head
   `11f0876c62b29208ba0b0243678ff4f65ae6cfc4` and remains unmerged.
2. Review the G05R hardening head. Transfer only the approved P1 fix and its
   regression test to `tb-g05-codex-hooks`, then produce new exact-head local
   and hosted CI evidence. Do not retarget PR #6 to the lab branch.
3. With the new candidate installed at the ordinary TabBeacon binary path,
   open a **new** interactive `codex` session using the literal daily command
   `codex`.
4. Run `/hooks` and identify exactly seven TabBeacon user hooks:
   `SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PermissionRequest`,
   `PostToolUse`, `Stop`, and `SessionEnd`.
5. Review their source and commands. Each Windows command must invoke the
   expected absolute `tabbeacon.exe` with `hook codex`, one-second timeout,
   synchronous execution, and fail-open exit neutralization.
6. Trust those seven hooks only after the review is satisfactory. Do not use
   the one-off trust-bypass flag. Exit and restart Codex if prompted.
7. Run `tabbeacon doctor`; require every check and `DOCTOR=PASS`, including
   `hooks.trust` reporting all seven trusted **and active**.
8. Perform the controlled real smoke for ready, working, approval,
   result-ready, and session-end/reset; capture the new candidate SHA and
   evidence without exposing prompts or configuration contents.
9. If all G05 gates pass, mark PR #6 ready and merge intentionally.
10. Fetch and prove local `main`, `origin/main`, remote `main`, and merged main
    are identical and the primary worktree is clean.

## Rollback and exact backup recovery

Use the ownership-aware rollback first:

```text
tabbeacon uninstall codex
```

It removes only exact TabBeacon declarations and restores the prior terminal
title. If it refuses because owned content or the manifest changed, stop Codex
and do not guess. Preserve copies of `%CODEX_HOME%\hooks.json`,
`%CODEX_HOME%\config.toml`, and
`%LOCALAPPDATA%\TabBeacon\codex-integration` before manual recovery.

For an intact manifest, `hooks_backup.path` and `config_backup.path` name the
exact pre-setup bytes and the adjacent `digest` fields are their SHA-256 values.
Verify each backup with `Get-FileHash -Algorithm SHA256`; only then copy an
`existed=true` backup to the exact corresponding `hooks_path` or `config_path`
recorded in that same manifest. Never restore a backup from another manifest or
delete a target when the manifest is missing/corrupt. Retain the failed state
for diagnosis.
