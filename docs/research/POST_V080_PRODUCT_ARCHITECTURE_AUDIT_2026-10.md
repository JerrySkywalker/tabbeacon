# Post-v0.8.0 Product / Architecture Audit — October 2026

## Authority and decision

```text
GOAL_ID=TB-POST-V080-PRODUCT-ARCHITECTURE-AUDIT-001
AUDIT_ONLY=true
REPOSITORY=JerrySkywalker/tabbeacon
CURRENT_PUBLIC_RELEASE=v0.8.0
MAIN_BASE=372637bdbc95e2fd5456f91d7b78a5ae1b3ebcfc
PRODUCT_CODE_CHANGED=false
RUNTIME_CHANGED=false
NEXT_ROADMAP_ADMITTED=false
V09_IMPLEMENTATION_AUTHORIZED=false
ASSUMED_ROLE=auditor
PROFILE_FALLBACK=current-session-default
```

Assessment date: 2026-10-01, Asia/Shanghai. This is a read-oriented audit with
explicit authority to write these audit documents on an isolated branch. It
does not amend release acceptance, authorize implementation, close issues, or
authorize production configuration, trust, provider upgrades, publication, or
merge. Estimates below are engineering judgments, not commitments or measurements.

**Product judgment: DAILY_USABLE, with demonstrated multi-provider operation
inside a bounded support envelope; not yet MULTI_PROVIDER_STABLE across version
upgrades and same-tab transitions.** v0.8 proves more than the two-provider
post-v0.7 audit: independent per-CLI policy and Cursor color-only operation are
real product additions. The most consequential next investment is repeatable
qualification and operational diagnosis, followed by a small provider read-model
seam. Adding another provider first would multiply today’s qualification cost.

The immediate prerequisite is a separate **POST-V080-ROADMAP-RECONCILIATION**
Goal. The released source still describes admission/candidate state and its docs
checker enforces that state. A green docs check therefore does not prove current
public truth. This audit identifies the mismatch without rewriting those files.

## Method, evidence and limitations

Evidence classes are explicit throughout:

| Class | Meaning |
| --- | --- |
| CURRENT_PRODUCT_FACT | Fact checked against the release tree, live public APIs or pinned source in this audit. |
| HISTORICAL_EVIDENCE | A recorded observation at its original source/binary/provider/host; no fresh execution implied. |
| KNOWN_LIMITATION | A disclosed gap retained by the released product. |
| OPTIONAL_FOLLOWUP | Unexecuted or insufficiently proved behavior outside the narrowed public promise. |
| PLANNING_PROPOSAL | A possible future change requiring its own admission. |

Read scope: AGENTS.md; QUALITY_GATES; RELEASE_CRITERIA; ROADMAP, ROADMAP_V08,
DEVELOPMENT_PAUSE, V08_OPTIONS; support/upgrade/release docs; implementation and
tests in the exact release tree; PR/issues/public registries; bounded existing
release receipts; current official upstream documentation. No live provider,
UIA, setup, uninstall or runtime qualification was executed. No credential,
private rollback bundle or daily provider configuration was opened.

The canonical checkout was clean on local `main`
`67e9d846967593ac4aa13713a0539b90bc525330` (September 2), behind the freshly
checked remote main. `origin/main` and remote `main` were the expected base.
Canonical main was preserved. The clean audit worktree was created from the
expected base on `audit/post-v080-product-architecture-001`.

### Evidence index

| ID | Source / observation | Scope |
| --- | --- | --- |
| R1 | [GitHub v0.8.0 release](https://github.com/JerrySkywalker/tabbeacon/releases/tag/v0.8.0), latest-release API | Live: public, not draft/prerelease; published 2026-09-30 16:36:43 UTC. |
| R2 | [crates.io 0.8.0 API](https://crates.io/api/v1/crates/tabbeacon/0.8.0) | Live: version 0.8.0, not yanked; created 16:33:57 UTC. |
| R3 | [PR #119](https://github.com/JerrySkywalker/tabbeacon/pull/119), Git refs and tree comparison | Live: merged 16:31:28 UTC; candidate and protected merge content trees equal. |
| R4 | [Main CI 36744840390](https://github.com/JerrySkywalker/tabbeacon/actions/runs/36744840390) | Live API confirms success at exact merge SHA. |
| R5 | `g06-final-11d5596.md`, `g07-final-11d5596.md`, `g07-release-11d5596.md` | Historical frozen preparation, public-consumer proof, Owner adoption. |
| R6 | `g07-owner-official-source-proof.json` | Historical minimal Cargo v1/v2 registry-source evidence, not a new host inspection. |
| R7 | `cargo-audit-c1ff551-20260930.json` | Historical RustSec: zero vulnerabilities and empty warnings; reused over unchanged lock. |
| R8 | [Support matrix](../coding-agent-support.md), [release notes](../v0.8.0-release-notes.md), [acceptance matrix](../../dev_governance_files/V080_ACCEPTANCE_MATRIX.md) | Checked release-tree declarations; qualifications retain their individual scopes. |
| R9 | [Issue #114](https://github.com/JerrySkywalker/tabbeacon/issues/114), [#116](https://github.com/JerrySkywalker/tabbeacon/issues/116) | Live: both OPEN; bodies checked against acceptance and code. |

R5–R7 are retained in the original local artifact directory
`TB-V080-MULTICLI-TRUSTED-STATE-TRAIN-001` under the TabBeacon artifacts root.
They are not portable public links. This report identifies receipt names and
minimal facts, rather than embedding private files. Audit command receipts and
public-source snapshots live separately under this Goal ID. An unavailable
receipt later means UNPROVEN for its historical claim, not permission to invent
replacement evidence. Web documentation is a dated discovery surface; future
admission must pin a source revision and executable independently.

## PRODUCT_CURRENT_TRUTH

### Release identity and official channel

CURRENT_PRODUCT_FACT: v0.8.0 is publicly released in both channels. PR #119 head
was `11d5596ff27f7b7422de773dd8767ad5c347dede`; merge/main/tag target is
`372637bdbc95e2fd5456f91d7b78a5ae1b3ebcfc`. Both trees are
`537f6398fec26db62cbbc94ea24f48201b27b3b4`; a direct Git content diff is empty.
The annotated tag object is `8bbb7d973a4fb9d64fca8136fc9889df30c233b4`.
GitHub’s API reports `immutable=false`; historical “no replacement or force
push” proves the recorded transaction, not a platform-enforced immutability flag.

The live registry checksum is
`712cebea47826db61459baa749a7d5f5b1f068f9da4f7e7beb34a41a103c8b2f`, matching
the release crate asset digest. Windows ZIP digest is
`d94bb86b6d3f028a077a919e8280064de2eb4186bf062c252b607edda867a6eb`.
The 12 published assets include checksums, license inventory, source crate,
support/upgrade/notes and RELEASE-PROVENANCE. R1/R2 prove current public metadata;
R5 records actual anonymous downloads, unpacked smoke and fresh registry consumer.
Those consumer tests were not repeated by this audit.

HISTORICAL_EVIDENCE: ZenBook Duo official-channel convergence completed in G07.
Cargo source metadata proves registry 0.8.0, `OWNER_GIT_REV_INSTALL=false`.
The installed registry-built product hash was
`e07c3650e809de8712886e854f97d17e6f8e354fcbca025c1f75d0af10682ca2`;
it matched the separately built public registry consumer. It is distinct from
the ZIP executable hash, not a bit-identical-build claim. Existing Codex setup
was idempotent; config/Hooks/preferences bytes were preserved; Doctor/runtime
probe passed. Recorded native Codex 0.159.2 was diagnostic-only, not new L4.
No new present-day install/config/trust assertion is made here.

### Supported provider envelope

| Provider | CURRENT_PRODUCT_FACT | HISTORICAL_EVIDENCE / boundary |
| --- | --- | --- |
| Codex, `codex` | Capability-based required baseline; typed normalization, generation rejection, manual trust, full admitted presentation channels. Missing capabilities fail safely; version ordering alone neither admits nor denies support. | Real 0.157.1 Interrupt/recovery/completion/exit is separately pinned. Observed 0.159.0 has the local admitted 11-event baseline; older Interrupt proof does not transfer. G07 0.159.2 runtime probe is not L4. |
| Cursor, `agent` | Exact-owned project Hooks, strict color-only or preserve-native; no TabBeacon title, progress or animation takeover. | Real dual-Cursor and mixed lifecycle/noninterference observations remain separately scoped. Native title retained. No daily Owner Cursor deployment inferred. |
| Agy, `agy` | Exact 1.1.19 and narrow 1.2.7 title-callback contracts. Actual initializing/working maps to Working; actual idle maps to Ready. No health, result-ready, color, progress or animation authority. | 1.1.19 admitted historical profile retained. 1.2.7 signed/authenticated real Working/default-color/mixed noninterference; isolated setup/reconcile/uninstall/restore/drift; deterministic delivered-idle mapping. Real Ready was not proved. 1.2.13 is not admitted. |

```text
ISSUE_116=OPEN_KNOWN_LIMITATION
AGY_READY_VISIBLE_L4=UNPROVEN
SAME_TAB_HANDOFF_L4=NOT_EXECUTED_OWNER_STOP
AGY_PRESERVE_NATIVE_L4=NOT_EXECUTED_OWNER_STOP
```

KNOWN_LIMITATION: the audited 0.157.1 original-command Hook path supplies no
authoritative Warning/Failed/original HTTP404 outcome; stale green is possible.
OPTIONAL_FOLLOWUP: the three results above stay outside v0.8’s qualified public
promise. Separate-tab noninterference does not prove sequential same-tab handoff;
deterministic idle mapping does not prove real callback delivery. Release PASS
does not turn these into PASS.

## POST_RELEASE_GOVERNANCE_DRIFT

The release tree correctly narrowed provider claims, but publication was later
than the source freeze. Active docs never completed the corresponding transition.

| File / surface | Checked drift | Separate reconciliation proposal |
| --- | --- | --- |
| ROADMAP.md | `LAST_VERIFIED_PUBLIC_RELEASE=v0.7.3`, `CURRENT_PUBLIC_TARGET=v0.8.0`, `V080_ADMITTED`, G00 resume/continuous implementation; prose says preparation does not prove publication. | Add current release identity/R1–R4, complete v0.8 implementation/publication, clear active feature authority. Preserve old release history. |
| DEVELOPMENT_PAUSE.md | Same old last-verified marker; active Train and G00 instruction. | Stable post-release pause; distinguish completed Train from optional qualification and this audit. |
| check-docs.ps1 | Lines 266–274 require v0.7.3 last-verified markers and reject current-public-v0.8 assertions; lines 291–314 require admitted Train state. | Replace active-state assertions with verified stable-state assertions in a separately authorized script/docs change. Add a regression that rejects stale admission after publication. |
| README.md / README.zh-CN.md | Provider scope is v0.8; release section still says candidate/preparation and publication not proved. Critical-invariants comment lists only Codex/Agy. | Update current distribution truth in both languages; reconcile comment/checker with three providers without expanding Agy claims. |
| docs/README.md, getting-started, SECURITY.md, release-process | Candidate/current-target wording remains; docs portal refers to active Train. | Public 0.8.0, verified sources, stable support envelope; retain historical source preparation as history. |
| v0.8.0 release notes / upgrade / support | Notes say “when published”; other support statements retain appropriately narrow provider scope. | Add post-publication navigation/annotation; preserve frozen receipt and original observations. Do not silently rewrite the published acceptance narrative. |
| ROADMAP_V08 / matrix / runbook | G06/pending-G07 directions remain in historical Train material. | Mark completed historical execution authority and link to post-release truth. Keep original heads, failures and Owner stops intact. |
| RELEASE_CRITERIA | Architecture/release prose remains Codex-centric despite the three-provider release. Release-tree AGENTS already admits C: ZenBook Duo roots; the older canonical checkout's V: text is not current release-tree authority. | Reconcile provider-scope wording with the bounded support matrix. No AGENTS storage correction or storage mutation is recommended from the obsolete checkout text. |

PLANNING_PROPOSAL for the stable state:

```text
CURRENT_PUBLIC_RELEASE=v0.8.0
LAST_VERIFIED_PUBLIC_RELEASE=v0.8.0
CURRENT_PUBLIC_TARGET=NONE
CURRENT_DEVELOPMENT_TARGET=NONE
V080_IMPLEMENTATION=COMPLETE
V080_PUBLICATION=COMPLETE
ACTIVE_FEATURE_DEVELOPMENT=NONE
NEXT_MAJOR_ROADMAP=NOT_ADMITTED
NEXT_RECOMMENDED_GOAL=OWNER_ROADMAP_DECISION
```

The reconciliation Goal should own exactly the affected active docs and docs
assertions, choose consistent NONE/PAUSED vocabulary once, and validate bilingual
support and relative links. Old V08_OPTIONS remains historical planning input.
Do not create ROADMAP_V09 or smuggle an option into an active Goal. A publication
receipt is evidence input, not authorization for a source change.

## ARCHITECTURE_ASSESSMENT

The following is a source audit, not fresh runtime acceptance. Links reference
the audited worktree’s release implementation; HEAD is the base plus audit docs.

### A. Core

| Required assessment | Finding |
| --- | --- |
| CURRENT_STRENGTH | [core/mod.rs](../../src/core/mod.rs) has open AgentProvider, provider+native-ID AgentSessionKey, AgentEvidence with source/authority/confidence/time/tie-break and independent phase/attention/health reconciliation. Backend declarations cannot grant authority to an observation. Raw provider events stay below the boundary. |
| CURRENT_DEBT | Observation-time reconciliation is not itself transport ordering. Durable generation tickets, session anchoring and terminal ownership remain adapter/runtime responsibilities; qualification must test delayed, duplicate and retired-generation events separately. |
| HARD_CODED_PROVIDER_SEAMS | None in the core contract. [Codex generation](../../src/providers/codex/generation.rs) and [Cursor runtime](../../src/providers/cursor_runtime.rs) implement different ordering/admission mechanisms outside core. |
| QUALIFICATION_COST | Low deterministic cost for core invariants; medium for proving an adapter’s native ID/ordering contract. Existing mixed-provider fixtures support separation but do not prove all real delivery races. |
| 1_0_BLOCKER | Explicit adapter ordering/correlation contract and repeatable stale-event/recovery family for each supported backend. |
| NOT_A_BLOCKER | Open string identifiers, a finite built-in provider set, or provider-specific generation logic by itself. No universal event enum is needed. |

### B. Provider layer

| Required assessment | Finding |
| --- | --- |
| CURRENT_STRENGTH | Codex capability compatibility, Cursor scoped project ownership, Agy exact title admission; normalized evidence and bounded registry projections. Owned setup/restore/drift refusal remains concrete and auditable. |
| CURRENT_DEBT | Capability, installed, declared, trusted, supported and live-applied facts are spread over diagnostics, registry, settings and adapters. Qualification profiles and support prose can diverge. |
| HARD_CODED_PROVIDER_SEAMS | [registry.rs](../../src/providers/registry.rs) from_diagnostics builds Codex, environment Agy and cwd Cursor directly. [cli.rs](../../src/cli.rs) SetupCommand/UninstallProvider and [main.rs](../../src/main.rs) dispatch remain closed; Cursor also has project-specific management. Capability constructors and [visual_identity.rs](../../src/providers/visual_identity.rs) retain concrete provider policy. |
| QUALIFICATION_COST | High for changed version/config/trust mapping; cheap read-model reuse is possible when wire/ownership risk is unchanged. Exact Agy admission adds per-version source and real-delivery work. |
| 1_0_BLOCKER | One coherent qualified-support read model and a small bounded version intake; no implicit version widening or installed=applied inference. |
| NOT_A_BLOCKER | Concrete TOML/JSON writers, callbacks, manual trust and separate authentication. Generic config mutation would increase risk. |

### C. Presentation

| Required assessment | Finding |
| --- | --- |
| CURRENT_STRENGTH | [presentation_policy.rs](../../src/presentation_policy.rs) is a pure requested→override→capability-limited effective resolver with origins, limitations and ApplicationStatus. Typed [presentation](../../src/presentation/mod.rs), title authority, identity and animation are separated. Cursor color-only proves that title/progress/activity channels can be disabled together. |
| CURRENT_DEBT | ApplicationStatus is supplied by callers; resolver output is not live proof. Capability constants and CliTarget are finite product policy. Human explanations and actual output must remain synchronized. |
| HARD_CODED_PROVIDER_SEAMS | CODEX / AGY_TITLE_ONLY / CURSOR_COLOR_ONLY constants; provider badges; [terminal_color.rs](../../src/terminal_color.rs) validates only codex/cursor owners. |
| QUALIFICATION_COST | Medium/high owned-terminal proof: UIA proves title and frames, not color/progress. Strict absence of title/progress writes needs deterministic byte proof plus real route evidence; Agy uses a different callback title route. |
| 1_0_BLOCKER | Supported same-tab channel transition/reset promises must be bounded and verified. If deferred at 1.0, UX/support must explicitly constrain them; no stale owner may reset a later owner’s color. |
| NOT_A_BLOCKER | TitleMarkBackend and lack of native icons; a broad visual backend rewrite. Current pure resolver is already a useful internal contract. |

### D. Runtime

| Required assessment | Finding |
| --- | --- |
| CURRENT_STRENGTH | One-shot Hook path, bounded [lock budget](../../src/lock_budget.rs), durable generation/leases, state-root isolation, fail-open branches and session-scoped workers. [worker_runtime.rs](../../src/worker_runtime.rs) publishes immutable content-addressed local worker images; [upgrade_preflight.rs](../../src/upgrade_preflight.rs) distinguishes lock ownership instead of killing arbitrary processes. |
| CURRENT_DEBT | Startup, binary hash/copy, locks, child execution and cleanup have different deadlines but lack one standard timing/failure receipt. Crash between color intent and flush conservatively loses reset authority; safe uncertainty can leave decoration behind. |
| HARD_CODED_PROVIDER_SEAMS | Codex worker/stdio/anchor path versus Cursor one-shot terminal route and Agy callback. No evidence that forcing these into one runtime improves correctness. |
| QUALIFICATION_COST | High on Windows process/terminal binding and cold startup; medium deterministic locks/generations. Same-tab handoff is explicitly unexecuted real L4. |
| 1_0_BLOCKER | Reproducible cold/warm timeout margin, owned cleanup/replaceability evidence and cross-provider write/reset races within the advertised envelope. |
| NOT_A_BLOCKER | No global daemon, differing adapter lifecycle implementations, or inability to prove a missing upstream error. Fail-open does not mean decoration correctness is guaranteed. |

### E. Human interface

| Required assessment | Finding |
| --- | --- |
| CURRENT_STRENGTH | Typed [setup](../../src/setup.rs), wizard/Control Center, preview/apply actions, JSON/plain interfaces, status/doctor and [bilingual terminology](../terminology.md). Preferences, capability and trust are named separately. |
| CURRENT_DEBT | Setup discovery remains Codex-shaped; registry builds environment-dependent rows. Saved preferences can be mistaken for active native integration. Candidate-era docs undermine otherwise careful explanations. |
| HARD_CODED_PROVIDER_SEAMS | Provider CLI selection, config previews, management actions and labels; Agy/ Cursor scope differences require visible provider-specific instructions. |
| QUALIFICATION_COST | Low deterministic projection; medium terminal/accessibility proof when visible UX changes. Avoid re-running all Visual for prose-only edits. |
| 1_0_BLOCKER | Consistent unsupported/unproven/applied explanations, stable installation/upgrade guidance and representative mixed-provider user validation. |
| NOT_A_BLOCKER | Finite management commands, English-canonical technical docs or the absence of a remote dashboard. |

### F. Release, CI and evidence

| Required assessment | Finding |
| --- | --- |
| CURRENT_STRENGTH | Exact checkout CI, locked package/dry-run, crate member/VCS verification, ZIP/hash/license inventory, public consumer checks, explicit risk-diff reuse and official-channel source proof. G06/G07 preserve failed attempts and optional non-PASS outcomes. |
| CURRENT_DEBT | [CI](../../.github/workflows/ci.yml) invokes a full code pipeline even for docs. Active docs assertions enforce a stale admission model. Receipts/probes/reviewer prompts accumulate per-head; archival evidence is outside the repo and discoverability is manual. |
| HARD_CODED_PROVIDER_SEAMS | Agy qualification tooling, Codex compatibility/probe scripts, Cursor project harness, Train-specific packet names and support matrix. |
| QUALIFICATION_COST | High initial Train; matrix has 27 task rows and eight cross-feature rows, not 35 independent gates. Many review files demonstrate operational overhead, not measured wasted hours. |
| 1_0_BLOCKER | Repeatable packet verification, release consumer automation and stable current-truth transition. Durable evidence must outlive temporary CI artifacts. |
| NOT_A_BLOCKER | One intentional release closure, manual Owner trust/adoption, or explicitly reused Visual/L4. Publication authority stays human even if preparation automates. |

## POST_V08_NEW_LESSONS

1. **Third-provider test of neutrality:** Cursor adds native-title/color-only
   operation without changing core’s session/evidence types. Mixed-provider
   fixtures and real separate-tab observations support adapter isolation and
   shared workspace identity. They do not prove universal adapter portability.
2. **Resolver contract is useful now:** global inheritance, explicit override,
   capability clamps and applied-state separation work across three unequal
   providers. Stabilize its input/output and explanation fixtures; do not replace
   it with a speculative universal Provider trait. Its caller-supplied application
   fact and closed capability catalog still need qualification metadata.
3. **Shared color is a sound narrow protocol:** write intent revokes old reset
   authority before I/O; only confirmed flush grants the new owner; a short
   shared lock bounds contention. It is not fully generic: explicit codex/cursor
   admission, route proof and crash uncertainty remain. Expand the allowlist only
   with a separately qualified writer, not merely a provider ID.
4. **Config ownership should remain specific:** Codex global Hooks/trust/TOML,
   Agy’s title member and Cursor project Hooks differ in scope and restoration.
   Reuse transaction invariants (snapshot, minimal write, compare/drift refusal,
   restore) and report vocabulary; retain concrete writers and native trust.
5. **Version maintenance is an evidence problem:** Agy 1.2.7→1.2.13 demonstrates
   that one admitted callback contract cannot safely grant arbitrary versions.
   Codex 0.157.1→0.159.0 separates compatible baseline from optional Interrupt
   authority. Exact pinning costs source/schema, binary, trust and real delivery
   review; it is not a blanket argument for exact-version denial.
6. **L4 cost is partly coordination:** release/debug confusion, Owner-stopped
   optional scenarios, isolated authentication/trust, terminal attribution and
   per-head packet repair lengthen a Train. Real delivery/noninterference and
   configuration reversibility still require real proof. Repeating fixtures or
   requesting Owner trust after a docs-only diff adds no equivalent assurance.
7. **Visual value is channel-specific:** exact-tab UIA, working frame sequence,
   correct workspace alias and cleanup are valuable. Color/progress require
   channel-specific visual evidence; title UIA cannot pass them. Desktop/capture
   failures and interactive launch failures are harness/environment classes until
   product causality is established. Preflight once, latch unchanged blockers,
   and never turn every matrix row into a full run.
8. **Release automation has concrete targets:** frozen input manifest, crate
   members/VCS comparison, zip contents/hash/provenance, license inventory,
   anonymous assets/checksum and isolated registry consumer verification. G07’s
   UTF-8-versus-GBK capture correction and same-head CI retry favor reusable
   verifier tooling. Merge/publish/Owner cutover remain explicitly authorized
   boundaries; no automatic production reconciliation is proposed.

### Bounded qualification packet — PLANNING_PROPOSAL

The 1–3 engineering-day target applies to a **compatible version/profile
refresh after infrastructure exists**, with Owner/trust/desktop availability
scheduled. It is not a promise for a new provider, protocol redesign, absent
upstream lifecycle signal, or unauthenticated/blocked environment.

| Time budget | Work and exit |
| --- | --- |
| Day 1 | Pin TabBeacon head/tree/binary, upstream tag/source/executable, exact profile/wire/capability diff, config scope/trust hash, shell/terminal/state-root fingerprint. Select changed risk and reusable evidence. Stop before config mutation if ownership unknown. |
| Day 2 | Deterministic normalizer/order/recovery family; isolated ownership/setup/restore/drift family only if changed; optimized cold/warm Hook/worker/lock timing. One representative owned Visual pack only for changed channels. |
| Day 3 | One attended native-command L4 for new claims; supported transitions, exit and recovery; compare neighbors and same-tab only when claimed. One final required exact-head CI; packet review/support update. Owner blockers remain BLOCKED, not calendar PASS. |

Packet metadata should contain schema, provider/backend/profile, upstream pin,
product head/tree and binary hash, terminal route/host classification, claim→proof
family mapping, old-head/risk-path diff reuse, dispositions, bounded timing,
cleanup/restore result, excluded claims, environment fingerprint and expiry
triggers. Raw payloads, transcripts, tokens, auth and private config are excluded.
Separate immutable observation receipts from a current acceptance index.

Evidence expires for reuse when relevant source, provider wire/capability,
trust/declaration, terminal route or environment changes; age alone warrants
scheduled review, not automatic invalidation. Keep durable minimized release and
qualification summaries with hashes; short-lived CI captures may expire without
erasing the retained claim/proof index. Do not delete historical failures.

## OPERATIONAL_RELIABILITY_REASSESSMENT

Compared with [V08_OPTIONS](../../dev_governance_files/V08_OPTIONS.md), v0.8
already supplies parts of the earlier reliability scope. A new plan should not
charge for implementing these again.

| Item | Disposition | Evidence / remaining bounded work |
| --- | --- | --- |
| Shell/environment fingerprint | PARTIALLY_DONE | Doctor, terminal/probe classification exist; standard sanitized shell/COMSPEC/toolchain/desktop/route packet remains needed. No environment dump. |
| Codex workspace-routing/bootstrap failures | STILL_NEEDED | Workspace anchoring exists; supported direct launch may fail before Hook delivery. Classify bootstrap/launcher/service versus provider delivery; absence of a Hook is not Failed authority. |
| Hook cold/warm timing | PARTIALLY_DONE | One-second production SLA test and preserved debug-failure/release timing evidence. Use optimized binary and native declaration route; add percentiles/sample counts and phase breakdown without changing deadlines blindly. |
| Child/worker startup timing | PARTIALLY_DONE | Worker image publication and bounded runtime/probe exist. Separate spawn/child work/pipe drain/lock/first terminal flush. |
| Lock/contention observability | PARTIALLY_DONE | Bounded lock helper and 100ms color lock; no unified contention receipt. Add bounded counts/deadline categories, not hot-path verbose logs. |
| Cleanup receipts | PARTIALLY_DONE | Leases, cleanup observer and safe image handling exist. Standard ending/superseded/crashed disposition and owned survivor list remain. |
| Binary replaceability | DONE_BY_V080 for mechanism; PARTIALLY_DONE for packet | Immutable worker images and preflight; G07 replacement found no known lock. That one observation does not qualify every live-worker upgrade. Do not add arbitrary process killing. |
| Provider version drift | NEW_DEBT in scale | Third provider and multiple Agy/Codex observations need an intake/reuse decision record. Preserve exact Agy and capability-based Codex boundaries. |
| Visual harness preflight | PARTIALLY_DONE | Typed desktop/UIA/capture blockers already exist. Standard route ownership and availability check before any expensive pack; separate title-only from color/progress prerequisites. |
| Runner/capture/environment classification | PARTIALLY_DONE | Quality Gates failure taxonomy and typed preflight exist. G06 timing retry/G07 encoding correction show integration classification needs a reusable packet. |
| Evidence retention/expiry | STILL_NEEDED | Durable Train artifacts exist but are locally organized; introduce indexed minimal custody and risk-trigger invalidation. Avoid blanket chronological expiry. |
| Requalification intake | STILL_NEEDED | Scattered profile/probe/qualification scripts; source-to-native-delivery checklist and stop/reuse decision are not one product-wide packet. |
| Release consumer verification | DONE_BY_V080 execution; PARTIALLY_DONE automation | Registry consumer, 12 public assets, ZIP smoke, provenance and Cargo source proof passed historically. Standardize verifier, encoding and immutable input manifest. |
| Global diagnostics daemon / automatic repair | NO_LONGER_NEEDED as default approach | Existing one-shot, bounded diagnostics and lease model suffice for this scope; no evidence justifies changing the direct-command baseline. |

Compressed v0.9 candidate: **Reliability & Qualification v2** owns intake/packet,
timing/failure classification, Visual preflight integration, owned cleanup/upgrade
receipt and release verifiers. It consumes current mechanisms instead of rebuilding
workers or all setup. Exit proposal: independently reproducible packet, one
compatible refresh completed in 1–3 engineering days, one failure-classification
exercise, no widened provider promise, and correct truth after release. Real
provider configuration/trust remains separately attended/authorized.

## PROVIDER_PLATFORM_REASSESSMENT

| Old seam | v0.8 outcome | Minimal next seam / disposition |
| --- | --- | --- |
| ProviderRegistry construction | STILL_NEEDED: third concrete row added; open ID did not remove environment-aware construction. | Supply bounded adapter observations to common snapshot assembly; environment probes stay in concrete adapters. |
| Setup enum / uninstall dispatch | STILL_NEEDED, intentionally finite. | Shared read-only action descriptions and safety receipts; keep typed explicit concrete mutations. No dynamic writer registry needed. |
| Capability projection | PARTIALLY_DONE: common status/authority/readiness model; separate provider projections remain. | Add qualified claim metadata and consistent installed/trusted/applied separation. |
| Version/profile admission | PARTIALLY_DONE: capability-based Codex; exact Agy retained/extended narrowly. | Common qualification identity and revalidation triggers; individual admission decisions stay provider-specific. |
| Config ownership | PARTIALLY_DONE: guarded import/global override/restore/drift work improved safety; format/scope remain specific. | Reuse invariant fixtures and minimal receipt vocabulary, not a universal JSON/TOML writer. |
| Presentation capabilities/policy | LARGELY_DONE: pure resolver across three channel envelopes. | Freeze internal contract and callers’ authority; catalog capability inputs from qualified metadata. |
| Provider identity | PARTIALLY_DONE: separate visual identity exists; provider badges/labels stay concrete. | Small static descriptors, unknown-provider safe fallback; no dynamic branding or icon backend. |
| Qualification tooling | STILL_NEEDED: provider-specific scripts and Train packets. | Common minimized packet verifier invoking concrete probes; no mandatory full matrix per version. |

Proposed minimum descriptor: stable ID/label/backend, declared semantic/channel
capabilities, qualified profile/source identity, safe readiness/action explanation,
evidence requirements and expiry triggers. Registration cannot grant support.
The core consumes evidence as today; the resolver consumes proved channel facts.

Do not generalize trust approval, authentication, callback installation, config
ownership or lifecycle authority mapping. These are the places where provider
differences are safety boundaries. “Platform-ready” means the three existing
adapters migrate without behavior/ownership regression and one packet can explain
their unequal claims. It does not require a plugin ecosystem or万能 Provider trait.

## ISSUE_114_DISPOSITION_RECOMMENDATION

```text
ISSUE_114_DISPOSITION=CLOSE_AS_SATISFIED_BY_LATER_WORK
ISSUE_114_ACTION_TAKEN=NONE
```

The recommendation follows technical and acceptance facts, not stale issue prose:

| Acceptance / tracked goal | Verified fact |
| --- | --- |
| Remove affected lru line | Release Cargo.lock has one `lru`, 0.18.5, replacing 0.12.5; ratatui 0.30.2 and ratatui-core 0.1.2. |
| Reach stricter safe line | [RUSTSEC-2026-0002](https://rustsec.org/advisories/RUSTSEC-2026-0002.html) and [RUSTSEC-2026-0253](https://rustsec.org/advisories/RUSTSEC-2026-0253.html) checked live; 0.18.5 satisfies the tracked remediation threshold (>=0.18.2). |
| chacha20 / paste disposition | chacha20 is 0.10.2; no package named paste remains in the release lock. |
| Findings classified | R7: zero vulnerabilities, empty warnings on the Windows x64 lock at c1ff551; release receipts explicitly reuse it across empty dependency diff. This is September 30 evidence, not a fresh full-database audit. |
| Tests, lint, formatting, hosted CI | G06 records final exact-source CI 36727664020 attempt 2; R4 confirms merge CI success. Pipeline includes clippy, fmt, all-target/all-feature tests, release locked Hook SLA and locked build. It is broader test coverage, not an independently repeated `cargo test --locked` invocation in this audit. |
| Preserve runtime/provider behavior | Original maintenance purpose survives: later v0.8 intentionally adds product behavior, while scoped qualification/configuration/Visual evidence remains recorded. Do not claim the entire 0.7.3→0.8 diff is behavior-neutral. |
| Separate release recommendation | v0.8 publication was separately authorized and completed; closing maintenance debt does not authorize another release. |

Owner may accept closure on superseding technical outcome plus later release
qualification. If literal behavior-neutral maintenance or a separately recorded
locked-test command is required, mark that narrow criterion UNPROVEN and attach
the relevant existing CI evidence before closure. Do not reopen a whole dependency
Train solely to manufacture a new checkpoint. No issue mutation occurred.

## ISSUE_116_DISPOSITION

```text
ISSUE_116_DISPOSITION=OPEN_KNOWN_LIMITATION
ISSUE_116_ACTION_TAKEN=NONE
DETECTION_PASS=false
```

Current TabBeacon normalizer maps Stop to result-ready and admitted Interrupt to
Interrupted; tool lifecycle does not assert Health::Warning/Failed. No authoritative
original404 evidence route was added. The version/path-specific release finding
remains unchanged. A healthy-looking tab is not an assurance of backend health.

Current [official Hook docs](https://developers.openai.com/codex/hooks) describe
Interrupt and session/turn lifecycle, but no general failed-turn/error Hook that
establishes this issue’s missing outcome. Hook-generated systemMessage or hook
execution failure is not proof of an agent backend failure.

Pinned official source discovery adds useful information: both
[0.157.1 Hook lib](https://github.com/openai/codex/blob/rust-v0.157.1/codex-rs/hooks/src/lib.rs)
and [0.159.0 Hook lib](https://github.com/openai/codex/blob/rust-v0.159.0/codex-rs/hooks/src/lib.rs)
declare 12 event names including Interrupt. Local 0.159 baseline being 11 events
does **not** prove upstream removed Interrupt. It reflects TabBeacon’s admitted
capability/path boundary; the existing exact Interrupt capability condition admits
0.156.1/0.157.1. Source presence is grounds for bounded requalification research,
not automatic support or health detection. [0.159.0 Stop request](https://github.com/openai/codex/blob/rust-v0.159.0/codex-rs/hooks/src/events/stop.rs)
still is not a general backend failed-turn outcome callback.

[App-server docs](https://developers.openai.com/codex/app-server) and
[pinned turn protocol](https://github.com/openai/codex/blob/rust-v0.159.0/codex-rs/app-server-protocol/src/protocol/v2/turn.rs)
offer typed turn status/error research surfaces. An independent app-server client
owning a different thread does not observe the Owner’s native `codex` session.
No supported passive bridge to that native session was established here.

Recommended ordering: research a supported native-command structured outcome or
observer bridge/upstream proposal first; consider app-server only if it can retain
literal daily command, ownership, minimal content and no hidden daemon/wrapper.
Reopen implementation only with a pinned supported signal proving failure class,
session/turn identity, delivery/order and recovery; deterministic fixtures plus
one original-command real L4 must show it. Never parse terminal text, ANSI,
transcripts, tool output or arbitrary error strings to recover missing authority.

## PROVIDER_ECOSYSTEM_REFRESH

This is architecture fit from official sources retrieved October 1, not installed
TabBeacon support. None was executed or admitted. Every adapter would discard
prompt/result/tool content and emit no permission decisions. All require native
terminal-route proof, isolated ownership/restore and bounded native-command L4.
Async delivery or stop-continuation can invalidate naive “stop means ready”.

### Lifecycle and identity fit

| Candidate / daily command | Structured lifecycle, identity, tool and permission | Result / idle / stop authority and risks |
| --- | --- | --- |
| Claude Code / `claude` | Official Hooks have session_id/cwd and prompt correlation; tool, permission, subagent, session and stop surfaces. | Stop and failure-specific surfaces are candidates, not universal success signals. Async hooks and continuation require generation/root filtering. Tool failure is not whole-session failure. |
| Gemini CLI / `gemini` | JSON stdin carries session_id/cwd/timestamp; Before/AfterTool and Before/AfterAgent; SessionStart/End; permission Notification is an alert. | AfterAgent is a candidate response boundary; hooks can retry/block, so observed final response is not unconditional settled result. Notification alone needs exact semantic/real delivery proof. |
| GitHub Copilot CLI / `copilot` | Current official reference includes sessionId/cwd, pre/post tool, permissionRequest, sessionStart/End and subagent surfaces. | agentStop supplies a turn-end candidate; errorOccurred has context/recoverable metadata. PermissionRequest is before native permission evaluation, not proof a user prompt is visible. Stop can force continuation. |
| Qwen Code / `qwen` | Hook schema includes session_id/cwd, tool, permission and subagent fields plus session/stop/notification events. | Stop and SessionEnd provide candidates; notifications distinguish permission/idle. Async SessionEnd may outlive CLI: delayed writes must not reset a newer terminal owner. |
| OpenCode / `opencode` | Plugin events include session lifecycle/status/error and permission asked/replied; tool before/after. Plugin context exposes directory/worktree; SDK/server supplies typed session identities. | session.idle/status/error are richer candidates; exact root session correlation and terminal binding must be proved. Server/plugin lifetime differs from one-shot command Hooks; idle is not automatically result-ready. |

Sources: [Claude Hooks](https://code.claude.com/docs/en/hooks),
[Gemini reference](https://geminicli.com/docs/hooks/reference/),
[Copilot reference](https://docs.github.com/en/copilot/reference/hooks-reference),
[Qwen Hooks](https://qwenlm.github.io/qwen-code-docs/en/users/features/hooks/),
[OpenCode plugins](https://opencode.ai/docs/plugins/) and
[server](https://opencode.ai/docs/server/). Field presence in docs is insufficient
to establish ordering or live terminal authority. Pin exact versions before admission.

### Installation safety and qualification fit

| Candidate | Config scopes / trust / Windows | Ownership reversibility and qualification difficulty (estimate) |
| --- | --- | --- |
| Claude Code | User/project/local/managed settings; native configuration/security approval must be retained. Windows native installation is documented. | Own only added Hook entries; preserve managed policies and foreign hooks. Medium: good lifecycle shape, Windows shell quoting/native-title/async proof remains. |
| Gemini CLI | User/workspace/system settings and enabled folder-trust controls; exact Hook-specific review/reload trust semantics remain UNPROVEN here. Official installation lists Windows 11 24H2+ and PowerShell. | Snapshot owned entry/hash; fail on drift, restore only owned configuration. Medium: permission/final-result semantics and parallel groups need mapping. |
| Copilot CLI | Repository/user Hook files, inline settings and policy scopes; native Hook permission, tool and organization policy apply. Windows requires PowerShell 6+. | Exact-owned file/member and foreign-content preservation; distinguish CLI from cloud/IDE Hook contracts. Medium: strong documented outcome surface, precise permission timing and shell routing need proof. |
| Qwen Code | Project/user/system/extension hooks; project hooks depend on trusted folder, user hooks do not. Windows install documented. | Own one declaration, retain folder/native trust. Medium: broad schema but multi-scope ordering and exit-time async need qualification. |
| OpenCode | Project/global config/plugin directories; executable plugin loading creates a larger code/dependency trust boundary. Native Windows installation available; docs recommend WSL. | Remove exact-owned local plugin/config only; avoid automatic npm plugin supply-chain broadening. Medium-high: SDK/server lifecycle, native Windows route and optional WSL add qualification surfaces. |

Config/platform sources: [Claude settings](https://code.claude.com/docs/en/settings)
and [setup](https://code.claude.com/docs/en/setup);
[Gemini configuration](https://geminicli.com/docs/reference/configuration/) and
[installation](https://geminicli.com/docs/get-started/installation/);
[Copilot Hook use](https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/use-hooks)
and [installation](https://docs.github.com/en/copilot/how-tos/copilot-cli/set-up-copilot-cli/install-copilot-cli);
[Qwen quickstart](https://qwenlm.github.io/qwen-code-docs/en/users/quickstart/);
[OpenCode config](https://opencode.ai/docs/config/) and [intro](https://opencode.ai/docs/).
Authentication remains native/Owner-owned; this audit inspected no credentials.
Where docs do not establish trust reload/restore or exact ordering, those claims
remain UNPROVEN until pinned source and isolated qualification.

```text
BEST_FUTURE_PROVIDER_CANDIDATES=Claude_Code,Gemini_CLI,GitHub_Copilot_CLI
SECONDARY_ARCHITECTURE_FIT=Qwen_Code,OpenCode
BUT_DO_NOT_RANK_AS_NEXT_IMPLEMENTATION_UNLESS_PLATFORM_READY=true
NEW_PROVIDER_ADMITTED=false
```

The shortlist is a fit judgment, not implementation priority. Copilot’s current
documented IDs/error surface improves its research fit compared with older
missing-ID assumptions. Claude/Gemini offer direct Hook-shaped integration;
Qwen is also plausible. Owner usage, maintainable native Windows qualification,
signal authority and reversibility should choose the next candidate only after
packet/read-model readiness. New provider proof is optional for 1.0 if 1.0 is
explicitly the present bounded product; it becomes mandatory if “provider platform”
extensibility is advertised as a supported product promise.

## NATIVE_ICON_DISPOSITION and terminal reach

```text
NATIVE_ICON_DISPOSITION=NO_GO
TERMINAL_BACKEND_ADDED=false
```

Current public [profile icon settings](https://learn.microsoft.com/en-us/windows/terminal/customize-settings/profile-general),
[actions](https://learn.microsoft.com/en-us/windows/terminal/customize-settings/actions),
[progress protocol](https://learn.microsoft.com/en-us/windows/terminal/tutorials/progress-bar-sequences)
and [upstream releases](https://github.com/microsoft/terminal/releases) were
checked. No supported child-process bridge to dynamically change the native tab
icon was established. Profile/media/icon picker improvements do not establish
such a bridge. This is a bounded negative finding, not proof that no unpublished
implementation exists. Retain [NO_GO](../design/native-tab-icon.md) and title marks;
do not reopen XAML Diagnostics, attachment, private ABI or terminal mutation.

Terminal reach proposal: first inventory title/color/progress/reset capabilities
and owned-route proof per terminal, including native Windows versus WSL/remote.
Shared ANSI title support does not imply WT palette or progress compatibility.
One isolated capability hypothesis may follow reliability/platform readiness;
no alternate terminal support, backend or broad portable claim is admitted here.

## ONE_POINT_ZERO_MODEL

| Stage | Completion meaning | Current status |
| --- | --- | --- |
| FOUNDATION | Neutral state, offline identity, typed presentation and fail-open integration. | Achieved. |
| DAILY_USABLE | Official installation, ownership-safe setup, useful observed states and honest limits. | Achieved within v0.8 support scope. |
| MULTI_PROVIDER_STABLE | Existing supported providers have repeatable version intake, cleanup/upgrade and advertised mixed-provider transitions. | Partial; separate-tab operation proved, optional Agy/same-tab and upgrade repeatability remain bounded gaps. |
| PLATFORMIZING | Shared qualified read model/policy/packets; concrete safe adapters; one reusable admission path. | Partial mechanisms present; not completed as a platform. |
| 1_0_READY | Chosen support promise reproducibly qualified, reliable operations/release/upgrade, truthful docs and owned long-term debt disposition. | Not reached. |

Proposed 1.0 acceptance is a checklist, not a percentage:

- Existing provider promises are pinned and stable; narrower Agy support is
  explicit, including what Ready/native switching cannot claim. Optional claims
  need real proof before being promoted; they need not be forced into 1.0.
- Compatible version qualification repeats in a bounded packet; two independent
  executions demonstrate it is a process rather than a single lucky receipt.
- Adapter read model has no authority-through-registration; resolver and native
  config ownership invariants survive migration and rollback.
- Native-command Hook latency, owned worker/cleanup, lock contention and upgrade
  replaceability have representative release-build evidence and diagnostics.
- Release preparation/consumer checks are reproducible; publication and daily
  adoption remain deliberately authorized; current docs match public support.
- Mixed-provider UX explains effective versus saved settings and actual state
  authority; advertised same-tab behavior is proved or clearly excluded.
- #114 has a factual maintenance disposition. #116 stays explicit: if 1.0
  promises trustworthy failure monitoring, lack of a lawful signal is a blocker;
  if 1.0 promises only supported lifecycle decoration, Owner may accept it as a
  documented limitation. Never market green as proof of backend health.
- New-provider proof is required only for a chosen extensible-platform promise;
  count of providers alone does not establish maturity. Native icon remains outside
  1.0 completion, as does terminal expansion without qualification.

## NEXT_ROADMAP_OPTIONS

All options start after reconciled truth and a new Owner-admitted scope. They
are alternatives; no task IDs, Train or implementation authority is created.

| Option | Objective and scope | Dependencies / risk | Estimated effort and deliberate deferrals | Path to 1.0 |
| --- | --- | --- | --- | --- |
| A — Reliability-first | Requalification packet/intake, timing and environment classification, integrate existing preflight, owned cleanup/upgrade receipts, release verifiers. One compatible-version rehearsal; fix only evidenced reliability defects in later admitted work. | Reconciled docs; minimized retained receipts; scheduled attended native-command window. Medium risk: diagnostic overhead and process ownership must stay bounded. | 15–25 engineering days (3–5 working weeks); defer new provider, universal trait, visual redesign, terminal/icon expansion and #116 implementation without signal. | Makes existing promises sustainable; small read-model extraction can then reuse packet evidence. |
| B — Platform-first | Static descriptor/read model, common capability/qualification metadata and action explanations; preserve resolver; move three observations behind it; concrete mutation adapters remain. | Reconciled truth and agreed claim model; needs at least minimal qualification packet first. Medium-high risk: accidental supported/applied/authority conflation during migration. | 20–30 engineering days (4–6 working weeks), including migration proof; defer new provider, dynamic plugin loading, generic config writer, operational instrumentation overhaul. | Reduces duplicated projections but does not alone shorten environment/trust/L4 waits. |
| C — Product-expansion-first | Choose one user-facing slice: mixed-provider explainability/UX or one new provider pilot; scope claim set before coding. | Packet and read-model readiness mandatory; cannot bypass A/B prerequisites. High risk if scheduled immediately: adds fresh config/trust/native-route/ordering and Visual cost. | After prerequisites: UX 10–15 days; one new provider 15–25 days (2–5 weeks depending on slice). Prerequisites add roughly 20–35 days; do not double-count reused infrastructure. Defer broad provider catalog, dashboard, native icons and multi-terminal launch. | Can validate extensibility/user value, but reaches 1.0 only after supported operation/release hardening. |

## RECOMMENDED_SEQUENCE and ESTIMATED_TIMELINE

Recommendation is A with a bounded part of B, because v0.8 already validated the
core and resolver while qualification/current-truth overhead remains visible.
Platform-first is appropriate only if Owner prioritizes public extensibility over
upgrade reliability; expansion-first has insufficient current infrastructure.

| Proposed sequence | Effort estimate | Decision / exit |
| --- | --- | --- |
| Post-v0.8 reconciliation | 1–2 days | Own active docs/assertions; public 0.8 stable, no feature admission. |
| Reliability & Qualification v2 | 15–25 days | Repeatable packet, classification and verifiers; compatible refresh rehearsal. Candidate v0.9 theme, no new provider required. |
| Minimal Provider Platform v2 | 8–15 incremental days after A | Qualified read model/static metadata and consistent management explanations; reuse unchanged runtime evidence. This is smaller than B as a standalone migration. |
| Mixed-provider UX or one new provider | 10–25 days | Owner chooses product need only after platform/packet exit; optional Agy qualifications separately attended. |
| 1.0 hardening and deliberate release | 10–15 days | Supported envelope, upgrade/release consumers, documentation, known debt and final scoped gates. |

**Estimated v0.9:** 3–5 engineering weeks for A; allow **4–6 elapsed weeks**
including a scheduled qualification window and deliberate release closure.
If both A and the minimal platform migration are in v0.9, estimate **5–8
engineering weeks**, rather than silently promising both inside the A budget.

**Estimated path to 1.0:** about **9–16 engineering weeks** including reconciliation,
A, minimal platform, one product slice and hardening; roughly **11–20 elapsed
weeks** with Owner/runner windows. These are single-engineer planning ranges,
assuming existing host/toolchain and reusable proofs, not a measured throughput
forecast. Blocked authentication/trust, upstream #116 authority, missing desktop
or provider protocol redesign can extend elapsed time without a finite estimate.
Choosing a bounded present-provider 1.0 and deferring expansion reduces the path
by the 10–25-day product slice. No percentage maturity score is assigned.

## OWNER_DECISIONS_REQUIRED

1. Admit a separate post-release reconciliation Goal and its exact docs/checker
   scope; this report does not perform that reconciliation.
2. Choose v0.9 A, B or a prerequisite-compliant C; decide whether minimal platform
   extraction fits v0.9 or follows it. No ROADMAP_V09 exists from this audit.
3. Define 1.0’s support promise: bounded lifecycle decoration versus authoritative
   abnormal-outcome monitoring; that choice determines whether #116 blocks 1.0.
4. Accept/decline #114 closure on superseding technical outcome and scoped later
   qualification, with any literal criterion needing additional existing evidence.
5. Decide whether and when to attend Agy Ready, Agy preserve-native and same-tab
   handoff follow-ups; unchanged Owner stops are not requests for automatic rerun.
6. Choose any future provider from usage and maintainable qualification, after
   infrastructure readiness; native icon and terminal expansion remain deferred.
7. Choose durable minimized evidence custody/retention and a scheduled Owner
   qualification window. No production action is authorized by this decision list.

## Validation and stop boundary

Risk vector: CODE_CHANGED=false, PRESENTATION_CHANGED=false,
PROVIDER_CHANGED=false, USER_PERSISTENT_CONFIG_CHANGED=false,
SECURITY_OR_PRIVACY_CHANGED=false, RELEASE_BOUNDARY=false. Under QUALITY_GATES
this change requires L0 docs/diff/link/privacy sanity. Rust, Visual and L4 are N/A
for the audit; historical evidence above remains scoped, not newly executed.

Run repository check-docs, changed-file Markdown/link/fence checks, diff whitespace,
secret/path scan and exact-scope verification. A passing legacy checker is recorded
as syntactic/contract PASS alongside POST_RELEASE_GOVERNANCE_DRIFT; its emitted
STALE_CURRENT_RELEASE_MARKERS=0 is not accepted as public-truth proof. Review must
run in a separate native Codex process with explicit read-only permission; a
same-process supervisor role alone is insufficient isolation. Final review/CI
receipts bind to the audit candidate and live outside this report to avoid a
self-referential commit hash.

Draft PR only. No merge, issue creation/closure, roadmap admission, configuration
mutation or v0.9 implementation follows. Owner discussion is the terminal handoff.
