# rust-to-prolog

A port-aware Rust implementation of a Prolog compiler targeting the LAM
VM. The compiler (`prologc`), the `.lam` assembler (`lamasm`), and a
reference VM (`refvm`) are written so a downstream agent can translate
each module to SNOBOL4 or PL/SW without creative redesign. See
[`docs/`](docs/) — especially [`architecture.md`](docs/architecture.md),
[`design.md`](docs/design.md), and [`plan.md`](docs/plan.md) — for the
pipeline, port rules, and saga plan.
