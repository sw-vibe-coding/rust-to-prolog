//! Refvm scenario tests. Each scenario is a hand-crafted `.lam`
//! string assembled through `rust_to_prolog::asm`, then executed on
//! `refvm::run_with`. Mirrors the subset of
//! `sw-cor24-prolog/scripts/run-tests.sh` that exercises the
//! ancestor-family opcodes.

use rust_to_prolog::asm::{assemble, Cells};
use rust_to_prolog::refvm::{run_with, RunResult};

fn cells_to_vec(c: &Cells) -> Vec<u32> {
    (0..c.len()).map(|i| *c.get(i).expect("in range")).collect()
}

fn run(src: &str) -> (RunResult, Vec<u8>) {
    let cells = assemble(src).expect("assemble ok");
    let code = cells_to_vec(&cells);
    let mut out = Vec::new();
    let res = run_with(code, &mut out);
    (res, out)
}

#[test]
fn fact_lookup_halts_on_matching_atom() {
    let src = "\
.atom 0 p
.atom 1 bob

    EXECUTE query

p_c1_body:
    GET_CONST A0, atom(bob)
    PROCEED

p_entry:
    EXECUTE p_c1_body

query:
    PUT_CONST A0, atom(bob)
    CALL p_entry
    HALT
";
    let (r, _) = run(src);
    assert_eq!(r, RunResult::Halt);
}

#[test]
fn fact_fail_when_head_does_not_unify() {
    let src = "\
.atom 0 p
.atom 1 bob
.atom 2 ann

    EXECUTE query

p_c1_body:
    GET_CONST A0, atom(ann)
    PROCEED

p_entry:
    EXECUTE p_c1_body

query:
    PUT_CONST A0, atom(bob)
    CALL p_entry
    HALT
";
    let (r, _) = run(src);
    assert_eq!(r, RunResult::Fail);
}

#[test]
fn put_var_binds_unbound_query_arg_to_atom() {
    let src = "\
.atom 0 p
.atom 1 bob

    EXECUTE query

p_c1_body:
    GET_VAR X0, A0
    PROCEED

p_entry:
    EXECUTE p_c1_body

query:
    PUT_CONST A0, atom(bob)
    CALL p_entry
    HALT
";
    let (r, _) = run(src);
    assert_eq!(r, RunResult::Halt);
}

#[test]
fn put_var_shared_arg_unifies_across_positions() {
    let src = "\
.atom 0 p
.atom 1 bob

    EXECUTE query

p_c1_body:
    GET_CONST A0, atom(bob)
    GET_CONST A1, atom(bob)
    PROCEED

p_entry:
    EXECUTE p_c1_body

query:
    PUT_VAR X0, A0
    PUT_VAL X0, A1
    CALL p_entry
    HALT
";
    let (r, _) = run(src);
    assert_eq!(r, RunResult::Halt);
}

#[test]
fn try_trust_first_clause_succeeds() {
    let src = "\
.atom 0 p
.atom 1 bob
.atom 2 ann

    EXECUTE query

p_c1_body:
    GET_CONST A0, atom(bob)
    PROCEED
p_c2_body:
    GET_CONST A0, atom(ann)
    PROCEED

p_entry:
    TRY p_c2
    EXECUTE p_c1_body
p_c2:
    TRUST
    EXECUTE p_c2_body

query:
    PUT_CONST A0, atom(bob)
    CALL p_entry
    HALT
";
    let (r, _) = run(src);
    assert_eq!(r, RunResult::Halt);
}

#[test]
fn backtrack_to_second_clause_succeeds() {
    let src = "\
.atom 0 p
.atom 1 bob
.atom 2 ann

    EXECUTE query

p_c1_body:
    GET_CONST A0, atom(bob)
    PROCEED
p_c2_body:
    GET_CONST A0, atom(ann)
    PROCEED

p_entry:
    TRY p_c2
    EXECUTE p_c1_body
p_c2:
    TRUST
    EXECUTE p_c2_body

query:
    PUT_CONST A0, atom(ann)
    CALL p_entry
    HALT
";
    let (r, _) = run(src);
    assert_eq!(r, RunResult::Halt);
}

#[test]
fn two_facts_both_fail_yields_fail() {
    let src = "\
.atom 0 p
.atom 1 bob
.atom 2 ann
.atom 3 liz

    EXECUTE query

p_c1_body:
    GET_CONST A0, atom(bob)
    PROCEED
p_c2_body:
    GET_CONST A0, atom(ann)
    PROCEED

p_entry:
    TRY p_c2
    EXECUTE p_c1_body
p_c2:
    TRUST
    EXECUTE p_c2_body

query:
    PUT_CONST A0, atom(liz)
    CALL p_entry
    HALT
";
    let (r, _) = run(src);
    assert_eq!(r, RunResult::Fail);
}

#[test]
fn retry_three_clauses_all_fail_yields_fail() {
    let src = "\
.atom 0 p
.atom 1 a
.atom 2 b
.atom 3 c
.atom 4 x

    EXECUTE query

p_c1_body:
    GET_CONST A0, atom(a)
    PROCEED
p_c2_body:
    GET_CONST A0, atom(b)
    PROCEED
p_c3_body:
    GET_CONST A0, atom(c)
    PROCEED

p_entry:
    TRY p_c2
    EXECUTE p_c1_body
p_c2:
    RETRY p_c3
    EXECUTE p_c2_body
p_c3:
    TRUST
    EXECUTE p_c3_body

query:
    PUT_CONST A0, atom(x)
    CALL p_entry
    HALT
";
    let (r, _) = run(src);
    assert_eq!(r, RunResult::Fail);
}

#[test]
fn retry_three_clauses_middle_matches() {
    let src = "\
.atom 0 p
.atom 1 a
.atom 2 b
.atom 3 c

    EXECUTE query

p_c1_body:
    GET_CONST A0, atom(a)
    PROCEED
p_c2_body:
    GET_CONST A0, atom(b)
    PROCEED
p_c3_body:
    GET_CONST A0, atom(c)
    PROCEED

p_entry:
    TRY p_c2
    EXECUTE p_c1_body
p_c2:
    RETRY p_c3
    EXECUTE p_c2_body
p_c3:
    TRUST
    EXECUTE p_c3_body

query:
    PUT_CONST A0, atom(b)
    CALL p_entry
    HALT
";
    let (r, _) = run(src);
    assert_eq!(r, RunResult::Halt);
}

#[test]
fn b_write_and_b_nl_capture_output() {
    let src = "\
.atom 0 hello

    EXECUTE query

query:
    PUT_CONST A0, atom(hello)
    B_WRITE A0
    B_NL
    HALT
";
    let (r, out) = run(src);
    assert_eq!(r, RunResult::Halt);
    assert_eq!(out, b"atom(0)\n");
}

#[test]
fn compiled_parent_runs_on_refvm() {
    use rust_to_prolog::compile::compile;
    use rust_to_prolog::emit::emit;
    use rust_to_prolog::parse::{parse, AtomTable};
    use rust_to_prolog::tokenize::tokenize;

    let prolog = "parent(bob, ann). parent(ann, liz). ?- parent(bob, ann).";
    let toks = tokenize(prolog).expect("tokenize ok");
    let mut atoms = AtomTable::new();
    let clauses = parse(&toks, &mut atoms).expect("parse ok");
    let instrs = compile(&clauses, &atoms).expect("compile ok");
    let lam = emit(&instrs, &atoms).expect("emit ok");
    let (r, _) = run(&lam);
    assert_eq!(r, RunResult::Halt);
}
