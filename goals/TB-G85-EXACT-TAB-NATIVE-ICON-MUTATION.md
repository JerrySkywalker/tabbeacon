# TB-G85 — Exact-Tab Correlation & Native Icon Mutation/Restore

## Status

PLANNED after accepted G84.

## Purpose

Answer the core feasibility question on a disposable stock Windows Terminal:
identify exactly one intended tab, snapshot its native icon state, mutate only
that tab to a deterministic probe icon, verify the visible/model result, and
restore the exact prior state without wrong-tab effects.

This is still experimental feasibility work. It does not integrate native-icon
mutation into the normal TabBeacon runtime or CLI.

## Preconditions

Required from G84:

```text
DISPOSABLE_WT_LAUNCH=PASS
XAML_DIAGNOSTICS_ATTACH=PASS
XAML_TAB_TREE_ENUMERATION=PASS
XAML_DIAGNOSTICS_DETACH=PASS
FOCUSED_SAFETY_REVIEW=PASS
ICON_MUTATION=false
```

If those facts no longer reproduce on the current environment/head, requalify
before mutation.

## Exact-tab ownership strategy

The admitted first strategy is a temporary, content-minimal, cryptographically
or otherwise collision-resistant unique title marker in the disposable target
tab.

Conceptual flow:

```text
disposable target tab
  -> emit TB-ICON-PROBE-<unique nonce> as temporary title marker
  -> attach diagnostics helper
  -> enumerate tab model/UI objects
  -> correlate marker to TabViewItem candidate(s)
  -> require MATCH_COUNT == 1
  -> mutation admitted
```

The marker must contain no workspace path, prompt, user identity, token, or
session content.

## Mutation authority

Only this state permits icon mutation:

```text
MATCH_COUNT=1
TARGET_IDENTITY_UNAMBIGUOUS=true
```

These states must perform zero mutation:

```text
MATCH_COUNT=0
MATCH_COUNT>1
TARGET_DISAPPEARED=true
TARGET_IDENTITY_CHANGED=true
```

Do not use active-tab state, visual ordinal/index, guessed window order,
process-enumeration order, or child offsets as substitute ownership proof.

## Snapshot contract

Before any write, capture enough semantic state to restore the target exactly.

Required proof:

```text
ICON_SNAPSHOT=PASS
ORIGINAL_ICON_STATE_CLASS=<none|custom|profile/default|other_proven_class>
```

Prefer preserving the original `IconSource` object/reference or an exact
supported representation. Do not redefine "restore" as assigning a generic
icon after mutation.

## Probe icon

Use a simple deterministic test asset created specifically for mechanism proof.
It should be visually obvious and structurally simple.

Do not use the final TabBeacon logo as the only mutation proof; mechanism and
branding must remain separable.

Probe asset requirements:

```text
LOCAL_TEST_ASSET=true
NO_EXTERNAL_URL=true
NO_SCRIPT=true
NO_NETWORK_FETCH=true
```

## Mutation proof

After exact correlation:

1. assign the deterministic probe icon using only the admitted XAML/object
   mechanism;
2. re-read model/UI state if possible;
3. obtain visual proof from the disposable WT;
4. verify non-target tabs remain unchanged;
5. restore original icon state;
6. verify restored model/UI/visual state;
7. restore/retire the temporary title marker;
8. detach and close the disposable target.

Required evidence:

```text
ICON_MUTATION=PASS
VISUAL_ICON_CHANGED=true
MODEL_ICON_CHANGED=true_OR_NOT_APPLICABLE
TARGET_TAB_ONLY=true
NON_TARGET_ICON_CHANGES=0
ICON_RESTORE=PASS
TITLE_MARKER_RETIRED=true
```

## Mandatory negative tests

Run bounded negative cases before broad reliability work:

- marker absent -> zero mutation;
- duplicate marker -> zero mutation;
- target closes after enumeration but before write -> zero wrong-tab mutation;
- target identity changes before write -> refuse/re-correlate according to
  explicit safe policy;
- helper fails before snapshot -> zero mutation;
- helper fails after mutation -> bounded best-effort restore path and evidence;
- pre-existing icon -> exact restore;
- no explicit prior icon -> exact semantic restore.

If helper failure after mutation cannot be made safely restorable, record it as
a major feasibility finding for G86 rather than hiding it.

## Wrong-tab zero-tolerance gate

The central hard gate is:

```text
WRONG_TAB_MUTATION=0
```

Any observed mutation of a non-target tab is an immediate rejection of the
current correlation/mutation design. Preserve the evidence and stop broadening
tests until the design is independently reassessed.

A later successful rerun does not cancel an observed wrong-tab event.

## Production separation

Do not add a normal user command such as `tabbeacon icon ...` in G85.
Do not wire the helper into Codex/Agy Hooks, workers, Control Center, setup, or
normal presentation backends.

If experimental code is stored in the repository, keep it clearly isolated,
non-installed, and excluded from normal package/runtime surfaces unless the
repository build structure requires a narrowly feature-gated test harness.

Required:

```text
PRODUCTION_RUNTIME_NATIVE_ICON=false
NORMAL_CLI_NATIVE_ICON=false
CARGO_PACKAGE_EXPERIMENTAL_HELPER=false_OR_EXPLICITLY_REVIEWED
```

## Privacy / safety

Persist only structural/correlation facts and probe evidence. Do not capture
unrelated tab content.

No elevation, private ABI, signature scanning, memory patching, Windows Terminal
package modification, or settings mutation is allowed.

## Risk vector

```text
CODE_CHANGED=experimental_or_harness
PRESENTATION_CHANGED=experimental_target_only
PROVIDER_CHANGED=false
USER_PERSISTENT_CONFIG_CHANGED=false
SECURITY_OR_PRIVACY_CHANGED=true
EXPERIMENTAL_INSTRUMENTATION=true
RELEASE_BOUNDARY=false
```

Require focused independent review of correlation authority, race handling,
restore semantics, and process-instrumentation boundaries.

## Acceptance

```text
EXACT_TAB_CORRELATION=PASS
MATCH_COUNT_REQUIRED=1
ZERO_MATCH_MUTATION=0
DUPLICATE_MATCH_MUTATION=0
ICON_SNAPSHOT=PASS
ICON_MUTATION=PASS
VISUAL_ICON_CHANGED=true
TARGET_TAB_ONLY=true
NON_TARGET_ICON_CHANGES=0
ICON_RESTORE=PASS
WRONG_TAB_MUTATION=0
TARGET_CLOSE_RACE=FAIL_OPEN
HELPER_FAILURE=BOUNDED
TITLE_MARKER_RETIRED=true
PRODUCTION_WT_ATTACHED=false
ACTIVE_OWNER_WT_TARGETED=false
PRODUCTION_RUNTIME_NATIVE_ICON=false
NORMAL_CLI_NATIVE_ICON=false
PRIVATE_ABI=false
SIGNATURE_SCANNING=false
MEMORY_PATCHING=false
ELEVATION=false
FOCUSED_SAFETY_REVIEW=PASS
```

## Stop conditions

Stop before G86 broad matrix if:

- `WRONG_TAB_MUTATION>0`;
- original icon state cannot be restored reliably;
- target-close races can redirect mutation to another tab;
- the mechanism needs forbidden private/elevated techniques;
- the disposable WT crashes or corrupts UI state under admitted use.

Such a stop should feed a likely `NO_GO` disposition rather than trigger unsafe
workarounds.

## Estimated effort

**6–10 effective engineering hours.**

## Next

`TB-G86 — Native Icon Reliability Matrix & Final Disposition`.