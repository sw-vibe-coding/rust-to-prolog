Implement the port-awareness primitives in src/port/.

Purpose: these are the SNOBOL4/PL/SW-shaped building blocks every downstream module uses. They exist so later Rust code looks like SNOBOL4 already.

Deliverables in src/port/:
- mod.rs: re-exports of Vmap, BoundedArr, BoundedStr.
- vmap.rs: Vmap<const N: usize>. Internally a fixed-size array of (BoundedStr<16>, i32) pairs plus a length. API: new(), insert(k: &str, v: i32) -> Result<(), PortError>, get(k: &str) -> Option<i32>, iter(). Mirrors SNOBOL4 VMAP = ' key:val key:val '. Linear scan — no hashing.
- bounded_arr.rs: BoundedArr<T, const N: usize>. Stack-allocated (no heap). push, get, get_mut, len, iter. Capacity N; push returns Err(PortError::Overflow) at capacity.
- bounded_str.rs: BoundedStr<const N: usize>. Copy type wrapping [u8; N] + length. from_str(&str) -> Result<Self,_>. as_str().
- error.rs: PortError enum (Overflow, InvalidUtf8, etc.) via thiserror.

Tests (unit, in each file): exhaustive round-trips, overflow cases, empty cases. Target >=10 tests total.

Constraints:
- No unsafe. No HashMap. No Vec.
- Every function <=50 lines.
- No string literals >120 chars.

Acceptance:
- cargo test passes.
- scripts/port-audit.sh still passes (stub).
- These types are imported by at least one later module stub compiling clean.

Commit: 'port: Vmap, BoundedArr, BoundedStr primitives'.