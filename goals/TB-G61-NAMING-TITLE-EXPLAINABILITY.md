# TB-G61 — Naming / Title Explainability

## Status

PLANNED after accepted G60.

## Purpose

Expose the deterministic reasoning already present in Adaptive Naming v2 and add a privacy-safe `Why this title?` surface so unexpected title identity/state can be diagnosed without reading internal files.

## Workspace candidate explanation

The naming engine already retains strategy, score, display width, and component scores. G61 makes that existing data first-class Human/TUI information rather than changing the scoring policy.

Workspace candidate presentation should include at least:

```text
#  Alias   Strategy                 Score
1  OWH     initialism                 92
2  OCWH    balanced-prefix            84
3  OPWH    token-compression          79
4  OWH-a1  hash-fallback            -128
```

Selected/detail view should show the actual accepted component breakdown:

```text
token coverage
acronym preservation
recognizable prefix
balanced representation
display width
information loss
trivial alias penalty
redundancy penalty
collision pressure
strategy adjustment
TOTAL
```

Do not invent a simplified score that differs from the engine.

## Formula / policy display

Human documentation/TUI may describe the additive formula and current integer weights. Structured machine output should expose stable named components and values, not localized formula text.

G61 must not make score weights user-editable. `adaptive-v2` determinism, stable generated alias history, collision behavior, and explicit local override semantics remain unchanged.

## CLI

Enhance existing alias explanation or add bounded surfaces so users can obtain machine and Human explanation without entering TUI. The exact command shape may reuse `tabbeacon alias explain` where compatible rather than proliferating redundant commands.

## Why this title?

Add a top-level or nested read-only surface conceptually:

```text
tabbeacon explain title
```

and a Control Center action/panel.

It should answer, using only safe typed state:

```text
Provider
semantic phase/attention/health
root workspace display hint
root binding source
identity class (Git remote/root-history/directory fallback, not raw private identity)
automatic alias
override alias if any
effective alias
naming policy
title owner
activity channel
provider badge policy/value
mismatch/conflict observation if any
```

The explanation should make it obvious whether a surprising title comes from workspace binding, alias scoring/override, provider state, presentation configuration, or a degraded/unavailable source.

## Privacy boundary

Do not expose raw native session IDs, raw canonical private identities, private absolute paths, prompt/tool/assistant content, Hook commands, raw agent IDs, terminal handles, or process internals in normal Human output.

Machine/debug transports may use stable digests only where already admitted and privacy-reviewed.

## TUI

Workspace screen should show top candidates with score and strategy. An explanation view should be width-aware, localized, keyboard-only, and preserve dirty drafts. `Why this title?` is read-only and must not rebind workspace or apply settings.

## Testing

- candidate order exactly matches engine order;
- displayed total equals sum of components;
- en-US/zh-CN component labels;
- CJK/Unicode candidate explanation width safety;
- collision pressure visible without leaking other workspace identity;
- generated vs override effective alias explanation;
- root-anchor source/mismatch explanation;
- degraded/unavailable provider/workspace facts remain truthful;
- Human/JSON/plain locale boundary;
- no private identity/path/session content in default outputs;
- TUI narrow/no-color/help interaction.

## Risk vector

```text
CODE_CHANGED=true
PRESENTATION_CHANGED=true
PROVIDER_CHANGED=false
USER_PERSISTENT_CONFIG_CHANGED=false
SECURITY_OR_PRIVACY_CHANGED=true
RELEASE_BOUNDARY=false
```

## Acceptance

```text
WORKSPACE_SCORE_VISIBLE=true
WORKSPACE_STRATEGY_VISIBLE=true
SCORE_COMPONENTS_VISIBLE=true
DISPLAYED_TOTAL_EXACT=true
NAMING_WEIGHTS_USER_EDITABLE=false
WHY_THIS_TITLE=PASS
ROOT_BINDING_EXPLAINABLE=true
ALIAS_SOURCE_EXPLAINABLE=true
PRIVATE_PATHS_EXPOSED=false
RAW_SESSION_IDS_EXPOSED=false
ZH_CN_EXPLAIN=PASS
EN_US_EXPLAIN=PASS
PRIVACY_REVIEW=PASS
CODE_CI=PASS
```

## Estimated effort

**6–9 effective engineering hours.**

## Next

`TB-G62 — Multi-Provider Management Foundation`.