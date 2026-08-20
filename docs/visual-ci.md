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

The harness maps UIA tab bounds into the owned `PrintWindow` frame using that
specific window/frame dimension ratio. It does not apply a global DPI multiplier
blindly: Windows may expose either logical or physical UIA bounds for a given
Terminal window, while the captured pixel buffer is always frame-relative.

Target discovery has a bounded six-second window before the harness reports a
classified UIA-target failure. This covers normal Terminal creation latency
without turning a missing owned tab into an unbounded wait or an unrelated-window
lookup.

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

Each structured assertion also carries a failure category where applicable:
`PRODUCT_CODE_DEFECT`, `TEST_DEFECT`, `RUNNER_ENVIRONMENT_DEFECT`,
`EXTERNAL_DEPENDENCY`, `EVIDENCE_MISMATCH`, or `UNPROVEN`. Exact-head
disagreement is always `EVIDENCE_MISMATCH`; desktop, UIA target, and capture
preconditions are `RUNNER_ENVIRONMENT_DEFECT`, not product color failures.

## TB-G03 implementation boundary

The visual infrastructure is above the G02 presentation system under test:

```text
VisualTestCase -> FixtureDriver -> TerminalTestSession -> TargetLocator
    -> CaptureBackend -> CapturedFrame -> VisualOracle -> AssertionResult
    -> EvidenceWriter
```

The fixture driver adds only a unique, G02-title-policy-sanitized run token to
an existing provider-free fixture. It does not change the fixture's semantic
state, presentation priority, palette, or VT bytes. UI Automation is
verification-only for product behavior; the G03R harness may use its maintained
Windows foreground/focus wrapper only after exact run-token/title correlation
to make the owned fixture window capture-visible. It never sends input, changes
Terminal settings, or targets another window. `src/core` has no UIA, HWND,
image, or evidence-path types.

The primary backend is `win-screenshot-printwindow-full-owned-window`, a
window-only `PrintWindow(PW_RENDERFULLCONTENT)` capture of the HWND resolved
through the admitted exact UIA window title. It does not desktop-copy the UIA
rectangle, so transparent Terminal backgrounds cannot pull pixels from a
browser or another desktop window into the evidence. The harness establishes
owned foreground activation and target continuity before capture. Windows
Terminal may report keyboard focus on a
child element rather than its top-level UIA Window, so that diagnostic is
recorded but is not mistaken for a foreground failure. Failure to establish
the owned foreground condition, missing handle, or any capture error is a
capture-preflight blocker rather than a color or animation failure. The capture
trait permits a later empirically justified Windows replacement without
changing oracle or evidence semantics.

## Deterministic oracle rules

Color uses deterministic upper/lower interior tab-background strip candidates
and records sample count, fixed-point RGB mean, median, and per-channel
variance. Non-default G02 colors are classified by a documented Euclidean mean
tolerance plus a variance limit that rejects text/icon/border-contaminated
samples. Ready and reset are compared against a same-run default baseline
rather than a theme-specific hard-coded RGB value.

Animation compares only a bounded progress/icon ROI across successive frames.
It records changed-pixel ratio and mean absolute RGB component delta. A pair
must cross both tested thresholds to be `ANIMATION_PRESENT`; stationary frames,
sub-threshold noise, insufficient frames, inconsistent dimensions, and blocked
capture remain distinct outcomes.

## Dependencies

TB-G03 adds only the following direct dependencies:

- `uiautomation` 0.25.0, Apache-2.0: maintained safe Rust UIA client and
  foreground wrapper. The standard library has no Windows UIA abstraction, and
  this keeps project code compatible with the repository's `unsafe_code =
  "forbid"` policy.
- `win-screenshot` 4.0.14, MIT OR Apache-2.0: maintained safe wrapper for
  window-only `PrintWindow(PW_RENDERFULLCONTENT)` RGBA buffers. The standard
  library and the UIA binding cannot capture an admitted HWND without sampling
  underlying desktop pixels.
- `serde` 1.0.229 and `serde_json` 1.0.151, both MIT OR Apache-2.0: stable,
  typed deterministic evidence serialization. The standard library has no JSON
  serializer/deserializer.
- `png` 0.18.1, MIT OR Apache-2.0: lossless RGBA evidence encoding. The
  standard library has no PNG encoder.
- `sha2` 0.10.9, MIT OR Apache-2.0: deterministic SHA-256 artifact and
  evidence-tree integrity records. The standard library has no SHA-256
  implementation.

No dependency is used for provider integration, UI control/injection, OCR,
whole-image golden comparison, or a cross-platform screenshot product feature.

## Local harness

The dedicated G03 binary has an `emit` mode and a bounded `run` harness. `emit`
is the child launched inside an owned Windows Terminal tab; it renders a named
G02 fixture, waits for a bounded interval, then writes the existing G02 reset
action. `run` is the outer supervisor and must be given an exact checked-out
SHA, a unique safe run ID, and a fresh owned evidence root:

```text
cargo run --locked --features visual-fixture --bin tabbeacon-visual-fixture -- run \
  --expected-head <40-lowercase-sha> \
  --run-id TB03-<unique-token> \
  --evidence-root target/visual-evidence \
  [--fixture working]
```

Without `--fixture`, it replays the complete G02 fixture set. `run` launches a
private `run-worker` process with a 90-second wall-clock limit. The worker is
admitted only by a one-time supervisor authorization for the exact staging run;
direct `run-worker` invocation cannot reach platform observation. UIA discovery,
owned-window activation, live-title sampling, and `PrintWindow` capture run
only in that worker. On expiry the supervisor uses the direct worker PID with
`taskkill /T /F`, then confirms a transient pre-termination owned-PID tree is
gone. If either taskkill or that confirmation cannot complete within its own
bounded cleanup step, the direct worker is still terminated but the result is
`BLOCKED` / `RUNNER_ENVIRONMENT_DEFECT`; a deadline is never a visual pass. The
transient PIDs are never written into evidence. The worker writes
`worker-supervision.json` into its staged bundle, and
the supervisor independently recomputes every staged artifact digest, validates
the manifest/run/head fields, then promotes the complete bundle atomically to
the final evidence directory. A `PASS` summary requires
`EXPECTED_HEAD == CHECKED_OUT_HEAD == VISUAL_HEAD`; the binary uses a classified
nonzero exit for `BLOCKED`, `UNPROVEN`, and `FAIL`. Final evidence is always
confined to a newly created
`<evidence-root>/<run-id>` directory; it refuses an existing run directory or
artifact name rather than overwriting it.

`--fixture root-workspace-anchor` first captures the normal `ready` color
baseline, then launches a distinct owned terminal tab whose child executes the
real Codex Hook runtime. That sequence binds a root workspace, observes an
alternate tool CWD and explicit subagent CWD, and proves that the live visible
title retains the root alias. The fixture owns only a temporary injected state
root; no Owner Hook configuration, Windows Terminal settings, or external
window is changed.

The runner writes `manifest.json`, `assertions.json`, `environment.json`,
`uia.json`, and a final `integrity.json`, plus target-only UIA diagnostics and,
when trusted capture executes, full-window, tab, ROI PNGs and per-fixture
color/frame-delta metrics. `integrity.json` is a sorted SHA-256 digest list and
tree hash for every other regular artifact; it intentionally excludes itself to
avoid a self-referential digest. The environment record intentionally excludes
user terminal text, environment variables, tokens, credentials, and unrelated
window screenshots.
