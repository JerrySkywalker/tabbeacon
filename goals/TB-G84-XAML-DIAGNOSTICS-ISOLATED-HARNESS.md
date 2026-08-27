# TB-G84 — Isolated XAML Diagnostics Harness

## Status

PLANNED after accepted G83 when XAML Diagnostics remains the admitted Native
Tab Icon feasibility route.

## Purpose

Prove only the minimum instrumentation foundation required for later native-icon
research: create a disposable stock Windows Terminal target, attach through the
current documented XAML Diagnostics mechanism, enumerate enough XAML/model state
to identify tab UI objects, and detach/cleanup without crashing or mutating tab
icons.

**G84 must not mutate `IconSource`.**

## Preconditions

Required from G83:

```text
V07_SCOPE_FROZEN=true
WT_SOURCE_REVALIDATED=true
XAML_ROUTE_STILL_RELEVANT=true
DISPOSABLE_WT_POLICY_FROZEN=true
```

If an official supported icon API superseded XAML, do not execute this file
unchanged.

## Isolation architecture

Use a purpose-created disposable Windows Terminal process/window that is not
hosting the Codex process running this Goal.

Required:

```text
TARGET_WT=DISPOSABLE
TARGET_CONTAINS_OWNER_WORK=false
TARGET_HOSTS_DEVELOPMENT_CODEX=false
PRODUCTION_WT_ATTACHED=false
```

Use an experiment root outside the canonical checkout for binaries/evidence,
consistent with repository V-drive ownership conventions. Experiment files must
not pollute production TabBeacon state roots.

## Harness responsibilities

The harness should provide bounded operations conceptually equivalent to:

1. launch a disposable stock Windows Terminal instance/window with a unique
   harmless marker namespace;
2. prove target process/window identity;
3. invoke the documented XAML Diagnostics attachment path;
4. receive/establish the supported diagnostics callback/site required by that
   API;
5. enumerate the XAML object/visual tree sufficiently to locate the tab control
   hierarchy and candidate `TabViewItem` objects;
6. record only content-minimal structural evidence;
7. detach/terminate the diagnostics helper cleanly;
8. close the disposable terminal and remove exact-owned temporary artifacts when
   allowed by environment guards.

## Privacy boundary

Do not persist arbitrary terminal text, prompt/assistant/tool content, command
history, environment dumps, authentication material, or raw unrelated window
content.

Allowed evidence includes bounded structural facts such as:

```text
TARGET_PROCESS_ID=<ephemeral test pid>
ATTACH_RESULT=<status>
TABVIEW_COUNT=<count>
TABVIEWITEM_COUNT=<count>
OBJECT_TYPE_SUMMARY=<bounded known types>
DETACH_RESULT=<status>
```

Ephemeral process IDs may be used in local evidence but need not enter public
release docs.

## No mutation boundary

G84 must not intentionally set:

- `IconSource`;
- title beyond harmless launch/correlation preparation needed for enumeration;
- tab color;
- terminal settings;
- Windows Terminal package files;
- production TabBeacon config/state.

Required:

```text
ICON_MUTATION=false
WT_SETTINGS_MUTATED=false
WT_PACKAGE_MUTATED=false
```

## Failure behavior

Attachment failure, diagnostics callback failure, unsupported object model, or
missing tab hierarchy must return a truthful `BLOCKED`/`FAIL` result. Do not
fall back to memory scanning, private structure offsets, injection frameworks,
or elevated process manipulation.

Explicitly forbidden escalation:

```text
PRIVATE_ABI=false
SIGNATURE_SCANNING=false
MEMORY_PATCHING=false
ELEVATION=false
UNRELATED_DLL_INJECTION=false
```

A diagnostics component required by the documented XAML API is within the
admitted experiment boundary; arbitrary process-injection tooling is not.

## Lifecycle tests

At minimum prove:

- launch disposable WT;
- attach once;
- enumerate expected tab-control structural objects;
- detach;
- disposable WT remains responsive;
- repeat attach/enumerate/detach on a fresh disposable WT;
- attach failure path leaves target unchanged;
- helper exit does not crash/hang target;
- target close during/after enumeration is bounded and safe.

No requirement exists yet to correlate one exact tab semantically; that belongs
to G85.

## Testing / evidence

Prefer deterministic machine receipts plus a small focused manual/visual check
only where necessary to prove that the target terminal remained healthy.

Do not require Owner to inspect production tabs.

## Risk vector

```text
CODE_CHANGED=experimental_only_or_harness
PRESENTATION_CHANGED=false
PROVIDER_CHANGED=false
USER_PERSISTENT_CONFIG_CHANGED=false
SECURITY_OR_PRIVACY_CHANGED=true
EXPERIMENTAL_INSTRUMENTATION_STARTED=true
RELEASE_BOUNDARY=false
```

Because process instrumentation begins here, require a focused independent
safety/privacy review before declaring the harness suitable for G85.

## Acceptance

```text
DISPOSABLE_WT_LAUNCH=PASS
XAML_DIAGNOSTICS_ATTACH=PASS
XAML_TAB_TREE_ENUMERATION=PASS
XAML_DIAGNOSTICS_DETACH=PASS
REPEATABILITY=PASS
TARGET_WT_RESPONSIVE_AFTER_DETACH=true
HELPER_FAILURE_TARGET_SURVIVES=true
PRODUCTION_WT_ATTACHED=false
ACTIVE_OWNER_WT_TARGETED=false
ICON_MUTATION=false
WT_SETTINGS_MUTATED=false
WT_PACKAGE_MUTATED=false
PRIVATE_ABI=false
SIGNATURE_SCANNING=false
MEMORY_PATCHING=false
ELEVATION=false
FOCUSED_SAFETY_REVIEW=PASS
```

## Stop conditions

Stop without G85 when:

- documented attachment cannot be made reliable;
- useful tab structures cannot be enumerated without private ABI/offset tricks;
- target WT crashes/hangs because of the admitted mechanism;
- exact isolation from the development/Owner terminals cannot be maintained.

A clean G84 failure is useful feasibility evidence; do not broaden scope to
force success.

## Estimated effort

**5–8 effective engineering hours.**

## Next

`TB-G85 — Exact-Tab Correlation & Native Icon Mutation/Restore`.