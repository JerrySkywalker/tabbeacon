# TB-G59 — Root Workspace Anchor & Subagent Observability

## Status

PLANNED after accepted G58.

## Purpose

Make terminal-tab workspace identity session-scoped instead of recomputing the visible alias from every accepted provider event's `cwd`, while retaining privacy-safe subagent/background observability.

## Problem statement

The Codex normalizer already classifies explicit subagent lifecycle events and events carrying `agent_id`/`agent_type` as subagent-owned and prevents them from mutating root presentation. However, accepted root-visible events still resolve workspace identity from that event's `cwd` immediately before title rendering.

In tool/subagent-heavy flows, provider-visible cwd can point at another repository, a temporary worktree, or a nested execution directory. That must not silently rename the root terminal tab.

## Root Workspace Anchor

Introduce a typed session-scoped binding, conceptually:

```text
RootWorkspaceAnchor {
  provider_session_digest,
  canonical_workspace_identity,
  effective_alias,
  binding_source,
  bound_at_generation,
  mismatch_observation,
}
```

Raw native session IDs remain internal and non-Human.

### Binding authority

Normative initial policy:

| Event | Root binding authority |
| --- | --- |
| SessionStart startup | may establish/rebind |
| SessionStart resume | may establish/rebind after compatibility validation |
| SessionStart clear | may establish/rebind |
| UserPromptSubmit | may establish only if no safe anchor exists |
| PreToolUse/PostToolUse | cannot rebind |
| PermissionRequest | cannot rebind |
| Stop | cannot rebind |
| PreCompact/PostCompact | preserve |
| SubagentStart/SubagentStop | never rebind |
| any event with proven subagent identity | never rebind |

If a provider version demonstrates materially different semantics, freeze the accepted compatibility rule in its provider profile rather than weakening the root-anchor invariant globally.

## Mismatch behavior

When an accepted root event has `cwd` resolving to a different workspace than the anchor:

- keep title/root alias anchored;
- record a bounded ephemeral mismatch fact for explainability/diagnostics;
- do not persist the alternate path or raw identity in Human state;
- do not treat the mismatch as failure unless independent evidence proves an error.

A later authorized SessionStart may replace the anchor.

## Subagent/background projection

Stop throwing away all useful subagent lifecycle semantics after isolation. Maintain only bounded count/state metadata when evidence proves it:

```text
active_subagents
background_tasks (only if provider evidence supports it)
root_binding_stable
workspace_mismatch_observed
```

Do not expose raw agent IDs, raw task IDs, prompts, assistant content, tool content, or persistent activity history.

## Sessions / status integration

Sessions should be able to display privacy-safe facts such as:

```text
TB — Codex — working
Subagents 3
Root workspace stable
```

Provider-aware Sessions formatting may complete in G62; G59 owns the underlying typed projection and correctness.

## Persistence / cleanup

Anchor state may use the existing durable generation/session state root only to the minimum extent required to survive one-shot Hook invocations. It must be bounded, ownership-safe, stale-cleanable, and content-minimal.

Session end/TTL/generation supersession must retire stale anchors so a new unrelated session cannot inherit an old workspace.

## Testing

Required families:

- SessionStart establishes anchor;
- same-session tool event with different cwd cannot change title alias;
- temporary linked worktree/subdirectory cannot steal root identity;
- explicit subagent events cannot rebind root;
- subagent-attributed ordinary events cannot rebind root;
- new authorized SessionStart can rebind;
- resume semantics follow admitted provider profile;
- missing anchor fallback is deterministic and safe;
- stale/ended session anchor cleanup;
- concurrent tabs/sessions never share anchors;
- mismatch state contains no private path/raw session/agent content;
- real-Codex or representative bounded provider acceptance reproduces the reported class of cwd drift;
- real-WT title remains stable during subagent/tool activity.

## Risk vector

```text
CODE_CHANGED=true
PRESENTATION_CHANGED=true
PROVIDER_CHANGED=true   # Codex workspace-binding semantics
USER_PERSISTENT_CONFIG_CHANGED=false
SECURITY_OR_PRIVACY_CHANGED=true
RELEASE_BOUNDARY=false
```

Use focused provider acceptance at the minimum required level, one privacy review, and representative title proof. Do not broaden into App Server work.

## Acceptance

```text
ROOT_WORKSPACE_IS_SESSION_SCOPED=true
ROOT_BINDING_SOURCE_TYPED=true
LATEST_EVENT_CWD_NOT_TITLE_AUTHORITY=true
SUBAGENT_CANNOT_REBIND_ROOT=true
TOOL_WORKTREE_CANNOT_REBIND_ROOT=true
AUTHORIZED_SESSIONSTART_REBIND=true
SUBAGENT_COUNT_OBSERVABLE=true
RAW_AGENT_IDS_HUMAN_EXPOSED=false
RAW_PRIVATE_PATHS_HUMAN_EXPOSED=false
TITLE_ALIAS_STABLE_UNDER_SUBAGENTS=true
CODE_CI=PASS
```

## Estimated effort

**8–12 effective engineering hours.**

## Next

`TB-G60 — Hook Inspector & Trust Explainability`.