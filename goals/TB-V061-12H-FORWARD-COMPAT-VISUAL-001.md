# TB-V061-12H-FORWARD-COMPAT-VISUAL-001

## Scope

Start the v0.6.1 train in priority order:

1. replace exact Codex version admission with local capability compatibility;
2. establish Provider Visual Identity v2 after the compatibility candidate;
3. record bounded Windows Terminal native-icon source truth.

This goal does not publish any TabBeacon release, change PR #79, or mutate an
Owner Codex/Agy configuration or Hook trust state.

## Train A candidate status

The candidate removes `CodexCompatibilityRegistry` and
`CodexHookProfile::for_version` from runtime and mutation decisions. It adds
local Hooks capability discovery, `Full`/`Degraded`/`Incompatible`/`Unproven`
states, executable-and-capability fingerprint cache behavior, forward parsing,
and ownership-preserving setup/repair/doctor behavior.

Focused deterministic coverage includes a synthetic `99.99.99` compatible
binary, additive fields and an `Interrupt` event, degraded optional schema
evidence, missing required capability, probe failure preserving an exact
runtime, cache hit, and cache invalidation after binary change.

Train A remains a candidate until the exact-head focused suite, repository
quality gates, hosted CI, and review/merge disposition are recorded.

## Risk vector

```text
CODE_CHANGED=true
PROVIDER_CHANGED=true
USER_PERSISTENT_CONFIG_CHANGED=true
SECURITY_OR_PRIVACY_CHANGED=false
PRESENTATION_CHANGED=false
RELEASE_BOUNDARY=false
```

Persistent configuration safety remains ownership-gated and fixture-only. No
real Owner configuration or trust state is an acceptance fixture.
