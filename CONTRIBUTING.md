# Contributing to TabBeacon

Thanks for helping improve TabBeacon. Small documentation and test improvements
are welcome; focused changes are easier to review, test, and revert.

## Project scope

TabBeacon is a Windows Terminal presentation layer for supported coding-agent
tabs. It preserves literal provider commands (`codex` and `agy`), fails open
for provider use, and fails closed when configuration/process ownership is not
proven. Start with the [documentation portal](docs/README.md) for product
context.

## Where to report what

- A reproducible bug or a Codex/Agy/Windows Terminal compatibility regression:
  open an [Issue](https://github.com/JerrySkywalker/tabbeacon/issues/new/choose).
- A concrete feature request: open an [Issue](https://github.com/JerrySkywalker/tabbeacon/issues/new/choose).
- A question, usage-help request, exploratory idea, design discussion, or
  showcase: start a [Discussion](https://github.com/JerrySkywalker/tabbeacon/discussions).
- A security-sensitive concern: follow the [Security Policy](SECURITY.md), not
  the public issue tracker.
- A proposed change: open a focused pull request after reading the relevant
  contribution guidance below.

## Prerequisites

- Git
- Rust 1.97.1 or newer
- PowerShell 7 (`pwsh`)
- Windows for Windows-specific paths

Clone and build:

```powershell
git clone https://github.com/JerrySkywalker/tabbeacon.git
Set-Location tabbeacon
cargo build --locked
```

## Focused tests and quality gates

For a focused Rust change, run the closest test first:

```powershell
cargo test --locked <test-name>
```

For documentation work, run the offline docs checker:

```powershell
pwsh -NoProfile -File scripts/ci/check-docs.ps1
```

Before a code PR, run the applicable repository checks described in
[Build and test](docs/development/build-and-test.md). Do not run an interactive
visual or real-provider experiment merely for a prose correction.

## Windows behavior and architecture

The provider-neutral core consumes normalized evidence. Keep provider-specific
logic in `src/providers/`; do not introduce provider event names into the core.
Terminal presentation is typed and testable. The production backend is
`TitleMarkBackend`; native tab icons are [NO_GO](docs/design/native-tab-icon.md)
under accepted current-host safety evidence.

## Provider and configuration boundary

Do not add a provider, expand a provider profile, bypass Hook trust, shadow a
provider executable, introduce a wrapper/PTY host, or add a global daemon as a
routine contribution. Setup, repair, import, uninstall, and process drain must
prove exact ownership and preserve unrelated user state.

Do not mutate an Owner's Codex/Agy configuration, Hook trust, or active sessions
in an unattended test. Use disposable, exact-owned fixtures only when a Goal
explicitly authorizes a high-risk experiment.

## Documentation and visual changes

Keep public product truth current. English is canonical; `README.zh-CN.md` is a
full counterpart for the bounded critical invariants. Brand assets must remain
pure SVG without script, external URLs, embedded raster data, or font dependency.
If a change alters title, progress, palette, VT bytes, animation, or visual
oracle behavior, a final representative owned visual proof is required.

## Pull requests

- Work on a focused branch; do not rewrite unrelated history.
- Explain the behavior, tests, and risk surface changed.
- Keep generated output, private logs, screenshots with private content, and
  build roots out of the PR.
- Let hosted CI validate the final exact head. A green run for another commit
  does not validate the PR head.
- Address review findings with a new exact-head CI run where the relevant risk
  surface changed.

## Security and privacy

Report security-sensitive concerns through the process in [SECURITY.md](SECURITY.md).
Avoid placing credentials, tokens, raw provider content, private paths, or
unredacted process output in issues, fixtures, screenshots, or commits.

## When to read deeper governance

Read [AGENTS.md](AGENTS.md), `dev_governance_files/QUALITY_GATES.md`, and the
relevant ADRs before high-risk changes involving provider configuration, trust,
process targeting, release/publication, privacy, or terminal instrumentation.
They are not prerequisites for a simple typo fix.
