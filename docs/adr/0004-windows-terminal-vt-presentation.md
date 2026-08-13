# ADR 0004 — Windows Terminal VT Presentation

- Status: Accepted
- Date: 2026-08-13

## Decision

The first presentation backend uses terminal control sequences for title, progress state, and dynamic content-driven tab/frame color.

Tab color is a core v0.1 feature because the VT path has been verified on the target Windows Terminal environment, but it remains capability-gated with graceful fallback in case Windows Terminal changes the implementation.

## Consequences

No UI Automation injection, settings-file rewriting, DLL injection, or Windows Terminal fork is required for normal presentation. UI Automation is reserved for verification/visual CI, not product control.
