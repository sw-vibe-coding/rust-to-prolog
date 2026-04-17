Write docs/port-notes.md: a per-module guide for the downstream agent that will port this Rust to SNOBOL4 + PL/SW.

For each Rust source file:
- What SNOBOL4 or PL/SW construct each non-trivial type maps to.
- Which fns port cleanly (leaf helpers) vs which need careful rewriting (stateful loops, backtracking).
- Specific gotchas: string-literal splitting, ARRAY cap interactions, label-goto translation.

Also document:
- Opcode/cell encoding invariants that must be preserved byte-for-byte.
- Places where the Rust code deliberately uses a non-idiomatic pattern for port-friendliness, with rationale.

Acceptance: docs/port-notes.md is committed; covers every module in src/ except refvm/ (explicitly out of port scope).

Commit: 'docs: port-notes for SNOBOL4/PL/SW translation'.