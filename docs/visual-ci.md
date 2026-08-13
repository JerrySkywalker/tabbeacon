# Visual CI Contract

## Purpose

TabBeacon's core feature is visible terminal state. Unit tests alone cannot prove that Windows Terminal rendered the expected title, animation, or tab color.

Visual CI therefore acts as a machine oracle for presentation behavior.

## Deterministic input

Visual CI must use a local fixture, not a real Codex/model request. The fixture drives known semantic states on a controlled timeline.

The minimum fixture state set is:

```text
Ready
Working
ResultReady
Approval
Warning
Interrupted
Failed
Reset
```

## Verification layers

### Title

Primary semantic assertion: UI Automation exposes the expected tab name/title.

Screenshot evidence is retained, but OCR is not the primary title oracle.

### Animation

Capture several frames over a bounded interval and crop the progress/icon ROI. `Working` passes when the ROI demonstrates sufficient frame change; states expected to be stationary must not show the same animated pattern.

### Color

Sample a tab-background ROI that excludes text/icons and evaluate a tolerance range (RGB/HSV or equivalent). Do not require full-window golden-pixel equality.

## Environment contract

Real visual CI requires:

- stock Windows Terminal version recorded in evidence;
- approved font/theme/profile;
- fixed or recorded DPI/scaling;
- controlled window size;
- an **interactive Windows desktop session**;
- no reliance on Session-0/service UI visibility.

A runner process existing is not enough to prove the GUI environment is suitable.

## Evidence bundle

Each run should preserve:

- exact source SHA;
- runner/machine identity;
- Windows version and Terminal version;
- full-window screenshot;
- tab crops;
- UIA dump;
- frame-delta metrics;
- color metrics;
- assertion summary;
- failure overlays when an assertion fails.

## Failure semantics

A capture failure is not a product-color failure. Distinguish:

- launch failure;
- UIA target-resolution failure;
- capture failure;
- assertion failure;
- runner/session precondition failure.

If the environment cannot provide trustworthy pixels, the visual result is `BLOCKED`/`UNPROVEN`, not PASS.
