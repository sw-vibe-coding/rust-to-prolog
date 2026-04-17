# Architecture — rust-to-prolog

## System context

```
   Prolog source (.pl)
          |
          v
   +-------------+         +-------------------+
   |   Rust      |         |  Rust reference    |  (fast unit tests)
   |   compiler  +-------> |  LAM VM (prove)    |
   |  (this repo)|         +-------------------+
   +------+------+
          | .lam (text)
          v
   +-------------+         +-------------------+
   |   Rust      |         |  cor24-run + real  |  (integration tests)
   |   lam_asm   +-------> |  LAM VM (PL/SW in  |  (ground truth)
   |  (this repo)| memory  |  sw-cor24-prolog)  |
   +-------------+ image   +-------------------+
```

External dependencies (read-only; not modified by this repo):
- `../../sw-embed/sw-cor24-prolog` — LAM VM source, opcode & asm specs,
  reference `.lam` examples, the SNOBOL4 compiler this Rust one mirrors
- `../../sw-embed/sw-cor24-plsw` — PL/SW compiler (future port target)
- `../../sw-embed/sw-cor24-snobol4` — SNOBOL4 interpreter (future port
  target for tokenize/parse/codegen)
- `cor24-run` — COR24 emulator CLI (used by integration tests)

## Components

### 1. `prologc` binary (Prolog compiler)

Four pipeline stages, each a leaf module, composable as functions so a
SNOBOL4 port keeps the same boundaries:

| Stage      | Input        | Output         | Module        |
|------------|--------------|----------------|---------------|
| tokenize   | `&str` (.pl) | `Vec<Token>`   | `src/tokenize.rs` |
| parse      | `Vec<Token>` | `Vec<Clause>`  | `src/parse.rs`    |
| compile    | `Vec<Clause>`| `Vec<Instr>`   | `src/compile.rs`  |
| emit       | `Vec<Instr>` | `String` (.lam)| `src/emit.rs`     |

Each stage is pure (no I/O), takes owned/bounded data, and returns
`Result<_, Error>`. The `prologc` binary wires them together and
handles file I/O at the edges.

### 2. `lamasm` binary (LAM assembler)

Reads `.lam` text, two-pass assemble (pass 1: count labels/addrs;
pass 2: encode opcodes into 32-bit cells), writes either a flat binary
or a cor24-loadable `.bin@addr` image. Mirrors `lam_asm.py` and the
in-progress `lam_asm.sno` byte-for-byte.

### 3. `refvm` library (reference VM)

Rust implementation of the 24-opcode LAM semantics (dispatch loop,
register file, heap, choice-point stack, trail, environment frames).
Used only for fast unit tests; the real VM (in PL/SW) is authoritative.

Mismatches between `refvm` and the real VM are first-class bugs —
resolved by reading `vm-spec.md` and the PL/SW source, never by
diverging.

### 4. `corrun` (integration bridge)

Thin wrapper around `cor24-run` that loads the SNOBOL4 or LAM binary
image plus the test `.lam` and parses UART output. Used by
`tests/integration/*.rs`.

## Data flow for the liar puzzle

1. `examples/liar.pl` (source)
2. `prologc` -> `build/liar.lam` (text, byte-matches SNOBOL4 codegen
   output for equivalent sources)
3. `lamasm build/liar.lam` -> in-memory cells, written to `build/liar.bin`
4. Integration test: `cor24-run --load-binary lam_vm.bin@0 --load-binary
   build/liar.bin@0x4000` -> UART output contains `day = tuesday`
5. Unit test: `refvm::run(cells)` -> same answer, in milliseconds

## Module layout

```
rust-to-prolog/
  Cargo.toml              (workspace root)
  src/
    bin/
      prologc.rs          (compiler CLI)
      lamasm.rs           (assembler CLI)
    tokenize.rs
    parse.rs
    compile.rs
    emit.rs
    asm.rs                (assembler core, shared with bin/lamasm.rs)
    refvm/
      mod.rs
      dispatch.rs
      heap.rs
      choice.rs
      builtin.rs
    port/
      mod.rs              (port-aware helpers: vmap, bounded arrays)
  tests/
    integration/
      ancestor.rs         (byte-diff vs codegen.sno output)
      liar.rs             (real-VM end-to-end)
      refvm_parity.rs     (refvm matches real VM on all 15 scenarios)
  examples/
    ancestor.pl           (copied from sw-cor24-prolog for byte-diff)
    color.pl
    member.pl
    liar.pl
  scripts/
    port-audit.sh
    run-tests.sh
    diff-codegen.sh       (compare our .lam to codegen.sno output)
  docs/
    prd.md architecture.md design.md plan.md
```

## Opcodes consumed (24)

Reused unmodified from the LAM VM (`sw-cor24-prolog/docs/vm-spec.md`):

- Control: `NOP HALT CALL EXECUTE PROCEED FAIL CUT`
- Choice: `TRY RETRY TRUST`
- Put: `PUT_CONST PUT_VAR PUT_VAL PUT_Y_VAL`
- Get: `GET_VAR GET_Y_VAR GET_CONST GET_STRUCT`
- Unify (stream): `UNIFY_VAR UNIFY_VAL`
- Frame: `ALLOCATE DEALLOCATE`
- Builtin: `B_WRITE B_NL B_IS_ADD B_IS_SUB B_LT B_GT`

No new opcodes are introduced. If the liar puzzle needs a primitive
not in this set (e.g., `B_EQ`, `B_NE`), we add it to the VM in a
coordinated sub-saga on `sw-cor24-prolog`, not here.

## What this repo does NOT own

- The LAM VM itself (authoritative PL/SW source lives in the embed
  project).
- The COR24 emulator / PL/SW compiler / SNOBOL4 interpreter.
- The `.lam` text format spec (lives in
  `sw-cor24-prolog/docs/asm-spec.md`; we implement a matching emitter
  and assembler).
