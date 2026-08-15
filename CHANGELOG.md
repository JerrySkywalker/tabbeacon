# Changelog

All notable changes to TabBeacon will be documented here.

## [Unreleased]

### Added

- Persistent user-global presentation configuration with typed title, tab-color,
  activity, spinner, and theme choices.
- Comfortable `muted-dark` palette, retained `classic` compatibility palette,
  static title activity fallback, `preview`, and compact config CLI/wizard.
- Ownership-aware transition between TabBeacon and Codex native title output.
- First production provider using owned Codex user-global hooks.
- Fail-open one-shot hook normalization through the existing core, repository
  identity, and Windows Terminal presentation layers.
- Idempotent setup, read-only doctor, and ownership-safe uninstall with atomic
  configuration writes and exact local backups.
- Initial repository governance and Rust bootstrap skeleton.
