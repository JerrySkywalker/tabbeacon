# TB-V061-12H-FORWARD-COMPAT-VISUAL-001

## Scope

Start the v0.6.1 train in priority order:

1. replace exact Codex version admission with local capability compatibility;
2. establish Provider Visual Identity v2 after the compatibility candidate;
3. record bounded Windows Terminal native-icon source truth.

This goal does not publish any TabBeacon release, change PR #79, or mutate an
Owner Codex/Agy configuration or Hook trust state.

## Train A completion

The candidate removes `CodexCompatibilityRegistry` and
`CodexHookProfile::for_version` from runtime and mutation decisions. It adds
local Hooks capability discovery, `Full`/`Degraded`/`Incompatible`/`Unproven`
states, executable-and-capability fingerprint cache behavior, forward parsing,
and ownership-preserving setup/repair/doctor behavior.

Focused deterministic coverage includes a synthetic `99.99.99` compatible
binary, additive fields and an `Interrupt` event, degraded optional schema
evidence, missing required capability, probe failure preserving an exact
runtime, cache hit, and cache invalidation after binary change.

Train A merged as PR #82 at `43cd07d6152e0acb99fb1a948cde23b45fbad5ff`
after hosted exact-head CI passed for
`59dc531747ed6d74341687102c1bc00a77c4684d`.

## Train B candidate status

Provider Visual Identity v2 introduces fixed, product-owned provider identity
metadata and a `TerminalVisualBackend` capability boundary. The production
`TitleMarkBackend` preserves the OSC title fallback while native tab-icon
capability remains false. The identity, runtime state, and workspace alias
occupy independent title slots.

## Risk vector

```text
CODE_CHANGED=true
PROVIDER_CHANGED=true
USER_PERSISTENT_CONFIG_CHANGED=true
SECURITY_OR_PRIVACY_CHANGED=false
PRESENTATION_CHANGED=true
RELEASE_BOUNDARY=false
```

Persistent configuration safety remains ownership-gated and fixture-only. No
real Owner configuration or trust state is an acceptance fixture.
