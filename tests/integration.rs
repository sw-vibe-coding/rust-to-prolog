//! Integration test crate. Submodules live under `tests/integration/`.
//! Per docs/architecture.md §'Module layout', each end-to-end scenario
//! is its own file in that directory.

#[path = "integration/ancestor.rs"]
mod ancestor;

#[path = "integration/asm.rs"]
mod asm;

#[path = "integration/refvm_scenarios.rs"]
mod refvm_scenarios;

#[path = "integration/ancestor_parity.rs"]
mod ancestor_parity;
