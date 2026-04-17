# Design — rust-to-prolog

Design decisions, port-aware conventions, and internal representations.
Idiomatic Rust yields where port-friendliness demands. The goal is that
a future agent (or a future run of this agent) can mechanically
translate each file to SNOBOL4 or PL/SW without creative redesign.

## Port-aware coding rules (hard constraints)

These apply to every file in `src/`. The port-audit script
(`scripts/port-audit.sh`) enforces them mechanically.

1. **No `HashMap`, no `BTreeMap` in pipeline code.** Use
   `port::Vmap<N>` — a bounded, space-delimited string map that maps
   directly to SNOBOL4's `VMAP = ' key:val key:val '` pattern. Limit
   capacity `N <= 50` where the data will land in a SNOBOL4 ARRAY.
2. **No growing `Vec` in hot paths.** Use `port::BoundedArr<T, 50>`
   for anything that ports to a SNOBOL4 ARRAY. Size 50 is SNOBOL4's
   ARRAY cap (`ARR_ELEMS` in `snoglob.msw`).
3. **Functions <= 50 lines, body flat.** Deep nesting doesn't
   translate to SNOBOL4's label-and-goto dispatch. Prefer early
   returns and named branch targets over nested `if let Some(x) = ...
   { if let Some(y) = ... { ... } }`.
4. **String literals <= 120 chars.** SNOBOL4's limit is 127; leave
   slack for the interpolated label. Split long concatenations
   explicitly: `let s1 = "..."; let s2 = "..."; let s = format!("{s1}{s2}")`.
5. **Identifiers <= 7 significant chars** for anything named in a
   table that ports to SNOBOL4 `SYMMAX=64`. Function names and local
   `let` bindings are exempt.
6. **Integer arithmetic only.** PL/SW has no floats. Never use `f32`,
   `f64`, or division that can produce a non-integer.
7. **No `async`, no `Box<dyn Trait>`, no trait objects.** Concrete
   types only; static dispatch.
8. **No `unsafe`.** Anywhere. Including `MaybeUninit`, transmutes.
9. **No external deps except `thiserror` and `anyhow`** (error
   ergonomics). Standard library otherwise. `clap` for CLI binaries
   is allowed but confined to `src/bin/*.rs`; never used in library
   code.
10. **Control flow readable as goto chains.** Match arms instead of
    polymorphism; explicit enum tags. SNOBOL4 is gotos; mirror that.

## Internal representations

### Token (`src/tokenize.rs`)

```
enum Token {
    Atom(BoundedStr<32>),   // 32 fits lowercase-prefix identifiers
    Var(BoundedStr<32>),
    Int(i32),               // PL/SW word
    LParen, RParen,
    LBracket, RBracket,     // for lists
    Comma, Dot, Pipe,
    Neck,                   // :-
    Cut,                    // !
    Not,                    // \+
    Eof,
}
```

Mirrors the tokens `tokenize.sno` emits (`ATOM`, `VAR`, `INT`, `NECK`,
`LPAREN`, `RPAREN`, `COMMA`, `DOT`). Spelling is chosen so a SNOBOL4
port uses the same strings.

### AST (`src/parse.rs`)

```
enum Term {
    Atom(AtomId),           // interned, index into atom table
    Var(VarSlot),           // clause-local var slot (0..MAX_VARS)
    Int(i32),
    Struct(AtomId, BoundedArr<Term, 8>),   // functor + up to 8 args
    // lists encoded as Struct with functor "."/2
    Nil,                    // atom '[]'
}

struct Clause {
    head: Term,             // head goal (Struct or Atom)
    body: BoundedArr<Term, 16>,   // conjunction, up to 16 goals
}
```

No `Rc<Term>`, no boxing. Struct args are inline up to 8 (matches
liar puzzle needs). If we hit a real case needing more, raise the cap
explicitly; don't introduce `Box<[Term]>`.

### WAM instruction (`src/compile.rs`)

```
enum Instr {
    PutConst { ai: u8, atom: AtomId },
    PutVar   { ai: u8, xi: u8 },
    PutVal   { ai: u8, xi: u8 },
    PutYVal  { ai: u8, yi: u8 },
    GetVar   { ai: u8, xi: u8 },
    GetYVar  { ai: u8, yi: u8 },
    GetConst { ai: u8, atom: AtomId },
    GetStruct { ai: u8, atom: AtomId, arity: u8 },
    UnifyVar { xi: u8 },
    UnifyVal { xi: u8 },
    Allocate { n: u8 },
    Deallocate,
    Call     { label: LabelId },
    Execute  { label: LabelId },
    Proceed,
    Try      { label: LabelId },
    Retry    { label: LabelId },
    Trust    { label: LabelId },
    Cut,
    Fail,
    BWrite   { ai: u8 },
    BNl,
    BIsAdd   { dst: u8, a: u8, b: u8 },
    BIsSub   { dst: u8, a: u8, b: u8 },
    BLt      { a: u8, b: u8 },
    BGt      { a: u8, b: u8 },
    Halt,
    Label(LabelId),         // pseudo-op; emitted as `NAME:`
    AtomDir { id: AtomId, name: BoundedStr<24> },   // `.atom N name`
}
```

Field order intentionally matches `asm-spec.md` mnemonic operand
order so the emitter is a single-pass formatter.

### `.lam` emitter output discipline

Byte-identical to `codegen.sno` output for equivalent input. Specific
rules:

- Four-space indent for instructions (matches SNOBOL4 output).
- No trailing whitespace.
- Atom directives emitted in order-of-first-reference (matches
  codegen.sno's `AMAP` iteration order).
- Labels are `pred_N` (predicate) or `pred_cK` (clause K of
  predicate), matching `codegen.sno`.
- Comments prefixed `; ` only where `codegen.sno` emits them.

A byte-diff regression test (`tests/integration/ancestor.rs`) runs
both compilers on `ancestor.pl` and fails on any byte difference.

## Compilation algorithm

Standard WAM compilation for a subset:

1. Variable classification: scan clause, mark each variable as
   permanent (Y-register, appears in more than one body goal) or
   temporary (X-register, one occurrence or all in head/first-goal).
2. Head compilation: for each head arg, emit `GET_*` into A-register
   `Ai`. Constants → `GET_CONST`, unbound var first-seen → `GET_VAR`,
   bound var → `GET_VAL`, struct → `GET_STRUCT` + `UNIFY_*` stream.
3. Body compilation: for each goal, emit `PUT_*` for each arg, then
   `CALL label/arity` (last goal is `EXECUTE` for tail-call).
4. Environment: emit `ALLOCATE N` at clause entry if Y-regs used;
   `DEALLOCATE` before `EXECUTE`/`PROCEED` of last goal.
5. Clause dispatch: multi-clause predicates get `TRY/RETRY/TRUST`
   chain; single-clause predicates call directly.

Builtins (`write/1`, `nl/0`, `is/2`, `</2`, `>/2`, `!`, `\+`,
`fail/0`) are recognized by functor+arity and emit the corresponding
`B_*` / `CUT` / `FAIL` opcode inline instead of `CALL`.

## Testing strategy

### Unit tests (fast, `refvm`)

Per-module tests in `#[cfg(test)]` blocks. Each pipeline stage has
round-trip tests against hand-written golden inputs/outputs. Compiler
tests run the resulting instructions on `refvm` and check query
solutions.

### Integration tests

1. **Byte parity**: `diff` Rust output `.lam` vs a checked-in golden
   copy of `codegen.sno`'s output for the same source. Fails loudly
   on any drift.
2. **Real-VM**: invoke `cor24-run` with the LAM VM image + assembled
   `.lam`, parse UART, check expected output. Marked `#[ignore]` by
   default (slow); run with `--ignored` in CI.
3. **`refvm` parity**: run all 15 LAM VM self-tests through `refvm`;
   every outcome must match the real VM. Drift = bug to fix here.

### `scripts/port-audit.sh`

Greps for the forbidden patterns listed in "Port-aware coding rules."
Zero-warning gate for commits.

## Error strategy

- Library: `Result<T, CompileError>` where `CompileError` is a
  `thiserror` enum with variants per pipeline stage (TokenizeError,
  ParseError, CompileError, EmitError).
- Binaries: `anyhow::Result` at the `main` edge only. Library code
  never sees `anyhow`.
- No panics in library code. `assert!`/`unreachable!` allowed only for
  true internal invariants the caller cannot violate.

## What we deliberately skip

- **Occurs check** — not needed for liar-class puzzles.
- **Indexed choice points** — `TRY/RETRY/TRUST` chain is enough.
- **Dynamic code modification** (`assert/1`, `retract/1`) — out of scope.
- **Backtrackable I/O** — `write/1` commits immediately, as on the VM.
- **Garbage collection** — programs are small; bounded arrays suffice.
