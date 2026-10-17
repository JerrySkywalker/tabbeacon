# Rust inline GuidedInput candidate

This candidate uses `terminal-ui-cliclack` directly for the existing guided setup
selection boundary. Choice indices, localization and default choices remain
product-owned. Selection returns a draft; presentation and Interface preference
flows retain their existing application/confirmation paths. Esc/Ctrl+C returns
the existing interrupted-input error without applying the draft.

Only the GuidedInput implementation changes. Other dialoguer uses, Ratatui Control
Center, Core/Runtime/Ratatui registry dependencies, Hook trust and provider runtime
remain in place. The shared adapter is pinned to an immutable Git revision and
has not been published by this program. TabBeacon's public 0.9.0 version and
release artifacts remain unchanged. No daily executable is replaced.

Risk: CODE_CHANGED=true, PRESENTATION_CHANGED=true; provider, persistent write
semantics, security/privacy and release boundaries are unchanged. Existing guided
state/preset/draft tests are relevant; a real terminal UX acceptance pack is still
required for inline redraw and recovery. Synthetic tests do not prove it.

Rollback: do not install this Draft. Continue the official v0.9.0 executable; source
rollback is the PR base or a normal revert of this focused adapter change.
