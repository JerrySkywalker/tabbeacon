# Fast Lane v2 — Execution Reference

## Authority

`dev_governance_files/QUALITY_GATES.md` is the authoritative gate-selection policy.
This file is a compact execution reference for agents and humans. If the two differ,
`QUALITY_GATES.md` wins.

## Core rule

```text
validate changed risk once
reuse unchanged-risk evidence
avoid Goal ceremony
```

Before work, classify:

```text
CODE_CHANGED
PRESENTATION_CHANGED
PROVIDER_CHANGED
USER_PERSISTENT_CONFIG_CHANGED
SECURITY_OR_PRIVACY_CHANGED
RELEASE_BOUNDARY
```

## Quick classes

### Class D — docs/planning/governance

Default:

```text
L0 diff/governance sanity
Rust CI=N/A
Visual=N/A
L4=N/A
```

A governance rule change receives one normal review/CI cycle under the old rules before
activation, but does not trigger product-specific lanes by itself.

### Class C — ordinary code

During iteration:

```text
cargo fmt
focused affected tests
```

At the settled candidate:

```text
one hosted code CI
```

No Visual or L4 unless another risk dimension is active.

### Class V — presentation

Add exactly one representative final owned UIA/Visual pack after the candidate settles.
Prefer one transition pack over separate UIA executions for every traceability row.

### Class P — provider/profile/trust

Add one focused L4 only when real provider semantics or trusted declarations changed, or
the claim cannot be proven synthetically. Reuse prior L4 when the provider risk diff is
empty.

### Class S — persistent configuration / safety

Add one ownership-safety family covering minimal mutation, restore, drift, idempotence,
and unrelated-content preservation. Do not automatically add Visual/L4.

### Class R — release

Run one release closure train. Freshly prove release-specific code/package/publication
work. Reuse accepted unchanged-risk Visual, provider, performance, configuration, and
convergence evidence, plus only a small representative final dogfood smoke where useful.

## One-final-candidate pattern

Preferred flow:

```text
implement
-> focused tests
-> settle candidate
-> one final code CI
-> one final additional gate per active risk dimension
-> merge
```

Avoid:

```text
full suite
-> CI
-> metadata commit
-> full suite again
-> CI again
-> auditor
-> second auditor
```

unless relevant source or evidence actually changed.

## Family acceptance

A scenario list is primarily a traceability catalog. Group scenarios by the invariant
and proof authority they exercise. One family PASS may cover multiple rows.

Do not build one executor, one artifact, and one release gate per row unless independent
execution is necessary to distinguish materially different failure modes.

## Evidence reuse

Evidence reuse is first-class.

Check the relevant path/risk diff between the old evidence head and new candidate. If it
is empty, record:

```text
<GATE>=REUSED
<GATE>_REUSED_FROM=<sha>
<GATE>_RISK_DIFF=EMPTY
```

Common examples:

```text
presentation unchanged -> Visual REUSED
provider unchanged -> L4 REUSED
worker/timing unchanged -> performance REUSED
ownership mutation unchanged -> config-safety REUSED
docs-only closeout -> all product evidence REUSED
```

Do not rerun merely because HEAD advanced.

## Audit policy

Dedicated audit is reserved for:

- changed destructive/persistent external configuration behavior;
- changed security/privacy boundaries;
- changed concurrency/ownership with plausible corruption risk;
- ambiguous defects;
- release/publication;
- explicit Implementer request.

No generic audit loop exists for routine work.

## Blockers

One sufficient unchanged blocker observation is enough:

```text
BLOCKER_LATCHED=true
```

Do not repeatedly rerun the blocked lane until the relevant prerequisite changes.

## Writer leases

The canonical single-writer lifecycle, deterministic orphan recovery contract,
and Owner break-glass boundary are documented in
[WRITER_LEASES.md](WRITER_LEASES.md). Lease JSON is never edited or deleted by
hand; use the canonical acquire, settle, or fully proven reclaim operation.

## Current v0.3 application

For TB-G18:

```text
32-row matrix = traceability catalog
6 risk families = normative acceptance
1 final code CI
1 normal owned UIA convergence pack
1 actual elevated PowerShell pack
```

For TB-G19:

```text
minimal compatibility registry
one admitted 0.147.0 profile is sufficient
lightweight source-diff report
no new profile admission required for v0.3
L4=N/A unless a new profile is actually admitted
one final code CI
```

For TB-G19R:

```text
one release train
reuse accepted unchanged-risk G15-G19 evidence
one representative final dogfood pack
package + artifact + publication verification
```
