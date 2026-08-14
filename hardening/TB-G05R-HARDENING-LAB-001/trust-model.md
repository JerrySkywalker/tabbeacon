# Codex hook trust forensics

All writes used disposable `CODEX_HOME` and `LOCALAPPDATA` roots. The owner's
real trust state was not changed.

## Identity and hash

Codex 0.147.0 identifies an unmanaged handler by source path, event label,
matcher-group index, and handler index. Its current hash is SHA-256 over a
canonical normalized handler structure: object keys are sorted, arrays retain
order, and the selected platform command plus normalized timeout/async options
are included. On Windows, `commandWindows` becomes the normalized `command`;
the unused Unix command is not part of that platform hash.

The isolated cases proved:

- JSON whitespace and object-property ordering preserve the hash and trust;
- changing only Unix `command` preserves the Windows hash, while TabBeacon's
  stricter ownership doctor still rejects the altered owned declaration;
- changing `commandWindows` changes the hash and yields `modified`;
- a duplicate handler gets a new positional key and is untrusted;
- inserting a matcher group changes positional keys; trust is not inherited;
- `enabled=false` keeps the content hash trusted but prevents execution;
- removing a declaration removes discovery, while inert trust state remains.

## Review and noninteractive behavior

Startup review and `/hooks` are the supported human review surfaces. The TUI
persists `trusted_hash` through Codex's configuration write API. The lab used
that same API only to simulate reviewed state in isolated homes.

Codex 0.147.0 also documents/advertises
`--dangerously-bypass-hook-trust` as a one-off noninteractive execution mode.
Therefore `NONINTERACTIVE_TRUST_SUPPORTED=true`, but it is a bypass, not an
owner review or persisted approval. It was not invoked. The G05 owner action
remains irreducible under this run's trust contract.

## Upgrade implications

- JSON reformatting alone: no re-trust.
- Same hook command/path after a TabBeacon binary replacement: the Codex hook
  hash is unchanged; ordinary binary provenance remains the owner's concern.
- Command, Windows command, timeout, async option, matcher index, handler index,
  or source-path change: review/re-trust may be required.
- Exact uninstall/reinstall can encounter retained matching trust state;
  doctor is authoritative and now also rejects disabled owned handlers.
