# TB-G52 — Adaptive Workspace Naming Engine v2

## Status

PLANNED after accepted G51.

## Purpose

Replace the current shortest-initialism-first abbreviation policy with a deterministic, explainable, scoring-based naming engine that handles common project naming styles and Unicode/CJK names while preserving offline operation and existing stable aliases.

This Goal builds and proves the engine. It does **not** silently migrate existing registry assignments; G53 owns persistence and user preference adoption.

## Current compatibility baseline

The current engine already provides useful deterministic behavior for separator/camel/acronym boundaries, for example:

```text
jerry-dotfiles          -> JD
workstation-manager     -> WM
opencode-workspace-hub  -> OWH
jerry-proxy-control     -> JPC
OpenCode Workspace_Hub  -> OCWH
```

It also provides readable expansion plus stable hash fallback and collision handling. These properties remain.

The weakness is candidate preference: shortest readable initialism wins, which can produce low-information aliases for single-token or unusual names.

## Engine architecture

Introduce a pure engine conceptually equivalent to:

```text
NameAnalysis {
  normalized_name,
  tokens[],
  style_hints[]
}

AliasCandidate {
  alias,
  strategy,
  score,
  rationale/components
}

AdaptiveNamingPolicy::candidates(...)
AdaptiveNamingPolicy::select(...)
```

No filesystem, network, registry mutation, Hook state, or terminal state belongs inside the scoring engine.

## Unicode normalization and width

Use a deterministic Unicode normalization policy before tokenization. Prefer a conservative normalization such as NFC unless a stronger normalization is explicitly justified by fixtures; do not silently erase meaningful brand distinctions merely to simplify ASCII handling.

Measure presentation budgets by terminal display width, not only `.chars().count()`.

Required support:

- grapheme-safe truncation;
- normal and CJK terminal display width;
- mixed Latin/CJK values;
- no split combining sequences.

Potential implementation crates such as `unicode-normalization`, `unicode-segmentation`, `unicode-width`, or an equivalent minimal stack are implementation choices, not product dependencies by decree.

## Tokenization contract

Recognize at least:

```text
kebab-case
snake_case
space separated
dot.separated
camelCase
PascalCase
acronym-to-word boundaries: XMLHttpRequest -> XML | Http | Request
letter/digit boundaries where meaningful
CJK runs
mixed Latin/CJK segments
```

Preserve all-uppercase acronyms as informative tokens. Do not reduce `XML`, `API`, `CLI`, `SSA`, etc. to arbitrary lowercase word fragments before scoring.

## Candidate families

Generate multiple deterministic candidates where applicable:

```text
INITIALISM
ACRONYM_AWARE
BALANCED_PREFIX
SINGLE_TOKEN_READABLE_PREFIX
CONSONANT_SKELETON      # Latin only when useful
UNICODE_PREFIX
TOKEN_COMPRESSION
HASH_FALLBACK
```

Candidate generation is bounded. Do not enumerate an exponential search space.

Examples are contract-shaping rather than frozen exact outputs until the corpus is accepted:

```text
opencode-workspace-hub -> OWH / OCWH / OPWH / ...
OpenCodeWorkspaceHub   -> OCWH / OWH / ...
XMLHttpRequest         -> XHR / XMLHR / ...
tabbeacon              -> TAB / TBCN / TABBEA / ...   # not simply T by default
project-v2-api         -> PV2A / P2A / ...
航天器设计工具             -> several readable CJK-width-bounded candidates
```

## Deterministic scoring

Use integer score components to avoid platform floating-point/tie ambiguity.

The model should reward:

```text
token coverage
important acronym preservation
recognizable prefix preservation
balanced representation across multiple tokens
reasonable target display width
retaining digits that distinguish versions/components
```

Penalize:

```text
one-character aliases for nontrivial names
severe information loss
awkward/unpronounceable compression when a clearer candidate exists
overlong display width
redundant characters
collision pressure
```

Recommended selection ordering:

```text
score descending
then preferred display-width distance
then strategy priority
then deterministic lexical/candidate-order tie-break
```

Exact weights must be justified by the accepted naming corpus and stored as source-level constants/tests, not tuned dynamically from user history.

No machine-learning model, network lookup, repository-language analysis, or AI call is permitted in the default naming engine.

## Display-width budget

Retain the existing compact-title intent. The implementation may keep current hard alias safety bounds while introducing a target readable width range rather than forcing every result to the shortest possible value.

Candidate policy should prefer a useful compact alias roughly in the 3–8 display-column range when the source name warrants it, while still allowing established short initialisms such as `OWH` or `JPC`.

Hard maximum remains bounded and must preserve title sanitization/safety.

## Collision integration boundary

The pure engine may accept a used-alias set or return ordered candidates for the existing registry to resolve. Collision handling remains deterministic.

Hash fallback stays a last-resort stable safety mechanism, not the default visible naming style.

## Explainability

Engine APIs should expose enough safe structured rationale for G53 `alias explain`, for example:

```text
Tokens      open | code | workspace | hub
Strategy    initialism
Candidate   OWH
Score       96
Components  coverage + acronym + width - loss
```

Do not expose canonical private paths or raw private identity merely to explain a display alias.

## Naming corpus

Create a versioned deterministic corpus with at least **50** cases before G52 completion.

Must include:

- existing v0.4 canonical examples;
- single-token project/brand names;
- 2/3/4/5+ token names;
- kebab/snake/dot/space/camel/Pascal variants of equivalent names;
- all-uppercase acronym boundaries;
- acronyms mixed with words and digits;
- version suffixes/prefixes;
- collision pairs;
- CJK-only names;
- mixed CJK/Latin names;
- combining/grapheme cases;
- names near width/hard-length limits;
- empty/invalid sanitization boundaries handled by existing typed inputs.

Not every corpus row must freeze one exact string when multiple equally good candidates are acceptable. Some rows may assert properties such as token coverage, max width, non-single-character output, stable top-N order, or equivalence across naming styles.

## Backward compatibility

G52 must not rewrite the production registry or existing assigned aliases. It may introduce `adaptive-v2` as a policy implementation behind tests/APIs, but switching new persisted assignments and migration behavior belongs to G53.

Existing v0.4 aliases remain valid `RepositoryAlias` values.

## Risk vector

```text
CODE_CHANGED=true
PRESENTATION_CHANGED=false
PROVIDER_CHANGED=false
USER_PERSISTENT_CONFIG_CHANGED=false
SECURITY_OR_PRIVACY_CHANGED=false
RELEASE_BOUNDARY=false
```

Ordinary focused tests + one final hosted exact-head code CI. No real WT, provider L4, or persistent-config audit unless the implementation unexpectedly broadens scope.

## Acceptance

```text
ADAPTIVE_NAMING_V2=PASS
NAMING_OFFLINE=true
NAMING_DETERMINISTIC=true
UNICODE_NORMALIZATION=PASS
GRAPHEME_SAFE=true
CJK_DISPLAY_WIDTH=PASS
STYLE_TOKENIZATION=PASS
ACRONYM_BOUNDARIES=PASS
SINGLE_TOKEN_NOT_TRIVIALLY_ONE_CHAR=true
CANDIDATE_SCORING=PASS
INTEGER_SCORING=true
DETERMINISTIC_TIEBREAK=true
HASH_FALLBACK=PASS
NAMING_CORPUS_CASES>=50
NAMING_CORPUS=PASS
EXISTING_REGISTRY_MUTATED=false
CODE_CI=PASS
```

## Non-goals

No user override persistence, registry migration, import/export, live TUI, provider changes, project-local config, network naming service, or machine-learning model.

## Estimated effort

**8–12 effective engineering hours.**

## Next

`TB-G53 — Local Workspace Preferences & Alias Control`.
