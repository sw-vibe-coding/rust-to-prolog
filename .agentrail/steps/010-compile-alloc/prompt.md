Fix src/compile.rs to emit ALLOCATE/DEALLOCATE for recursive rules, and add ALLOCATE/DEALLOCATE/PUT_Y_VAL/GET_Y_VAR support in src/refvm/.

Background: step 005 deferred Y-register classification under the (incorrect) assumption that X-registers are preserved across CALL. They're not — CALL saves PC+2 to CP and does not touch X-regs per se, but CP is clobbered on every CALL. The recursive ancestor_c2 clause does CALL parent_entry immediately followed by more instructions and a tail-call EXECUTE ancestor_entry; without ALLOCATE/DEALLOCATE the CP from the outer CALL ancestor_entry is lost, and the resulting .lam infinite-loops under a correct VM. The refvm step (009) deliberately used hand-crafted .lam for its tests so it wasn't blocked on this.

Scope:
1. Y-reg classification in compile.rs: a variable is *permanent* if it appears in more than one body goal (including across CALL boundaries). Emit ALLOCATE N at the top of each rule body that needs permanent vars; emit DEALLOCATE just before the tail-call EXECUTE / PROCEED.
2. Use GET_Y_VAR Yi, Ai to seed a Y-reg from an A-reg on first occurrence; PUT_Y_VAL Yi, Ai to load a Y-reg back to an A-reg for a body goal.
3. In refvm: add environment-frame support (stack of frames, each with saved_CP + N Y-slots). Implement ALLOCATE, DEALLOCATE, PUT_Y_VAL, GET_Y_VAR opcodes. Choice points must save/restore the env-stack top so backtracking unwinds env frames too.
4. Regenerate tests/fixtures/ancestor.lam (the golden .lam) and tests/fixtures/ancestor.bin (the golden assembler output from lam_asm.py).
5. Update the compile.rs docstring to remove the stale 'X-registers preserved across CALL' comment and document the Y-reg classification rule.
6. Add a new refvm scenario test: the full Rust-compiled ancestor.pl runs on refvm and halts.

Acceptance: cargo test passes; the refvm scenario 'compiled_ancestor_runs_on_refvm' is new and green; the existing 'compiled_parent_runs_on_refvm' still passes; the ancestor emit + asm byte-diff tests pass against the regenerated golden fixtures.

Commit: 'compile+refvm: ALLOCATE/DEALLOCATE for recursive rules'.