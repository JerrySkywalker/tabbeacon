# v0.8.0 acceptance matrix

Parent: [ROADMAP_V08.md](ROADMAP_V08.md)

```text
GOAL_ID=TB-V080-MULTICLI-TRUSTED-STATE-TRAIN-001
MATRIX_STATE=REQUIREMENTS_NOT_EXECUTION_EVIDENCE
```

Every row below is initially UNPROVEN. This document defines what must be shown;
its existence is not a PASS. Record actual results in the runbook's durable receipt
with evidence head, gate/family, risk paths, sanitized artifact reference, and
PASS/FAIL/BLOCKED/UNPROVEN/REUSED/N/A disposition. N/A is for an inapplicable gate,
not a way to remove a required product task. One representative proof family may
cover several rows under [QUALITY_GATES.md](QUALITY_GATES.md).

## 1. Task-level obligations

### P — independent presentation

| ID | Required proof | Representative family |
| --- | --- | --- |
| P01 | Existing global defaults plus partial Codex/Agy/Cursor overrides round-trip without cross-provider edits. | CONFIG_RESOLUTION |
| P02 | CLI, wizard, Control Center, and runtime agree on effective values, inheritance provenance, capability limits, and application state. | CONFIG_RESOLUTION |
| P03 | Full/title-only/color-only/native/custom combinations have defined bytes; color-only emits no title, activity, badge-animation, or progress output. | CHANNEL_POLICY |
| P04 | v0.7.3 input keeps existing behavior; read-only inspection does not mutate; explicit migration has backup/rollback and collision handling. | LEGACY_MIGRATION |
| P05 | Target provider, preview, cancel, inheritance reset, and saved-versus-effective messages are correct and usable. | CONFIG_UX |
| P06 | Exact ownership, minimal atomic mutation, idempotence, drift refusal, restore/uninstall, and unrelated-content preservation. | CONFIG_SAFETY |
| P07 | Legacy import is accepted safely; new overrides survive export/import preview and apply; secrets, Hook trust, and machine state are excluded. | PORTABLE_CONFIG |
| P08 | Switching/exit releases only owned output; no stale-title restoration, stale color worker, or writes to a newer session generation. | PRESENTATION_OWNERSHIP |

### C — Cursor

| ID | Required proof | Representative family |
| --- | --- | --- |
| C01 | Resolved executable and original daily command identify the actual CLI, with version/host/Windows-or-WSL facts, not editor assumptions. | CURSOR_ADMISSION |
| C02 | Current primary documentation plus actual installed-profile observations establish working/completion events and terminal routing; unknown outcomes are named. | CURSOR_ADMISSION |
| C03 | Repeated install/reconcile/uninstall in an isolated home preserves user Hooks and unrelated authentication, model, and approval settings. | CONFIG_SAFETY |
| C04 | Normalizer consumes only necessary structured metadata; prompt, assistant, tool output, and arbitrary raw Hook payloads are not retained. | PROVIDER_PRIVACY |
| C05 | Color output reaches the exact owned terminal session; protocol stdout remains valid; no recursive Hook calls or provider blocking. | ROUTING_PROTOCOL |
| C06 | Native title remains under provider control throughout work, completion, switching, and cleanup; strict color-only has zero hidden title/progress writes. | CHANNEL_POLICY |
| C07 | Two concurrent Cursor sessions and mixed Codex/Agy/Cursor sessions remain isolated; original-command real lifecycle and fail-open behavior are proved. | MIXED_PROVIDER_L4 |

### E — Codex abnormal outcomes

| ID | Required proof | Representative family |
| --- | --- | --- |
| E01 | Enumerate authoritative evidence for warning/interruption/failed-turn outcomes and separately classify the original Issue #116 HTTP 404 symptom. | CODEX_EVIDENCE |
| E02 | Proven abnormal outcomes enter existing typed health state; uncertain proved severity may be Warning, but missing evidence is not fabricated failure. | ANOMALY_RECOVERY |
| E03 | Main/session/turn/subagent identities and authority prevent child or foreign activity corrupting the parent tab. | GENERATION_ISOLATION |
| E04 | Warning/failure/interruption recover on a proved new turn; duplicate and late successes/failures cannot reverse newer state. | ANOMALY_RECOVERY |
| E05 | Abnormal output respects provider overrides; color-only changes color only, preserve-native produces no unauthorized output. | CHANNEL_POLICY |
| E06 | Deterministic fixtures cover all anomaly/recovery classes and approval/question/result-ready regressions; real changed-provider claims receive appropriate L4. | ANOMALY_RECOVERY |
| E07 | Diagnosis and support docs state observable/unobservable classes; Issue #116 remains open for an unresolved original defect, with no blanket fix claim. | SUPPORT_TRUTH |

### T — English/Chinese terminology

| ID | Required proof | Representative family |
| --- | --- | --- |
| T01 | Every entry has a unique stable ID, English term, preferred Chinese wording, project definition, and needed boundary notes. | GLOSSARY |
| T02 | All core roadmap terms are covered; Ready/ResultReady, native/off, capability/config/trust/application, and ownership meanings are distinct. | GLOSSARY |
| T03 | English and Chinese entry points and existing CLI/TUI help can reach the glossary through stable links. | HELP_NAVIGATION |
| T04 | Configuration, diagnostics, errors, modes, and support matrix use the same definitions without translating machine-facing API tokens. | PRODUCT_TEXT |
| T05 | Required-term, duplicate-ID, and internal-link checks are automated using lightweight repository tooling. | DOC_CHECKS |

## 2. Cross-feature acceptance

| ID | Scenario | Mandatory outcome |
| --- | --- | --- |
| X01 | Upgrade from a representative v0.7.3 configuration | Preserve settings, aliases, unrelated integrations, and old meanings; do not install Cursor or replace preferences automatically. |
| X02 | Codex, Agy, and Cursor run concurrently | Independent policy and session state, no shared writer interference, no unsupported Agy capabilities. |
| X03 | Cursor updates its own title repeatedly | TabBeacon never races or restores a cached title; only exact-owned color updates are allowed in strict color-only mode. |
| X04 | Codex fails and later succeeds | Correct abnormal state, generation-safe recovery, no stale success and no permanently sticky red state. |
| X05 | Switch an active provider to preserve-native | Stop persistent presentation, release only proved ownership, preserve foreign sessions/processes and native title. |
| X06 | User config drifts or a capability is missing | Refuse unsafe overwrite; explain requested/effective mismatch and restart needs; never silently claim success. |
| X07 | Export/import and downgrade/rollback | Round-trip overrides and prove an actionable restore path; replacing the executable alone is not accepted rollback proof. |
| X08 | Missing/broken integration or unsupported host | Original provider remains usable; display failure is bounded and content-minimal; no automatic repair or permission changes. |

Exercise forced exit, terminal closure, and lost ownership as bounded negative
cases. If exact native-state restoration is not possible, document the boundary;
never claim arbitrary original title/color restoration. Do not terminate unknown
processes to make cleanup pass.

## 3. G01 evidence decisions

The first capability investigation must produce a compact record:

```text
CURSOR_EXECUTABLE=<resolved-non-secret-path>
CURSOR_ORIGINAL_COMMAND=<observed-command>
CURSOR_HOST_PROFILE=<native-Windows-or-observed-boundary>
CURSOR_VERSION=<observed>
CURSOR_WORKING_EVIDENCE=<proved-source-or-UNPROVEN>
CURSOR_COMPLETION_EVIDENCE=<proved-source-or-UNPROVEN>
CURSOR_TERMINAL_ROUTE=<proved-owned-route-or-UNPROVEN>
CURSOR_PROTOCOL_STDOUT_SAFE=<PASS|FAIL|UNPROVEN>
CODEX_PROFILE=<capability-fingerprint>
CODEX_WARNING_SOURCE=<source-or-unobservable>
CODEX_INTERRUPTION_SOURCE=<source-or-unobservable>
CODEX_FAILURE_SOURCE=<source-or-unobservable>
ISSUE_116_ORIGINAL_404=<OBSERVABLE|PARTIAL|UNAVAILABLE|UNPROVEN>
RELEASE_IMPACTING_GAP=<none-or-specific>
```

Source/version/profile changes require a bounded reassessment, not blind reuse of
old wire assumptions. A documentation-only claim is not real terminal routing proof.
A fixture can prove a normalizer but cannot create upstream observability.

When the original #116 class remains unsupported, show the exact missing signal and
safe fail-open behavior; leave the issue unresolved. Required release scope is not
reduced automatically. Continue safe independent work and obtain a specific Owner
decision only for an actual unresolved release-impacting limitation.

## 4. Candidate and publication acceptance

G06 requires a settled candidate and all required work packages qualified. Fresh
required gates use `EXPECTED_HEAD == checked_out_head == evidence_head`. Reused
Visual/L4/safety proof records its accepted head and empty relevant risk diff.

A release preparation family covers the locked build/code suite, relevant current
security triage, package/dry-run contents, licenses, version metadata, Windows
artifact/checksum, safe upgrade/rollback, and current support/limitation notes.
Keep the existing hosted exact-head CI check; do not migrate runners, bypass a
required check, change protection, or conceal a failed check as N/A.

A single owned Windows Terminal/UIA pack may cover visible transitions across the
policy and recovery families. Title-only UIA proof does not prove tab-color pixels;
use an appropriate existing or bounded extended color oracle. If the required
visual observation cannot run, mark the claim BLOCKED/UNPROVEN, not PASS from VT
unit tests alone. Real-provider proof must be limited to changed claims and privacy-
minimal synthetic content; no repeated paid model calls per matrix row.

G07 is not admitted by code completion. It requires explicit exact-candidate Owner
publication authorization, existing release-criteria satisfaction, and intentional
public mutation. Record separate results for tagged source, crates.io publication,
GitHub release/assets, public consumer verification, and Owner official-channel
convergence. The historical v0.7.2 production-non-adoption exception must not be
reused for v0.8.0 without a separately approved policy change.

## 5. Efficient evidence accounting

The 27 task IDs and eight cross-feature scenarios are traceability requirements,
not 35 mandatory separate jobs or receipts. Prefer shared invariant/failure-family
fixtures, focused iteration, one final gate per changed risk, and explicit evidence
reuse. Independent review is required where existing policy requires it, not for
every documentation task. Do not self-label an Implementer review independent.

Retain content-minimal receipts and useful reproducible fixtures. Do not commit raw
conversation/Hook payloads, credentials, environment dumps, screenshots containing
private work, or session identifiers. Use sanitized aliases/hashes where identity
is needed. An unchanged blocker is latched once and only reopened on relevant
source, evidence, trust, Owner action, or external prerequisite change.
