# Codex Capability Compatibility V3

## Invariant

Codex version ordering neither grants nor denies support. Compatibility is
admitted from positively established local protocol and capability evidence.
A newer or otherwise unseen version with the same compatible evidence remains
supported. A missing or contradictory required capability remains fail-safe.

## States

| State | Meaning | Setup, repair, reconciliation | Existing exact integration |
| --- | --- | --- | --- |
| `Full` | Required Hooks evidence and optional schema fingerprint succeeded | permitted after ordinary ownership and trust-safety checks | healthy if exact |
| `Degraded` | Required Hooks evidence succeeded; optional schema evidence unavailable | safe command-Hook subset only, after ownership checks | healthy if exact |
| `Incompatible` | Required Hooks capability explicitly absent or disabled | refuse affected mutation with a precise diagnostic | do not claim runtime compatibility |
| `Unproven` | Local discovery did not complete safely | do not create or rewrite configuration | preserve a proven exact manifest-owned runtime; report actionably |

The states do not contain a version field. `codex --version` is reported for
support and bug reports only.

## Contract boundaries

The bounded V1 contract covers the user-global Hook envelope, the eleven known
lifecycle declarations, the title delegation setting, and the manual
trust-review model. It does not turn an unrecognized upstream event into
authority. Codex-specific payloads remain in the provider backend; only
normalized evidence crosses into the provider-neutral core.

Fresh compatible installation selects the conservative command-Hook V1
declaration. A previously manifest-owned hybrid MCP integration retains its
exact ten-MCP-plus-SessionEnd-command transport; no version update or failed
optional probe may rewrite its trust-reviewed declarations.

## Safety

Capability admission and ownership proof are separate. A compatible probe does
not adopt foreign hooks, overwrite ambiguous configuration, or approve trust.
TabBeacon still requires the Owner to review Codex Hook definitions manually.
Failure of discovery or decoration cannot block literal `codex` use.
