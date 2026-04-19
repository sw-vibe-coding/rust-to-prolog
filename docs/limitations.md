# Limitations — what the Rust Prolog subset does and doesn't do

Snapshot as of 2026-04-18 (after step 017-liar-puzzle +
post-liar env-frame fix). This document is the honest answer to
"does it solve the class of problems?"

## What works

### Compiler pipeline

- **Tokenizer** (`src/tokenize.rs`): atoms, variables, integers,
  parens, brackets, `,`, `.`, `|`, `!`, `\+`, `:-`, `?-`, plus
  the infix operators `+`, `-`, `<`, `>`, `=`. Line comments
  (`%` ... eol) and block comments (`/* ... */`).
- **Parser** (`src/parse.rs`): Horn clauses, lists with `[H|T]`
  desugar, struct terms. Infix expression layer for the five
  operators above with precedence `is` < `<,>,=` < `+,-` < term.
  Interns `[]` and `.` as atom names on demand.
- **Compiler** (`src/compile.rs`): chunk-based Y-register
  classification, `ALLOCATE`/`DEALLOCATE` emission, head +
  body + query compilation, multi-clause `TRY`/`RETRY`/`TRUST`
  dispatchers, list build-up via write-mode `GET_STRUCT`.
- **Emitter** (`src/emit.rs`): byte-identical `.lam` format
  matching a pinned golden fixture.
- **Assembler** (`src/asm.rs`, `src/bin/lamasm.rs`): two-pass
  `.lam` → 32-bit cells; byte-matches upstream `tools/lam_asm.py`
  on `ancestor.lam`.

### Runtime (`src/refvm/`)

- Tagged cells per `vm-spec.md` §1 (REF, INT, ATOM, STR, LIST,
  FUN). 21-bit two's-complement for `TAG_INT`.
- Unification with trail-based undo.
- Choice-point stack with full A-register, CP, HP, TR, EP save/
  restore.
- Env-frame stack with monotonically-growing backing `Vec` so
  that `DEALLOCATE` + backtrack restores frames cleanly.
- Opcodes implemented: `NOP`, `HALT`, `CALL`, `EXECUTE`,
  `PROCEED`, `FAIL`, `TRY`, `RETRY`, `TRUST`, `CUT`, `PUT_VAR`,
  `PUT_VAL`, `PUT_CONST`, `PUT_Y_VAL`, `GET_VAR`, `GET_VAL`,
  `GET_CONST`, `GET_STRUCT`, `GET_Y_VAR`, `UNIFY_VAR`,
  `UNIFY_VAL`, `UNIFY_CONST`, `ALLOCATE`, `DEALLOCATE`,
  `B_WRITE`, `B_NL`, `B_IS_ADD`, `B_IS_SUB`, `B_LT`, `B_GT`.
- Builtins: `write/1`, `nl/0`, `fail/0`, `is/2`, `</2`, `>/2`,
  `=/2`, `\+/1`, `!/0` (via `CUT`).
- `write/1` prints atom names, integers, unbound vars (as
  `_G<n>`), lists (`[a, b, c]`; `[a, b | T]` for partial), and
  generic structs (`f(a, b, c)`).

### Class of programs that work

Empirically verified to run correctly end-to-end on refvm:

1. **Ancestor relations** — recursive rules with multi-clause
   dispatch (`examples/ancestor.pl`).
2. **Color-style backtracking** — `pred(X), write(X), nl, fail.`
   idiom over ground fact sets (`examples/color.pl`).
3. **List membership and construction** — `member/2`, ground
   lists, partial lists (`examples/member.pl`).
4. **Integer accumulators** — tail-recursive arithmetic with
   `is/2`, `</2`, `>/2` (`examples/sum.pl`, `/tmp/fib.pl`).
5. **Cut commitment** — single-clause cuts for deterministic
   head selection (`examples/max.pl`).
6. **Negation on simple goals** — `\+` over unification or
   single-clause bodies (`examples/neq.pl`).
7. **List processing** — `append/3` and similar structural-
   recursion patterns.
8. **Rule chaining with permanent vars** — `grandparent/2`
   style two-hop reasoning.
9. **The liar puzzle** — 25-clause program combining multi-
   clause facts, 2-clause rules with `ALLOCATE` + 3-goal
   bodies, and deep nested backtracking
   (`examples/liar.pl`).

## What doesn't work (yet)

### `is/2` with a seen-permanent LHS

```prolog
fact(N, F) :- N > 0, N1 is N - 1, fact(N1, F1), F is N * F1.
%                                               ^^^^^^^^^^
% F is permanent (head + last-chunk) and already 'seen' by the
% time this is/2 emits. The compile path errors HeadVarRepeat.
```

Fixing needs a 3-instruction detour: `PUT_Y_VAL Yf, A_scratch;
GET_VAR X_tmp, A_scratch; GET_VAL X_tmp, A0`. Workaround:
reformulate with a fresh accumulator var so the LHS is first-
occurrence (see `examples/sum.pl`).

### Nested arithmetic expressions

```prolog
X is (A + B) - C.
```

Only flat `X is A op B` compiles today. Nested expressions need
temporary registers for intermediate results.

### Multiplication, division, modulo

LAM's opcode set has `B_IS_ADD` and `B_IS_SUB` but no `B_IS_MUL`
or friends. Adding them is a coordinated upstream change, not
something we can do in this repo alone.

### `\+` over multi-clause goals

```prolog
disjoint(X, L) :- member(X, L1), \+ member(X, L2).
%                                ^^^^^^^^^^^^^^^^
% member/2 has two clauses, so its dispatcher leaves a choice
% point on the stack even after a successful match. Our \+
% compilation assumes G leaves at most one residual CP, so the
% first TRUST in the \+ epilogue pops the wrong frame.
```

Works fine for `\+ X = Y`, `\+ p(X)` where `p/1` is single-
clause, or any builtin-only G. Fixing properly needs scoped cut
(B0 register) or a "pop top CP" opcode.

### Cut is not scoped

```prolog
outer :- p(X), inner, q(X).
inner :- r, !, s.
```

Classical Prolog: `!` in `inner` only prunes choice points made
since entering `inner`. Our `CUT` implementation matches
upstream `vm_ctrl.plsw` — it clears the entire choice stack
including `outer`'s backtracking points. Wrong for deeply-
nested cut. Flat top-level cut works correctly.

### Nested struct arguments beyond list spines

```prolog
tree(node(a, leaf, node(b, leaf, leaf))).
```

`emit_build_list` linearizes cons-cell spines but doesn't
recurse into general nested structures in body-arg position.
Head-side pattern matching on flat structs works; body-side
build-up doesn't. Lists with cons-spine structure work (the
liar/member examples).

### No disjunction, no if-then-else

Prolog's `;/2` and `->/2` aren't parsed or compiled. Write the
alternatives as separate clauses of a helper predicate.

### No `assertz`/`retract` / dynamic predicates

The LAM VM has no notion of runtime-modifiable code. All facts
and rules are compile-time.

### No `catch`/`throw`

No exception handling.

### No strings or chars

Only atoms and integers. "hello" doesn't parse.

### No operator precedence beyond the five built in

Can't define new infix operators. Can't use standard-library
operators like `\=`, `==`, `@<`, `>=`, `=..`, etc. unless they
happen to be already handled (`=/2`, `\+/1`).

### Bounded static limits

- `MAX_CLAUSES = 64` clauses per program.
- `MAX_BODY = 16` goals per clause.
- `MAX_CLAUSE_VARS = 16` variables per clause.
- `MAX_ATOMS = 50` atoms globally.
- `MAX_SUBTERMS = 64` subterms per clause.
- `MAX_ARGS = 8` args per struct.
- `NAME_CAP = 32` bytes per atom name.
- `MAX_INSTR = 2048` instructions.
- `MAX_CELLS = 4096` in the assembler output.
- refvm `DEFAULT_TICK_LIMIT = 1_000_000` instructions before
  the runaway guard fires.

All are compile-time `const` and easy to raise; they're tuned
for the liar-class puzzles and the SNOBOL4 port constraints.

## Real-VM integration

`sw-cor24-prolog/build/lam.bin` is the authoritative LAM VM but
has no runtime bytecode-injection path — `VM_INIT` zeroes the
code area on every test run, so `cor24-run --patch` writes are
erased before `VM_RUN` sees them. Step 020-integration-ancestor
(blocked) tracks the upstream feature request for a UART-based
cell loader or build-time `LOAD_*` substitution.

Until that unblocks, `docs/demos.md` §"How the demos get run in
CI" is the closest thing to a real-VM smoke test — the parity
check at `tests/integration/ancestor_parity.rs` asserts that our
compiler's bytecode shape matches the upstream hand-written
reference (modulo intentional `ALLOCATE`-count differences).

## Known fragile spots

These aren't "limitations" exactly — they're places where a
future change is likely to break things and needs eyes.

- **Scratch X-reg at `n_temp`**. `emit_build_list` and the
  first-occ-perm body emitter both reuse X-reg index `n_temp`
  as a scratch. Safe today because scratches are always
  consumed before any CALL, but tight.
- **`\+` label uniqueness via RegMap**. Labels are
  `<pname>_c<ix>_neg<nix>`. Query uses `q_c0_neg<nix>`. If
  two modules had the same pred + clause + neg-index they'd
  collide, but we compile whole programs in one pass so this
  hasn't bitten yet.
- **Env monotonic growth**. `ALLOCATE` always pushes to the
  `env` Vec (never overwrites) to protect choice-point-
  saved frames. A long-running backtrack session allocates
  new frames each time rather than reusing popped slots,
  so memory grows with backtrack depth × clause complexity.
  Classical WAM tracks EP_MAX to reuse safely; we don't.

## How I know what works

Every item in "What works" has at least one integration test in
`tests/integration/refvm_scenarios.rs`. Every item in "What
doesn't work" was either observed to fail, rejected by the
compiler with a specific error, or punted at step time with a
documented decision in the corresponding saga summary
(`.agentrail/steps/<NNN>-*/summary.md`).

Re-verify any claim here with:

```
cargo test                  # runs all refvm scenarios
./target/debug/prologc <file.pl>   # run any .pl end-to-end
```
