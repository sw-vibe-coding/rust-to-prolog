//! Port-aware primitives: `Vmap`, `BoundedArr`, `BoundedStr`.
//!
//! These shapes mirror SNOBOL4 ARRAY and the `' key:val '` VMAP pattern so
//! every downstream module ports mechanically.

pub mod bounded_arr;
pub mod bounded_str;
pub mod error;
pub mod vmap;

pub use bounded_arr::BoundedArr;
pub use bounded_str::BoundedStr;
pub use error::PortError;
pub use vmap::Vmap;
