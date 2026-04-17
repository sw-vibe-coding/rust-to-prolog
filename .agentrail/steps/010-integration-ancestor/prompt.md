End-to-end integration test: examples/ancestor.pl compiled by Rust pipeline runs on the real LAM VM via cor24-run.

Deliverables:
- tests/integration/ancestor_real.rs: builds the pipeline (tokenize to parse to compile to emit), calls lamasm to produce a binary, invokes cor24-run with the LAM VM image at 0 and the compiled ancestor at 0x4000 (check paths in sw-cor24-prolog/scripts/run-tests.sh for the right offsets).
- Parses cor24-run's UART output and asserts the expected ancestor query results.
- Marked #[ignore] so 'cargo test' is fast; 'cargo test -- --ignored' runs it.
- scripts/run-tests.sh --full flag runs ignored tests too.

Precondition: the sw-cor24-prolog build must exist. Document this in a README section: 'To run full integration tests: cd ../../sw-embed/sw-cor24-prolog; ./scripts/build-all.sh'.

Acceptance: 'cargo test --ignored integration_ancestor_real' passes on a machine with sw-cor24-prolog built.

Commit: 'integration: end-to-end ancestor on real LAM VM'.