# Porting plan — Rust → SNOBOL4 + PL/SW

This repo is the **source** of a downstream port to the upstream
project [`sw-cor24-prolog`](https://github.com/sw-embed/sw-cor24-prolog).
The goal is a mechanical translation — module-by-module, with no
creative redesign — that leaves the two codebases semantically
equivalent on the ancestor/liar class of programs.

`docs/rationale.md` explains *why* the project is structured this
way; this file describes *how* the port actually happens.

## Target toolchain

| Module here (Rust) | Port target | Target language | Notes |
|---|---|---|---|
| `src/tokenize.rs` | `sw-cor24-prolog/src/prolog/tokenize.sno` | SNOBOL4 | Token kinds already named to match (`ATOM`, `VAR`, `NECK`, ...). |
| `src/parse.rs` | `sw-cor24-prolog/src/prolog/parse.sno` | SNOBOL4 | Flat-subterm arena (`TermIdx`) translates to a single SNOBOL4 `ARRAY` per clause. |
| `src/compile.rs` | `sw-cor24-prolog/src/prolog/codegen.sno` | SNOBOL4 | Instruction enum → text-with-sentinel records; `RegMap` fields become `VMAP` strings. |
| `src/emit.rs` | `sw-cor24-prolog/src/prolog/codegen.sno` (fused with above) | SNOBOL4 | Emitter is already a single-pass formatter; translates to `:OUTPUT = ...` lines. |
| `src/asm.rs` + `src/bin/lamasm.rs` | `sw-cor24-prolog/tools/lam_asm.sno` | SNOBOL4 | In-progress upstream. Our implementation is the cleaner reference. |
| `src/refvm/` | **NOT ported** | — | Rust-only test aid. Upstream's authoritative VM is the existing `sw-cor24-prolog/src/vm/*.plsw`. |
| `src/port/*` (`BoundedArr`, `BoundedStr`, `Vmap`) | Absorbed into SNOBOL4 idiom | — | `BoundedArr<T,N>` → `ARRAY[N]`; `Vmap<N>` → space-delimited `VMAP` string. |

Separately, the PL/SW side of the toolchain already exists at
`sw-embed/sw-cor24-plsw` and hosts `lam.bin` (the compiled LAM VM
itself). Our `src/refvm/` mirrors its semantics but is not a
source for that binary — `lam.bin` is the authoritative VM.

## Why the port is mechanical

Everything in `src/` (except `refvm/`) follows the port-aware
coding rules in `docs/design.md` §"Port-aware coding rules". The
rules are designed so each Rust construct has a fixed SNOBOL4 /
PL/SW counterpart:

- `BoundedArr<T, N>` → `ARRAY[N]` with explicit index discipline
- `Vmap<N>` → `VMAP = ' key:val key:val '` with linear scan
- `enum` variants → tagged records with a discriminator field
- `match` → `SELECT` / goto chain
- `Result<T, E>` → early-return via a distinguished failure label
- `for i in 0..n` → `I = I + 1 :F(DONE)` loop idiom
- Function ≤50 lines → one SNOBOL4 `:DEFINE` body, flat enough
  to fit a SNOBOL4 procedure
- String literal ≤120 chars → under SNOBOL4's 127-char limit with
  slack for label interpolation

`scripts/port-audit.sh` is the mechanical gate (currently a stub;
saga step 018 `port-audit-strict` implements the full ruleset).
Any Rust line that fails port-audit is a line the port can't
translate mechanically.

## Saga steps that prepare for the port

- **018 `port-audit-strict`** — promotes `scripts/port-audit.sh`
  from "stub that passes" to the full rule set from
  `docs/design.md`. Fixes any violations that accumulated in
  steps 1-17.
- **019 `port-notes`** — writes `docs/port-notes.md`: a per-module
  cheat-sheet for the downstream agent. For each Rust source
  file, lists the SNOBOL4 / PL/SW constructs that map to each
  named type, the known-tricky translation points, and any
  expected deviations (e.g., the Y-reg conservatism vs. the
  upstream hand-optimised ALLOCATE counts).

After step 019, this saga on the Rust side is considered complete
(`agentrail complete --done`). The port itself runs as a separate
saga on the `sw-cor24-prolog` repo, not here.

## Parity contract

The port succeeds when, for every `.pl` program currently in
`examples/`, the upstream SNOBOL4 pipeline + PL/SW VM produces
the same UART output as `prologc <file>.pl` here. The benchmarks:

| Example | Expected UART |
|---|---|
| `ancestor.pl` | (no output; HALT on solution) |
| `color.pl` | `red\ngreen\nblue\n` then FAIL |
| `member.pl` | `a\nb\nc\n` then FAIL |
| `sum.pl` | `6\n` then HALT |
| `max.pl` | `5\n` then FAIL |
| `neq.pl` | (no output; FAIL) |
| `liar.pl` | `thursday\n` then HALT |

`tests/integration/refvm_scenarios.rs` pins those expectations on
the Rust side. A matching `run-tests.sh` on the ported SNOBOL4
side is the acceptance criterion.

## Not part of the port

- **`src/refvm/`**. It's a Rust-only executor for fast tests.
  The upstream VM (`sw-cor24-prolog/build/lam.bin`) is the
  authoritative runtime. The port doesn't replace it.
- **Real-VM integration**. Saga step 020 tracks the blocker —
  `lam.bin` needs runtime bytecode injection before we can run
  our compiled `.lam` on the real VM. That's an upstream feature
  request, not port scope.
- **Anything we punted**. Non-scoped cut, multi-clause `\+`,
  seen-perm `is/2` LHS, multiplication — all documented in
  `docs/limitations.md`. The port preserves the same gaps; it
  doesn't introduce features.

## Open questions for the port agent

1. **Compiler layout**. Our `compile.rs` + `emit.rs` are separated
   for Rust clarity; upstream's `codegen.sno` fuses them. Keep
   separate in SNOBOL4 or merge? (Leaning merge — matches upstream
   shape, saves one file of inter-module wiring.)
2. **`Vmap` vs linear ARRAY**. `Vmap<N>` is a space-delimited
   key:val string in SNOBOL4 idiom. Some of our usages are
   small enough that a flat `ARRAY[N]` of pairs would be simpler.
   Decide per-module in step 019.
3. **Negation-label uniqueness**. Our `\+` compilation generates
   unique labels via `<pname>_c<ix>_neg<nix>`. SNOBOL4's 64-char
   SYMMAX limit could bite on deeply-nested clauses. Flag during
   port-audit-strict.

These three are the known "creative decisions" the port agent
will have to make. Everything else should be a rote translation.
