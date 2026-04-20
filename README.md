# rust-to-prolog

**Live demo:** <https://sw-vibe-coding.github.io/rust-to-prolog/> —
the full Rust compiler + reference VM running in your browser as WASM.
Thirteen canonical Prolog demos (hello, ancestor, append, color, fib,
**liar**, max, member, neq × 2, path × 2, sum) ship bundled; pick one
from the dropdown and hit **Run**. **Upload .pl** takes any local
`.pl` file and loads it into the editor (client-side, nothing uploads
to a server).

[![rust-to-prolog live demo: liar puzzle solved to "thursday"](images/screenshot.png?ts=1776578699034)](https://sw-vibe-coding.github.io/rust-to-prolog/)

A port-aware Rust implementation of a Prolog compiler and reference
VM, targeting the LAM (Logic Abstract Machine) bytecode from the
[`sw-cor24-prolog`](https://github.com/sw-embed/sw-cor24-prolog)
project.

**This is a full Rust reimplementation**, not a wrapper around the
PL/SW-based LAM VM. `prologc` compiles a `.pl` file through
tokenize → parse → WAM-style compile → `.lam` emission → 24-bit
cell assembly → reference VM execution, all in-process in Rust.
No PL/SW interpreter, no COR24 emulator, no external subprocesses.

See [`docs/rationale.md`](docs/rationale.md) for the detailed
answer to "why reimplement instead of calling into the real VM"
and how we stay honest against the upstream specification.

## Quick start

```
cargo build --bin prologc
./target/debug/prologc examples/hello.pl     # → hello_world
./target/debug/prologc examples/liar.pl      # → thursday
./target/debug/prologc examples/sum.pl       # → 6
./target/debug/prologc examples/member.pl    # → a / b / c
./target/debug/prologc examples/ancestor.pl  # (silent HALT)
```

Full walkthrough of each demo in [`docs/demos.md`](docs/demos.md).
What the Prolog subset does and doesn't handle is in
[`docs/limitations.md`](docs/limitations.md).

## Relationship to sw-cor24-prolog

This repo is the **source of an eventual mechanical port** to the
upstream [`sw-cor24-prolog`](https://github.com/sw-embed/sw-cor24-prolog)
project. The plan is module-by-module translation of the Rust
pipeline to SNOBOL4 (tokenizer, parser, compiler, emitter,
assembler) and, for the VM half, alignment with the existing
PL/SW `lam.bin` already in that repo.

To make the port mechanical rather than creative, every file in
`src/` (except `src/refvm/` — a Rust-only test aid) follows the
port-aware coding rules in [`docs/design.md`](docs/design.md):

- `BoundedArr<T, N>` in place of `Vec` where the data maps to a
  SNOBOL4 `ARRAY`.
- `Vmap<N>` in place of `HashMap` (mirrors SNOBOL4's
  `' key:val key:val '` string idiom).
- Functions ≤50 lines, flat bodies, goto-shaped control flow.
- String literals ≤120 chars (SNOBOL4's limit is 127).
- Integer arithmetic only, no `async`, no `unsafe`, no trait
  objects.

Module-to-module mapping, acceptance criteria, and the open
design questions the port agent will need to resolve are laid out
in [`docs/porting-plan.md`](docs/porting-plan.md).

## Docs

| File | Purpose |
|---|---|
| [`rationale.md`](docs/rationale.md) | Why Rust, what's shared with upstream, how we keep the two honest. |
| [`demos.md`](docs/demos.md) | How to run the seven canonical demos end-to-end. |
| [`limitations.md`](docs/limitations.md) | What the Prolog subset does, doesn't, and the known fragile spots. |
| [`porting-plan.md`](docs/porting-plan.md) | Module-by-module translation target, parity contract, open port decisions. |
| [`architecture.md`](docs/architecture.md) | Pipeline stages, component boundaries, data flow. |
| [`design.md`](docs/design.md) | Port-aware coding rules, internal representations. |
| [`plan.md`](docs/plan.md) | The agentrail saga plan (17 planned steps). |
| [`demo-plan-status.md`](docs/demo-plan-status.md) | Current-state snapshot — step progress, real-VM status. |

## Running the tests

```
cargo test                    # 119 lib + 36 integration tests
scripts/run-tests.sh          # same, plus the port-audit gate
scripts/run-regression.sh     # reg-rs CLI suite: 12 demos end-to-end
```

The reg-rs suite captures stdout+stderr of `prologc examples/*.pl` as
byte-exact baselines (`reg-rs/r2p_*.{rgt,out}`). It catches CLI-layer
regressions — argv parsing, I/O buffering, exit codes — that the
in-process integration tests miss. See [`docs/demos.md`](docs/demos.md).

## CI signals

`scripts/run-tests.sh` is the single green/red line:

```
$ scripts/run-tests.sh
test result: ok. 119 passed; 0 failed
test result: ok. 36 passed; 0 failed
port-audit: clean
```

Any of those three regressing blocks the current saga step.

## License

MIT. See [`LICENSE`](LICENSE).
