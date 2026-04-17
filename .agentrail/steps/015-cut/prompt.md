Add cut (!) compilation.

Semantics: ! discards all choice points created since the entry to the current clause. VM opcode CUT uses the B register barrier stashed in the environment frame at ALLOCATE time.

Implementation:
- Parser already tokenizes ! as an Atom('!'). Compile.rs recognizes '!' as a body goal and emits the CUT opcode plus any needed cut-barrier bookkeeping.
- Update Allocate emission to reserve a slot for the cut barrier if the clause contains !.

Tests:
- max(X, Y, X) :- X >= Y, !. max(_, Y, Y). (or a similar deterministic predicate).
- Verify that the second clause is NOT tried after ! commits.
- Refvm and real-VM tests.

Acceptance: cargo test passes.

Commit: 'cut: ! compilation and refvm CUT opcode'.