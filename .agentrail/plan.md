# Plan — rust-to-prolog

The implementation plan, expressed as a saga of agentrail steps. Each
step lands something runnable and is followed by a commit +
`agentrail complete`. Step slugs here match those seeded in
`.agentrail/`.

## Saga: rust-to-prolog

### Principle

Every step leaves the tree in a green state:

- `cargo build` succeeds.
- `cargo test` passes (including whatever new tests the step added).
- `scripts/port-audit.sh` passes.

If a step would violate this, split it.

### Step list (rough order; revisable via `agentrail plan --update`)

#### Foundation (days 0-1)

1. **scaffold-cargo** — `cargo init`, workspace layout per
   `architecture.md`, `src/lib.rs` with empty modules, `src/bin/`
   stubs, `.gitignore`, `scripts/port-audit.sh` (stub that passes),
   `scripts/run-tests.sh`. Commit. `cargo test` passes (zero tests).

2. **port-helpers** — implement `port::Vmap`, `port::BoundedArr`,
   `port::BoundedStr`, with unit tests. These are the SNOBOL4-shaped
   primitives every later module depends on.

3. **tokenize** — implement `tokenize.rs` for the lexical subset used
   by `ancestor.pl` and `liar.pl` (atoms, vars, ints, parens,
   brackets, `:-`, `,`, `.`, `|`, `!`, `\+`). Golden-file tests for
   each example. Match `tokenize.sno`'s token names.

4. **parse** — implement `parse.rs`: clause parser producing `Clause`
   structures. Operator precedence kept trivial (no user-defined
   ops); list sugar `[H|T]` desugared to `'.'(H, T)` in the AST.
   Golden-file tests.

#### Ancestor end-to-end (day 1-2)

5. **compile-ancestor** — implement `compile.rs` sufficient for
   `ancestor.pl` (facts, one-body-goal rules, recursive rules,
   multi-clause dispatch, Y-reg classification). `TRY/RETRY/TRUST`,
   `ALLOCATE`/`DEALLOCATE`, `CALL`/`EXECUTE`/`PROCEED`.

6. **emit-lam** — implement `emit.rs` that formats `Vec<Instr>` into
   the `.lam` text format. Integration test: byte-diff against a
   checked-in golden copy of `codegen.sno`'s `ancestor.lam`. Must
   pass.

7. **lamasm** — implement `asm.rs` + `src/bin/lamasm.rs`. Two-pass
   assembler producing the same 32-bit cell layout as `lam_asm.py`.
   Cross-check: assemble Rust-emitted `ancestor.lam`, assemble
   SNOBOL4-emitted `ancestor.lam`, diff the binaries — must match.

8. **refvm-core** — implement `refvm/` sufficient to run
   `ancestor.pl`'s compiled code. Test: `cargo test refvm_ancestor`
   produces the same answers as `cor24-run`.

9. **integration-ancestor** — `tests/integration/ancestor.rs` runs
   `cor24-run` on the Rust-produced `.bin` and checks UART output.
   Marked `#[ignore]`; `scripts/run-tests.sh --full` includes it.

#### Liar-puzzle feature set (days 2-5)

10. **builtins-io** — `write/1`, `nl/0`, `fail/0` in compile + refvm.
    Drives the `color.pl` demo end-to-end.

11. **lists** — list compilation (`[H|T]`, `[]`, `member/2`). Uses
    `GET_STRUCT` / `UNIFY_VAR` / `UNIFY_VAL` for the `.`/2 functor.
    Test with `member(X, [a,b,c]), write(X), nl, fail.`

12. **arithmetic** — `is/2`, `</2`, `>/2`. Emits `B_IS_ADD`,
    `B_IS_SUB`, `B_LT`, `B_GT`. Tests: factorial-like recurrences.

13. **cut** — `!` in clause bodies, semantics via `CUT` opcode
    (barrier set by `ALLOCATE`; restored on deallocate).

14. **negation** — `\+ Goal` compiles to a meta-call-and-fail pattern
    that uses `CUT` + `FAIL`. Test: classic `not-equal` via `\+ =`.

15. **liar-puzzle** — port "Lion Lies on Tuesdays" to
    `examples/liar.pl` (adapted from a standard formulation; roughly
    ~50 lines of Prolog). End-to-end test runs on the real VM and
    produces the answer.

#### Port prep (day 6)

16. **port-audit-strict** — tighten `scripts/port-audit.sh` to the
    full rule set in `design.md` (§"Port-aware coding rules"). Fix
    any violations surfaced in steps 1-15.

17. **port-notes** — `docs/port-notes.md`: for each Rust module, what
    SNOBOL4 or PL/SW constructs it maps to, plus the known-hard
    translation points (e.g., `BoundedArr<Term, 8>` → SNOBOL4 ARRAY
    with explicit index discipline). Informs the downstream porting
    agent.

### How the plan stays live

- Each step is added as it's ready to work via `agentrail add`, or
  sequenced ahead of time via `--next-slug`/`--next-prompt`.
- If a step blows up scope, abort and re-plan: `agentrail abort
  --reason "..."` then `agentrail plan --update`.
- "Done" for the saga = step 17 completes and all tests pass.
  `agentrail complete --done` on the final step.

### Daily progress signal

The single telemetry line to watch:

```
$ scripts/run-tests.sh
tests: N passed, 0 failed
port-audit: clean
.lam byte-diff vs codegen.sno (ancestor): clean
```

Regression on any of those three = blocker for the current step.

### Known risks

- **Byte parity with `codegen.sno`** may surface `codegen.sno` bugs
  that the Rust version happens not to reproduce. Decision rule: if
  the SNOBOL4 output is wrong per `asm-spec.md`, open an issue on
  `sw-cor24-prolog`, freeze the Rust behavior as per-spec, and relax
  the byte-diff test to "diff against a pinned golden file we
  manually verified." Don't chase `codegen.sno` bugs.
- **VM opcode gaps for liar puzzle** — if the puzzle surfaces a
  primitive the 24-opcode LAM doesn't have, we stop and file it as
  work on `sw-cor24-prolog` rather than hacking around it here.
- **Rust ergonomics tension with port rules** — if a port rule
  starts producing obviously-bad Rust in several places, reconsider
  the rule in `design.md`, don't work around it silently.
