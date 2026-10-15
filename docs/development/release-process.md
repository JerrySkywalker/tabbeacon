# Release process / 发行流程

This is a public-facing procedure, not release authority. The current source prepares an unreleased v0.9.0 material candidate. As verified2026-10-15, the latest official release is v0.8.1; v0.8.2 was never formally released. Earlier v0.8.0 release candidate planning remains history, not current release status.

本文件是流程说明，不是执行授权。本源码仅准备未发布的 v0.9.0 材料候选；截至2026-10-15已核验正式版本为 v0.8.1。不得从文档、依赖公开或 CI 成功推断发布、安装、信任权限。

## Exact source and public inputs / 精确来源与公开输入

Settle one clean candidate. Source HEAD, normal crate `.cargo_vcs_info.json`, full package members, release binary/ZIP manifests and the required CI evidence must agree. Record the actual HEAD in the evidence receipt. Do not put a commit's own future SHA in its source bytes. Public shared dependencies are Core=0.2.0, Runtime=0.1.0 and Adapter=0.1.0 at exact crates.io versions and locked checksums. Frozen API `eb16cd91f924c145180b5befcf9f2cacc7a46d84` differs from package source `206325e2367cda90f8cbe5d418dc7761da5f81bb`.

## Normal artifacts and raw reproduction / 正常产物与原始复现

Use the installed pinned Rust1.97.1 x86_64-pc-windows-msvc toolchain, default features and unchanged release optimization profile. The included root `build.rs` emits `/Brepro` only for the normal `tabbeacon` binary when PROFILE=release, target OS=windows and environment=msvc. This controls linker determinism during a normal Cargo Release build, not a post-build rewrite. Other targets, dev/test profile, fixture bins, product logic, panic and optimization settings are unchanged. The script is in the normal archive and limits the option to this release binary, avoiding target-global rustflags on dev/test/dependency links. An explicitly included `.cargo/config.toml` can appear in a Cargo package; Windows list output uses backslashes and must be compared with correctly parsed member paths. Record effective toolchain/environment inputs and reject unadmitted overrides.

The normal archive must be created with verification enabled, without --allow-dirty or --no-verify:

```powershell
cargo package --locked --registry crates-io
```

Inspect every member, normalized and original manifests/lock/VCS metadata, bilingual READMEs, LICENSE, MSRV/toolchain and exact public dependency checksums. Extract the exact crate twice into separate new roots. From each extracted package root, use a separate initially empty target directory and the same fixed public source cache/toolchain/environment:

```powershell
cargo build --release --locked --offline --bin tabbeacon
```

The offline cache must already contain the locked public sources; a missing cache is not permission to install or use a private/Git substitution. Record complete commands, actual compile output/exit, source-tree hashes, environment selection, compiler identity and both entire EXE SHA256. Separate source/target roots and real compiles are required; copying one EXE twice or touching only its timestamp is not an independent build. Pass only if both raw binary byte sequences and SHA256 match. No timestamp/GUID stripping, binary signing/rewrite, normalized hashing or ignored-byte comparison may manufacture a PASS. Preserve every prior FAIL and both old EXEs. Two matching builds qualify only the recorded host/toolchain/input scope, not every machine.

Windows ZIP contains the accepted EXE plus LICENSE, both READMEs, CHANGELOG and current support/review/upgrade/release-process documentation. Sort entries, set a fixed ZIP metadata timestamp and record ZIP/EXE/crate/checksum-manifest SHA256. ZIP metadata determinism packages original EXE bytes; it never normalizes the EXE. Reopen ZIP and compare each member with its accepted source bytes. Keep artifacts outside Git, linked by the private evidence index.

两份独立干净正常 Release 构建须具有真实编译日志，整个原始 EXE 字节及 SHA256 均一致。仅缓存公共依赖源码，不共享已编译目标。禁止后处理 EXE、忽略时间戳/GUID或复制同一文件制造复现；保留全部原 FAIL。ZIP 的固定条目时间只用于归档容器，不修改 EXE。正常 Cargo 包包含双语 README 和必要发行文档。

## Gates, upgrade and approval / 门禁、升级与批准

Run focused checks for changed risks and one required exact-head hosted CI on the settled candidate. Reuse accepted unchanged-risk evidence only with an explicit empty source-risk diff. Do not change timeout/assertions or repeat unrelated full CI to get green. Obtain one necessary scoped independent review in a separate explicitly read-only native process; require actual exact-source reads, output and exit proof.

Use the [official v0.8.1 → v0.9.0 guide](../v0.9.0-upgrade.md) for future ownership-qualified installation, reconciliation, manual trust and rollback. Harmless isolated --help/--version or task-owned selection restoration is not real migration/rollback. Do not replace an in-use binary, drain processes, install, setup, trust or invoke a real Provider without its exact new permission.

Train D remains PARTIAL (2 PASS/18 SKIP). Optical P95, actual terminal/Shell recovery, DEC private modes, current real Provider and strict OS stalls remain UNPROVEN. Preserve the700ms historical failure and unresolved stability. Arbitrary foreign-writer CAS and cross-file crash atomicity are unsupported NON_GATES; do not silently turn other incomplete gates into limits or PASS.

材料、CI和审查通过后仍须 Owner 接受精确源码/包/EXE/ZIP、逐项处理剩余门禁，并明确下一步永久操作及重试范围。发布、版本 tag、GitHub Release、正式安装、Hook trust、真实 Provider均需要各自精确授权；失败时记录已经完成的边界并停止，不自动重试、覆盖资产或移动标签。

## Separately authorized future public steps / 另行授权的未来发布步骤

Only after exact Owner authority and all applicable required gates: verify acting account/name scope without exposing credentials; perform only the authorized publication/tag/Release/install steps; independently download actual public assets and compare expected raw hashes and immutable source provenance. Record partial completion truthfully and stop on any ambiguous result. This material train performs none of those public operations and stops at STOP_FOR_OWNER_V090_RELEASE_MATERIAL_REVIEW.
