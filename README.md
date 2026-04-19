# rust-to-prolog

A port-aware Rust implementation of a Prolog compiler targeting the LAM
VM. The compiler (`prologc`), the `.lam` assembler (`lamasm`), and a
reference VM (`refvm`) are written so a downstream agent can translate
each module to SNOBOL4 or PL/SW without creative redesign.

## Quick start

```
cargo build --bin prologc
./target/debug/prologc examples/liar.pl      # → thursday
./target/debug/prologc examples/sum.pl       # → 6
./target/debug/prologc examples/member.pl    # → a / b / c
```

See [`docs/demos.md`](docs/demos.md) for the full demo walkthrough and
[`docs/limitations.md`](docs/limitations.md) for what the Prolog subset
does and doesn't handle.

## Docs

- [`architecture.md`](docs/architecture.md) — pipeline stages and
  component boundaries.
- [`design.md`](docs/design.md) — port-aware coding rules and internal
  representations.
- [`plan.md`](docs/plan.md) — the agentrail saga plan.
- [`demos.md`](docs/demos.md) — how to run each example end-to-end.
- [`limitations.md`](docs/limitations.md) — scope boundaries, known
  gaps, fragile spots.
- [`demo-plan-status.md`](docs/demo-plan-status.md) — current-state
  snapshot.

## Running the tests

```
cargo test                    # fast lib + integration tests
scripts/run-tests.sh          # same, plus port-audit
```
