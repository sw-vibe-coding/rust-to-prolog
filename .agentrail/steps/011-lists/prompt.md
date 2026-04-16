Add list compilation.

Parse.rs already desugars [H|T] to Struct('./2', [H, T]) and [] to Nil. Compile.rs must now emit the right GET_STRUCT / UNIFY_VAR / UNIFY_VAL sequence for '.'/2 terms in heads, and the matching PUT_STRUCT build-up for bodies (use existing opcodes; if a PUT_STRUCT emitter-side compile pattern is needed, introduce it).

Add examples/member.pl:
    member(X, [X|_]).
    member(X, [_|T]) :- member(X, T).
    ?- member(X, [a,b,c]), write(X), nl, fail.

Tests: refvm run of member.pl captures 'a\nb\nc\n'.

Acceptance: cargo test passes; member.pl works on refvm; integration test on real VM also passes (add to the --ignored list).

Commit: 'lists: GET_STRUCT / UNIFY_* for .\\/2, member/2'.