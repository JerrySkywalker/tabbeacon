# TB-G19R — v0.3 Hardening and Release

## Status

COMPLETE. The final Codex-only v0.3 closure Goal published `0.3.0` from
`ddb2a218b9fe7601a09caaa1c8c3a0c1d0af9419` under `v0.3.0` after completed
TB-G18 and TB-G19. No new product feature was admitted.

## Purpose

Publish one accepted v0.3 source while proving the integrated daily Windows Terminal +
Codex workflow. G19R is one release train, not a replay of every historical Goal and
scenario.

## Release principle

Use Fast Lane v2 evidence reuse.

Freshly rerun release-specific gates and any risk gate whose relevant source changed.
Reuse accepted G15–G19 evidence when its risk surface is unchanged.

A new release SHA alone does not require rerunning every Visual, L4, performance,
configuration, and convergence matrix.

## Mandatory fresh release work

At the settled release candidate run once:

```text
full locked Rust/static/build CI
repository/publication sanity
package + publish dry-run + package content inspection
Windows x64 artifact build
artifact/checksum verification
crates.io publication/verification
GitHub tag/release/assets publication/verification
public consumer verification
```

Publication still requires explicit Owner authorization under the repository's existing
release policy.

## Representative integrated dogfood

Run one small final dogfood pack sufficient to show the integrated product still works:

```text
normal PowerShell title authority
100 ms working animation (>=3 frames within 1s)
result-ready convergence
stable workspace alias
actual Administrator PowerShell smoke
real Codex smoke for the admitted production profile
```

Do not recreate the whole G18 traceability matrix.

## Evidence reuse

When risk diffs are empty, reuse accepted evidence such as:

```text
G16 1/4/8-tab performance
G17 Windows Terminal policy/remediation safety
G18 lifecycle family
G18 generation/isolation family
G18 workspace identity family
G18 recovery family
prior exact profile/source audit
```

Record the source head and empty relevant risk diff for each reuse.

If G19 admits no new Codex profile and provider/trust declarations are unchanged, the
release may reuse the admitted 0.147.0 provider evidence plus one representative real
Codex smoke instead of repeating a profile-admission L4 matrix.

## Presentation release contract

```text
DEFAULT_ACTIVITY=title-spinner
DEFAULT_SPINNER=braille
TARGET_FRAME_INTERVAL_MS=100
VISIBLE_WORKING_FRAMES_GE_3_WITHIN_1S=PASS
WORKSPACE_ALIAS_STABLE=true
```

A healthy title channel must prove visible convergence; successful OSC writes alone do
not establish health.

## PowerShell/title regression contract

```text
NORMAL_POWERSHELL=PASS
ADMIN_POWERSHELL=PASS_ACTUAL_ELEVATED
TB_REG_TITLE_OWNERSHIP_001=CLOSED
```

Do not substitute a synthetic Administrator label for actual elevation.

## Exact-head/reuse contract

Fresh release gates bind to the release candidate exactly:

```text
CODE_HEAD=RELEASE_HEAD
PACKAGE_HEAD=RELEASE_HEAD
PUBLICATION_HEAD=RELEASE_HEAD
```

A reused gate may bind to its earlier accepted head only when its recorded risk diff to
the release head is empty.

Do not create a metadata-only commit that forces unrelated expensive gates to rerun.
Prefer PR/release receipts for final evidence bookkeeping.

## Audit policy

G19R gets one final release/publication review because publication is an explicit audit
trigger. Do not chain multiple generic auditors over unchanged evidence.

## Release limitations

Do not add during release closure:

- global daemon;
- wrapper/PATH shadow;
- automatic shell-profile edits;
- unsupported Codex profile inheritance;
- Claude/OpenCode/App Server production work;
- unrelated packaging/self-update features.

## Completion definition

```text
TB_G15=COMPLETE
TB_G16=COMPLETE
TB_G17=COMPLETE
TB_G18=COMPLETE
TB_G19=COMPLETE
TB_G19R=COMPLETE

DEFAULT_ACTIVITY=title-spinner
DEFAULT_SPINNER=braille
TARGET_FRAME_INTERVAL_MS=100
VISIBLE_WORKING_FRAMES_GE_3_WITHIN_1S=PASS
WORKSPACE_ALIAS_STABLE=true

NORMAL_POWERSHELL=PASS
ADMIN_POWERSHELL=PASS_ACTUAL_ELEVATED
TB_REG_TITLE_OWNERSHIP_001=CLOSED

CODEX_PROFILE_POLICY=explicit
DAILY_COMMAND=codex
GLOBAL_DAEMON_INTRODUCED=false

PACKAGE=PASS
PUBLICATION=PASS
V0_3_RELEASE=PASS
```

## Exit receipt

```text
GOAL_ID=TB-G19R
DISPOSITION=PASS_RELEASED
STARTING_MAIN=a78fb1f79488e75ad5af224854db8f2680293fe0
RELEASE_HEAD=ddb2a218b9fe7601a09caaa1c8c3a0c1d0af9419
VERSION=0.3.0
TAG=v0.3.0
CODE_CI=PASS
VISUAL=REUSED
REAL_CODEX=REUSED
TITLE_AUTHORITY=REUSED
NORMAL_POWERSHELL=REUSED
ADMIN_POWERSHELL=REUSED_FROM_G18
ANIMATION_100MS=REUSED
MULTI_TAB_1_4_8=REUSED
CONFIG_SAFETY=REUSED
CONVERGENCE_FAMILIES=REUSED
PACKAGE=PASS
PUBLICATION=PASS
V0_3_RELEASE=PASS
OWNER_ACTION=none
NEXT_GOAL=maintenance
```

Estimated effective engineering effort after accepted G18/G19: **3–5 h** when no new
product defect is discovered.
