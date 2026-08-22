//! Agent-provider adapters live below the provider-neutral core contract.
//!
//! Codex Hooks are the only production adapter. `agy` contains qualification
//! primitives only and cannot enable a provider before `TB-G64` admission.

pub mod agy;
pub mod codex;
pub mod registry;
