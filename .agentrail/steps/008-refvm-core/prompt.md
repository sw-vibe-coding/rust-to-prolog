Implement src/refvm/ sufficient to run compiled ancestor.pl.

Structure per docs/architecture.md:
- refvm/mod.rs: public fn run(cells: &[u32]) -> RunResult { Halt, Fail, Error(E) } and supporting state.
- refvm/dispatch.rs: decode-and-dispatch loop over cells.
- refvm/heap.rs: heap cells (tagged: REF, ATOM, INT, STR), unification, trail.
- refvm/choice.rs: choice-point stack, backtracking.
- refvm/builtin.rs: B_WRITE, B_NL (wire to a Writer trait so tests capture output).

Only implement opcodes needed for ancestor.pl at this step. Stub the rest with 'todo: implemented in later step'.

Tests:
- Replicate each of the 15 LAM VM self-tests from sw-cor24-prolog/scripts/run-tests.sh that exercise the ancestor-subset opcodes. Cite test names.
- Test harness: assemble a .lam through Rust toolchain, run on refvm, assert HALT plus captured output.

Constraints:
- No unsafe.
- No HashMap; cells are a Vec<u32> for the heap but size-bounded per test.
- Port-awareness is relaxed for refvm since it won't be ported (it's a Rust-only test aid). Document this exception at the top of refvm/mod.rs.

Acceptance: cargo test passes; at least 6 refvm tests pass matching the ancestor-subset scenarios.

Commit: 'refvm: reference LAM interpreter for unit tests'.