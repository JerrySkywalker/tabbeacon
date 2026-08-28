# TB-G86 — Native Icon Reliability Matrix & Final Disposition

## Status

PLANNED after accepted G85 or after an earlier feasibility finding that already
requires a durable NO-GO conclusion.

## Purpose

Convert the one-tab mechanism proof into a conclusive engineering decision.
Exercise the Native Tab Icon approach across lifecycle, concurrency, restore,
failure, and currently available Windows Terminal variation; preserve every
safety failure; then publish exactly one final disposition:

```text
GO_EXPERIMENTAL
GO_PRODUCTION_CANDIDATE
NO_GO
```

v0.7 does not productize native icon support in this Goal regardless of outcome.

## Entry requirements

Normal positive-path entry from G85:

```text
EXACT_TAB_CORRELATION=PASS
ICON_MUTATION=PASS
ICON_RESTORE=PASS
WRONG_TAB_MUTATION=0
```

If G85 stopped because of a hard safety violation, G86 may instead enter a
NO-GO closeout path. Do not force the unsafe mechanism through the full matrix
just to satisfy a checklist.

## Reliability matrix

### A. Single-window tab topology

Cover:

- one tab;
- two or more tabs;
- target first/middle/last;
- target active/inactive where the diagnostics model permits mutation without
  unsafe focus assumptions;
- non-target tabs with distinct icons/titles;
- target with an existing icon;
- target with no explicit icon.

Required:

```text
WRONG_TAB_MUTATION=0
NON_TARGET_ICON_CHANGES=0
RESTORE_FAILURE=0
```

### B. Multiple Windows Terminal windows

Launch multiple disposable stock WT windows/process relationships as actually
implemented by the installed Windows Terminal channel. Prove the unique marker
cannot accidentally authorize an identically structured tab in another window.

The correlation system must bind the target within the admitted diagnostics
scope rather than assuming a global active window.

### C. Split panes

Cover tabs containing multiple panes and active-pane changes. Native tab icon
ownership must remain tab-level and must not silently follow pane ordinal/CWD.

### D. Title lifecycle

Exercise:

- marker set;
- marker correlation;
- title rename before mutation;
- title rename after mutation;
- normal TabBeacon title/activity updates in the disposable test session;
- marker retirement/normal title restoration.

A title race must cause refusal/re-correlation, not mutation of a guessed tab.

### E. Close/disappearance races

At minimum:

- target tab closes before enumeration;
- target closes after enumeration but before mutation;
- target closes after mutation but before restore;
- target window closes;
- helper loses target process/site.

Required principle:

```text
TARGET_DISAPPEARED => NO_NEW_MUTATION
```

If restore becomes impossible because the target itself no longer exists,
classify separately from a restore failure on a surviving tab; do not claim the
same semantics.

### F. Settings / UI refresh

Exercise settings reload or equivalent supported WT refresh that may reconstruct
tab UI objects. Determine whether the stored/restorable state remains valid or
whether the mechanism must re-correlate.

### G. Helper/diagnostics failures

Cover:

- attach failure;
- callback/site initialization failure;
- helper exits before mutation;
- helper exits after mutation;
- detach failure;
- diagnostics target already in a conflicting state if the platform exposes
  such a case.

Windows Terminal must survive. Any restoration guarantee must be evidence-based.

### H. Marker ambiguity

Explicitly test:

```text
MATCH_COUNT=0
MATCH_COUNT=2+
```

Both must produce zero icon writes.

### I. Windows Terminal variation

Where practical and safely available, test more than one current stock channel
or version (for example Stable and Preview). Record exact versions. Do not
install/alter Owner production Terminal solely to manufacture a matrix; use
isolated/disposable availability.

The purpose is not to guarantee all future builds but to learn whether the
approach is obviously tied to a brittle one-build object shape.

## Metrics / evidence

Record bounded per-scenario facts, not unrelated terminal content.

At minimum aggregate:

```text
SCENARIOS_RUN=<count>
ICON_MUTATIONS_ATTEMPTED=<count>
EXPECTED_TARGET_MUTATIONS=<count>
WRONG_TAB_MUTATIONS=<count>
NON_TARGET_ICON_CHANGES=<count>
RESTORE_FAILURES=<count>
WT_CRASHES=<count>
FAIL_OPEN_REFUSALS=<count>
```

Keep scenario receipts tied to exact experimental code/source refs.

## Hard safety disposition rules

The studied route cannot be `GO_PRODUCTION_CANDIDATE` if any of these is true:

```text
WRONG_TAB_MUTATIONS>0
RESTORE_FAILURES>0 on surviving target tabs
WT_CRASHES>0 attributable to admitted interaction
ELEVATION_REQUIRED=true
PRIVATE_ABI_REQUIRED=true
SIGNATURE_SCANNING_REQUIRED=true
MEMORY_PATCHING_REQUIRED=true
WT_PACKAGE_MUTATION_REQUIRED=true
```

Wrong-tab evidence is a direct NO-GO for the current design unless G86 first
returns to a separately reviewed correlation redesign and reruns the entire
relevant matrix. Do not simply ignore the failed architecture.

## Final disposition definitions

### GO_EXPERIMENTAL

Use when the mechanism works safely and repeatably in the admitted matrix but
has significant implementation/version fragility or process-instrumentation
cost that does not justify normal product integration yet.

Required consequences:

```text
PRODUCTION_RUNTIME_NATIVE_ICON=false
NORMAL_CLI_NATIVE_ICON=false
EXPERIMENTAL_ARTIFACTS_NON_INSTALLED=true
```

### GO_PRODUCTION_CANDIDATE

Use only when the mechanism has strong exact-tab, restore, lifecycle, and
multi-build evidence without forbidden techniques.

This authorizes only a **future planning decision**. It does not authorize G86
or G90 to insert native-icon instrumentation into the v0.7 production runtime.

Required:

```text
PRODUCTION_RUNTIME_NATIVE_ICON=false
FUTURE_PRODUCTIZATION_PLAN_REQUIRED=true
```

### NO_GO

Use when correlation, restore, target survival, API stability, or safety is not
reliable enough for TabBeacon's fail-open model.

NO_GO is not a failed v0.7 release. It is a successful research conclusion.
Production remains on the stable title-mark/terminal-control backend.

## ADR / design output

Create/update durable documentation under the established ADR/design/research
structure containing:

- exact Windows Terminal/source versions;
- admitted XAML mechanism;
- correlation design;
- restore design;
- matrix summary;
- all safety-significant failures;
- final disposition and rationale;
- explicit statement that v0.7 runtime integration is not implied.

`docs/design/native-tab-icon.md` may be created later in G88 as the newcomer-
facing design explanation; G86 owns the authoritative engineering disposition
record.

## Experiment code retention

### GO_EXPERIMENTAL / GO_PRODUCTION_CANDIDATE

Experimental code may remain only if useful and reviewable, preferably under:

```text
experiments/native-tab-icon/
```

It must be non-installed and excluded from normal package/runtime paths unless a
narrow build fixture is explicitly justified.

### NO_GO

Retain only the minimum code/evidence needed for reproducibility. It is
acceptable to merge the ADR/research conclusion without keeping a hazardous
helper if its maintenance value is low.

## Risk vector

```text
CODE_CHANGED=experimental_or_docs
PRESENTATION_CHANGED=experimental_target_only
PROVIDER_CHANGED=false
USER_PERSISTENT_CONFIG_CHANGED=false
SECURITY_OR_PRIVACY_CHANGED=true
EXPERIMENTAL_INSTRUMENTATION=true
RELEASE_BOUNDARY=false
```

A focused independent review is mandatory for the final disposition.

## Acceptance

```text
RELIABILITY_MATRIX=COMPLETE_OR_SAFETY_TERMINATED
WRONG_TAB_MUTATION=<0_or_recorded_failure>
RESTORE_FAILURES=<count>
WT_CRASHES=<count>
MULTI_TAB_TESTED=true_or_reasoned_NA
MULTI_WINDOW_TESTED=true_or_reasoned_NA
SPLIT_PANE_TESTED=true_or_reasoned_NA
TITLE_RACES_TESTED=true
CLOSE_RACES_TESTED=true
HELPER_FAILURES_TESTED=true
AMBIGUOUS_MATCH_FAIL_OPEN=PASS
WT_VARIATION_TESTED=<true|bounded_unavailable>
NATIVE_TAB_ICON_DISPOSITION=<GO_EXPERIMENTAL|GO_PRODUCTION_CANDIDATE|NO_GO>
ADR_WRITTEN=true
PRODUCTION_RUNTIME_NATIVE_ICON=false
NORMAL_CLI_NATIVE_ICON=false
ACTIVE_OWNER_WT_TARGETED=false
FOCUSED_INDEPENDENT_REVIEW=PASS
```

## Estimated effort

**5–8 effective engineering hours**, unless a hard G85 safety failure permits a
shorter evidence-backed NO-GO closeout.

## Next

`TB-G87 — Brand System & README v2`.

Native-icon productization, if ever admitted, belongs to a later separately
planned release.