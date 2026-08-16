//! Provider-neutral core boundary for `TabBeacon`.
//!
//! Runtime behavior is intentionally not implemented in the repository bootstrap.

pub mod activity;
pub mod core;
pub mod diagnostics;
pub mod presentation;
pub mod providers;
pub mod repo;
pub mod settings;
pub mod setup;
pub mod visual;

/// Public product name used by bootstrap smoke tests.
pub const PRODUCT_NAME: &str = "TabBeacon";

/// Bootstrap schema version for the initial repository contract.
pub const BOOTSTRAP_SCHEMA_VERSION: u32 = 1;
