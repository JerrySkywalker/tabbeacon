# TabBeacon Writer Leases

## Purpose and ownership

A writer lease is the durable, content-minimal record that admits the one active
Implementer allowed to change a TabBeacon Goal. It binds the Goal, repository,
source-main commit, worktree, branch, writer role, and active phase. It is a
coordination boundary, not a source of authority: scope and Owner authorization
still govern every repository, configuration, trust, network, and release action.

Only the admitted Implementer may acquire or settle its lease. An active lease
blocks another acquire at the same lease path. `Status` is read-only and returns
machine-readable facts; it never authorizes a writer.

The current `tabbeacon-writer-lease.v1` acquire output has no holder or expiry
field. Its `writer_role=implementer`, `worktree`, `branch`, `start_remote_main`,
and `state` fields provide the available identity. Do not invent unsupported
fields. If an already-admitted v1 record contains a non-empty optional `holder`,
normal settlement requires an exact `-ExpectedHolder` confirmation; orphan
reclaim always refuses it.

## Canonical lifecycle primitive

The only lease lifecycle primitive is
`tools/governance/Invoke-TabBeaconWriterLease.ps1`. It supports `Status`,
`Acquire`, `Settle`, `ReclaimOrphan`, and `RecoverPrepared`, and emits one
machine-readable JSON result on success. `RecoverPrepared` is only for the
durable `TRANSACTION=PREPARED` state left by an interrupted tool operation; it
is not an alternate way to settle an ordinary active lease.

The tool creates a lease with atomic create, serializes acquire, settle, reclaim,
and recovery through the same lease-specific OS mutex, rechecks the exact digest immediately before archive,
rejects reparse-point paths, and refuses to replace an existing lease, archive,
or receipt. Archive moves read and rename the exact opened source file and use
an already-open, identity-checked archive-directory handle; there is no
path-based move fallback.

`Acquire` uses the fixed safe registry `V:\build\tabbeacon`; a caller-supplied
registry root is accepted only when it resolves to that exact value. A canonical
lease path is exactly `V:\build\tabbeacon\<task-root>\writer-lease.json`.
Under two global scope mutexes, acquire scans its direct task roots and refuses
any active v1 lease for the same repository and either the same worktree or the
same branch. This prevents a second registry or task-root pathname from bypassing
the one-writer boundary. It also refuses a `TRANSACTION=PREPARED` marker from
another task root when that marker records the same repository/worktree or
repository/branch scope, so an interrupted settlement cannot be bypassed by
choosing a new lease pathname.

## Normal acquire and settle

Create an exact task root before acquire. Supply an active phase, full source
SHA, exact worktree and branch, repository identity, and Goal. `Acquire` checks
that the worktree is Git, on the named branch, and descends from the source SHA.
It refuses an existing active lease path.

At normal Goal completion, run `Status` and pass its exact digest plus schema,
Goal, phase, repository, source head, worktree, and branch to `Settle`. Supply
an existing safe archive root and fresh explicit archive and receipt paths beneath
it. `Settle` moves the unchanged lease on the same volume, verifies the archived
SHA-256, writes a content-minimal receipt with final phase/disposition, and leaves
no active lease at the original path. An optional non-empty record holder is not
an orphan signal: normal settlement must name it with `-ExpectedHolder` exactly.

## Orphans and deterministic reclaim

An orphan is not simply an old-looking lease. It qualifies only when the exact
SHA-256, schema, Goal, phase, repository, source main, worktree, and branch are
known; the schema supports no holder or the holder is explicitly empty; relevant
bounded process/worktree checks prove `ACTIVE_WRITER_COUNT=0`; and no active
holder evidence exists in the schema or task record.

That zero-writer result must be a separate hash-bound proof record, not an
unbound command-line assertion. The record uses
`PROOF_SCHEMA=tabbeacon-writer-active-proof.v1`, binds the exact lease path and
SHA-256, and records `ACTIVE_WRITER_COUNT=0`,
`ACTIVE_LEASE_HOLDER_PROVEN=false`, repository, worktree, branch, bounded
observation scope, observer identity, and UTC observation/expiry timestamps.
The proof is valid for at most five minutes. `ReclaimOrphan` verifies the proof
path and its expected SHA-256, records both in the settlement receipt, and
rejects a malformed, stale, non-zero, mismatched, or unscoped proof.

`ReclaimOrphan` requires every expected identity field, an active expected
phase, `-ExpectedHolderless`, the fresh proof record, and exact non-existing
archive and receipt destinations inside a safe explicit archive root. It refuses
wrong digests, schema, Goal, or phase; a non-empty holder; concurrent drift;
reparse targets; and archive collisions. Successful reclaim archives the
original bytes and writes a receipt; it never edits the lease to mark it closed.

Leases, transaction markers, and prepared receipts are first flushed to adjacent
unique staging names and then atomically published, so interruption cannot leave
partial canonical records that block recovery. Settlement first creates a durable task-root
`writer-lease.transaction.v1.txt` marker with `TRANSACTION=PREPARED`; acquire
refuses that marker even when the original lease pathname has disappeared, at
the same task root or a conflicting recorded scope.
The operation then creates a `TRANSACTION=PREPARED` receipt that
binds the operation, original digest, archive path, final disposition, final
phase, writer count, and any writer proof. It atomically replaces that receipt
with the final receipt only after the exact opened source file has been renamed,
then atomically converts the task-root marker to `TRANSACTION=FINALIZED`.
`RecoverPrepared` revalidates all expected lease identities and, for orphan
reclaims, requires a fresh replacement writer proof if the original proof has
expired. The original proof remains durable provenance; the final receipt records
the fresh proof used to complete recovery. It either resumes the exact archive or
finalizes the already-verified archived bytes. If the only interrupted step was
the task-marker finalization after a valid final external receipt, recovery
validates that receipt and archive then finalizes the marker without rewriting
either artifact. A legacy prepared record created
by an earlier tool build requires explicit final phase and disposition; it is
accepted only through `RecoverPrepared` and is immediately converted to the
current final receipt. No state needs a manual JSON rewrite to recover.

## Owner break-glass

Before this primitive existed, a holderless, non-expiring v1 lease could create
an unrecoverable deadlock. An Owner may authorize one bounded bootstrap settlement
only after fresh remote and PR admission, exact-byte hashing, schema/Goal/phase
confirmation, and bounded proof that no writer owns the associated worktree. The
exception must create a pre-settlement receipt, move rather than rewrite the
original lease to same-task-root evidence, verify the archived digest, and create
a settlement receipt before a new lease is admitted.

Now that the canonical primitive exists, manual JSON edits or deletions remain
forbidden. A missing holder, expired-looking timestamp, absent PID, or stalled
terminal is never permission to alter a lease by hand. Use normal `Settle` or
fully proven `ReclaimOrphan`; otherwise stop for the required Owner decision.

## Validation

Run `scripts/ci/test-tabbeacon-writer-lease.ps1` after any lifecycle-tool change.
It covers normal acquire/settle, second-acquire refusal, prepared-acquire refusal,
exact orphan reclaim,
digest/phase/holder/drift/reparse/collision rejection, byte preservation,
prepared-transaction resume/finalization (including final receipt before marker),
cross-task prepared-scope exclusion, receipt generation, fresh acquire after
recovery, normal closure without an active holderless lease, explicit holder
confirmation for normal settlement, and cleanup of all active v1 test fixtures.
