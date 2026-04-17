# Product Requirements — rust-to-prolog

## Problem

Ship a Prolog implementation that can solve "The Lion Lies On Tuesdays"
class of logic puzzle (knights-and-knaves with day-dependent truth
values) end-to-end. The existing SNOBOL4-hosted compiler at
`sw-cor24-prolog` can compile the `ancestor` family but stalls on the
features the liar puzzle needs (lists, arithmetic, cut,
negation-as-failure) due to SNOBOL4 interpreter quirks (127-char string
literals, ARRAY cap 50, no GC, no POS/RPOS). Progress there is
measured in days-per-feature; LLM authorship of SNOBOL4 is
non-deterministic.

## Solution

A Rust implementation of the Prolog compiler front-end that emits the
same `.lam` text format the existing LAM VM already consumes. The VM
is reused as-is (PL/SW source at `sw-cor24-prolog/src/vm/*.plsw`, 24
opcodes). Rust gets us fast iteration and deterministic output; the
code is written from day one with mechanical port to SNOBOL4/PL/SW in
mind, so the embed-project target remains viable.

## Users

- **Primary**: this vibe-coding session (Mike + Claude) iterating on
  Prolog features with 15-second feedback loops.
- **Downstream**: a future agent porting this Rust to SNOBOL4 + PL/SW
  to meet the COR24 embed project's language policy.

## Scope

### In scope
- Prolog tokenizer, parser, WAM-style compiler, `.lam` emitter in Rust
- `.lam` assembler in Rust (replaces `lam_asm.py`; deterministic cell
  output)
- Rust reference VM for unit-speed regression tests
- Integration bridge that invokes `cor24-run` with the real LAM VM to
  prove ground truth
- Byte-identical `.lam` output to current `codegen.sno` for the
  `ancestor` example (diff regression test)
- Features needed for liar puzzle: facts, rules, unification,
  backtracking, lists, `is/2` arithmetic, `!` cut, `\+`
  negation-as-failure, `write/1`, `nl/0`, `fail/0`, `member/2`

### Out of scope (initially)
- Module system, DCGs, assert/retract, I/O beyond `write/nl`
- Self-hosting the compiler in Prolog
- Full ISO Prolog conformance
- Source-level debugger, tracing (beyond `--verbose` dumps)

## Success criteria

1. **Primary**: `cargo run --bin prologc examples/liar.pl | cor24-run
   --lam -` prints the solution (e.g., `day = tuesday`) to stdout.
2. **Byte parity**: `cargo test diff_ancestor` passes — the Rust
   compiler's `.lam` output is byte-identical to the current
   `codegen.sno` output for `ancestor.pl`.
3. **Regression shield**: all existing 15 LAM VM self-tests continue
   to pass when the Rust-produced `.lam` is substituted for the
   SNOBOL4-produced one.
4. **Port-readiness**: a port-audit checklist (`scripts/port-audit.sh`)
   passes — no `HashMap` in hot paths, no function >50 lines, no string
   literal >120 chars, no array >50 elements in SNOBOL4-bound code.

## Timeline

Rough day-per-feature budget (calendar days, single-agent working
session):

- Day 1: ancestor.pl end-to-end (baseline + byte-diff harness)
- Day 2: lists + `member/2`
- Day 3: arithmetic (`is/2`, `<`, `>`)
- Day 4: cut (`!`) + negation-as-failure (`\+`)
- Day 5: liar puzzle demo
- Day 6: port-audit pass + SNOBOL4/PL/SW porting notes

"Lion Lies on Tuesdays" target: end of day 5 from saga start.

## Non-goals

- Performance beyond "puzzle completes in seconds on the emulator."
- Beautiful Rust idioms — port-friendliness beats idiomatic Rust
  where they conflict (see `design.md`).
- Solving Prolog's theoretical corners (occurs-check, sound
  negation, constraint solving).
