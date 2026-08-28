# Getting started

TabBeacon gives supported coding-agent tabs a compact workspace identity and
evidence-driven status in Windows Terminal. It does not replace the terminal or
the coding-agent command.

## Before you start

Use Windows with Windows Terminal and a Rust toolchain capable of installing the
current package. The current public release is **v0.6.1**; v0.7 documentation
is development work and is not a published release.

## Install

Install the public CLI from crates.io:

```powershell
cargo install tabbeacon --locked
```

Run the guided setup:

```powershell
tabbeacon setup
```

The flow explains what it can safely own. If the supported Codex integration
asks for Hook trust or review, review it manually. TabBeacon does not bypass or
grant trust for you.

## Use Codex

After setup, start Codex exactly as you normally would:

```powershell
codex
```

There is no TabBeacon launcher, PATH shadow, or terminal wrapper. A supported
session can show a stable workspace alias and a compact title/activity state;
if evidence is unavailable, Codex remains usable.

## Use the admitted Agy profile

TabBeacon production support for Agy is intentionally narrow: **Agy 1.1.19**.
Install its owned title callback once, then retain Agy's literal daily command:

```powershell
tabbeacon setup agy
agy
```

Read [Agy setup](agy-setup.md) before changing an existing Agy configuration.
Unsupported or unproven profiles are diagnosed without being guessed into
support.

## Check the result

These read-only commands are useful first checks:

```powershell
tabbeacon status --json
tabbeacon doctor --json
tabbeacon alias show
```

If a title does not appear, begin with [Troubleshooting](troubleshooting.md).
Do not hand-edit Hook configuration or terminate processes by image name.

## Next steps

- Adjust presentation in [Configuration](configuration.md).
- Understand support boundaries in [Supported coding agents](coding-agent-support.md).
- Read the privacy and safety answers in [FAQ](faq.md).
