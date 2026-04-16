//! Error types for the port-aware primitives.
//!
//! `PortError` is the single error surface for `Vmap`, `BoundedArr`, and
//! `BoundedStr`. Downstream modules re-wrap into their own pipeline errors.

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq, Clone, Copy)]
pub enum PortError {
    #[error("capacity overflow")]
    Overflow,
    #[error("invalid utf-8")]
    InvalidUtf8,
}
