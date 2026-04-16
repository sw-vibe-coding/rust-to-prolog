//! rust-to-prolog: a port-aware Prolog compiler targeting the LAM VM.
//!
//! See `docs/architecture.md` for the pipeline stages and `docs/design.md`
//! for the port-aware coding rules that every module obeys.

pub mod asm;
pub mod compile;
pub mod emit;
pub mod parse;
pub mod port;
pub mod refvm;
pub mod tokenize;
