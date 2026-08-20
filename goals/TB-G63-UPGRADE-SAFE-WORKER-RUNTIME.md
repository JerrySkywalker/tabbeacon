# TB-G63 — Upgrade-Safe Worker Runtime

## Status

PLANNED after accepted G62.

## Purpose

Eliminate the Windows package-upgrade failure caused by long-lived session workers executing directly from the package-installed `tabbeacon.exe`.

## Problem

Current architecture permits a one-shot Hook invocation to spawn an ephemeral session/turn worker that outlives the Hook process. On Windows, a running executable remains mapped/locked, so `cargo install --force` cannot replace the package-installed binary while such workers exist.

G57 provides diagnosis and optional bounded draining. G63 provides the permanent runtime ownership model.

## Runtime image model

The package-installed CLI remains the stable one-shot entrypoint:

```text
%USERPROFILE%\.cargo\bin\tabbeacon.exe
```

Before spawning a long-lived worker, publish/use an immutable user-local runtime image conceptually:

```text
%LOCALAPPDATA%\TabBeacon\runtime\worker-images\<binary-or-content-hash>\tabbeacon-worker.exe
```

The worker executes from that image, not from the package-installed CLI path.

## Requirements

### Image publication

- content/version/hash bound;
- deterministic image directory;
- atomic publication;
- reject symbolic-link/path redirection attacks;
- verify existing image contents before reuse;
- no executable bytes downloaded from network by this subsystem;
- source image is the trusted currently running TabBeacon release executable/package content.

### Worker handoff

- existing session/turn/terminal binding semantics remain authoritative;
- Hook timeout remains fail-open;
- copying/publishing an image must be bounded enough for Hook constraints, with caching/reuse where necessary;
- multiple concurrent Hooks/processes cannot corrupt the image;
- old and new release workers may coexist safely because generation/session authority prevents cross-write.

### Upgrade behavior

Required real scenario:

```text
public/installed v0.5.0 or candidate
  -> active TabBeacon worker(s)
  -> install/replace newer package binary
  -> replacement succeeds without killing valid old worker solely for file access
  -> existing worker remains bounded/fail-open or retires normally
  -> new sessions use new runtime image
```

The exact predecessor release used in qualification may be v0.5.0 or a G63 pre-release fixture as long as the Windows file-lock mechanism is real.

## Garbage collection

Old images may be removed only after ownership-safe proof that no valid lease/generation can still require them. GC must be bounded and fail-open.

Safe behavior if deletion fails: retain stale image and report/clean later. Never block Codex because cleanup failed.

No machine-global background janitor daemon. GC happens opportunistically through bounded TabBeacon invocations or explicit maintenance surfaces.

## Upgrade preflight integration

G57 preflight should understand runtime-image state:

- package binary replaceability;
- active runtime image hashes/versions;
- stale image count;
- whether cleanup is safe.

After G63, normal healthy operation should no longer report the package executable as locked by TabBeacon's own long-lived workers.

## Security/privacy boundary

Runtime image directories store executable copies and minimal ownership metadata only. Do not store prompt/tool/assistant content, credentials, raw Hook payloads, or private session history.

Use permissions/path containment consistent with existing per-user runtime state.

## Testing

Required families:

- atomic image create/reuse;
- corrupt/mismatched existing image refusal;
- concurrent image publication;
- symlink/reparse/path-containment safety on Windows as applicable;
- worker starts from runtime image path;
- installed CLI exits and becomes replaceable while worker remains active;
- real Windows replace/install proof with active worker;
- old/new image coexistence does not cross-write;
- generation/lease expiry cleanup;
- failed GC does not block Hook/Codex;
- stale image retained safely if ownership cannot be proven;
- uninstall/cleanup interaction does not remove an image still required by a valid worker;
- package contents and public install semantics remain unchanged except runtime behavior.

## Risk vector

```text
CODE_CHANGED=true
PRESENTATION_CHANGED=false
PROVIDER_CHANGED=false
USER_PERSISTENT_CONFIG_CHANGED=true  # runtime state only
SECURITY_OR_PRIVACY_CHANGED=true
RELEASE_BOUNDARY=false
```

This is high-risk runtime/ownership work. Use focused independent safety review and real Windows process/file-lock acceptance. No provider L4 unless provider behavior itself changes.

## Acceptance

```text
WORKER_RUNS_FROM_VERSIONED_IMAGE=true
PACKAGE_BINARY_LONG_LIVED_WORKER_LOCK=false
RUNTIME_IMAGE_HASH_BOUND=true
ATOMIC_IMAGE_PUBLISH=true
CONCURRENT_IMAGE_PUBLISH=PASS
OLD_NEW_WORKERS_ISOLATED=true
GC_OWNERSHIP_SAFE=true
GC_FAIL_OPEN=true
NO_GLOBAL_DAEMON=true
SELF_UPDATE=false
REAL_WINDOWS_UPGRADE_WITH_ACTIVE_WORKER=PASS
SECURITY_REVIEW=PASS
CODE_CI=PASS
```

## Estimated effort

**8–12 effective engineering hours.**

## Next

`TB-G63R — v0.5.1 Hardening & Release`.