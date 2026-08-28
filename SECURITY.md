# Security Policy

## Supported versions

The current published release is **v0.7.0**. Security fixes target the current
`main` branch and the latest published release when practical.

## Reporting a vulnerability

Please do not open a public issue for a vulnerability that could expose credentials, alter agent execution, or cause unsafe command execution. Use GitHub's private vulnerability reporting for this repository when available.

## Security posture

TabBeacon is a presentation/observability layer. Its core security invariant is **fail open for the agent runtime, fail closed for configuration ownership**:

- a TabBeacon failure must not prevent the underlying agent CLI from running;
- setup/uninstall must not overwrite or delete configuration it cannot prove it owns;
- provider events are treated as untrusted input at parsing boundaries;
- terminal escape output must be generated from typed internal state, not arbitrary provider strings.

## Security-sensitive surfaces

Please treat these areas with particular care in reports and pull requests:

- Hook and provider configuration ownership, including trust/review boundaries;
- process discovery, ownership proof, and any owned-only drain path;
- terminal instrumentation and escape/title/path handling;
- content and privacy leakage through diagnostics, screenshots, logs, or
  provider payload handling.

Do not include credentials, tokens, raw prompts, assistant/tool content, or
private configuration in a public report. This policy does not define an SLA or
an additional contact channel beyond GitHub's private vulnerability reporting.
