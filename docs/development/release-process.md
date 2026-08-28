# Release process

This is a high-level public-facing flow, not release authorization. Public
publication is irreversible and requires explicit release authority.

1. Settle an accepted exact candidate head.
2. Create and validate the release candidate with the applicable tests, clippy,
   package checks, and Windows ZIP/hash work.
3. Run fresh-install and upgrade consumer smoke checks appropriate to the
   changed risk.
4. Obtain explicit release authorization.
5. Publish crates.io, create the signed/tagged release boundary, and create the
   GitHub Release only after authorization.
6. Audit public consistency: package metadata, release notes, hashes, download
   assets, README/version references, and upgrade guidance.
7. Close out release evidence truthfully, including any partial or blocked
   public boundary.

## Exact-head discipline

The accepted head, checked-out head, and final CI evidence must agree. A green
run for a similarly named branch or an earlier commit does not authorize a
different head.

## Release artifacts

Release artifacts require a reproducible build, a Windows ZIP plus hash, and
the relevant consumer smoke evidence. Do not replace an in-use local binary or
drain a process without a fresh ownership-qualified preflight.

## Public boundaries

Crates.io publication, a version tag, and a GitHub Release are public changes.
If one step fails, record the exact completed boundary and stop for a truthful
recovery decision rather than silently claiming a release. Do not expose
credentials or use an unreviewed token path in release evidence.

The current public release is **v0.7.0**. Its package, immutable tag, and
GitHub Release are public release records; future releases must establish their
own accepted source and publication evidence.
