//! Structural parity between our Rust-compiled ancestor bytecode and
//! the upstream VM's hardcoded `LOAD_ANCESTOR_COMPILED` reference in
//! `sw-cor24-prolog/src/vm/vm_tests.plsw`.
//!
//! With real-VM integration deferred (see saga step 019-integration-
//! ancestor, blocked on upstream runtime bytecode injection), this
//! test is how we close the loop on "our compiler emits correct
//! bytecode": decode the output and check structural invariants
//! (opcode set, env-frame balance, CALL/EXECUTE counts, terminator).
//!
//! Known intentional differences from the upstream hand-written
//! reference — documented, not enforced:
//!
//! | Axis            | Upstream LOAD_ANCESTOR_COMPILED     | Rust pipeline                           |
//! |-----------------|-------------------------------------|-----------------------------------------|
//! | Layout          | query @ 0, ancestor @ 7, parent @ 23 | atom dirs + EXECUTE query, parent, ancestor, query |
//! | Env frame size  | ALLOCATE 1 (Y0=Y; Z kept in X2)     | ALLOCATE 2 (Y0=Y, Y1=Z)                 |
//! | Scratch strategy| relies on parent not touching X2    | conservative — permanent vars go to Y   |
//! | Cell count      | 36                                   | 52                                       |
//! | Atom IDs        | {bob=1, ann=2, liz=4}               | {parent=0, bob=1, ann=2, liz=3, ancestor=4} |
//!
//! The two bytecodes produce the same answer on the same input; they
//! are not byte-compatible and are not intended to be. The real-VM
//! integration will run the upstream reference through cor24-run
//! once step 019 unblocks, at which point a cross-check becomes
//! possible via a shared parity fixture.

use rust_to_prolog::asm::assemble;

const ANCESTOR_LAM: &str = include_str!("../fixtures/ancestor.lam");

fn cells_of() -> Vec<u32> {
    let c = assemble(ANCESTOR_LAM).expect("assemble ok");
    (0..c.len()).map(|i| *c.get(i).expect("in range")).collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Decoded {
    addr: usize,
    opcode: u8,
    op1: u8,
    op2: u8,
    imm: Option<u32>,
}

fn decode(cells: &[u32]) -> Vec<Decoded> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < cells.len() {
        let c = cells[i];
        let opcode = ((c >> 16) & 0xFF) as u8;
        let op1 = ((c >> 8) & 0xFF) as u8;
        let op2 = (c & 0xFF) as u8;
        let (imm, width) = if is_two_cell(opcode) {
            let imm = cells.get(i + 1).copied();
            (imm, 2)
        } else {
            (None, 1)
        };
        out.push(Decoded {
            addr: i,
            opcode,
            op1,
            op2,
            imm,
        });
        i += width;
    }
    out
}

fn is_two_cell(opcode: u8) -> bool {
    matches!(opcode, 2 | 3 | 6 | 7 | 12 | 18 | 19)
    // CALL, EXECUTE, TRY, RETRY, PUT_CONST, GET_CONST, GET_STRUCT
}

fn count_opcode(decoded: &[Decoded], op: u8) -> usize {
    decoded.iter().filter(|d| d.opcode == op).count()
}

fn opcode_set(decoded: &[Decoded]) -> Vec<u8> {
    let mut seen: Vec<u8> = decoded.iter().map(|d| d.opcode).collect();
    seen.sort_unstable();
    seen.dedup();
    seen
}

#[test]
fn ancestor_bytecode_uses_only_expected_opcodes() {
    let cells = cells_of();
    let decoded = decode(&cells);
    let set = opcode_set(&decoded);
    // Expected: HALT(1), CALL(2), EXECUTE(3), PROCEED(4), TRY(6),
    // TRUST(8), PUT_VAR(10), PUT_VAL(11), PUT_CONST(12), PUT_Y_VAL(13),
    // GET_VAR(16), GET_CONST(18), GET_Y_VAR(20), ALLOCATE(28),
    // DEALLOCATE(29). No RETRY (2-clause preds only), no builtins.
    let expected: Vec<u8> = vec![1, 2, 3, 4, 6, 8, 10, 11, 12, 13, 16, 18, 20, 28, 29];
    assert_eq!(set, expected, "unexpected opcodes: {set:?}");
}

#[test]
fn ancestor_bytecode_has_balanced_env_frame() {
    let cells = cells_of();
    let decoded = decode(&cells);
    assert_eq!(
        count_opcode(&decoded, 28),
        1,
        "expected exactly one ALLOCATE"
    );
    assert_eq!(
        count_opcode(&decoded, 29),
        1,
        "expected exactly one DEALLOCATE"
    );

    // ALLOCATE must come before DEALLOCATE in cell order.
    let alloc_ix = decoded
        .iter()
        .position(|d| d.opcode == 28)
        .expect("ALLOCATE present");
    let dealloc_ix = decoded
        .iter()
        .position(|d| d.opcode == 29)
        .expect("DEALLOCATE present");
    assert!(alloc_ix < dealloc_ix, "ALLOCATE must precede DEALLOCATE");

    // ALLOCATE's op1 is the frame size. Conservative 2-Y layout.
    let alloc = decoded[alloc_ix];
    assert_eq!(alloc.op1, 2, "expected ALLOCATE 2 (conservative Y-reg)");
}

#[test]
fn ancestor_bytecode_terminates_in_single_halt() {
    let cells = cells_of();
    let decoded = decode(&cells);
    assert_eq!(count_opcode(&decoded, 1), 1, "expected exactly one HALT");
    let last = decoded.last().expect("non-empty");
    assert_eq!(last.opcode, 1, "HALT must be the final instruction");
}

#[test]
fn ancestor_bytecode_has_two_try_trust_pairs() {
    // Two multi-clause predicates (parent/2 and ancestor/2), each
    // dispatched by a TRY-TRUST pair — no RETRY because neither
    // predicate has 3+ clauses.
    let cells = cells_of();
    let decoded = decode(&cells);
    assert_eq!(
        count_opcode(&decoded, 6),
        2,
        "expected 2 TRY (one per predicate)"
    );
    assert_eq!(
        count_opcode(&decoded, 8),
        2,
        "expected 2 TRUST (one per predicate)"
    );
    assert_eq!(
        count_opcode(&decoded, 7),
        0,
        "expected 0 RETRY (2-clause preds)"
    );
}

#[test]
fn ancestor_bytecode_emits_one_nontail_call_to_parent() {
    // Inside ancestor_c2_body, parent/2 is invoked via CALL (not
    // EXECUTE) because the recursive EXECUTE ancestor_entry follows.
    // The body of ancestor_c1_body is a single-goal tail call to
    // parent (EXECUTE, no env frame needed).
    let cells = cells_of();
    let decoded = decode(&cells);
    assert_eq!(
        count_opcode(&decoded, 2),
        2,
        "expected 2 CALLs (query+ancestor_c2_body)"
    );
}

#[test]
fn ancestor_bytecode_uses_three_y_slot_ops() {
    // ancestor_c2_body: GET_Y_VAR Y0, A1 (save Y from head);
    // GET_Y_VAR Y1, A1 (save fresh Z); PUT_Y_VAL Y1, A0; PUT_Y_VAL Y0, A1.
    // That's 2 GET_Y_VAR + 2 PUT_Y_VAL.
    let cells = cells_of();
    let decoded = decode(&cells);
    assert_eq!(count_opcode(&decoded, 20), 2, "expected 2 GET_Y_VAR");
    assert_eq!(count_opcode(&decoded, 13), 2, "expected 2 PUT_Y_VAL");
}

#[test]
fn ancestor_bytecode_cell_count_matches_golden() {
    let cells = cells_of();
    assert_eq!(
        cells.len(),
        52,
        "cell count changed — inspect bytecode drift"
    );
}
