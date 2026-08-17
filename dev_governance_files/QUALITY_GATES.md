# Quality Gates — Fast Lane v2

## Authority

This file is the authoritative gate-selection policy for TabBeacon development.
`AGENTS.md` defines repository-wide operating constraints. `FAST_LANE.md` is a
compact execution reference and must not override this file.

The governing rule is:

> validate changed risk once, not Goal ceremony repeatedly.

A check is required because a change can invalidate that behavior class. A Goal,
scenario row, documentation update, or new commit does not by itself require every
lane to run again.

## Risk vector

Before implementation, classify the changed risk surface:

```text
CODE_CHANGED
PRESENTATION_CHANGED
PROVIDER_CHANGED
USER_PERSISTENT_CONFIG_CHANGED
SECURITY_OR_PRIVACY_CHANGED
RELEASE_BOUNDARY
```

A Goal may activate more than one dimension. Gates are selected from the active
dimensions only.

### Default mapping

| Changed risk | Required acceptance |
| --- | --- |
| Docs/planning/governance only | L0 diff/governance sanity only |
| Ordinary Rust/code | focused tests while iterating + one final hosted code CI |
| Presentation | ordinary code gate + one final owned UIA/Visual acceptance pack |
| Provider/profile/trust | ordinary code gate + one focused L4 only when the real provider boundary changed or cannot be proven synthetically |
| User persistent configuration | ordinary code gate + ownership/restore/drift fixture + one focused safety review |
| Security/privacy | focused tests + one focused safety/privacy review |
| Public release | one release closure train using fresh release-specific gates plus reusable unchanged-risk evidence |

Do not add a lane merely because code for that subsystem exists elsewhere in the
repository.

## Core efficiency rules

### One risk, one proof

One representative proof may cover a family of scenarios when they exercise the
same invariant. A traceability matrix is not automatically a list of independent
release gates.

For example, multiple lifecycle events may be accepted by one Hook-fixture family,
and multiple workspace identity variants may use deterministic identity coverage plus
one representative real Windows Terminal/UIA presentation smoke.

Split a family into separate gates only when the scenarios have materially different
failure modes or authority requirements.

### One settled candidate, one final gate

During implementation use focused local tests. After the candidate settles, run the
required final gates once:

```text
focused iteration
-> settle candidate
-> one final hosted code CI
-> one final UIA/L4/safety gate only for activated risk dimensions
-> merge
```

Do not run full CI or Visual after every intermediate commit.

### Evidence reuse

Evidence may be reused across heads when the relevant risk surface did not change.
A new SHA alone does not invalidate unrelated evidence.

Before reuse, compare the prior evidence head and candidate head against the paths or
subsystems that can affect that gate. A simple bounded `git diff <old>..<new> --
<risk paths>` is sufficient unless a Goal defines a stronger ownership mechanism.

Record reuse explicitly, for example:

```text
VISUAL=REUSED
VISUAL_REUSED_FROM=<sha>
VISUAL_RISK_DIFF=EMPTY
```

Typical reuse rules:

- no presentation-path change -> prior Visual/UIA evidence may be reused;
- no provider/profile/trust change -> prior L4 may be reused;
- no worker/timing change -> prior performance evidence may be reused;
- docs/governance-only closeout -> prior Rust/Visual/L4 evidence remains valid;
- no persistent-config mutation change -> prior ownership fixture may be reused.

Do not reuse evidence when the changed source can plausibly alter the proven behavior.

## L0 — Repository and governance sanity

Typical checks:

- expected files/licenses present;
- whitespace/line-ending policy for changed text;
- no generated runtime state or secret material committed;
- governance files remain internally consistent.

Docs-only work normally stops here. A governance change that changes future gates
receives one review/CI cycle under the previously effective rules before it becomes
active; it does not require Visual or provider L4 unless those product surfaces also
changed.

## L1 — Code/static gate

During implementation prefer focused tests.

Final ordinary-code acceptance is one hosted exact-candidate CI running the repository
quality suite. Local repetition of the whole suite is optional when hosted CI will run
it once at the settled head.

Mechanical formatting failures are fixed mechanically; they do not require an audit.

## L2 — Functional/family gate

Use focused integration tests for the changed invariant or risk family. Do not turn a
coverage catalog into one executor/artifact per row unless independent execution is
needed to distinguish real failure modes.

Representative family names may include:

```text
LIFECYCLE_FAMILY
GENERATION_ISOLATION
WORKSPACE_IDENTITY
RECOVERY_FAMILY
```

One family PASS may be backed by several deterministic tests or fixtures without
creating separate durable receipts for each test case.

## L3 — Visible presentation gate

Required only when title, progress, color, VT bytes, animation, or the product visual
oracle can change.

Prefer one owned Windows Terminal/UIA acceptance pack that exercises the important
visible transitions together. For a healthy title channel this normally covers:

```text
working animation
result-ready
approval/attention where applicable
stable workspace alias
cleanup/title authority
```

Do not require separate full UIA runs for every workspace or lifecycle variant when
those variants are already proven by deterministic logic and share the same
presentation path.

Pixel/screenshot capture is not mandatory for title correctness when the approved
exact-tab UIA oracle proves the required visible title semantics and a known runner
capture limitation is latched.

## L4 — Provider gate

Required only when provider/profile/config/trust semantics changed or when a focused
real-provider claim cannot be proven by the admitted fixtures.

Do not request Owner trust or rerun real Codex merely because an unrelated source head
changed.

A new Codex release/profile requires explicit admission. Merely restructuring the
compatibility registry around an already admitted profile does not require fresh L4 if
wire semantics and trust declarations are unchanged.

## Persistent configuration safety

Changes that can write external user configuration require focused proof of:

```text
exact ownership
minimal mutation
idempotence
restore/uninstall safety
concurrent drift refusal
unrelated-content preservation
```

This is a safety family, not a reason to rerun unrelated Visual or provider matrices.

## Audit policy

A separate auditor is not a default development stage.

Require one focused independent review only when at least one applies:

- destructive or persistent external configuration writes changed;
- security/privacy boundary changed;
- concurrency/ownership logic changed with plausible cross-session corruption;
- an ambiguous defect needs independent classification;
- public release/publication is being closed;
- the Implementer explicitly requests it.

Routine code, diagnostics, test-harness, docs, and mechanical fixes do not require a
separate auditor after relevant tests and gates pass.

Do not chain multiple auditors over unchanged evidence.

## Exact-head and risk-head semantics

Fresh required gates bind to the settled candidate:

```text
EXPECTED_HEAD == checked_out_head == evidence_head
```

Reused evidence is valid only when its relevant risk diff is empty and the reuse is
recorded explicitly. Therefore release or Goal receipts may legitimately contain:

```text
CODE_HEAD=<current>
VISUAL=REUSED_FROM_<prior>
```

when presentation paths are unchanged.

Exact-head means exact for a gate that is freshly required; it does not mean every
unrelated gate must be rerun after every metadata-only SHA.

## Failure classification

Classify failures as:

- product/code defect;
- test/harness defect;
- runner/environment defect;
- external dependency/service defect;
- evidence/risk-head mismatch;
- unproven.

Fix mechanical failures mechanically. Do not create a root-cause audit for a simple
formatting mismatch unless repetition indicates a deeper toolchain issue.

## Blocker latch

After one sufficient observation of an unchanged external/Owner blocker, record a
stable fingerprint and set:

```text
BLOCKER_LATCHED=true
```

Do not rerun the blocked full lane until source affecting it, Owner evidence, trust
state, or the external prerequisite changes.

## Release policy

A public release still gets one deliberate closure train. That train does not need to
re-execute every historical scenario independently.

Required fresh release-specific work normally includes:

- one full locked code/static/build CI at the release candidate;
- package/dry-run/content inspection;
- release artifact/checksum creation;
- publication and public verification.

Presentation, provider, performance, configuration-safety, and convergence evidence
may be reused when the corresponding risk surfaces have not changed since their
accepted proof. The release Goal may require a small representative final dogfood
smoke for integration confidence, but must not recreate every prior Goal's matrix.

## Receipt discipline

Receipts contain only decision-relevant gates. Use `N/A` or `REUSED` instead of
inventing irrelevant fields.

A typical ordinary code Goal is:

```text
DISPOSITION=<PASS|FAIL|BLOCKED|UNPROVEN>
EXPECTED_HEAD=<sha>
CODE_CI=<PASS|FAIL|N/A>
VISUAL=<PASS|REUSED|N/A>
L4=<PASS|REUSED|N/A>
PERSISTENT_CONFIG_SAFETY=<PASS|REUSED|N/A>
OWNER_ACTION=<none-or-specific>
```

The objective is fewer repeated checks while preserving the proof that matters to the
user-visible and safety-critical behavior.
