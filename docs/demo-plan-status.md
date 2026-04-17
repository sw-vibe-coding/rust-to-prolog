# Demo Plan & Status — rust-to-prolog

Date: 2026-04-17. Scope: the Rust port of the SNOBOL4 Prolog compiler
targeting the LAM VM. This file is the status page, not a retrospective.

## TL;DR

- 7 of 17 saga steps complete: scaffold → port-helpers → parse →
  tokenize → compile-ancestor → emit-lam → lamasm.
- Green gate: 98 unit tests + 2 integration tests pass;
  `scripts/port-audit.sh` clean (stub).
- `.lam` emitter is byte-identical to the per-spec golden fixture;
  `lamasm` is byte-identical to `tools/lam_asm.py` on `ancestor.lam`.
- Next step: 008-refvm-core. After that (009), the ancestor pipeline
  runs end-to-end on the real COR24 VM.
- SNOBOL4 / PL/SW port is NOT this saga. Steps 016-017 prepare for it;
  the port itself is a downstream saga on `sw-cor24-prolog` /
  `sw-cor24-plsw`.

## What runs today

```
$ cargo test
test result: ok. 98 passed; 0 failed
test result: ok. 2 passed; 0 failed  (integration)

$ scripts/run-tests.sh
port-audit: clean (stub)
```

Concretely: `examples/ancestor.pl` walks the full Rust pipeline
(tokenize → parse → compile → emit → assemble) and produces
`ancestor.bin` whose bytes match what `lam_asm.py` produces for the
same source. The bytes are not yet executed — no refvm, no cor24-run
wiring.

## Demo menu (by step dependency)

Each entry is gated on the listed step landing. Order follows the saga
plan in `plan.md`.

| # | Demo | Gated on | Proves |
|---|------|----------|--------|
| 1 | Ancestor query returns solution | 009-refvm-core ✅ | Full Rust pipeline solves a Prolog query in-process |
| 2 | Ancestor query on real COR24 VM | 019-integration-ancestor (blocked) | Same, on the authoritative VM via `cor24-run` |
| 3 | Color backtracking (`color(X), write(X), nl, fail.`) | 012-builtins-io | `write/1`, `nl/0`, `fail/0` + retry chain |
| 4 | `member(X, [a,b,c])` | 013-lists | List compilation via `GET_STRUCT`/`UNIFY_*` for `./2` |
| 5 | Factorial-style recurrence | 014-arithmetic | `is/2`, `</2`, `>/2`, integer builtins |
| 6 | Cut-pruned choice (`!`) | 015-cut | `CUT` barrier semantics |
| 7 | Negation-as-failure (`\+ Goal`) | 016-negation | Meta-call + `CUT` + `FAIL` pattern |
| 8 | "Lion Lies on Tuesdays" puzzle | 017-liar-puzzle | End-to-end logic puzzle on refvm |

Velocity so far: steps 001-007 landed across 2 calendar days
(2026-04-16 → 2026-04-17); steps 008-011 landed on day 2. Later
steps carry more VM-side complexity (list cells, arithmetic
semantics, cut), so assume lower per-step throughput.

## Real-VM status

The authoritative LAM VM (`sw-cor24-prolog/build/lam.bin`) does
not currently support loading external LAM bytecode at runtime —
`VM_INIT` zeroes the MEM array before every test run, so
`cor24-run --patch` writes are erased, and test programs are
hardcoded via `LOAD_*` procedures in
`sw-cor24-prolog/src/vm/vm_tests.plsw` at build time. Step
019-integration-ancestor is blocked on upstream support for a
UART-based bytecode loader (or an equivalent build-time hook
that substitutes a user-provided `LOAD_*` proc).

In the meantime, step 011-cell-parity (validation) closes the
loop on "our compiler emits correct bytecode" via a structural
parity test: `tests/integration/ancestor_parity.rs` decodes our
`ancestor.lam` output and asserts the shape (opcode set,
env-frame balance, `CALL`/`EXECUTE`/`TRY`/`TRUST` counts, Y-slot
ops, terminator). The two bytecodes — ours and the upstream's
hand-written `LOAD_ANCESTOR_COMPILED` — produce the same answer
but are not byte-identical; the upstream hand-optimizes
`ALLOCATE 1` by keeping `Z` in X-regs across the `CALL` to
`parent`, while our compiler conservatively uses `ALLOCATE 2`
and puts `Z` in `Y1`. Both are correct per `vm-spec.md` §4.

Treat refvm as the working VM for demos. Real-VM integration
becomes possible once upstream ships the loader hook.

## SNOBOL4 / PL/SW port ETA

- **Not this saga.** The downstream agent that ports Rust → SNOBOL4
  and Rust → PL/SW consumes `docs/port-notes.md`, which is written in
  step 017. Before that agent can run, all pipeline features for the
  target demo must exist in the Rust source (the port is mechanical;
  it doesn't invent compiler features).
- **Earliest port start:** after step 017 (`port-notes`) lands. At
  current velocity that's 10 steps out.
- **Gating for a ported liar-puzzle demo:** 017 + the downstream
  port saga on each target project, i.e., weeks after step 017, not
  days.

## Progress signal

Watch one line:

```
$ scripts/run-tests.sh
tests: N passed, 0 failed
port-audit: clean
(.lam byte-diff: clean, once 009 wires the real VM)
```

Any of the three regressing = blocker on the current step.

Secondary signal: `agentrail history` — one-line summary + commit
hash per completed step.

## Known risks (live)

- **Byte parity with upstream** (`codegen.sno`, `lam_asm.py`) is
  enforced against per-spec golden fixtures, not against upstream
  output directly. If upstream drifts buggy, we freeze per-spec
  behavior and file the bug on `sw-cor24-prolog`. Policy in
  `plan.md` §"Known risks".
- **VM opcode gaps for the liar puzzle.** If the puzzle surfaces a
  primitive the 24-opcode LAM doesn't have, we stop and escalate
  to a coordinated sub-saga on `sw-cor24-prolog` rather than
  grow the Rust side past the shared opcode set.
- **Port-audit is a stub.** Step 016 promotes it to the full rule
  set from `design.md` §"Port-aware coding rules". Violations
  accumulated during steps 1-15 are cleaned up there, not now.

## What this file is not

- Not a test catalog — that's `cargo test` output.
- Not an architecture doc — that's `docs/architecture.md`.
- Not a retrospective — that's the saga step history
  (`agentrail history`).
