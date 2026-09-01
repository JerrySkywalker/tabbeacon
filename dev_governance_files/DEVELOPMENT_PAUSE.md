# Development scope after v0.7.3

```text
CURRENT_PUBLIC_RELEASE=v0.7.3
CURRENT_PUBLIC_TARGET=v0.7.3
V073_IMPLEMENTATION=COMPLETE
ACTIVE_FEATURE_DEVELOPMENT=PAUSED
NO_ACTIVE_FEATURE_DEVELOPMENT=true
DOGFOOD_MODE=LONG_TERM_DOGFOOD_NO_ACTIVE_DEVELOPMENT
DOGFOOD_MINIMUM_WEEKS=4
DOGFOOD_PREFERRED_WEEKS=6-8
V08_OPTIONS_STATUS=NON_AUTHORITATIVE
ROADMAP_V08_CREATED=false
NEXT_RECOMMENDED_GOAL=LONG_TERM_DOGFOOD_NO_ACTIVE_DEVELOPMENT
```

TabBeacon v0.7.3 is publicly released from
`1485b4dc0fe634a21634adc9ec539deb76beaad2`. The crates.io package, immutable
tag, GitHub Release, Windows x64 ZIP and sidecar, exact locked install, default
install, and fresh GitHub-asset consumer are verified.

```text
CRATES_IO_VERSION=0.7.3
CRATES_IO_YANKED=false
TAG=v0.7.3
GITHUB_RELEASE=v0.7.3
WINDOWS_ZIP_SHA256=9d78a49319c3e8680479d47bbbf8fd5e459b008300029164b768b896609be14c
```

## Long-term dogfood contract

The repository is in long-term dogfood with no active feature development.
The minimum observation period is four weeks; six to eight weeks is preferred.
Normal dogfood observations do not authorize source changes or a v0.8 train.

A bounded maintenance admission may interrupt the pause only for:

- a P0/P1 production defect;
- a security issue; or
- an upstream compatibility break that requires bounded maintenance.

Do not automatically start Operational Reliability v2, Provider Platform v2,
Multi-Agent Presentation UX v3, a new provider, native tab-icon work, terminal
expansion, an installer, package-manager distribution, or auto-update work.

Suggested evidence before any future planning admission:

```text
P0_COUNT=0
P1_COUNT=0
REPEATED_UNKNOWN_HOOK_FAILURE=false
REPEATED_MANUAL_RECOVERY=false
STALE_TEMP_WT=false
UNSAFE_PROCESS_CLEANUP=false
MEANINGFUL_REAL_USE_ACROSS_MULTIPLE_CODEX_UPGRADES=true_required
```

## Product and provider boundary

The v0.7.3 train did not change runtime or provider behavior.

```text
RUNTIME_BEHAVIOR_CHANGED=false
PROVIDER_BEHAVIOR_CHANGED=false
NEW_PROVIDER_ADDED=false
DAILY_COMMAND_CODEX=codex
DAILY_COMMAND_AGY=agy
NO_WRAPPER=true
NO_PATH_SHADOW=true
NO_PTY_HOST=true
NO_GLOBAL_DAEMON=true
HOOK_TIMEOUT_CHANGED=false
HOOK_TRUST_BOUNDARY_CHANGED=false
NATIVE_TAB_ICON_DISPOSITION=NO_GO
CODEX_SUPPORT=production_capability_based
AGY_SUPPORT=production_exact_1.1.19
CLAUDE_SUPPORT=deferred
OPENCODE_SUPPORT=deferred
CODEX_APP_SERVER=deferred
```

`Codex and more` remains discovery positioning rather than a compatibility
wildcard. Deferred providers are not partially supported.

## Historical v0.7.3 train disposition

The clean successor train was accepted through PRs #108, #109, and #110.
Historical PR #100 was closed without merge after its desired G99/G100 scope
was proven represented and strengthened by PR #108.

```text
PR100_STATE=CLOSED_SUPERSEDED
PR100_SUPERSEDED=true
PR100_MERGED=false
PR100_SUPERSEDED_BY_PR=108
SOCIAL_PREVIEW_UPLOAD=WAITING_OWNER_UI
```

The committed social-preview SVG and PNG remain ready for an Owner Settings UI
upload. The missing Settings UI action is non-blocking and does not authorize
browser-session automation.

## Owner production environment boundary

The public release and repository dogfood state do not authorize production
adoption or configuration mutation.

```text
OWNER_PRODUCTION_TABBEACON_UPGRADE=false
PRODUCTION_CODEX_CONFIGURATION_MUTATED=false
PRODUCTION_HOOK_TRUST_MUTATED=false
PRODUCTION_AGY_CONFIGURATION_MUTATED=false
```

An Owner production upgrade to the official v0.7.3 binary is a separate,
explicitly authorized adoption step after this release Goal.

## Future admission boundary

`V08_OPTIONS.md` remains non-authoritative and `ROADMAP_V08.md` does not exist.
Any future implementation or release requires a fresh Goal, exact current-main
admission, one active Implementer writer, risk-based gates, and explicit
authority for any public or production mutation.
