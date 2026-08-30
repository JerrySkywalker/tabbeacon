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

The current `tabbeacon-writer-lease.v1` schema has no holder or expiry field.
Its `writer_role=implementer`, `worktree`, `branch`, `start_remote_main`, and
`state` fields provide the available identity. Do not invent unsupported fields.

## Canonical lifecycle primitive

The only lease lifecycle primitive is
`tools/governance/Invoke-TabBeaconWriterLease.ps1`. It supports `Status`,
`Acquire`, `Settle`, and `ReclaimOrphan` and emits one machine-readable JSON
result on success.

The tool creates a lease with atomic create, serializes tool mutations with a
lease-specific OS mutex, rechecks the exact digest immediately before archive,
rejects reparse-point paths, and refuses to replace an existing lease, archive,
or receipt.

`Acquire` also requires an explicit safe lease-registry root. Under two global
scope mutexes, it scans that registry fail-closed and refuses any active v1 lease
for the same repository and either the same worktree or the same branch. This
prevents a second task-root pathname from bypassing the one-writer boundary.

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
no active lease at the original path.

## Orphans and deterministic reclaim

An orphan is not simply an old-looking lease. It qualifies only when the exact
SHA-256, schema, Goal, phase, repository, source main, worktree, and branch are
known; the schema supports no holder or the holder is explicitly empty; relevant
bounded process/worktree checks prove `ACTIVE_WRITER_COUNT=0`; and no active
holder evidence exists in the schema or task record.

That zero-writer result must be a separate hash-bound proof record, not an
unbound `-ActiveWriterCount 0` assertion. The record uses
`PROOF_SCHEMA=tabbeacon-writer-active-proof.v1` and binds the exact lease path
and SHA-256 while recording `ACTIVE_WRITER_COUNT=0` and
`ACTIVE_LEASE_HOLDER_PROVEN=false`. `ReclaimOrphan` verifies the proof path and
its expected SHA-256, records both in the settlement receipt, and rejects a
malformed, stale, or non-zero proof.

`ReclaimOrphan` requires every expected identity field, `-ExpectedHolderless`,
`-ActiveWriterCount 0`, and exact non-existing archive and receipt destinations
inside a safe explicit archive root. It refuses wrong digests, schema, Goal, or
phase; a non-empty holder; concurrent drift; reparse targets; and archive
collisions. Successful reclaim archives the original bytes and writes a receipt;
it never edits the lease to mark it closed.

Leases are first flushed to an adjacent unique staging name and then atomically
published, so interruption cannot leave a partial canonical JSON lease that
blocks recovery. Settlement first creates a `TRANSACTION=PREPARED` receipt and
replaces it atomically with the final receipt only after the exact opened source
file has been renamed. An interrupted transaction therefore leaves either a
classifiable prepared record and source lease, or a classifiable prepared record
and archived lease; it never needs a manual JSON rewrite to recover.

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
It covers normal acquire/settle, second-acquire refusal, exact orphan reclaim,
digest/phase/holder/drift/reparse/collision rejection, byte preservation, receipt
generation, fresh acquire after recovery, and normal closure without an active
holderless lease.
