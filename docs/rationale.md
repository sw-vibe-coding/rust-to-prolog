# Rationale — what this repo is, what it isn't

> Short answer: this is a **full Rust reimplementation** of a Prolog
> compiler and the LAM virtual machine. It does not call into the
> upstream PL/SW VM, the SNOBOL4 compiler, or any part of
> `sw-cor24-prolog` at runtime.

## What runs at runtime

When you run `prologc examples/liar.pl`, every stage executes
in-process in Rust:

```
tokenize → parse → compile → emit → asm → refvm
```

No `cor24-emu` subprocess, no PL/SW interpreter, no `cor24-run`,
no Python (once the golden `ancestor.bin` fixture is in place).

## What's shared with `sw-cor24-prolog`

The **specification**, not the code or binaries.

| Upstream artefact | How this repo relates |
|---|---|
| `docs/vm-spec.md` — cell encoding, opcode table, frame layouts | `src/refvm/` implements exactly these semantics in Rust. Tagged 24-bit cells, choice-point frames, env frames, the 24-ish opcodes — all match. |
| `docs/asm-spec.md` — `.lam` text format | `src/emit.rs` produces matching text; `src/asm.rs` parses and assembles it. |
| `tools/lam_asm.py` — Python `.lam` assembler | `src/asm.rs` is an independent Rust reimplementation. `tests/integration/asm.rs` byte-matches our output against `lam_asm.py`'s on `ancestor.lam`. |
| `src/prolog/*.sno` — SNOBOL4 compiler | `src/tokenize.rs`, `src/parse.rs`, `src/compile.rs` are a clean-room port. Not called; just mirrored in shape. |
| `build/lam.bin` — PL/SW-compiled LAM VM | `src/refvm/` is a from-scratch Rust VM with the same opcode semantics. The real `lam.bin` never executes here. |

## Why reimplement instead of calling into the real VM

Two reasons, both in `docs/architecture.md` and `docs/plan.md`:

### 1. Fast iteration

The PL/SW VM runs inside `cor24-emu`. Upstream's test suite takes
seconds per scenario and is bounded by a hard instruction counter.
Our `cargo test` finishes in milliseconds because everything stays
in Rust. That lets us iterate on compiler changes and unification
semantics without waiting for a whole VM bring-up cycle.

### 2. Port target, not production target

The long-term goal of this repo is to be a **port source**, not a
runtime. A downstream agent (human or AI) will translate each
pipeline module to SNOBOL4 (`src/tokenize.rs` → `tokenize.sno`) and
to PL/SW (`src/compile.rs` → `compile.plsw`), module by module,
without creative redesign.

To make that port mechanical, the code is written under the
port-aware rules in `docs/design.md`:

- `BoundedArr<T, N>` instead of `Vec<T>` wherever the data maps to
  a SNOBOL4 `ARRAY`.
- `Vmap<N>` instead of `HashMap` — mirrors SNOBOL4's
  `VMAP = ' key:val key:val '` pattern.
- Functions ≤50 lines, flat bodies, goto-shaped control flow.
- String literals ≤120 chars (SNOBOL4 literal limit is 127).
- Integer arithmetic only (PL/SW has no floats).
- No `async`, `unsafe`, trait objects, or dyn dispatch.

`src/refvm/` is the one module explicitly **exempt** from those
rules — it uses `Vec`, `std::io::Write`, closures, etc. because
it's a Rust-only test aid that will never be ported.
`docs/limitations.md` §Known fragile spots covers the one spot
where `refvm`'s monotonic env-growth assumes more memory than a
bounded port would have.

## How we stay honest without running on the real VM

Three mechanisms keep the Rust implementation from drifting away
from the PL/SW reference:

1. **Byte-match at the asm boundary**.
   `tests/integration/asm.rs` asserts that our `.lam` → cells
   output matches `tools/lam_asm.py`'s output on `ancestor.lam`.
   `tests/fixtures/ancestor.bin` is the pinned reference; any
   drift in cell encoding breaks the test.

2. **Structural parity at the compiler boundary**.
   `tests/integration/ancestor_parity.rs` decodes our compiled
   bytecode and asserts its structural shape matches the upstream
   hand-written `LOAD_ANCESTOR_COMPILED` in
   `sw-cor24-prolog/src/vm/vm_tests.plsw` — opcode set, env-frame
   balance, CALL/EXECUTE counts, Y-slot ops, HALT terminator.
   Byte-identity is *not* asserted because the upstream hand-
   optimises (`ALLOCATE 1`, Z in X-reg) and ours is conservative
   (`ALLOCATE 2`, Z in Y-reg).

3. **Specification as ground truth**.
   When our interpretation and the upstream's diverge, the
   arbiter is `sw-cor24-prolog/docs/vm-spec.md` or `asm-spec.md`.
   If upstream's implementation drifts from spec, we file it
   there and hold our ground (policy written into `docs/plan.md`
   §Known risks).

## Future integration paths

### Forward — port Rust to the upstream

Planned as saga steps 018 (`port-audit-strict`) and 019
(`port-notes`): write a per-module translation guide pointing a
downstream agent at the specific SNOBOL4/PL/SW constructs each
Rust module maps to. After that, the port saga runs on
`sw-cor24-prolog` itself, consuming the notes.

### Backward — run our bytecode on the real VM

Saga step 020 (`integration-ancestor`) is blocked. The PL/SW
`lam.bin` has no runtime bytecode injection path — `VM_INIT`
zeroes the code area on every test run, so `cor24-run --patch`
writes are wiped before `VM_RUN` sees them. Upstream would need
either a UART-based cell loader or a build-time
`LOAD_USER_PROGRAM` substitution mechanism. Until that lands,
`refvm` is the working executor and the parity tests are the
bridge.

## TL;DR

- **Runtime**: pure Rust. `prologc` is self-contained.
- **Shared with upstream**: the spec (vm-spec.md, asm-spec.md,
  opcode table, cell encoding). One reference binary fixture.
- **Eventually consumed by**: a downstream port agent that
  translates this codebase to the SNOBOL4 + PL/SW toolchain in
  [`sw-cor24-prolog`](https://github.com/sw-embed/sw-cor24-prolog).
- **Never at runtime**: `lam.bin`, `cor24-run`, `cor24-emu`,
  `lam_asm.py`, or anything in `sw-cor24-prolog/src/`.
