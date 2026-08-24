# Agy pre-admission qualification

## Status

This is preparation for `TB-G64`, not Agy provider admission.

```text
AGY_PRODUCTION_ADMISSION=false
AGY_PROVIDER_ENABLED=false
AGY_LOGIN_REQUIRED=false
OWNER_AGY_CONFIG_MUTATED=false
RAW_AGY_CONTENT_PERSISTED=false
PROJECT_LOCAL_CONFIG=false
```

The only production provider remains Codex. The ordinary Integrations catalog
now shows Agy as `known + unadmitted`, with qualification available and
production disabled. It is excluded from registered production IDs, runtime
Sessions, title badges, and guided Setup apply actions.

## Read-only audit snapshot (2026-08-22)

The public [Agy title documentation](https://antigravity.google/docs/cli/title/)
describes a command receiving JSON on stdin and returning a plain title on
stdout. The [status-line schema](https://antigravity.google/docs/cli/statusline)
lists `agent_state`, workspace fields, conversation identity, task count, and
approval-pending state, but it also includes content-capable fields such as
transcript path, account email, model, quota, and token data. The public
[Hooks documentation](https://antigravity.google/docs/hooks) likewise exposes
transcript/artifact locations, tool arguments, and error text.

The local direct, non-authenticating `agy --version` probe observed `1.1.17`.
The public source release was `1.1.17` at commit
`adfa9eb8b76d1f370a829115e71a05316f302b5f`; the documentation navigation was
labelled `1.1.14`. This is a version-drift diagnostic, not a compatibility or
production admission claim.

## Durable qualification primitives

`providers::agy` supplies only pre-admission machinery:

- a typed `AgyCapabilityProfile` fixed to `unadmitted` and
  `provider_enabled=false`;
- direct-command plan for literal `agy --version`, with no wrapper, PATH
  shadow, PTY host, or daemon; the later Owner-present invocation must pin an
  explicit native executable path and SHA-256 identity before it runs;
- version drift classifier that does not infer support from a newer version;
- title/status and Hook recorders that parse one disposable JSON payload and
  discard source content;
- candidate-only normalization, Root Workspace Anchor stability fixtures, and
  count-only background-task fixtures;
- a title/Windows Terminal protocol harness that records no title text and
  keeps WT and worker feasibility `not_run` until G64;
- a no-mutation setup/backup/restore plan plus in-memory disposable drift
  fixture; and
- a provider-registry constructor for explicit tests that cannot admit Agy or
  give it a title badge.

The ordinary registry continues to register only Codex for production. Its Agy
readiness row always reports `admission=unadmitted`, unavailable capabilities,
unavailable Hook inspection, production disabled, and Owner-present
qualification as a manual next action.

## Privacy allow-list

The recorders retain only the following fields:

| Surface | Retained fact |
| --- | --- |
| title/status state | bounded parsed CLI version; known lifecycle spelling; presence of conversation identity; workspace presence/equality only; bounded task count; `tool_confirmation_pending=true` |
| Hooks | known event category; presence of conversation identity; bounded workspace-path count |
| Root anchor fixture | stable/mismatch booleans and an observation count |
| title protocol | plain-output safety classification only |

They never retain, serialize, render, or write raw conversation IDs, workspace
or transcript paths, artifact locations, email, model, quota/token data, tool
names/arguments, error text, prompt/assistant content, or arbitrary unknown
event names. Duplicate JSON keys, excessive nesting/collection sizes, unknown
states, and unknown events fail closed; they do not enter core reconciliation.
In-memory comparison fingerprints are also excluded from `Debug` output.
An observed root-anchor divergence is latched: a later matching sample cannot
make the candidate stable again or rebind it from dynamic observations.

## Execution and ownership bounds

The qualification CLI accepts at most 64 KiB and gives stdin two seconds to
close; timeout and I/O failures return a content-free disposition. The
PowerShell runner requires an absolute, non-reparse `.exe` plus a matching
SHA-256 for both TabBeacon and the direct Agy version probe. It rejects a
non-contained/reparse disposable sample path, caps its read at 64 KiB, bounds
the version process to 10 seconds by default, drops stderr, and kills a timed
out version process tree. None of these checks reads Agy configuration.
The disposable ownership fixture classifies contained, drifted, outside-root,
reparse-point, and oversized candidates as refusals only; it has no write path
and refuses even an unchanged contained candidate while Agy is unadmitted.

## G64 runbook

The concise prepared Owner workflow is
[`agy-g64-owner-runbook.md`](agy-g64-owner-runbook.md). It uses the cohesive
`tabbeacon agy qualification` command family to initialize a disposable
workspace, run literal `agy --version`, accumulate minimized callback/Hook
observations, inspect them, compile a separate unreviewed candidate, produce a
pending review packet, and safely clean the managed workspace.

The callback command is
`tabbeacon agy qualification __title-callback-v1 --root <qualification-root>`.
It always emits the plain fallback title and persists only the same typed
allow-list used by the adversarial recorder. A candidate artifact cannot be
converted to the private admitted-profile token; the production capability
gate rejects every profile version until a later real G64 change explicitly
adds one exact Owner-approved schema version.

### Exact-binary legacy runner details

The runbook is intentionally split at the Owner boundary. The script never
writes Agy configuration, opens login, invokes a model, starts an interactive
`agy` session, or creates a local provider integration. It only provides a
direct version probe and streams explicitly supplied Owner samples into the
content-minimizing recorder.

1. Re-admit the exact TabBeacon candidate, then inspect the no-mutation plan:

   ```powershell
   $tabbeacon = '<absolute path to the admitted tabbeacon.exe>'
   $tabbeaconSha256 = '<matching SHA-256>'
   .\scripts\invoke-agy-g64-qualification.ps1 -Mode Plan `
     -TabBeaconExecutablePath $tabbeacon -TabBeaconExecutableSha256 $tabbeaconSha256
   ```

2. Before the Owner gate, the only permitted Agy interaction is the direct
   version probe. It must not be interpreted as authentication or admission:

   ```powershell
   $agy = '<absolute path to the Owner-approved agy.exe>'
   $agySha256 = '<matching SHA-256>'
   .\scripts\invoke-agy-g64-qualification.ps1 -Mode DirectVersion `
     -TabBeaconExecutablePath $tabbeacon -TabBeaconExecutableSha256 $tabbeaconSha256 `
     -AgyExecutablePath $agy -AgyExecutableSha256 $agySha256
   ```

3. Stop unless all three G64 prerequisites are verified in the Owner-present
   terminal: public v0.5.1 remains valid, the Owner is present, and a real
   authenticated Agy environment is usable. If any is absent, return
   `BLOCKED_OWNER_ENVIRONMENT`; do not use these fixtures as a substitute.

4. If the Owner explicitly supplies a disposable title-state or Hook capture,
   read it only from a fresh, bounded, non-reparse disposable root without
   retaining raw input. The caller must create a fresh sanitized evidence
   destination before redirecting output:

   ```powershell
   .\scripts\invoke-agy-g64-qualification.ps1 -Mode TitleState -OwnerPresent `
     -TabBeaconExecutablePath $tabbeacon -TabBeaconExecutableSha256 $tabbeaconSha256 `
     -DisposableRoot <owner-approved-disposable-root> -InputPath <capture-within-root>
   .\scripts\invoke-agy-g64-qualification.ps1 -Mode HookState -HookEvent post-tool-use -OwnerPresent `
     -TabBeaconExecutablePath $tabbeacon -TabBeaconExecutableSha256 $tabbeaconSha256 `
     -DisposableRoot <owner-approved-disposable-root> -InputPath <capture-within-root>
   ```

5. A real settings/title callback experiment is an Owner-approved G64
   transaction, not a pre-admission action. First create a fresh exact backup,
   prove target ownership and absence of unrelated edits, use only a
   user-global supported surface, verify a broken/missing callback fails open,
   then restore the exact owned change and verify the restore. Do not touch
   workspace-local configuration. Freeze the real observed profile only after
   those facts and the representative Windows Terminal result are recorded.

`tabbeacon agy __title-callback-v1` is a disposable protocol harness only. It
returns the static plain fallback `Agy` and drops its stdin state; it does not
emit VT bytes, register a provider, or create any Agy setting.

## Deferred to G64/G65

These preparation primitives do not settle session identity stability,
workspace-root semantics, lifecycle authority, approval authority, title
ownership, WT progress/color, animation workers, Hook precedence, setup shape,
or restore implementation. They remain `UNKNOWN` or `UNAVAILABLE` until a real
Owner-present G64 experiment proves or rejects them.
