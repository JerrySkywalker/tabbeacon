# TB v0.2 Amendment — Workspace Identity and Fast-Lane Execution

Date: 2026-08-16

## Status

This amendment updates the execution order and validation strategy of `TB-V02-CODEX-FIRST-TRACK.md` after G10 completion, G11 feasibility PASS, and non-Git dogfood findings.

Where this amendment conflicts with the older planning document, this amendment governs the remaining Codex-first v0.2 work until the roadmap is consolidated again.

## Current accepted baseline

```text
main=3c489a3528275aa624a26d4606d59bb833fa700b
TB-G09=COMPLETE
TB-G10=COMPLETE
TB-G11_FEASIBILITY=PASS
TB-G11_PRODUCTION=NOT_STARTED
```

G11 feasibility proved:

- Hook exit survival;
- originating terminal binding;
- two-tab isolation;
- bounded session/tab cleanup;
- fail-open worker failure.

The feasibility result removes the largest architectural uncertainty from the original v0.2 plan. It does not mean the production animator is implemented.

## New dogfood finding

Codex sessions launched from ordinary non-Git directories do not currently receive useful TabBeacon presentation because the runtime requires Git repository identity before rendering.

This must be corrected before production G11 so worker presentation state is based on a correct workspace identity contract.

## Revised dependency order

```text
TB-G09   COMPLETE
   ↓
TB-G10   COMPLETE
   ↓
TB-G10A  Non-Git Workspace Identity Fallback
   ↓
TB-G11   Session-Scoped Ephemeral Activity Animator — production implementation
   ↓
TB-G12   Guided Setup / Configuration Wizard v2
   ↓
TB-G13   Operational Status / JSON Diagnostics
   ↓
TB-G14   Codex-only v0.2 Hardening + Release — COMPLETE
```

`TB-G10A` is mandatory for v0.2 completion.

## Release closure record

`TB-G14` completed with TabBeacon `0.2.0` released from
`0b1d5136833a05bf94b7d32c414a21da2f5ac78e` under tag `v0.2.0`. The final
exact-head code, Visual, terminal-close, and isolated real-Codex gates passed;
the crates.io package and GitHub Windows x64 ZIP were independently verified.

## Identity terminology correction

The product-facing concept is now **workspace identity**.

Git repository identity remains the preferred stable specialization for Git workspaces. Ordinary-directory workspaces receive a deterministic local fallback. Presentation and future animator state should carry a workspace alias rather than assume every Codex session belongs to a repository.

The compact grammar is unchanged:

```text
<status-slot> <workspace-alias>
```

## Revised effort envelope

```text
TB-G10A  workspace identity fallback     3–6 h
TB-G11   production animator             8–16 h
TB-G12   guided setup                    4–8 h
TB-G13   structured diagnostics          2–4 h
TB-G14   hardening/release               3–6 h
----------------------------------------------
Remaining nominal                        20–40 h
Expected after feasibility de-risking    ~26–32 effective h
```

The expected range is an engineering estimate, not a schedule guarantee.

## Development-process correction

The remaining track uses `dev_governance_files/FAST_LANE.md`.

The purpose is not to reduce correctness. It removes repeated low-signal governance work now that the repository has stable CI, visual infrastructure, Hook trust rules, and release machinery.

Key changes:

- validation is selected by changed risk surface;
- docs-only work does not run Rust/Visual/L4 suites;
- ordinary code receives one final-head hosted CI rather than repeated full local/hosted cycles;
- Visual CI runs only for presentation-visible changes;
- real Codex L4 runs only for provider/config/trust changes or focused integration behavior that synthetic tests cannot prove;
- unchanged blockers latch after one audit;
- dedicated auditors are reserved for destructive config, security/privacy, concurrency/ownership, ambiguous failures, and releases;
- `TB-G14` still receives the complete closure matrix.

## Remaining functional checklist

### G10A

- non-Git directory workspace identity;
- shared alias namespace with Git repositories;
- home/root/same-basename/Unicode/hostile-path behavior;
- Git identity compatibility;
- non-Git Codex smoke.

### G11

- productionize the admitted session/turn-scoped worker architecture;
- animate left status slot only;
- preserve workspace alias position;
- generation supersession;
- bounded cleanup;
- worker crash/missing binary fail-open;
- upgrade-safe worker ownership;
- final Visual + focused Codex smoke.

### G12

- guided `tabbeacon setup`;
- discovery + compact configuration selection;
- live/near-live preview;
- Apply/Cancel;
- supported `/hooks` trust handoff;
- reuse existing settings primitives.

### G13

- `tabbeacon status`;
- `tabbeacon status --json`;
- structured doctor output;
- version/profile/hook/settings/worker health metadata;
- no prompt/tool/content leakage.

### G14

- multi-tab / same-workspace / worktree / non-Git hardening;
- stale generation and subagent paths;
- animation cleanup and crash paths;
- setup/upgrade/relocation;
- exact release-candidate code/visual/provider evidence;
- GitHub Release + crates.io from the same accepted source version.

Completed: the accepted source version is `0.2.0` at
`0b1d5136833a05bf94b7d32c414a21da2f5ac78e` / `v0.2.0`.

## Deferred items remain deferred

This amendment does not promote:

- Codex App Server (`TB-G06X`);
- Claude provider (`TB-G20`);
- OpenCode provider (`TB-G30`);
- global daemon baseline;
- PTY/wrapper/PATH interception;
- package-manager/self-update work;
- ARM64 release without validated hardware/CI.
