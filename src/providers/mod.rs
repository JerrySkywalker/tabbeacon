//! Agent-provider adapters live below the provider-neutral core contract.
//!
//! Codex Hooks and the exact admitted Agy 1.1.19 title callback are production
//! adapters. `agy` also retains historical qualification primitives, which are
//! isolated from the production setup and runtime paths.

pub mod agy;
pub mod agy_backend;
pub mod agy_qualification;
pub mod codex;
pub mod registry;
pub mod visual_identity;
