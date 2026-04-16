Add negation-as-failure (\\+ Goal).

Compilation: parse.rs already produces Struct('\\+', [Goal]). Compile.rs treats '\\+ Goal' as a meta-call pattern: on entry, push a choice point; compile Goal; if Goal succeeds, cut-and-fail; if Goal fails, skip to after the negation (succeed).

Concretely emit something like:
    TRY not_succ_label
    <Goal instructions>
    CUT
    FAIL
  not_succ_label:
    <continuation>

Tests:
- ne(X, Y) :- \\+ X = Y.
- Combined with member/2: 'disjoint(L1, L2) :- member(X, L1), \\+ member(X, L2).'

Acceptance: cargo test passes.

Commit: 'negation: \\+ as meta-call + cut-fail'.