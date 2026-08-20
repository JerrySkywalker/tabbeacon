# TB-G60 — Hook Inspector & Trust Explainability

## Status

PLANNED after accepted G59.

## Purpose

Add provider-neutral Hook inventory and precise trust/currentness explanation to CLI and Control Center without exposing arbitrary handler content by default or weakening manual trust boundaries.

## CLI surface

Add first-class inspection commands, conceptually:

```text
tabbeacon hooks
tabbeacon hooks --json
tabbeacon hooks --plain
```

Human output is localized. JSON/plain keys and stable IDs remain locale-independent.

## Typed inventory

Create one provider-neutral projection conceptually containing:

```text
HookInventoryEntry {
  provider
  event
  owner
  enabled
  trust_state
  currentness
  source_kind
  handler_kind
  timeout
  fingerprint
  command_visibility
}
```

Provider adapters own raw Hook/config parsing. The management/TUI layer sees only safe typed inventory.

## Trust/currentness vocabulary

At minimum distinguish:

```text
exact/current/trusted
trust review required
trust hash stale or changed
declaration modified/missing
integration currentness stale
disabled
unowned/ambiguous
unsupported/unavailable
```

Do not collapse a trusted-hash mismatch into wording that asserts the Hook declaration itself changed when declaration/currentness checks already prove exactness.

## Control Center

Add a mandatory first-class `Hooks / 钩子` screen. It should show a compact inventory such as:

```text
Event              Owner       Enabled   Trust       Current
PreToolUse         TabBeacon   yes       trusted     yes
PermissionRequest  TabBeacon   yes       trusted     yes
...
```

A selected/detail view may show provider, source class, handler type, timeout and fingerprint.

## Command/privacy handling

Default Human and all machine outputs must not expose arbitrary full Hook command strings. Third-party Hook commands may contain credentials, private paths, environment arguments, or proprietary tooling.

A future/optional explicit Human-only reveal action may be implemented if it is clearly marked sensitive and bounded, but G60 does not require command reveal.

Never expose trusted hashes as authentication secrets; they are fingerprints. Raw native Hook state keys should remain machine/internal unless they are necessary stable identifiers and privacy-reviewed.

## Ownership and third-party Hooks

The screen should be able to represent non-TabBeacon Hooks without claiming ownership or trust semantics TabBeacon cannot prove. Unknown third-party entries are inspectable summaries, not mutation targets.

No delete/disable/edit action for arbitrary Hooks in this Goal.

Hook trust remains manual. The Control Center may direct users to the provider's supported trust/review flow (Codex `/hooks`) but must not mark trust automatically.

## Testing

Required families:

- 11 exact Codex owned Hooks inventory;
- trusted / review-required / stale-hash / disabled / modified declaration states;
- mixed owned + third-party Hooks;
- no arbitrary command leakage in Human/JSON/plain/TUI buffers;
- stable schema/IDs/localization boundary;
- TUI narrow/no-color/en-US/zh-CN;
- Hook inventory refresh is read-only;
- manual trust boundary preserved;
- malformed/unsupported provider Hook shape fails safely.

## Risk vector

```text
CODE_CHANGED=true
PRESENTATION_CHANGED=true
PROVIDER_CHANGED=true
USER_PERSISTENT_CONFIG_CHANGED=false
SECURITY_OR_PRIVACY_CHANGED=true
RELEASE_BOUNDARY=false
```

Use a focused privacy/security review and one representative real provider/TUI proof. No Hook trust mutation.

## Acceptance

```text
HOOKS_CLI=PASS
HOOKS_JSON=PASS
HOOKS_PLAIN=PASS
HOOKS_TUI=PASS
HOOK_INVENTORY_PROVIDER_NEUTRAL=true
TRUST_HASH_MISMATCH_DISTINCT=true
THIRD_PARTY_HOOKS_READ_ONLY=true
ARBITRARY_COMMANDS_REDACTED=true
AUTO_HOOK_TRUST=false
ZH_CN_HOOKS=PASS
EN_US_HOOKS=PASS
PRIVACY_REVIEW=PASS
CODE_CI=PASS
```

## Estimated effort

**6–9 effective engineering hours.**

## Next

`TB-G61 — Naming / Title Explainability`.