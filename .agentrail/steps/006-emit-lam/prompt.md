Implement src/emit.rs to format BoundedArr<Instr, 2048> into .lam text.

Must produce BYTE-IDENTICAL output to codegen.sno for examples/ancestor.pl.

Steps to implement:
1. Write a function fn emit(instrs: &BoundedArr<Instr, 2048>) -> String with deterministic formatting rules from docs/design.md §'.lam emitter output discipline':
   - 4-space indent for instructions.
   - No trailing whitespace.
   - Atom directives in order of first reference.
   - Label names pred_N / pred_cK.
2. Integration test tests/integration/ancestor.rs:
   a. Compile examples/ancestor.pl with the Rust pipeline.
   b. Read the checked-in golden file tests/fixtures/ancestor.lam (obtained by running tokenize.sno to parse.sno to codegen.sno on the same source; check it in verbatim).
   c. Byte-diff the two strings; fail on any difference.

If there is drift, decision rule per plan.md §'Known risks': if codegen.sno output disagrees with asm-spec.md, the Rust side follows the spec and the golden file is replaced with a human-verified fixture (note the deviation in a comment in the golden file).

Tests:
- Unit tests for individual instruction formatting.
- The byte-diff integration test.

Acceptance: cargo test passes; byte-diff test passes; port-audit passes.

Commit: 'emit: byte-identical .lam formatter'.