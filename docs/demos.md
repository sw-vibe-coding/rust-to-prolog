# Demos — running Prolog programs end-to-end

All demos run on the reference VM (`src/refvm/`) via the `prologc`
binary. The real LAM VM path (`sw-cor24-prolog/build/lam.bin`) is
still blocked upstream — see `docs/limitations.md` §Real-VM
integration.

## One-time setup

```
cargo build --bin prologc
```

## The CLI

```
prologc <file.pl>           Run through tokenize → parse → compile
                             → emit → asm → refvm. Prints UART
                             output (write/1, nl/0), then a verdict
                             line (-- HALT, -- FAIL, or -- ERROR)
                             on stderr.
prologc <file.pl> --lam     Dump the assembled .lam text and exit.
prologc <file.pl> --cells   Dump 24-bit cells (hex, one per line).
```

For running assembled `.lam` files directly (useful when working
below the compiler level):

```
lamasm <file.lam> -o out.bin    Two-pass assembler → flat LE u32.
lamasm <file.lam> --verbose     Dump cells to stderr.
```

## The canonical demos

Each is in `examples/` and has a corresponding integration test
(`tests/integration/refvm_scenarios.rs`) plus a `reg-rs` CLI
baseline (`reg-rs/r2p_<name>.{rgt,out}`) that runs on every
`cargo test` / `scripts/run-regression.sh`.

### 0. Hello — the first program

```
prologc examples/hello.pl
# → hello_world
#   -- HALT
```

`hello :- write(hello_world), nl.` Uses the bareword atom
`hello_world` because our subset doesn't support single-quoted
atoms (`'Hello World!'`). See
[`limitations.md`](limitations.md) §"No quoted atoms, no strings"
for the gap and the plan to lift it.

### 1. Ancestor — recursion + pattern match

```
prologc examples/ancestor.pl
# (no output; HALT)
```

The file holds a yes/no query (`?- ancestor(bob, liz).`). Prolog
convention is that a silent HALT means "proved"; there's no
`write` in the query. Used for the early byte-parity work —
`tests/fixtures/ancestor.lam` and `.bin` are checked in and
byte-matched against `lam_asm.py`, so keeping the query minimal
keeps the golden fixtures small.

The web UI's "ancestor" demo uses a lightly-modified query —
`?- ancestor(bob, liz), write(yes), nl.` — so the output pane
shows `yes\n` instead of staying blank. Source is inlined in
`web-ui/src/demos.rs`.

### 2. Color backtracking — `write/1`, `nl/0`, `fail/0`

```
prologc examples/color.pl
# → red
#   green
#   blue
#   -- FAIL (all solutions exhausted)
```

`color(X), write(X), nl, fail.` enumerates the three colors via
`TRY/RETRY/TRUST` and prints each.

### 3. `member/2` — lists + structural unification

```
prologc examples/member.pl
# → a
#   b
#   c
#   -- FAIL
```

Exercises `GET_STRUCT` / `UNIFY_*` on cons cells and list
construction for the query's `[a, b, c]` literal.

### 4. `sum/3` — integer arithmetic + recursion

```
prologc examples/sum.pl
# → 6
#   -- HALT
```

Tail-recursive sum with accumulator. Uses `is/2` with `+` and `-`,
`>/2` as a guard, and integer constants.

### 5. `max/3` — cut commitment

```
prologc examples/max.pl
# → 5
#   -- FAIL
```

`max(X, Y, X) :- X > Y, !.` commits on the first clause; the
subsequent `fail` can't retry the second clause because `!`
removed its choice point. Without the cut the output would be
`5\n3\n`.

### 5a. `path/2` — graph reachability

Two variants, both over the same four-edge DAG (a→b, b→c, a→d,
d→c). Imported from user-paste examples during the demo build.

```
prologc examples/path.pl            # yes/no: is c reachable from a?
# → yes
#   -- HALT

prologc examples/path_show.pl       # print each route, backtrack-driven
# → a
#   b
#   c
#   done        — first proof: a → b → c
#   a
#   d
#   c
#   done        — second proof: a → d → c
#   -- FAIL (all solutions exhausted)
```

`path/2`'s recursive clause is structurally identical to
`grandparent/2` — one non-tail CALL followed by a tail CALL, so
the permanent-variable machinery (Y-regs + ALLOCATE/DEALLOCATE)
is fully exercised. `show_path/2` prints each node via
tail-recursive traversal and avoids collecting the route into a
list (the natural list approach hits the UNIFY-stream permanent-
var gap documented in [`limitations.md`](limitations.md)).

### 6. `ne/2` — negation-as-failure + `=/2`

Two files, two outcomes:

```
prologc examples/neq.pl       # ne(red, red) — same atoms, fails
# → -- FAIL (no output)
#
# Why: red = red unifies, so \+ X = Y fails, so ne fails, so
# the conjunction fails before reaching write(ok). Status = FAIL
# is the correct verdict; the empty output is a direct consequence.

prologc examples/neq_ok.pl    # ne(red, blue) — distinct atoms, succeeds
# → ok
#   -- HALT
#
# Why: red = blue fails, so \+ X = Y succeeds, so ne succeeds,
# so the conjunction reaches write(ok) and prints. Status = HALT.
```

Both cases share the `ne(X, Y) :- \+ X = Y.` definition. They
demonstrate that `\+` reports the expected verdict on simple
unification goals. Note the `\+` compilation is correct only for
goals `G` that leave at most one residual choice point (see
[`limitations.md`](limitations.md)).

### 7. Liar puzzle — the demo target

```
prologc examples/liar.pl
# → thursday
#   -- HALT
```

"The Lion Lies on Tuesdays" (Raymond Smullyan). Given that the
Lion lies on Mon/Tue/Wed, the Unicorn lies on Thu/Fri/Sat, and
yesterday both said "yesterday I lied," the puzzle reduces to
finding the day that makes both statements self-consistent. The
Rust pipeline compiles the full program (25 clauses, ~260
instructions) and refvm executes it to the unique answer.

Reformulated without `\+` — uses explicit `lion_truth`/
`unicorn_truth` fact sets — because the `\+` compilation can't
yet handle multi-clause `G`. See `docs/limitations.md`.

## Writing and running your own

Quick script / throwaway in `/tmp`:

```
cat > /tmp/fib.pl <<'EOF'
fib(0, A, _, A).
fib(N, A, B, R) :-
    N > 0,
    NewB is A + B,
    N1 is N - 1,
    fib(N1, B, NewB, R).

?- fib(10, 0, 1, F), write(F), nl.
EOF
./target/debug/prologc /tmp/fib.pl
# → 55
#   -- HALT
```

Queries always start with `?-` and end with `.`. `write/1`
prints the (dereferenced) term; lists print as Prolog syntax
(`[a, b, c]`, `[a, b | T]` for partial lists).

For debugging a program that doesn't behave:

```
prologc /tmp/your.pl --lam        # inspect the compiled .lam
prologc /tmp/your.pl --cells      # inspect 24-bit bytecode
```

The `--lam` output is usually the most useful — it shows the
instruction stream the compiler emitted, which you can hand-trace
against the VM spec in `sw-cor24-prolog/docs/vm-spec.md`.

### In the browser

The [live demo](https://sw-vibe-coding.github.io/rust-to-prolog/)
has an **Upload .pl** button next to Run / Reset / Clear. It opens
a file picker, reads the selected `.pl` client-side via
`FileReader`, and replaces the source textarea. Nothing is
uploaded to a server — the whole pipeline runs in the WASM
bundle. Drag-and-drop isn't wired up; use the button.

## How the demos get run in CI

`scripts/run-tests.sh` runs `cargo test`, which builds and runs
the `refvm_scenarios` integration tests. Each demo above has a
corresponding `compiled_<name>_*` test there, so if you regress a
demo the gate turns red.

```
scripts/run-tests.sh            # fast tests only
scripts/run-tests.sh --full     # also runs #[ignore] tests once
                                 # step 020-integration-ancestor
                                 # unblocks (real-VM smoke test)
```

## CLI regression suite (reg-rs)

The integration tests above exercise the pipeline via the Rust API. A
complementary suite under `reg-rs/` exercises the **`prologc` binary**
end-to-end — one `.rgt` + `.out` pair per demo captures stdout+stderr
as a byte-exact baseline. This catches regressions in CLI-only layers
(argv parsing, I/O buffering, exit codes) that the in-process tests
miss.

```
scripts/run-regression.sh        # run all 12 demos in parallel
scripts/run-regression.sh -vv    # full diffs on any failure
```

Rebaseline after an intentional output change:

```
REG_RS_DATA_DIR="$(pwd)/reg-rs" reg-rs rebase -p r2p_<demo>
```

Requires [`reg-rs`](https://github.com/softwarewrighter/reg-rs) on
`PATH`. Baselines (`.rgt`, `.out`) are tracked in git; runtime SQLite
state (`.tdb`, `.tdb.lock`) is ignored.
