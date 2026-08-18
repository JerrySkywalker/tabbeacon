//! Provider-neutral core boundary for `TabBeacon`.
//!
//! Runtime behavior is intentionally not implemented in the repository bootstrap.

pub mod activity;
pub mod cli;
pub mod console_output;
pub mod convergence;
pub mod convergence_evidence;
pub mod core;
pub mod diagnostics;
pub mod management;
pub mod presentation;
pub mod providers;
pub mod repo;
pub mod settings;
pub mod setup;
pub mod title_authority;
pub mod visual;
pub mod windows_terminal_policy;

/// Public product name used by bootstrap smoke tests.
pub const PRODUCT_NAME: &str = "TabBeacon";

/// Bootstrap schema version for the initial repository contract.
pub const BOOTSTRAP_SCHEMA_VERSION: u32 = 1;
