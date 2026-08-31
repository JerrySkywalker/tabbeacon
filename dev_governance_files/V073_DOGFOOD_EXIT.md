# Post-v0.7.3 dogfood exit criteria

Recommended pause: four weeks minimum, six to eight weeks preferred.

Preferred v0.8-admission threshold:

```text
DOGFOOD_WEEKS>=4
P0_COUNT=0
P1_COUNT=0
REPEATED_UNKNOWN_HOOK_FAILURE=false
HIGH_FREQUENCY_MANUAL_RECOVERY=false
TEMP_WT_RESIDUE=false
CODEX_UPGRADES_SURVIVED>=2_preferred
```

During the pause, observe Codex upgrades, multi-subagent turns, ResultReady and
SessionEnd, abnormal exits, multi-tab/worktree use, binary replacement, worker
and lease cleanliness, temporary Terminal cleanup, diagnostics, and Agy
coexistence.
