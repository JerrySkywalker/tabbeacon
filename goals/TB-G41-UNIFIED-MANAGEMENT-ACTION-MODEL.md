# TB-G41 — Unified Management / Action Model

## Status

PLANNED after accepted G40.

## Purpose

Create one human-management projection consumed by status, doctor, guided setup, and the future TUI so the frontends cannot diverge on health, remediation, or configuration semantics.

## Conceptual model

Names may differ, but the implementation should provide typed equivalents of:

```text
ManagementSnapshot
HealthIssue
RecommendedAction
ChangePlan
```

A HealthIssue should carry stable identity, severity, concise title/explanation, and a typed remediation classification when applicable.

Recommended actions need safety classification:

```text
READ_ONLY
MANUAL_ACTION
PREVIEWABLE_SAFE_REPAIR
OWNER_EXPLICIT_REQUIRED
UNSUPPORTED_AUTOMATION
```

## Required semantics

Examples:

- `hooks.review_required` -> manual action: launch Codex, open `/hooks`, review TabBeacon definitions;
- Windows Terminal application-title suppression -> previewable safe repair only when the existing ownership-safe policy subsystem proves scope;
- unsupported Codex profile -> explanation/upgrade guidance, not fabricated automatic support;
- malformed settings -> bounded explanation and explicit reset/repair option, never silent destructive overwrite.

## Frontend boundary

```text
NO_FRONTEND_DIRECT_CONFIG_WRITES=true
NO_FRONTEND_DIRECT_HOOK_WRITES=true
NO_FRONTEND_TRUST_BYPASS=true
```

Frontends request typed plans/actions from the shared layer. Existing stores/integration modules remain the ownership authorities.

## Privacy

No management snapshot/action may expose or persist prompt, assistant, tool, credential, raw hook payload, raw session/turn ID, or canonical private workspace identity.

## Validation

- deterministic issue/action mapping tests;
- same diagnostic condition projects consistently to doctor/guided/TUI adapters;
- action safety classification tests;
- settings/config mutation remains staged through existing stores;
- one final hosted code CI.

## Exit gate

```text
MANAGEMENT_MODEL=PASS
ACTION_SAFETY_CLASSES=PASS
FRONTEND_DIRECT_WRITES=false
HOOK_TRUST_AUTOMATION=false
PRIVACY_CONTRACT=PASS
CODE_CI=PASS
```

Estimated effort: **2–4 h**.
