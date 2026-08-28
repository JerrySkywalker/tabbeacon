# TabBeacon visual language

TabBeacon presents three independent concepts in a terminal tab:

| Slot | Meaning | Authority |
| --- | --- | --- |
| Provider identity | Which admitted integration supplied evidence. | Fixed and product-owned. |
| Runtime state | Whether evidence indicates work, readiness, attention, or a question. | Evidence-driven and fail-open. |
| Workspace identity | A stable offline-first alias for the current workspace. | Repository/workspace identity, not provider output. |

Title marks, spinner or indicator motion, tab color, and Windows Terminal
progress are presentation channels. They make state easier to see; they never
grant provider compatibility, Hook trust, configuration ownership, or process
control. A missing channel therefore degrades to a safe title/identity fallback
rather than changing the underlying coding-agent workflow.

The semantic palette is summarized in the
[state strip](../assets/brand/tabbeacon-state-strip.svg): muted neutral for
ordinary identity, cyan for active work, green for ready, amber for attention,
and rose for a question or blocked attention state. Color is corroborating
presentation, not a source of truth.

Provider identity remains visually distinct from state: a provider integration
may be configured but have no current session evidence, and a workspace alias
must not churn with every lifecycle event. This separation keeps the title
stable, explainable, and testable.

## Native tab icons

Native Windows Terminal tab icons are not a TabBeacon production channel. The
accepted current-host feasibility result is
[NO_GO](../research/WT_NATIVE_ICON_DISPOSITION.md): stock Windows Terminal has
an internal icon pipeline but no supported public bridge, and no safely isolated
disposable process was available for the only remaining instrumentation route.
`TitleMarkBackend` remains the production path.
