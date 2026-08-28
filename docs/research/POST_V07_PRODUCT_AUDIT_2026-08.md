# Post-v0.7 product, architecture, and ecosystem audit

- Goal: `TB-POST-V07-BRAND-REPAIR-V08-AUDIT-001`
- Audited repository baseline: `origin/main` `0aacf549c809d7f6c7d4610e1dc416c97ac8b491`
- Recorded: 2026-08-28
- Disposition: planning audit; no v0.8 feature, provider, runtime, user-configuration, Terminal, or release mutation

## Scope and method

This audit separates current product truth from retained historical evidence.
Source code, Rustdoc, public documentation, architecture records, CI/release
materials, and the published upstream documentation cited below were read. A
historical receipt was never changed. Where an upstream document did not prove a
field needed by TabBeacon, the field is marked `UNPROVEN`, not inferred from a
similarly named hook.

```text
CURRENT_PUBLIC_RELEASE=v0.7.0
PRODUCTION_PROVIDERS=codex,agy-1.1.19
CLAUDE_PROVIDER=DEFERRED
OPENCODE_PROVIDER=DEFERRED
NEW_PROVIDER_ADDED=false
RUNTIME_BEHAVIOR_CHANGED=false
PUBLIC_RELEASE_MUTATION=false
```

## Brand repair

The existing TabBeacon mark was reproduced and inspected. No independent mark
defect was observed, so its geometry remains unchanged. The former wordmark
used individually drawn paths with inconsistent cap heights and baselines; its
`C` ended at x=695 while `O` began at x=675, a 20-unit overlap. Correcting only
that pair would have left the rest of the spacing system inconsistent.

`tabbeacon-logo.svg` now defines local vector glyphs on a deterministic grid:

| Contract | Value |
| --- | ---: |
| Grid unit | 12 |
| Glyph cell | 60 × 84 |
| Cap line | y=48 in the logo viewBox |
| Baseline | y=132 in the logo viewBox |
| Advance | 76 |
| Explicit inter-glyph gap | 16 |
| `TABBEACON` cell ranges | `[0,60]`, `[76,136]`, `[152,212]`, `[228,288]`, `[304,364]`, `[380,440]`, `[456,516]`, `[532,592]`, `[608,668]` |

The intervals are closed, non-overlapping, and each glyph path stays within its
own local 60 × 84 cell. There is no text element, font dependency, embedded
font, script, external URL, or external SVG content. The committed browser
render proof is
[`tabbeacon-logo-render-evidence.png`](../assets/brand/tabbeacon-logo-render-evidence.png),
reproducible from its adjacent HTML render sheet. It shows light and dark
surfaces at 210 px, README hero 420 px, and 840 px.

```text
WORDMARK_CAP_HEIGHT_UNIFORM=true
WORDMARK_BASELINE_UNIFORM=true
GLYPH_OVERLAP_COUNT=0
WORDMARK_SPACING_REVIEW=PASS
README_HERO_LOGO=PASS
LOGO_RENDERER=Edge-headless
RENDER_VIEWPORT=2000x2200
RENDER_EVIDENCE_SHA256=d2eaf0e2ff04180ac1d9ffbabf9e28349c37091b45cd621aac89c446f158967a
```

## Current-truth repair

Fourteen distinct active statements were stale because they still described the
pre-G64 period as if it were current. They are corrected below. Historical
G64 evidence, release notes, and receipts keep their original facts.

| Surface | Stale current implication | Current correction |
| --- | --- | --- |
| `src/providers/mod.rs` | Codex Hooks were the only production adapter. | Codex Hooks and the exact admitted Agy 1.1.19 title callback are production adapters; historical qualification code is isolated. |
| `src/providers/registry.rs` | The constructor was called the only v0.5.1 production registration. | It is now documented as a Codex observation plus an initially-unadmitted Agy row; `from_diagnostics` overlays an Agy production readiness probe. |
| `src/cli.rs` setup Rustdoc | Direct setup implied Codex only. | Direct setup is documented for a supported provider. |
| `src/cli.rs` qualification Rustdoc | The `agy` utility family and its plan remained described as perpetually pre-admission/future. | They are labelled historical/disposable utilities that cannot alter production setup. |
| `src/providers/agy.rs` Rustdoc | The qualification module and provider identifier claimed a future G64 admission. | The module is explicitly historical; its provider identifier is shared with the production adapter. |
| `src/providers/agy_backend.rs` Rustdoc | Six active docstrings described the backend, normalizer, capability gate, readiness projection, and user-global setup surface as future or unadmitted. | They now distinguish the historical qualification path from the admitted exact 1.1.19 production path; bounded rejection and workspace-local refusal remain explicit. |
| `docs/architecture.md` | The active provider model named Codex/Claude/OpenCode and omitted the admitted Agy backend. | It names Codex Hooks and the exact Agy 1.1.19 structured title callback, while retaining Claude/OpenCode as deferred. |
| `docs/agy-preadmission.md` | A retained preparation document presented its `Codex only` state as current. | A prominent historical-record notice links readers to the live Agy setup and ADR 0015; original checkpoint facts are retained as historical. |
| `docs/adr/README.md` | ADR 0014’s conditional pre-G64 wording could be read as current availability. | The index now records that G64 was accepted and points to ADR 0015 as current production truth. |

```text
CURRENT_TRUTH_STALE_FINDINGS=14
CURRENT_TRUTH_FIXED=14
HISTORICAL_RECEIPTS_REWRITTEN=false
```

## Actual product structure

| Layer | Current responsibility | Neutrality assessment |
| --- | --- | --- |
| Core evidence and reconciliation | `AgentProvider`, `AgentSessionKey`, `AgentEvidence`, `StatePatch`, authority, freshness, deterministic winners. | Provider-neutral. Raw provider event types do not cross this boundary. |
| Provider registry | Read-only Integrations/Control Center projection and bounded capabilities. | Partially neutral: `ProviderId` is open, but construction explicitly creates Codex and Agy rows and projects their capabilities in per-provider code. |
| Codex integration | Global Hooks normalization, capability admission, owned TOML/JSON configuration, MCP stdio continuity, hook inspection and repair. | Provider-specific and the deepest operational path. |
| Agy integration | Exact 1.1.19 structured title callback, readiness/ownership inspection, setup and uninstall. Historical G64 qualification is retained separately. | Provider-specific, intentionally narrower than Codex; only Ready/Working title evidence is proven. |
| Presentation | Typed title, activity, tab color, progress, title authority, visual fixture/oracle. | Mostly provider-neutral after normalized evidence; fixed provider label/mark metadata is intentionally provider-specific. |
| Activity workers | Hashed session/turn/terminal ownership, atomic leases, successor handoff, fail-open cleanup. | Core lease/cleanup model is general; process launch and MCP continuity are currently Codex-specific. |
| Workspace identity | Offline Git specialization plus non-Git local identity, aliases, worktree/root anchoring. | Provider-neutral. Workspace is metadata/binding, never the session primary key. |
| Control Center and CLI | Human and JSON diagnostics, guided setup, status/sessions/hook projections, config, uninstall, upgrade preflight. | Shared presentation/diagnostics are neutral; command grammar and mutation dispatch are hard-coded per provider. |
| Upgrade/preflight | Exact-owned worker/image/process classification and optional ownership-safe drain. | Generic ownership principles, but the proven live transport is the Codex MCP worker. |
| Docs, CI, release | Fast Lane v2, docs checker, exact-head hosted Rust CI, Owner-dispatched visual CI, package/release evidence. | Governance is generally provider-neutral; fixtures, version records, support tables, and visual examples carry Codex/Agy facts. |

### Important seams

**`ProviderVisualIdentity`.** The value object correctly separates identity
from workspace and runtime state, and unknown IDs map to a fixed safe fallback.
Its `match` still explicitly knows `codex`, `agy`, and `unknown`; it is not a
registry-supplied branding catalogue. Native icon data remains unavailable.

**`SetupCommand` and `UninstallProvider`.** Both are closed Clap enums. `setup
codex` and `setup agy` dispatch to separate ownership models, while the generic
Hook input enum currently accepts only Codex. This protects current ownership
contracts but means a provider cannot be added by registration alone.

**Provider registry construction.** `ProviderRegistry::from_diagnostics`
starts with a Codex observation and then overlays `AgyProductionSetup` when an
environment probe is available. Its capability lists, short badges (`C`/`A`),
labels, backend names, and readiness rules are built in Rust. `ProviderId` being
open makes the read model extensible, but construction is not adapter-driven.

**Capability projection.** The common `ProviderCapability` enum is useful for
the UI, but the truth mapping is manually maintained in `from_codex_probe` and
`from_agy_readiness`. It handles unavailable and unsupported states honestly;
it does not yet accept a provider-owned capability declaration as data.

**Setup/configuration ownership.** The product has the right safety shape:
configuration is provider-specific, preflighted, exact-owned, minimal,
drift-refusing, restore-aware, and manually trusted where the provider requires
it. It does not have a generic provider configuration interface—correctly so
until an adapter contract can represent target discovery, ownership proof,
preview, apply, restore, and no-op semantics without weakening either existing
provider.

**Qualification infrastructure.** The G64 machinery is historical and
content-minimizing, not a reusable provider-onboarding framework. The admitted
Agy 1.1.19 production profile lives in `agy_backend`, beside the retained
qualification types. A v0.8 platform effort should separate historical
evidence tools from a future provider qualification contract before adding any
provider.

## Dogfood debt audit

These are planning debts, not authorization to alter runtime behavior.

| Family | Evidence/current state | Debt to close before broadening scope |
| --- | --- | --- |
| Local shell startup and runtime probe timing | Codex runtime probes deliberately classify an unqualified Windows shell rather than fabricate a pass; tests keep probe setup outside the measured window. | Establish a durable shell/profile/environment fingerprint and a minimal cold/warm timing receipt so diagnostics distinguish product latency from inherited-shell variance. |
| Worker/process startup overhead | The release-only Hook SLA avoids judging debug artifacts; prior `commandWindows`/EOF timing is host-sensitive. | Capture worker image generation, process launch, stdin close, and successor timing separately. Preserve fail-open before optimizing. |
| Upgrade replaceability | `upgrade-preflight` proves an exact owned TabBeacon MCP process before drain; ambiguity blocks replacement. | Test replacement while a realistic Agy configuration exists and make transport-specific ownership explicit instead of implying generic multi-provider replaceability. |
| Visual harness reliability | Exact-tab UIA is the semantic oracle; the hosted visual lane is Owner-dispatched, self-hosted, and may classify environment blockers. | Make runner health, capture limitation, evidence root, and cleanup state queryable before a release train; do not turn screenshots into the title oracle. |
| Artifact retention and owned cleanup | The visual workflow uploads owned evidence for 14 days. Worker images are garbage-collected only with lease proof. | Define which receipts must survive runner/artifact retention and which can be safely cleaned; publish only allowlisted summary fields. |
| Agy exact-version fragility | Production admission is deliberately exact to Agy 1.1.19. A newer version has no inherited support. | Add a requalification intake/expiry policy and a fixture corpus that makes exact-profile re-admission cheaper without relaxing it. |
| Provider badge usability | `C`/`A` are fixed, safe title badges; provider identity is separate from workspace alias. | Validate whether one-character marks remain comprehensible with mixed providers, color reduction, screen readers, and an unknown provider fallback. |
| Defect tracking | Documentation records latches and test classifications, but there is no single current debt register linking timing, visual, cleanup, and profile findings to owners/evidence expiry. | Create a planning-only debt ledger or issue taxonomy before a reliability train; do not treat old passing evidence as live authorization. |

```text
DOGFOOD_DEBT_AUDIT=PASS
RUNTIME_BEHAVIOR_CHANGED=false
```

## Native Windows Terminal tab icon: retained `NO_GO`

```text
NATIVE_TAB_ICON_DISPOSITION=NO_GO
```

The result is a safety and product decision, not a statement of physical
impossibility:

1. Windows Terminal has an **internal** native icon pipeline: `Tab::UpdateIcon`
   reaches its tab model and `TabViewItem.IconSource`.
2. A public child-process bridge that safely sets that icon was not found. The
   current public request, [microsoft/terminal#1868](https://github.com/microsoft/terminal/issues/1868), remains the relevant upstream issue.
3. The v0.7 XAML route stopped **before attachment**. A fresh named window
   could not be proven isolated from the Windows Terminal process hosting Owner
   or development work. The zero mutation count is negative safety evidence,
   not a failed or successful mutation experiment.
4. `InitializeXamlDiagnosticsEx` is documented process instrumentation: it
   takes a target PID and injects a diagnostic-site DLL/CLSID. It is therefore
   not a child-process terminal protocol or a safe default product transport.
   See [Microsoft’s API documentation](https://learn.microsoft.com/en-us/windows/win32/api/xamlom/nf-xamlom-initializexamldiagnosticsex).

`WINDOWS_TERMINAL_UPSTREAM_NATIVE_ICON` is a separate, non-blocking future
option: prepare an upstream issue/comment, proposal, or narrowly scoped public
API experiment against a separately admitted upstream contribution goal. It
does not reopen XAML Diagnostics, attach to a process, mutate Windows Terminal,
or block TabBeacon reliability work.

## Coding-agent ecosystem snapshot

All upstream records were refreshed from official documentation on 2026-08-28.
They indicate integration potential only. No document, version string, fixture,
or capability similarity admits a provider to TabBeacon.

### Claude Code — `claude` — deferred

| Facet | Official current record / TabBeacon implication |
| --- | --- |
| Daily CLI command | `claude`. |
| User-global extension/hook location | `~/.claude/settings.json`, resolving to `%USERPROFILE%\\.claude\\settings.json` on Windows; project and managed scopes also exist. |
| Structured lifecycle events | `SessionStart`, `SessionEnd`, `Stop`, `Notification`, subagent, config, worktree, prompt, and compaction events are documented. |
| Session and workspace evidence | Hook input includes `session_id` and `cwd`; transcript paths must be ignored because TabBeacon has no content/path-retention authority. |
| Tool lifecycle | `PreToolUse`, `PostToolUse`, `PostToolUseFailure`, batches, and tool names/input are structured. |
| Stop/idle/result | `Stop` and `SessionEnd` are direct evidence; Notification may add waiting-user context. A TabBeacon adapter would need an explicit authority mapping rather than treating all Stop states as a result. |
| Permission evidence | `PermissionRequest` and pre-tool decisions can allow/deny/ask; managed deny rules remain stronger. |
| Sync/async semantics | Command hooks block by default. `async: true` continues immediately; async hooks cannot control the action and completion is deferred (except constrained re-wake behavior). |
| Trust/config ownership | User/project/local/managed scopes have precedence. Setup must own exactly one declared handler and preserve other hooks/settings. |
| Windows | Official paths resolve under `%USERPROFILE%`; documentation includes PowerShell hook support and Windows shell behavior. |
| Integration fit | Richest documented lifecycle/permission surface, but **DEFERRED**. It needs a new, exact-version/provider-authority admission and ownership design; no current TabBeacon code may consume it. |

Sources: [hooks reference](https://code.claude.com/docs/en/hooks),
[settings](https://code.claude.com/docs/en/settings), and
[CLI reference](https://docs.anthropic.com/en/docs/claude-code/cli-usage).

### Gemini CLI — `gemini` — candidate only

| Facet | Official current record / TabBeacon implication |
| --- | --- |
| Daily CLI command | `gemini`. |
| User-global extension/hook location | `~/.gemini/settings.json`; project settings are `.gemini/settings.json`; Windows system policy files are under `C:\\ProgramData\\gemini-cli`. |
| Structured lifecycle events | `SessionStart`, `SessionEnd`, `Notification`, and `PreCompress`; agent and model hooks are also documented. |
| Session and workspace evidence | The hook reference documents `session_id`; workspace-scoped configuration and folder trust are documented. Exact hook payload `cwd`/root semantics must be re-read from the admitted release schema before implementation. |
| Tool lifecycle | `BeforeTool` and `AfterTool` use structured stdin JSON/stdout JSON and a block exit code. |
| Stop/idle/result | `SessionEnd` has exit/clear/logout reasons. A per-turn idle/result mapping is `UNPROVEN` from the reviewed hook reference and must not be inferred from `AfterModel`. |
| Permission evidence | Approval modes include default, auto-edit, plan, and explicit auto-approval; folder trust is a separate boundary. |
| Sync/async semantics | Blocking exit-code semantics are documented. No supported background-hook mode was found in the reviewed reference; treat async delivery as `UNPROVEN`. |
| Trust/config ownership | Defaults, user, project, system overrides, environment, and CLI arguments have defined precedence. Workspace settings can override user settings. |
| Windows | Official docs cover PowerShell use and Windows system configuration locations. |
| Integration fit | Strong structured candidate, but requires exact schema/field minimization and a trust/ownership admission; no provider status is assigned by this audit. |

Sources: [hooks reference](https://geminicli.com/docs/hooks/reference/),
[configuration](https://geminicli.com/docs/reference/configuration/), and
[CLI reference](https://geminicli.com/docs/cli/cli-reference/).

### GitHub Copilot CLI — `copilot` — candidate only

| Facet | Official current record / TabBeacon implication |
| --- | --- |
| Daily CLI command | `copilot`. |
| User-global extension/hook location | `%USERPROFILE%\\.copilot\\hooks\\*.json` (or `$COPILOT_HOME/hooks/`); repository hooks are `.github/hooks/*.json`. |
| Structured lifecycle events | CLI hooks include session and agent lifecycle, `agentStop`, tool pre/post events, notifications, and permission events. |
| Session and workspace evidence | Hook inputs document `cwd`; exact session-ID semantics need an admitted schema check before using it as a TabBeacon session key. |
| Tool lifecycle | `preToolUse` can control a tool request and `postToolUse` reports successful completion; hook matchers operate on tool names. |
| Stop/idle/result | `agentStop` is the direct completion hook. Session-end examples are documented; result authority must remain narrower than a generic stop. |
| Permission evidence | `permissionRequest` runs before the permission service and can return decision control, subject to the documented sandbox-bypass exception and higher policy. |
| Sync/async semantics | Repository hooks are documented as synchronous and blocking. No async hook contract was accepted in this audit. |
| Trust/config ownership | Policy, user, repository, local settings, and plugin sources are merged; Windows policy hooks may be machine-wide and not user-disableable. |
| Windows | Official Windows hook examples use PowerShell 7+; policy directory is `C:\\ProgramData\\GitHub\\Copilot\\policy.d`. |
| Integration fit | Well-shaped Hook candidate, but needs conflict-aware multi-scope ownership and an exact session-key review before any future admission. |

Sources: [hooks reference](https://docs.github.com/en/copilot/reference/hooks-reference)
and [Copilot CLI hook guide](https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/use-hooks).

### Qwen Code — `qwen` — candidate only

| Facet | Official current record / TabBeacon implication |
| --- | --- |
| Daily CLI command | `qwen`. |
| User-global extension/hook location | `~/.qwen/settings.json`; project configuration is `.qwen/settings.json` and `.qwen` can contain skills. |
| Structured lifecycle events | `SessionStart`, `SessionEnd`, `Stop`, `StopFailure`, notification, subagent, compaction, prompt, and tool/permission events are documented. |
| Session and workspace evidence | The SDK exposes session identity and working-directory options; the exact CLI Hook payload field contract must be sampled only in a future content-minimizing qualification. |
| Tool lifecycle | `PreToolUse`, `PostToolUse`, and `PostToolUseFailure` match structured tool IDs. |
| Stop/idle/result | `Stop` and `SessionEnd` are direct lifecycle events; Stop is not automatically a result-ready claim. |
| Permission evidence | `PermissionRequest` and `PermissionDenied` carry tool-ID events; CLI permission modes are separately documented. |
| Sync/async semantics | Command/HTTP/function/prompt hooks are supported; command hooks have timeout and explicit `async`, and the docs warn an async hook may outlive CLI exit. |
| Trust/config ownership | Defaults, user, project, system defaults/system overrides, environment, and CLI layers have precedence; workspace-provided sensitive configuration is constrained by documented safety rules. |
| Windows | Official configuration has Windows system paths and supports PowerShell hook execution. |
| Integration fit | Broad lifecycle surface with a particularly important async-cleanup hazard. It needs an ownership/exit model before a future qualification. |

Sources: [hooks](https://qwenlm.github.io/qwen-code-docs/en/users/features/hooks/),
[configuration](https://qwenlm.github.io/qwen-code-docs/en/users/configuration/settings/),
and [SDK session/options reference](https://qwenlm.github.io/qwen-code-docs/en/developers/sdk-python/).

### OpenCode — `opencode` — deferred

| Facet | Official current record / TabBeacon implication |
| --- | --- |
| Daily CLI command | `opencode`. |
| User-global extension/hook location | Global config and extensions are user-wide; documented global custom tools live under `~/.config/opencode/tools/`, while project tools live in `.opencode/tools/`. Exact plugin installation ownership must be re-admitted before any future work. |
| Structured lifecycle events | Plugin event stream includes `session.created`, `session.idle`, `session.status`, `session.updated`, session deletion/error/compaction, permission events, and tool before/after events. |
| Session and workspace evidence | Custom-tool context exposes `sessionID`, `directory`, and `worktree`; this is a strong session/root signal but must be narrowed to a safe callback contract. |
| Tool lifecycle | `tool.execute.before` and `tool.execute.after` hooks receive structured inputs/results. |
| Stop/idle/result | `session.idle` is a direct idle event; session status/error are structured. A semantic result-ready mapping remains a provider-specific authority decision. |
| Permission evidence | `permission.asked`/`permission.replied` events and `allow`/`ask`/`deny` rules are documented, including scoped external-directory protection. |
| Sync/async semantics | Runtime interceptors are awaited and a hook failure fails the intercepted operation. The public event stream’s delivery/ordering guarantee is not sufficient here for a lifecycle authority claim. |
| Trust/config ownership | Global, project, and managed configuration coexist; tool replacement and broad permissions make exact ownership/revert design high-risk. |
| Windows | Official configuration selects `pwsh` or `cmd.exe` on Windows when no shell is configured. |
| Integration fit | Very capable plugin/event surface, but **DEFERRED**. It needs a dedicated safety design because plugin power, tool replacement, and permission policy exceed TabBeacon’s present ownership abstraction. |

Sources: [plugins](https://opencode.ai/docs/plugins/),
[custom tools](https://opencode.ai/docs/custom-tools),
[permissions](https://opencode.ai/docs/permissions/), and
[configuration](https://opencode.ai/docs/config).

```text
CODING_AGENT_ECOSYSTEM_AUDIT=PASS
CLAUDE_PROVIDER=DEFERRED
OPENCODE_PROVIDER=DEFERRED
```

## Planning handoff

The comparative v0.8 option set is intentionally non-authoritative and lives
in [`V08_OPTIONS.md`](../../dev_governance_files/V08_OPTIONS.md). The proposed
theme does not admit a new provider or reopen native icon diagnostics.

```text
PRODUCT_ARCHITECTURE_AUDIT=PASS
V08_OPTIONS_WRITTEN=true
RECOMMENDED_V08_THEME=Operational Reliability v2
NEXT_RECOMMENDED_GOAL=TB-V08-OPERATIONAL-RELIABILITY-V2-ADMISSION-001
```
