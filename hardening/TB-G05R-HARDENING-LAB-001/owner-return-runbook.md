# Owner return runbook (draft)

This runbook is finalized only after the independent lab evidence is complete.
It never instructs an automated trust bypass.

1. Verify pull request #6 still points to the frozen candidate.
2. Open a new interactive `codex` session.
3. Run `/hooks`.
4. Review exactly seven TabBeacon hooks, their source, and their commands.
5. Trust them only after the review is satisfactory.
6. Restart Codex if the reviewed upstream version requires it.
7. Run `tabbeacon doctor`.
8. Perform the controlled real smoke and capture exact-head evidence.
9. If all remaining G05 gates pass, mark pull request #6 ready and merge it.
10. Fetch and prove local, origin, remote, and merged `main` are identical.

Rollback command:

```text
tabbeacon uninstall codex
```

Exact backup-restoration instructions are added after the lifecycle drills
prove the generated backup layout and refusal behavior.
