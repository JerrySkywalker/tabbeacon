# Development pause after v0.7.1

```text
CURRENT_PUBLIC_TARGET=v0.7.1
CURRENT_PUBLIC_RELEASE=v0.7.1
ACTIVE_FEATURE_DEVELOPMENT=PAUSED_EXCEPT_V072_MAINTENANCE
```

The Owner has authorized v0.7.1 as the stable dogfood baseline and continues to
pause broad TabBeacon feature development. A narrow v0.7.2 maintenance exception
is now admitted solely for GitHub discoverability, deterministic promotional
evidence, and Rust/crates.io distribution polish.

The admitted maintenance roadmap is
[`ROADMAP_V072.md`](ROADMAP_V072.md). It does not admit runtime/provider feature
expansion and does not create a v0.8 roadmap.

## Active narrow exception

The only active implementation scope under the current Owner authorization is:

```text
v0.7.2 — Discoverability & Automated Demo
TB-G99  GitHub Discovery Surface
TB-G100 Automated Real-WT Promo Demo
```

TB-G101 (README & crates.io Distribution Polish) and TB-G102 (v0.7.2
Hardening & Public Release) remain roadmap/planning-only. Their implementation,
including any version bump, publication, tag, or release mutation, requires a
separate explicit Owner admission. A separately authorized goal may perform
read-only G101 preparation without admitting G101 implementation.

The v0.7.2 train explicitly excludes Windows installers, a TabBeacon Winget or
Scoop package, PATH mutation, new provider support, production runtime changes,
Native Tab Icon work, XAML Diagnostics, and v0.8 feature implementation.

## Deferred, not active

`V08_OPTIONS.md` remains **NON_AUTHORITATIVE**. `ROADMAP_V08.md` is
**NOT_CREATED**. None of the following is active work:

- Operational Reliability v2;
- Provider Platform v2;
- Multi-Agent Presentation UX v3;
- Distribution / Terminal Reach beyond the narrow v0.7.2 Cargo/GitHub polish;
- Windows Terminal upstream Native Icon experiment;
- Claude provider;
- OpenCode provider; or
- Codex App Server.

The Native Windows Terminal tab-icon disposition remains `NO_GO`. This
maintenance exception does not reopen XAML Diagnostics, process attachment, or
Windows Terminal native-icon mutation.

After successful public v0.7.2 closeout, the exception ends automatically and
the repository returns to ordinary dogfood-only pause:

```text
CURRENT_PUBLIC_RELEASE=v0.7.2
ACTIVE_FEATURE_DEVELOPMENT=PAUSED
NEXT_RECOMMENDED_GOAL=DOGFOOD_ONLY_NO_ACTIVE_DEVELOPMENT
```

Any later implementation still requires a new explicit Owner admission with an
exact current source head, scoped write boundary, risk vector, and acceptance
evidence. No speculative v0.8 schedule is established here.

```text
V08_OPTIONS_STATUS=NON_AUTHORITATIVE
ROADMAP_V08_CREATED=false
NEW_PROVIDER_ADDED=false
RUNTIME_BEHAVIOR_CHANGED=false_expected
PROVIDER_BEHAVIOR_CHANGED=false_expected
CLAUDE_PROVIDER=DEFERRED
OPENCODE_PROVIDER=DEFERRED
CODEX_APP_SERVER=DEFERRED
NATIVE_TAB_ICON_DISPOSITION=NO_GO
```
