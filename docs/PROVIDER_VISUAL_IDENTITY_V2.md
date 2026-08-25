# Provider Visual Identity v2

Provider identity, runtime state, and workspace identity are independent axes.
The production title-mark fallback composes them in this order:

```text
[Provider] [Runtime state] [Workspace]

Codex ⠋ tabbeacon
Codex ○ tabbeacon
Agy ⠋ tabbeacon
```

The workspace alias is never rewritten to carry provider information. A state
transition changes only the middle slot; a provider transition changes only
the first slot. This gives same-workspace Codex and Agy panes independent,
deterministic identities.

## Registry model

`ProviderVisualIdentity` is product-owned fixed metadata:

- `provider_id`;
- `short_name`;
- `accessible_name`;
- `title_mark` for a capable future backend;
- `text_fallback` for the production title backend;
- optional declarative `native_icon_spec`.

Unknown IDs map to the fixed `Unknown` identity. Their raw values are not
rendered. The registry accepts no executable path, image path, URL, or
provider-supplied image payload. Current Codex and Agy identities have no
native icon specification because this train ships no third-party logo asset
without separately verified provenance and terminal support.

The existing `provider_badge` preference is retained as a visibility choice:
`off` suppresses the optional title identity, `auto` suppresses it for a
single admitted provider, and `always` requests it. It no longer changes the
workspace identity or encodes runtime state.

## Runtime and accessibility behavior

Codex and Agy resolve their identity from the same provider registry/model.
Identity does not grant compatibility, configuration, Hook, or trust
authority. Codex compatibility remains capability-derived; Agy admission
remains independently bounded.

The textual fallback uses the full fixed name (`Codex`, `Agy`, or `Unknown`),
while the accessible name is a stable provider label. Light/dark palette
selection changes decoration only and cannot replace provider or workspace
identity.

## Acceptance coverage

The presentation tests cover Codex, Agy, and unknown identity; provider
switching for a shared workspace; state transitions without provider churn;
separate same-workspace pane state; accessibility names; and the title-mark
fallback across both shipped themes.
