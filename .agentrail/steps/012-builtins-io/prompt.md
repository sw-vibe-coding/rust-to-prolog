Add write/1, nl/0, fail/0 builtins to compile.rs and refvm/builtin.rs.

Recognize write/1, nl/0, fail/0 by functor+arity in compile.rs; emit B_WRITE, B_NL, FAIL opcodes inline instead of CALL.

Add a new example examples/color.pl: color facts plus query 'color(X), write(X), nl, fail.' Equivalent to tests/retry.lam hand-written backtracking in sw-cor24-prolog.

Tests: refvm run of color.pl captures 'red\ngreen\nblue\n' output. Byte-diff against equivalent SNOBOL4 codegen output (may require running the SNOBOL4 compiler on color.pl to get a golden file; if codegen.sno cannot handle write/1 yet, compile the target by hand and pin it as a reference).

Acceptance: cargo test passes; color.pl works end-to-end on refvm.

Commit: 'builtins: write/1, nl/0, fail/0 + color.pl example'.