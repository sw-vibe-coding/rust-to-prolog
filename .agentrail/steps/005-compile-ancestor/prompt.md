Implement src/compile.rs sufficient for examples/ancestor.pl.

Input: BoundedArr<Clause, 64> (from parse). Output: BoundedArr<Instr, 2048>.

Instr enum: full variant set per docs/design.md (PutConst, PutVar, PutVal, PutYVal, GetVar, GetYVar, GetConst, GetStruct, UnifyVar, UnifyVal, Allocate, Deallocate, Call, Execute, Proceed, Try, Retry, Trust, Cut, Fail, BWrite, BNl, BIsAdd, BIsSub, BLt, BGt, Halt, Label, AtomDir).

For this step, implement only what ancestor.pl needs:
- PutConst, PutVar, PutVal, PutYVal, GetVar, GetYVar, GetConst.
- Allocate, Deallocate.
- Call, Execute, Proceed, Halt.
- Try, Retry, Trust.
- Label, AtomDir.
- Later steps add GetStruct, UnifyVar, UnifyVal, Cut, Fail, B_* variants.

Algorithm:
1. Variable classification: scan each clause; mark vars appearing across more than one body goal as permanent (Y-reg).
2. Head compilation: for each head arg, emit GET into A-register Ai. Constants GET_CONST; first-seen var GET_VAR (X or Y); second-seen var GET_VAL.
3. Body compilation: for each goal, emit PUT into A-registers then CALL functor/arity. Last goal is EXECUTE (tail call).
4. Environment: ALLOCATE N at entry if any Y-regs used; DEALLOCATE before EXECUTE/PROCEED of last goal.
5. Clause dispatch: TRY/RETRY/TRUST chain for multi-clause predicates.

Labels: pred_N and pred_cK per codegen.sno.

Tests:
- Compile examples/ancestor.pl; assert instruction count matches a pinned reference (obtained by running codegen.sno).
- Spot-check specific instructions at known positions (e.g., 'first instr is a Label for ancestor_1').

Acceptance: cargo test passes; port-audit passes.

Commit: 'compile: WAM compilation for ancestor subset'.