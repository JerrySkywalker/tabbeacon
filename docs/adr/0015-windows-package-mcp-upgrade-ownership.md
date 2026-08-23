# ADR 0015: Windows package-MCP upgrade ownership

## Status

Accepted for post-v0.5.2 maintenance. This ADR does not change the protected
v0.5.2 publication transaction.

## Context

Codex 0.149 runs TabBeacon's normal lifecycle transport through a long-lived,
session-scoped stdio MCP child. Today that child executes the normal
package-installed `tabbeacon.exe`. Windows keeps an executing EXE mapped, so a
future `cargo install tabbeacon --version <version> --locked --force` can fail
until that child exits.

Activity workers already avoid this issue through content-addressed immutable
runtime images. The MCP server has a different lifecycle and different
configuration authority. Moving it to the worker-image design merely because
the worker uses it would broaden the upgrade and trust surface without proving
that it is needed.

## Decision

Keep the MCP server package-installed and use a short-lived, local
`mcp-runtime-v1` lease solely as a safe upgrade-drain proof:

```text
Codex
  -> exact manifest-owned TabBeacon MCP declaration
  -> package-installed tabbeacon.exe __mcp-hook-stdio-v1
  -> ephemeral lease (PID, creation time, path digest, EXE digest, generation)
  -> tabbeacon upgrade-preflight [--drain]
```

The actual internal MCP runtime creates a lease only after it has a system
runtime and the current Codex manifest, Hook declarations, and MCP server
declaration prove exact ownership. Lease state contains no raw Codex session
ID, prompt, assistant/tool content, workspace data, WT_SESSION value, or raw
process command line. A normal exit removes its exact generation. Forced death
can leave a stale record, but stale or malformed state is only a warning and
never drain authority.

Lease registration is deliberately deferred behind immediate stdio-server
readiness. It is a one-shot, per-MCP task that blocks on server shutdown after
registration; it is neither a machine-global daemon nor normal-event polling.
Until the proof appears, upgrade preflight preserves the child as ambiguous.

`upgrade-preflight` considers an MCP process `PROVED_TABBEACON_MCP` only when
all of these agree during the same bounded observation:

1. the process executable canonically resolves to the inspected package target;
2. its current lease was created by the actual internal MCP stdio runtime only
   after manifest-owned MCP authority was established;
3. one current lease names that PID;
4. the lease and process creation times are identical;
5. the canonical-path SHA-256 and executable-byte SHA-256 match the current
   target; and
6. there is exactly one valid lease for that PID.

Windows observation carries only a transient canonical-image digest, PID, and
creation time; it never reads a raw process command line. The manifest-bound
lease supplies the internal-entrypoint proof. The command, image name, or
Codex parent name alone never grants ownership.
The resulting diagnostic distinguishes `proved_tabbeacon_mcp`,
`unowned_or_ambiguous`, `stale_or_invalid_lease`, and
`process_identity_mismatch`.

The default preflight remains read-only. `--drain` re-reads process and MCP
lease state immediately before every mutation. It opens the exact PID and
compares the expected creation time on that same process handle before calling
the non-tree `Kill()` operation. A vanished process is a successful no-op;
PID reuse, a new executable, an unknown lease, or metadata uncertainty causes
refusal. TabBeacon never kills a Codex parent, a process tree, a name-matched
`tabbeacon.exe`, or any third-party MCP server.

No daemon, recurring WMI polling, or normal-event shell work is introduced.
The only Windows process-time query occurs once during MCP lease registration;
ordinary already-connected MCP Hook events retain their existing hot path.

## Official-channel source proof

Release closeout verifies Cargo's installed-package record rather than inferring
the source from `tabbeacon --version` or its executable path. The repository
helper `scripts/verify-cargo-install-source.ps1` consumes a selected Cargo home
and exact package/version, emits only the four bounded source fields required
by release receipts, and recognizes `REGISTRY_OFFICIAL`, `GIT_REVISION`,
`LOCAL_PATH`, or `UNKNOWN_OR_UNPROVEN`. It does not emit metadata contents,
tokens, or unrelated package records.

For the current official channel, a successful cutover must prove:

```text
OWNER_INSTALL_SOURCE=REGISTRY_OFFICIAL
OWNER_INSTALL_SOURCE_PROVEN=true
OWNER_GIT_REV_INSTALL=false
OWNER_OFFICIAL_CHANNEL=crates.io
```

## Consequences

- Package replacement can become a bounded, explicit operation instead of a
  manual `Stop-Process` search.
- Ambiguous processes are intentionally left running; users can still close
  them normally if a package remains blocked.
- A stale lease can reduce automatic convenience but cannot authorize the wrong
  termination.
- Activity-worker runtime-image ownership remains independent from MCP
  ownership. Any future MCP image migration requires a separate reviewed ADR.
