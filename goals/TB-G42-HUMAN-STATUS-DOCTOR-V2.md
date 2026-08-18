# TB-G42 — Human Status & Doctor v2

## Status

COMPLETE. Default `status` and `doctor` render the shared management projection
for humans, while `--json` and `--plain` retain their established contracts.
`TB-G43` is next.

## Purpose

Make the default status and doctor output readable by humans without weakening the structured interfaces used by Codex and automation.

## Status UX

`tabbeacon status` answers “what is the current state?” Healthy output should normally fit in one terminal screen and group information such as:

```text
Overall
Integration
Presentation
Runtime
```

Use concise status glyphs/labels with a monochrome-safe fallback. Avoid dumping every internal field when healthy.

## Doctor UX

`tabbeacon doctor` answers “what is wrong and what should I do next?”

Healthy output should summarize passed checks. Warning/failure output should prioritize the affected check, plain-language explanation, and a copy-friendly next action when one exists.

## Compatibility

```text
tabbeacon status          -> human-first
tabbeacon status --json   -> existing stable structured contract
tabbeacon status --plain  -> legacy/key-value form

tabbeacon doctor          -> human-first
tabbeacon doctor --json   -> existing stable structured contract
tabbeacon doctor --plain  -> legacy/key-value form
```

Do not bump the JSON schema in G42 unless a necessary field cannot be represented compatibly.

## Output requirements

- no ANSI dependence for meaning;
- no prompt/tool/session/credential leakage;
- copy/paste-friendly commands/instructions;
- width-aware but deterministic layout;
- redirected/non-TTY output remains useful and does not emit control sequences unexpectedly.

## Validation

- deterministic golden/structured text tests for healthy/warning/failure cases;
- no-color/monochrome tests;
- narrow-width tests;
- JSON regression tests;
- plain-mode compatibility tests;
- one final hosted CI.

Real Windows UIA is not required for text-only output formatting.

## Exit gate

```text
STATUS_DEFAULT=HUMAN_FIRST
DOCTOR_DEFAULT=HUMAN_FIRST
HEALTHY_STATUS_ONE_SCREEN=true
DOCTOR_FAILURE_HAS_NEXT_ACTION=true
STATUS_JSON_COMPATIBLE=true
DOCTOR_JSON_COMPATIBLE=true
LEGACY_PLAIN_MODE=true
NO_COLOR_MEANING_DEPENDENCY=true
CODE_CI=PASS
```

Estimated effort: **3–5 h**.
