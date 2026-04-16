Add arithmetic builtins: is/2, </2, >/2.

Recognize in compile.rs; emit B_IS_ADD, B_IS_SUB, B_LT, B_GT opcodes. is/2 with a compound right-hand side requires evaluating the expression tree; implement the minimum needed for the liar puzzle (add, subtract, constants, vars).

Tests:
- Factorial-like recurrence: fact(0,1). fact(N,F) :- N > 0, N1 is N - 1, fact(N1, F1), F is N * F1. (NOTE: if multiplication is not yet a VM opcode, emit the expression as repeated addition or punt to a later step; document the decision in the commit.)
- Simple is/2 cases.

Acceptance: cargo test passes.

Commit: 'arithmetic: is/2, </2, >/2'.