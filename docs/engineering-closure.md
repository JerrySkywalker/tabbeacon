# Scoped engineering closure

Goal `TUI-PLATFORM-ENGINEERING-CLOSURE-12H-001` continues Draft PR #123.
Exact source, binary hashes, actual command exits, trace statistics and remaining
blockers are recorded in the Goal's `FINAL-RECEIPT.json` and linked reports under
the artifact identified by `TUI-PLATFORM-ENGINEERING-CLOSURE-12H-001` in the
[coordination evidence index](https://github.com/JerrySkywalker/terminal-ui-platform-coordination/pull/2).
This source work authorizes neither merge/publication nor production configuration writes.

Repair plans carry an opaque `TitleRepairBaseline` captured by the product's
read-only collector. The same baseline survives the Preview overlay and Confirm
command through the caller to `repair_if_unchanged`. It binds the canonical
document, installation, profile, admitted action, document digest and receipt
baseline. Workspace observations similarly retain exact preference bytes,
workspace identity/root, preference/registry paths and the generated assignment
snapshot. Dirty drafts keep their original baseline; refresh drift produces a
conflict. Confirm never silently adopts new bytes as an approved snapshot.

Repair/restore cooperate on a lock derived from the canonical settings path;
their document comparison and configuration/receipt writes occur while held.
Workspace alias set/reset use the existing registry lock and pass the original
preference snapshot into the existing preference lock-held compare/write.
Lower preference-store writers also use that preference lock. These guarantees
cover those cooperating product writers only. The Preview token grants no
authority: action allowlists, safe paths, exact ownership and explicit user
confirmation remain required. Different document aliases, foreign writers,
lock-file replacement and uncooperative tools remain outside this guarantee.

| Guarantee | Engineering scope |
| --- | --- |
| PREVIEW_BASELINE_BINDING | Original product baseline reaches the final writer; drift is refused |
| COOPERATING_WRITER_CONDITIONAL_COMMIT | Listed product writers hold their existing/shared lock across compare/write |
| FOREIGN_WRITER_ATOMICITY | UNPROVEN; advisory locks and atomic replace do not exclude a foreign writer |
| CROSS_FILE_CRASH_CONSISTENCY | UNPROVEN; config/receipt and registry/preferences are distinct files |

Release blockers for the latter two requirements remain. No `FULL_CAS_PASS`
or stronger publication claim follows from these scoped tests. A failed real
settings commit compensates its receipt; a failed receipt recovery is an error,
and a later ownership mismatch refuses restoration. This is not crash recovery.

## Release risk closure candidate

The release risk closure adds observed Windows volume/file-index and Unix
device/inode binding to Repair and Workspace Preview. Repair rechecks settings
after staging its receipt and refuses observed receipt drift before replacing it.
Compensation and restore cleanup verify receipt bytes and observed identity
before overwriting/deleting; foreign receipt changes are retained and refused.

Goal-owned process crash tests exercise before-receipt, after-receipt,
after-settings and restore-settings-before-receipt-removal, plus registry and
preference commit boundaries. Fresh Preview resumes verified owned cases;
ambiguous orphan and foreign receipts are retained. Unconditional atomic replace
still lacks strict foreign-writer CAS and the files still cannot commit together.
Restart recovery is partial and does not establish cross-file crash atomicity.
The source review, exact candidate hashes and new receipts are indexed under
`TUI-V090-RELEASE-RISK-CLOSURE-12H-001`; publication and Owner freeze stay pending.

The Git regression retains all six original budgets/production discovery chains.
Its marker wait polls the runtime and observes fixture child entry/liveness.
Additional held gates cover before-stage timeout and in-stage timeout/cancel.
Missing executable/marker injection is separately diagnosed; successful directory
fallback is not proof that the target Git stage ran. Historical missing-marker
root cause remains UNRESOLVED.

Software timing is disabled by default. Only an explicit `with_software_timing`
call allocates the fixed 8192-entry trace. The actual event-loop read return,
reducer return and successful corresponding backend draw return are recorded
with a monotonic process clock. No event text, editor content or provider payload
is persisted. No-op, coalesced, superseded, missing, overflow, errors and quit
remain visible; slow valid samples are not discarded. Restore, Join, Drop and
accepted quit have separate points; process exit is an owned-parent observation.
The trace is written once after exit cleanup to an explicitly owned new file.

`engineering-fixture` enables `tabbeacon-engineering-fixture`, which hosts that
same event loop with in-memory synthetic observations and denied persistence
callbacks. It requires an explicit artifact directory and ownership marker.
It does not resolve ambient stores, run Provider/Hook commands or grant trust.
This capability boundary is audited source, not a separate OS sandbox.

Measured synthetic event read-to-state/submission timings are engineering
baselines. Physical keyboard/mouse and physical input-to-screen P95 remain
UNPROVEN; no latency SLA is frozen here. The owned-parent forced-stop recovery
case is harness recovery, distinct from graceful product exit and OS-stall limits.
