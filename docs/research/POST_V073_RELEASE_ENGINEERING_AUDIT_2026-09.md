# Post-v0.7.3 release engineering audit

- Goal: `TB-POST-V073-CLOUD-ENGINEERING-HYGIENE-001`
- Public release: `v0.7.3` at `1485b4dc0fe634a21634adc9ec539deb76beaad2`
- Audited source baseline: `origin/main` `aaa052e6b51c716345a5d851685a20c0524fced2`
- Recorded: 2026-09-02
- Scope: post-release build, dependency, repository-security, and remote-branch
  hygiene only. This is not a v0.8 or production-adoption change.

```text
RUNTIME_BEHAVIOR_CHANGED=false
PROVIDER_BEHAVIOR_CHANGED=false
NEW_PROVIDER_ADDED=false
PUBLIC_RELEASE_MUTATION=false
OWNER_PRODUCTION_TABBEACON_UPGRADE=false
PRODUCTION_CODEX_CONFIGURATION_MUTATED=false
PRODUCTION_HOOK_TRUST_MUTATED=false
PRODUCTION_AGY_CONFIGURATION_MUTATED=false
```

## Windows release-build reproducibility

The v0.7.3 source and lockfile were built sequentially in clean disposable
source/target contexts. No shared TabBeacon Cargo target was used. The observed
toolchain was Rust `1.97.1` / Cargo `1.97.1`, host
`x86_64-pc-windows-msvc`, LLVM `22.1.6`, and PE linker version `14.51`.
`link.exe` was not directly available on `PATH`. The `Cargo.lock` SHA-256 was
`2e704fb5e09677c4e5a76c8d0a70d03e98d50df7a2242ac816a322e3977a9fbe`.

| Build | Command and controls | SHA-256 | Size |
| --- | --- | --- | ---: |
| A | `cargo build --release --locked`, clean target, two jobs | `2df87e3d5bd8a7fdf02aa3536eac32110af436d55a7406c60ee90eb8fb1f7867` | 5,597,696 |
| B | same source and controls, separate clean target | `b6370b3ba5d572e0b5908f2a93eadf2be8874d9ffa53bf8341712aa400fce29f` | 5,597,696 |
| C | same source, `CARGO_INCREMENTAL=0`, `-C link-arg=/Brepro` | `090a492065b73b7a825c302cf1ca51549d502382b103c669e2c599883819cf2d` | 5,597,696 |
| D | same as C, separate clean target | `090a492065b73b7a825c302cf1ca51549d502382b103c669e2c599883819cf2d` | 5,597,696 |

The baseline pair differs in exactly 24 bytes across five ranges. The first
difference is the PE COFF timestamp at offset `0x00000100`. The other changed
bytes are the timestamps of all three PE debug-directory records and the
16-byte CodeView RSDS PDB GUID. Section layout, binary size, linker version,
and the relative PDB name remained the same. No absolute build path was found
in the CodeView record.

```text
BASELINE_REPRODUCIBILITY=NOT_BIT_REPRODUCIBLE
REPRO_ROOT_CAUSE_CLASS=MULTIPLE_FACTORS
CAUSES=linker-generated COFF/debug timestamps plus CodeView PDB GUID
CONTROLLED_BREPRO_PAIR=PASS
CONTROLLED_BREPRO_VERSION_CHECK=tabbeacon 0.7.3
REPRODUCIBILITY_CURRENT=NOT_BIT_REPRODUCIBLE_CAUSE_CLASSIFIED
```

### Recommendation

Use a separately admitted future release-tooling change to evaluate adding
`RUSTFLAGS=-C link-arg=/Brepro` to the Windows release-artifact build. On this
exact toolchain it made two fresh release builds byte-identical without a
source or runtime change. It is not committed here: the future change must
cover the release workflow, pinned toolchain behavior, artifact checksums, and
Windows consumer smoke evidence. Rust path remapping is not a substitute for
this finding, because Rust documents that Windows linkers can independently
embed debug information that compiler path remapping does not control.

## Locked dependency health

`chacha20 0.10.1` is transitive, not direct:

```text
tabbeacon 0.7.3
  -> atomic-write-file 0.3.1
  -> rand 0.10.2
  -> chacha20 0.10.1
```

Fresh crates.io metadata reports `chacha20 0.10.1` as yanked. Yank status is
not a security classification. Its GitHub advisory `GHSA-j2r6-2m5c-vgh5`
applies only to versions below `0.2.3`; it does not affect `0.10.1`. The
RustSec database snapshot used for the audit has no active `chacha20 0.10.1`
security finding.

The parent `rand 0.10.2` declares `chacha20 ^0.10.0`. In a disposable source
copy, `cargo update -p chacha20 --precise 0.10.2` changed only that locked
package version. `chacha20 0.10.2` has the same declared MSRV (`1.85`) as
`0.10.1`, below TabBeacon's declared MSRV (`1.97.1`). This is a suitable
candidate for the next normal maintenance refresh, not an emergency response
to the yank.

The complete 170-package locked graph was checked against the fetched RustSec
advisory database and a version-specific OSV query. The latter identified two
affected package/version rows:

| Package | Evidence | Classification | Required follow-up |
| --- | --- | --- | --- |
| `lru 0.12.5` via `ratatui 0.29.0` | `RUSTSEC-2026-0002`, `RUSTSEC-2026-0253` | Active memory-safety advisories; the latter requires `lru >= 0.18.2` | Separate security-maintenance investigation; this may require a `ratatui` compatibility update. |
| `paste 1.0.15` | `RUSTSEC-2024-0436` | Unmaintained warning, not a version-patched security advisory | Include in the same maintenance review, without mislabeling it as a vulnerability. |

After Dependabot alerts were enabled, GitHub reported one open low-severity
default-branch alert: `GHSA-rhfx-m35p-ff5j` for `lru`, first patched in
`0.16.3`. It corroborates `RUSTSEC-2026-0002`; the later
`RUSTSEC-2026-0253` still makes `0.18.2` the stricter known target for the
separate maintenance investigation.

```text
CHACHA20_YANKED=true
CHACHA20_SECURITY_ADVISORY=NONE_CURRENTLY_AFFECTING_0.10.1
CHACHA20_SECURITY_SEVERITY=NONE_CURRENTLY_AFFECTING_0.10.1
LOCKED_GRAPH_ACTIONABLE_SECURITY_FINDINGS=2
DEPENDENCY_DISPOSITION=SECURITY_MAINTENANCE_REQUIRED
DEPENDENCY_LOCKFILE_MUTATED=false
```

The disposable `chacha20 0.10.2` candidate passed `cargo test --locked`
(417 tests across the executed test binaries, zero failures). No dependency
update is included in this post-release documentation change.

## GitHub protection and security posture

The initial supported-API audit found no `main` ruleset or classic protection,
no automatic merged-head-branch deletion, and disabled Dependabot security
updates/private vulnerability reporting. Secret scanning and push protection
were already enabled. GitHub Actions defaults were already read-only and could
not approve pull-request reviews.

The check context `Windows / Hosted / Exact Head` is safe to require: it was
the successful exact-head check on each of merged PRs #108 through #112, and
the current CI workflow declares that stable job name.

The supported-API hardening was then applied and independently reread. Classic
protection is used rather than a ruleset. The deliberate solo-maintainer
break-glass path is preserved by not enforcing the rule for administrators;
normal work still requires a pull request.

```text
MAIN_PROTECTED=true
PROTECTION_METHOD=CLASSIC
RULESETS_COUNT=0
REQUIRE_PULL_REQUEST_BEFORE_MERGE=true
REQUIRED_APPROVING_REVIEWS=0
REQUIRED_STATUS_CHECKS=Windows / Hosted / Exact Head
BLOCK_FORCE_PUSH=true
BLOCK_BRANCH_DELETION=true
ADMIN_BYPASS=BREAK_GLASS_ALLOWED
AUTO_DELETE_MERGED_HEAD_BRANCHES=true
DEPENDENCY_GRAPH=enabled
VULNERABILITY_ALERTS=enabled
DEPENDABOT_SECURITY_UPDATES=enabled
DEPENDABOT_VERSION_UPDATE_CONFIG=NOT_CREATED
SECRET_SCANNING=enabled
SECRET_SCANNING_PUSH_PROTECTION=enabled
PRIVATE_VULNERABILITY_REPORTING=enabled
ACTIONS_PERMISSION_CHANGE=PASS_ALREADY_SAFE
```

## Remote branch hygiene

The fresh inventory contained 103 remote branches excluding the symbolic
`origin/HEAD` reference. Four remote branches were checked out by registered
local TabBeacon worktrees and were protected regardless of merge ancestry. No
open pull request existed at initial inventory time. Seven historical remote
tips were not proven ancestors of current `origin/main`; they remain protected
pending explicit future evidence.

Only exact, individually recorded branch deletions that satisfy all of these
conditions are eligible: not `main`, not this goal branch, no open PR, no local
worktree or relevant running-process reference, and a successful fresh
`merge-base --is-ancestor <tip> origin/main` proof. Tags are excluded.

Ninety-three individually revalidated historical branches were removed. Each
had an exact tip match, successful `merge-base --is-ancestor <tip>
origin/main`, no open PR, no registered local worktree, and no relevant active
process reference. The compact branch/tip receipt is retained on this audit
PR; every row has the same recorded ancestry, PR, worktree, and process checks.

At the post-delete observation point, 11 remote branches remained: `main`,
this open audit-PR head, four locally checked-out branches, and five unmerged
or planning/recovery branches. The audit head remains protected until merge;
the enabled automatic head-branch deletion will remove it after the PR is
merged. No tag was mutated.

```text
REMOTE_BRANCH_COUNT_START=103
REMOTE_BRANCHES_DELETED=93
REMOTE_BRANCH_COUNT_POSTDELETE_PREMERGE=11
REMOTE_TAGS_MUTATED=false
OPEN_PR_BRANCHES_PROTECTED=1
LOCAL_WORKTREE_BRANCHES_PROTECTED=4
AMBIGUOUS_OR_UNMERGED_REMOTE_BRANCHES_PROTECTED=5
```

## Explicit deferrals

- Do not add release linker flags until a dedicated release-tooling change can
  verify the exact workflow/toolchain/artifact contract.
- Do not treat the `chacha20` yank as a present security vulnerability.
- Start a dedicated security-maintenance Goal for the `lru` advisories and its
  upstream compatibility path; do not turn this audit into a new release.
- Do not enable broad scheduled Dependabot version-update PRs by default.
- Do not create v0.8 scope, add providers, change provider/runtime behavior,
  or adopt the release in the Owner production environment.
