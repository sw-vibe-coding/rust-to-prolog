Cell-level parity check between our Rust-compiled ancestor bytecode and the upstream VM's hardcoded LOAD_ANCESTOR_COMPILED from sw-cor24-prolog/src/vm/vm_tests.plsw.

Intent: with real-VM integration deferred (step 019, blocked on upstream bytecode-injection), this step closes the loop on 'our compiler emits correct bytecode' by comparing cell structures against a human-verified reference that the real VM does execute to HALT.

Known differences to document, not paper over:
- Upstream hand-optimizes: ALLOCATE 1 (keeps Z in X-regs across CALL parent), saving 3 cells.
- Our compiler is conservative: ALLOCATE 2 (puts Z in Y1 too), safe for general callees.
- Upstream layout: query first at MEM(0), then ancestor, then parent. Ours: atom dirs + EXECUTE query, then parent, then ancestor, then query at the end. Addresses differ throughout.

Deliverable: tests/integration/ancestor_parity.rs that:
1. Assembles our tests/fixtures/ancestor.lam via rust_to_prolog::asm.
2. Parses the upstream LOAD_ANCESTOR_COMPILED proc from vm_tests.plsw (or checks in a pinned copy of its MEM(k)=v lines).
3. Verifies semantic equivalence: same set of opcodes + atom references, different ordering justified by the ALLOCATE-2 strategy.
4. Documents each structural difference with a code-level comment.

Do NOT try to byte-match — the two bytecodes aren't expected to be identical. The test proves 'our compiler's output would run correctly on the real VM if upstream could load it.'

Also: extend docs/demo-plan-status.md with a short 'real-VM status' subsection flagging 019 as the upstream-blocked work.

Commit: 'parity: Rust compiler output matches upstream ancestor semantics'.