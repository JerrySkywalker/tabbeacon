# v0.8 option set — non-authoritative planning input

- Inputs: [`POST_V07_PRODUCT_AUDIT_2026-08.md`](../docs/research/POST_V07_PRODUCT_AUDIT_2026-08.md)
- Recorded: 2026-08-28
- Status: options only; this file is **not** `ROADMAP_V08.md` and does not authorize implementation

## Guardrails common to every option

```text
DAILY_COMMAND_CODEX=codex
DAILY_COMMAND_AGY=agy
FAIL_OPEN=true
NO_WRAPPER=true
NO_PATH_SHADOW=true
NO_PTY_HOST=true
GLOBAL_DAEMON_ADDED=false
NEW_PROVIDER_ADDED=false
CLAUDE_PROVIDER=DEFERRED
OPENCODE_PROVIDER=DEFERRED
NATIVE_TAB_ICON_DISPOSITION=NO_GO
PUBLIC_RELEASE_MUTATION=false
```

Each option needs a fresh admitted Goal with an exact source head, explicit
write boundary, risk vector, acceptance evidence, and deferrals. This document
does not replace that admission.

## Option 1 — Provider Platform v2

| Aspect | Planning definition |
| --- | --- |
| Product objective | Turn current two-provider construction into an adapter/qualification platform without adding a third provider. |
| User value | Consistent capability explanations, safer future onboarding, and fewer provider-specific surprises in the Control Center. |
| Architecture changes | Split provider registration, visual identity, capability declarations, setup/uninstall ownership contracts, and qualification evidence from the current Codex/Agy branches. Preserve concrete adapters. |
| Dependency sequence | 1. Map current hard-coded seams. 2. Define an internal adapter manifest/read model. 3. Move Codex and Agy projections behind it with no behavior change. 4. Prove configuration ownership/restore equivalence. 5. Qualify a future provider only under a separate Goal. |
| Risk | High: abstraction could accidentally weaken exact ownership, admission, capability authority, or direct-command invariants. |
| Estimated effort | 4–7 weeks including migration evidence and focused provider safety review. |
| Explicitly deferred | New provider, generic config writer, automatic trust, dynamic provider branding, native icons, remote dashboard. |

## Option 2 — Operational Reliability v2

| Aspect | Planning definition |
| --- | --- |
| Product objective | Make shell-startup/runtime probe, worker launch, cleanup, upgrade replaceability, and visual-harness outcomes observable, bounded, and repeatable. |
| User value | Fewer unexplained `UNPROVEN` diagnostics, safer upgrades, clearer recovery instructions, and more trustworthy dogfood evidence. |
| Architecture changes | Add only bounded diagnostic/evidence seams: environment classification, timing decomposition, worker-image/lease cleanup receipts, harness preflight health, and retention taxonomy. No new runtime behavior until separately admitted. |
| Dependency sequence | 1. Freeze timing/cleanup terminology and safe receipt schema. 2. Measure current baselines. 3. Separate product, shell, runner, and external failures. 4. Improve owned cleanup/replaceability evidence. 5. Retain one representative exact-head operational pack. |
| Risk | Medium: process and timing work can regress Hook latency or accidentally overreach process ownership. |
| Estimated effort | 3–5 weeks. |
| Explicitly deferred | New provider, public release, terminal expansion, XAML Diagnostics, provider-configuration redesign. |

## Option 3 — Multi-Agent Presentation UX v3

| Aspect | Planning definition |
| --- | --- |
| Product objective | Improve mixed Codex/Agy/unknown presentation, badges, explainability, accessibility, and Control Center comprehension from proven state only. |
| User value | Quicker recognition of provider, state authority, unsupported capabilities, and why a tab has a particular appearance. |
| Architecture changes | Extend typed presentation explanations, accessible labels, provider-badge policy, multi-provider fixture coverage, and the owned UIA/Visual oracle. |
| Dependency sequence | 1. Complete Provider Platform v2 read-model seam. 2. Research badge/accessibility usage. 3. Add fixtures/oracles. 4. Implement one bounded presentation slice. 5. Run final owned Visual proof. |
| Risk | Medium: visible semantics can overstate provider capability or impose unnecessary visual-CI cost. |
| Estimated effort | 3–5 weeks after platform/read-model stabilization. |
| Explicitly deferred | New lifecycle claims, generic provider integration, native icon, terminal replacement, dashboard. |

## Option 4 — Distribution / Terminal Reach

| Aspect | Planning definition |
| --- | --- |
| Product objective | Reduce installation/upgrade friction and clarify supported terminal reach without turning TabBeacon into a launcher or mutating user environments implicitly. |
| User value | More predictable package provenance, upgrade guidance, release evidence, and an honest support boundary. |
| Architecture changes | Package/consumer verification improvements, docs/diagnostics reach matrix, and potentially a separately admitted terminal capability abstraction. |
| Dependency sequence | 1. Finish Operational Reliability v2 replaceability evidence. 2. Re-admit package/release consumer flow. 3. Identify one terminal-reach hypothesis. 4. Build an isolated fixture. 5. Consider product support only after a separate release decision. |
| Risk | Medium to high: distribution is one-way and terminal surfaces are host/version dependent. |
| Estimated effort | 3–6 weeks, excluding any public release train. |
| Explicitly deferred | Auto-update, shell/profile modifications, alternate-terminal claim, native icon, new provider. |

## Option 5 — `WINDOWS_TERMINAL_UPSTREAM_NATIVE_ICON` experiment

| Aspect | Planning definition |
| --- | --- |
| Product objective | Seek a supported public Windows Terminal child-process tab-icon bridge upstream, without building a TabBeacon native-icon backend. |
| User value | Potentially unlocks a future safe decoration path if upstream supplies a stable API. It has no near-term product dependency. |
| Architecture changes | None in TabBeacon. Prepare an upstream-compatible problem statement, use case, protocol/API proposal, or isolated contribution proof only. |
| Dependency sequence | 1. Re-check public upstream state and issue #1868. 2. Obtain explicit upstream-contribution/Owner scope. 3. Perform public-source/proposal work. 4. Re-admit TabBeacon only if a documented public bridge materially lands. |
| Risk | Low to TabBeacon while kept upstream-only; high schedule uncertainty and no control over upstream acceptance. |
| Estimated effort | 1–2 weeks for an upstream proposal; unbounded for upstream delivery. |
| Explicitly deferred | XAML Diagnostics, process attachment, local Windows Terminal mutation, `IconSource` writes, TabBeacon native-icon implementation. |

## Recommended order

1. **Operational Reliability v2** — it closes current dogfood evidence gaps without broadening provider or Terminal authority.
2. **Provider Platform v2** — it should follow a reliable operational baseline, and it creates a safer internal boundary before any new provider is contemplated.
3. **Multi-Agent Presentation UX v3** — it benefits from the platform read model and demands a focused owned Visual proof.
4. **Distribution / Terminal Reach** — it is more credible after replaceability and artifact-retention work is settled.
5. **`WINDOWS_TERMINAL_UPSTREAM_NATIVE_ICON`** — keep it non-blocking and upstream-only; it must not gate the product train.

```text
V08_OPTIONS_WRITTEN=true
RECOMMENDED_V08_THEME=Operational Reliability v2
RECOMMENDED_FIRST_ADMISSION=TB-V08-OPERATIONAL-RELIABILITY-V2-ADMISSION-001
CLAUDE_PROVIDER=DEFERRED
OPENCODE_PROVIDER=DEFERRED
NATIVE_TAB_ICON_DISPOSITION=NO_GO
```
