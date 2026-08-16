# Security Policy

## Supported versions

TabBeacon 0.2.x is the current supported release line. Security fixes target
the current `main` branch and the latest published release when practical.

## Reporting a vulnerability

Please do not open a public issue for a vulnerability that could expose credentials, alter agent execution, or cause unsafe command execution. Use GitHub's private vulnerability reporting for this repository when available.

## Security posture

TabBeacon is a presentation/observability layer. Its core security invariant is **fail open for the agent runtime, fail closed for configuration ownership**:

- a TabBeacon failure must not prevent the underlying agent CLI from running;
- setup/uninstall must not overwrite or delete configuration it cannot prove it owns;
- provider events are treated as untrusted input at parsing boundaries;
- terminal escape output must be generated from typed internal state, not arbitrary provider strings.
