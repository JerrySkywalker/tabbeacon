# TabBeacon v0.7.3 deferred execution roadmap

## Status

**DEFERRED / FROZEN UNTIL v0.7.2 HOTFIX CLOSEOUT.**

This roadmap preserves the previously admitted Discoverability & Automated Demo
maintenance train after a production Codex subagent Hook defect preempted the
`v0.7.2` release number.

```text
CURRENT_PUBLIC_RELEASE=v0.7.1_at_admission
TARGET_PUBLIC_RELEASE=v0.7.3
PREEMPTING_RELEASE=v0.7.2_CODEX_SUBAGENT_HOTFIX
ACTIVE_IMPLEMENTATION=false
PROMO_PR=100
PROMO_PR_STATE=FROZEN_DRAFT
ROADMAP_V08_CREATED=false
```

No v0.7.3 implementation may resume until v0.7.2 is publicly closed out and the
Owner explicitly re-admits this train against then-current `main`.

## Product theme

**v0.7.3 — Discoverability & Automated Demo**

The preserved scope remains:

1. GitHub-native repository description/topics and social-preview asset;
2. deterministic real-Windows-Terminal promotional GIF/poster generation;
3. README/crates.io presentation polish while keeping the primary install
   command `cargo install tabbeacon`; and
4. a normal maintenance release after those surfaces are accepted.

No installer, TabBeacon Winget/Scoop package, PATH mutation, new provider,
Native Tab Icon work, or v0.8 feature work is admitted.

## Stable Goal IDs

The already-admitted Goal IDs remain stable:

```text
TB-G99   GitHub Discovery Surface
TB-G100  Automated Real-WT Promo Demo
TB-G101  README & crates.io Distribution Polish
TB-G102  Hardening & Public Release
```

Their existing filenames include `V072` because they were created before the
hotfix preemption. This roadmap overrides only their **target release number**:
all references to `v0.7.2` as the future promotion release are to be reconciled
to `v0.7.3` before implementation resumes. Historical evidence and Goal IDs are
not renumbered merely because the public version slot changed.

## Preserved Train A evidence

PR #100 is intentionally retained as Draft.

At hotfix admission:

```text
PR100_REMOTE_HEAD=4731a3ffbca643a4e3d3afcd3b61f1d849eaa434
PR100_MERGED=false
G99_SOURCE_PREPARED=true
G100_UIA_CAPTURE=BLOCKED
GITHUB_METADATA_APPLIED=false
SOCIAL_PREVIEW_UPLOAD=WAITING_OWNER_UI
```

The controlled UIA diagnosis subsequently established:

```text
ROOT_CAUSE=TOPLEVEL_WINDOW_NAME_ASSUMPTION_INVALID
EXACT_WINDOW_NAME_MATCH_COUNT=0
EXACT_TABITEM_NAME_MATCH_COUNT=1
CORRELATION_STRATEGY=EXACT_TABITEM_TO_ANCESTOR_WINDOW
```

The Owner reported a local recovery commit:

```text
LOCAL_RECOVERY_HEAD=31c076d4458a4c0606e494c1dea452946a92fb15
```

That local SHA is preserved as recovery context if still present, but is not
remote/public source truth until a future v0.7.3 re-admission explicitly
reconciles it.

## External promotional tooling

The previously admitted media policy remains:

```text
TOOL=FFmpeg
INSTALL_SOURCE=Microsoft Winget
PACKAGE_ID=Gyan.FFmpeg
PURPOSE=encode exact-owned PNG frame sequences only
```

The capture authority remains exact-owned Windows Terminal/UIA correlation;
desktop-wide FFmpeg capture remains prohibited.

## Cargo distribution contract

The primary user install command remains:

```powershell
cargo install tabbeacon
```

The future exact v0.7.3 release consumer, if/when authorized, becomes:

```powershell
cargo install tabbeacon --version 0.7.3 --locked
```

The older `--version 0.7.2 --locked` text in retained G101/G102 planning files
belongs to their preemption-era target and must be reconciled before v0.7.3
execution.

## Resume gate

Before any v0.7.3 write after the hotfix:

1. fetch exact current `main` after public v0.7.2 closeout;
2. rebase/reconstruct PR #100 without mixing v0.7.2 hotfix code into promo
   semantics incorrectly;
3. reconcile any retained local recovery commit;
4. update G99-G102 target-version wording to v0.7.3;
5. run fresh exact-head validation; and
6. obtain explicit Owner authorization to resume the deferred promotion train.

Until then:

```text
V073_IMPLEMENTATION=FROZEN
PR100_MERGE_ALLOWED=false
PUBLIC_RELEASE_MUTATION=false
```
