# v0.8.0 Goal Train — independent CLI presentation and trustworthy state

```text
GOAL_ID=TB-V080-MULTICLI-TRUSTED-STATE-TRAIN-001
TARGET_RELEASE=v0.8.0
OWNER_SCOPE_APPROVED=2026-09-26
ADMISSION_BASE_MAIN=8b2f6098ee73809d79d3acb8c84a34243e2017dc
ADMISSION_EFFECTIVE=ON_MERGE_OF_THIS_DOCUMENTATION_PR
IMPLEMENTATION_STATE=NOT_STARTED_AT_ADMISSION
CURRENT_PUBLIC_RELEASE_AT_ADMISSION=v0.7.3
PUBLIC_RELEASE_AUTHORIZED=false
OWNER_PRODUCTION_MUTATION_AUTHORIZED=false
```

This is the authoritative v0.8.0 scope and execution sequence approved by the
Owner in the development conversation. It replaces the feature-development
pause for this bounded train only. [V08_OPTIONS.md](V08_OPTIONS.md) is historical,
non-authoritative input, not a second implementation plan. Approval of this
roadmap does not assert that any new feature or local qualification is complete.

The existing [AGENTS.md](../AGENTS.md), [QUALITY_GATES.md](QUALITY_GATES.md), and
[RELEASE_CRITERIA.md](RELEASE_CRITERIA.md) remain effective. This train adds product
acceptance, not a replacement governance framework. If an authority conflict
cannot be reconciled within those rules, report it rather than weakening a gate.

Read with:

- [Acceptance matrix](V080_ACCEPTANCE_MATRIX.md): task-level and cross-feature proof.
- [Execution runbook](V080_EXECUTION_RUNBOOK.md): ZenBook Duo launch, continuation,
  permissions, receipts, and blocker handling.

## 1. Owner decisions and release shape

One stable release contains all four work packages:

| Package | Required user value |
| --- | --- |
| P — per-CLI presentation policy | Codex, Agy, and Cursor can have independent presentation preferences, with honest capability constraints. |
| C — Cursor integration | Preserve the existing Cursor CLI command and native title; formally qualify strict color-only operation. |
| E — Codex abnormal outcomes | Project structured warning, interruption, and failure evidence and reconcile subsequent recovery; track Issue #116 explicitly. |
| T — bilingual terminology | Provide a consistent English/Chinese glossary and align configuration, diagnosis, help, and support wording. |

Implement through focused, independently revertible PRs. Do not publish a partial
v0.7.x hotfix or silently move a required package to a later release. Internal
candidate artifacts are not public stable releases. If a required package is
blocked, the whole v0.8.0 public release remains blocked until it is qualified or
the Owner explicitly revises the scope. Cursor cannot be replaced by a color demo
or relabeled experimental to declare this train complete.

The daily runtime commands remain literal provider commands. The Owner's
`codex --yolo` development invocation is not a TabBeacon launcher or a change to
TabBeacon's direct-command product invariant.

## 2. Explicit non-goals

Do not migrate GitHub Actions to self-hosted runners, transfer the repository to
an organization, change branch protection, adopt Fleet orchestration, or create
a coordination repository. Keep the existing hosted CI infrastructure and check
identity. Necessary tests may evolve without changing the runner infrastructure.

Do not add another provider, dynamic plugins, a generic plugin platform, a theme
redesign, native tab icons, terminal expansion, an installer, auto-update, a PTY
host, command wrapper, executable/PATH shadow, or global resident daemon. Do not
promote Codex app-server into a hidden dependency. Do not add Agy color, progress,
animation, or error-detection capabilities merely for cross-provider symmetry.

Diagnostics, compatibility, install/uninstall safety, migration, tests, and release
materials are supporting work for P/C/E/T, not separate open-ended projects.
Dependency Issue #114 receives only current release-safety triage and minimal
necessary correction; do not turn this train into a general dependency upgrade.

## 3. Baseline and allowed implementation surface

The admission baseline is remote `main` at the SHA above. G00 must re-read the
current remote main, PRs, local worktrees, installed binary, and actual CLI
capabilities. Public release, repository HEAD, installed version, and real-session
adoption are different facts. Do not assume any of them from another.

Allowed subsystems are the relevant portions of `src/`, `tests/`, and existing
validation scripts for configuration, provider adapters/normalization, typed
presentation, ownership-safe integration, diagnostics, CLI/Control Center/help,
and their test fixtures. Documentation may change in `docs/`, the two READMEs,
`CHANGELOG.md`, and train/governance entry points. Cargo metadata/lockfile changes
are limited to the target version and justified dependencies required by this
scope. Final package metadata is settled before release-specific qualification.

Resolve concrete paths and the risk vector before each car writes. This subsystem
allowlist does not authorize sweeping refactors or unrelated fixes. Preserve all
foreign work. Repository-wide invariants, quality rules, and external production
boundaries cannot be weakened as an implementation shortcut.

## 4. P — independent presentation policy

### Resolution contract

```text
built-in defaults
  -> existing user-global defaults
  -> partial per-provider override
  -> positively established provider/terminal capabilities
  -> effective settings + explanation + application status
```

Use one resolver for runtime, CLI, wizard, and Control Center. Overrides are
partial: changing Cursor color policy must not copy or change Codex/Agy settings.
Old configuration retains its meaning and appearance after upgrade. Read-only
inspection must not migrate or rewrite files. Preserve the existing `native` and
`off` semantics; do not reinterpret either as suppressing the provider's own title.

### User-facing combinations

| Mode | Title | Tab color | Activity and progress |
| --- | --- | --- | --- |
| Full takeover / 完整接管 | TabBeacon-managed | TabBeacon-managed where supported | Existing preference, constrained by capability |
| Title only / 仅标题 | TabBeacon-managed | Unmanaged | Only existing, supported title presentation; no added color/progress |
| Color only / 仅颜色 | Native, zero TabBeacon title writes | TabBeacon-managed | Off: no spinner, badge animation, or progress ring |
| Preserve native / 保留原生 | Unmanaged | Unmanaged | Unmanaged; no continuing presentation writes |

Also expose inheritance and custom advanced combinations. Native title plus color
plus progress may be a custom combination, but it is not strict color-only mode.
Do not offer impossible combinations as successfully effective settings. Distinguish
saved preferences, installed integration, trusted Hook definitions, and live-session
application. When safe immediate application cannot be proved, explain the need to
restart that CLI rather than pretending a saved setting is already effective.

Codex supports the qualified combinations through existing channels. Agy retains
its admitted title-only capability boundary. Cursor must deliver color-only and
preserve-native; further modes are not required and must not be advertised without
separate proof within the same scope.

| ID | Deliverable |
| --- | --- |
| P01 | Global defaults and partial provider-override schema. |
| P02 | Shared effective-settings resolver with provenance and capability explanations. |
| P03 | Mode/channel semantics, including strict zero-title/zero-progress color-only output. |
| P04 | v0.7.3 compatibility and explicit, reversible migration; no read-side writes. |
| P05 | CLI, wizard, and Control Center target selection, preview, cancel, and application status. |
| P06 | Atomic/ownership-aware writes, idempotence, drift refusal, rollback, and isolation. |
| P07 | Import/export round trip, legacy format compatibility, and preview-before-apply; no portable trust or machine integration state. |
| P08 | Switching and cleanup that release only owned presentation; no stale-title restoration or cross-session interference. |

## 5. C — Cursor strict color-only integration

G01 establishes the actual executable, existing launch command, installed version,
Windows/WSL boundary, structured event contract, and safe terminal routing. Do not
confuse editor capabilities with CLI capabilities or assume the command is named
`cursor`. Read current primary provider documentation and verify the installed
profile; document a reproducible capability-based admission rather than inventing
support from a version string.

Required minimum lifecycle: enable the integration in an isolated owned fixture,
launch the original CLI command, observe evidence-backed working and completion,
start another turn, then exit with ownership-safe cleanup. Prove interruption and
failure mappings wherever the admitted structured outcomes support them. Unavailable
states are explicit, not guessed. Minimum working/completion/routing requirements
cannot be waived by calling the provider experimental.

Hook protocol responses and terminal control output are separate channels. Never
write VT sequences into JSON/protocol stdout. TabBeacon observes state; it must not
change approvals, inject context, auto-continue turns, alter authentication/model
settings, or block the provider because a display update failed.

| ID | Deliverable |
| --- | --- |
| C01 | Actual CLI identity and host/profile inventory; original command preserved. |
| C02 | Versioned structured-event/capability and terminal-routing qualification. |
| C03 | Exact-owned install/check/reconcile/uninstall preserving all unrelated Hooks. |
| C04 | Content-minimal normalization into provider-neutral state. |
| C05 | Correct session/terminal binding and protocol-safe color output. |
| C06 | Strict color-only/preserve-native operation, generation-safe cleanup, and zero title writes. |
| C07 | Two-Cursor and mixed-provider isolation, fail-open behavior, real minimum lifecycle, and honest support matrix. |

## 6. E — Codex abnormal-state and recovery closure

[Issue #116](https://github.com/JerrySkywalker/tabbeacon/issues/116) is the source
problem. Existing Warning/Interrupted/Failed semantics should be fed by proven
provider evidence; do not patch a title string to simulate a state-model fix.

Do not scrape terminal text, ANSI colors, screen pixels, conversation content,
assistant output, or tool output to infer failure. Allowlisted, bounded structured
metadata is the only evidence path. A proved abnormal outcome of uncertain severity
may use Warning; absence of evidence is not itself a proved failure.

State transitions must account for session, turn/generation, and event authority.
A delayed previous success must not override a new failure. A delayed failure must
not poison a new successful turn. Severity alone is not permanent precedence;
arrival order alone is not causality. Child-agent/tool errors do not automatically
become main-session terminal failure. New evidence-backed work clears/reconciles the
old abnormal state; completion returns to the appropriate Ready/ResultReady state.

| ID | Deliverable |
| --- | --- |
| E01 | Structured warning/interruption/failure inventory, including the original reported HTTP 404 class. |
| E02 | Typed normalization into the existing health/lifecycle model. |
| E03 | Session/turn/main-agent/subagent attribution and evidence authority. |
| E04 | Deterministic duplicate/late/out-of-order behavior and non-sticky recovery. |
| E05 | Correct output under every permitted mode without bypassing presentation policy. |
| E06 | Deterministic anomaly/recovery fixtures and real changed-boundary qualification; approval/question/result-ready regressions. |
| E07 | Read-only diagnostics, observable/unobservable class documentation, and truthful Issue #116 disposition. |

G01 must record whether the original 404 class is observable, partially observable,
or unavailable. If required evidence cannot be obtained, latch that limitation,
continue safe independent work, and do not close #116 or declare the release ready
on unrelated failures' evidence. Resolving a release-impacting gap requires proof
or an explicit Owner scope/limitation decision, not an Implementer-created waiver.

## 7. T — bilingual terminology and product-text consistency

Maintain a lightweight versioned Markdown glossary with stable IDs/anchors, English
term, recommended Chinese translation, project-specific definition, boundaries,
and relevant help/source links. Do not add a database, translation service, search
engine, or independent terminology application. Start the skeleton at G01 and finish
it at G05; do not turn all historical English technical guides into a translation
project.

Initial coverage includes Provider, CLI, Session, Turn, Workspace, Title, Tab Color,
Activity, Progress, Ownership, Native, Off, Capability, Evidence, Hook Trust, Ready,
ResultReady, Warning, Interrupted, Failed, inheritance, and override. Distinguish
ready-to-work from result-ready, config ownership from output ownership, and saved
configuration from installed/trusted/effective integration. ResultReady is not proof
that the user's entire development goal is complete.

| ID | Deliverable |
| --- | --- |
| T01 | Stable entry schema and preferred English/Chinese wording. |
| T02 | Core terms used by P/C/E and exact native/off semantics. |
| T03 | Bilingual navigation and discoverability from existing CLI/TUI help. |
| T04 | Consistent configuration, diagnosis, errors, and support-matrix wording. |
| T05 | Lightweight checks for required entries, duplicate IDs, and broken internal links. |

## 8. Train sequence and gates

| Car | Scope | Exit evidence |
| --- | --- | --- |
| G00 | Resume audit; exact remote/local baseline, storage, binaries, integrations, worktrees; freeze resolved paths | Clean owned implementation worktree and recorded differences; no production mutation |
| G01 | C01-C02, E01, P behavior/schema decisions, T01-T02 skeleton | Concrete provider evidence/routing conclusions and non-ambiguous release-impacting gaps |
| G02 | P01-P08 on existing Codex/Agy adapters | Configuration compatibility/isolation, safety family, applicable code/presentation/provider proof |
| G03 | E02-E07 | Anomaly/recovery and ordering families; original #116 class disposition; no false closure |
| G04 | C03-C07 using the shared resolver | Original command, real minimum lifecycle, strict color-only, protocol/ownership/isolation proof |
| G05 | T03-T05 and bounded supporting diagnosis, setup, help, support/upgrade docs | Coherent terminology and effective-capability/application explanations |
| G06 | Integrated candidate and release preparation | All required package/family evidence, one settled locked candidate CI, package/dry-run/checksum/upgrade/rollback proof, release recommendation |
| G07 | Authorized public v0.8.0 release and required closeout | Exact-SHA publication authorization, public verification, and release-policy adoption/convergence disposition |

G00-G06 are admitted implementation work, not eight rounds of Owner permission.
Proceed automatically after the applicable evidence is accepted. Record per-car
exact start/candidate heads and risk vectors. Use focused tests during iteration,
one final hosted exact-head code CI per settled code candidate, and a representative
owned Windows Terminal Visual/UIA pack only for changed presentation risk. Changed
provider/trust boundaries require focused L4; persistent writes and high-risk
ownership/concurrency require the focused review selected by QUALITY_GATES.

Evidence reuse requires an empty relevant risk diff and explicit REUSED provenance;
new SHAs do not invalidate unchanged-risk proof. The acceptance matrix groups
invariants and is not an instruction to run a full model conversation per row.
Do not manufacture commits, PRs, auditors, or repeated full suites for ceremony.

If a gate is externally blocked, latch its fingerprint once. Continue only
independent work that cannot invalidate the blocker record or masquerade as its
acceptance. A dependent car and public release do not pass while the prerequisite
remains BLOCKED/UNPROVEN. Use the runbook for a single actionable handoff.

## 9. Public and production boundaries

This documentation PR changes no product source, Cargo version, workflow, binary,
Hook trust, or production configuration. Its merge admits the plan, not release.

G07 needs explicit external Owner authorization for the exact public transaction
and candidate. Do not infer permission to publish crates.io, tag/release assets,
replace the daily binary, mutate real provider configuration, or grant Hook trust
from `--yolo`, a Goal ID, or the roadmap's target version. Preserve the existing
release convergence requirements; the historical v0.7.2 exception does not apply
here. Distinguish code-ready, qualified candidate, publicly released, and Owner
adopted in every closeout.

Before that authorization, a fully qualified G06 receipt may recommend release but
must show `PUBLIC_RELEASE_AUTHORIZED=false` and `PUBLIC_RELEASED=false`. If real
provider/UIA evidence is still missing, report BLOCKED/UNPROVEN instead of ready.
