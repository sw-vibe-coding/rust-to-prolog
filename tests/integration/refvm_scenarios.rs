//! Refvm scenario tests. Each scenario is a hand-crafted `.lam`
//! string assembled through `rust_to_prolog::asm`, then executed on
//! `refvm::run_with`. Mirrors the subset of
//! `sw-cor24-prolog/scripts/run-tests.sh` that exercises the
//! ancestor-family opcodes.

use rust_to_prolog::asm::{assemble, Cells};
use rust_to_prolog::refvm::{run_with, run_with_atoms, RunResult};

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

fn compile_pipeline(src: &str) -> String {
    use rust_to_prolog::compile::compile;
    use rust_to_prolog::emit::emit;
    use rust_to_prolog::parse::{parse, AtomTable};
    use rust_to_prolog::tokenize::tokenize;
    let toks = tokenize(src).expect("tokenize ok");
    let mut atoms = AtomTable::new();
    let clauses = parse(&toks, &mut atoms).expect("parse ok");
    let instrs = compile(&clauses, &atoms).expect("compile ok");
    emit(&instrs, &atoms).expect("emit ok")
}

fn compile_pipeline_with_atoms(src: &str) -> (String, Vec<String>) {
    use rust_to_prolog::compile::compile;
    use rust_to_prolog::emit::emit;
    use rust_to_prolog::parse::{parse, AtomId, AtomTable};
    use rust_to_prolog::tokenize::tokenize;
    let toks = tokenize(src).expect("tokenize ok");
    let mut atoms = AtomTable::new();
    let clauses = parse(&toks, &mut atoms).expect("parse ok");
    let instrs = compile(&clauses, &atoms).expect("compile ok");
    let lam = emit(&instrs, &atoms).expect("emit ok");
    let names: Vec<String> = (0..atoms.len() as AtomId)
        .map(|i| atoms.name(i).expect("id in range").as_str().to_string())
        .collect();
    (lam, names)
}

fn run_with_compiled_atoms(src: &str) -> (RunResult, String) {
    let (lam, names) = compile_pipeline_with_atoms(src);
    let cells = assemble(&lam).expect("assemble ok");
    let code = cells_to_vec(&cells);
    let mut buf = Vec::new();
    let res = run_with_atoms(code, names, &mut buf);
    (res, String::from_utf8(buf).expect("utf8"))
}

#[test]
fn compiled_parent_runs_on_refvm() {
    let lam = compile_pipeline("parent(bob, ann). parent(ann, liz). ?- parent(bob, ann).");
    let (r, _) = run(&lam);
    assert_eq!(r, RunResult::Halt);
}

#[test]
fn compiled_ancestor_runs_on_refvm() {
    // Full Rust pipeline on examples/ancestor.pl. Exercises the
    // ALLOCATE / GET_Y_VAR / PUT_Y_VAL / DEALLOCATE env-frame path
    // that the recursive ancestor_c2 clause needs. Pre-fix, this
    // case infinite-looped because CALL parent_entry clobbered CP
    // and the subsequent tail-call EXECUTE ancestor_entry couldn't
    // return to the query's HALT.
    let src = include_str!("../../examples/ancestor.pl");
    let lam = compile_pipeline(src);
    let (r, _) = run(&lam);
    assert_eq!(r, RunResult::Halt);
}

#[test]
fn compiled_color_prints_all_colors_via_backtracking() {
    // examples/color.pl: `color(X), write(X), nl, fail.` enumerates
    // red/green/blue through TRY/RETRY/TRUST, printing each via the
    // inline B_WRITE builtin. `fail` forces the next retry; when
    // color's choice points exhaust, the whole query returns FAIL.
    let src = include_str!("../../examples/color.pl");
    let (r, out) = run_with_compiled_atoms(src);
    assert_eq!(r, RunResult::Fail);
    assert_eq!(out, "red\ngreen\nblue\n");
}

#[test]
fn compiled_member_prints_each_element_via_backtracking() {
    let src = include_str!("../../examples/member.pl");
    let (r, out) = run_with_compiled_atoms(src);
    assert_eq!(r, RunResult::Fail);
    assert_eq!(out, "a\nb\nc\n");
}

#[test]
fn compiled_write_hello_terminates_via_halt() {
    // `?- write(hello), nl.` — all inline builtins, no user-pred
    // CALL, so no ALLOCATE. Falls through to HALT after nl.
    let (r, out) = run_with_compiled_atoms("?- write(hello), nl.");
    assert_eq!(r, RunResult::Halt);
    assert_eq!(out, "hello\n");
}

#[test]
fn compiled_ancestor_fails_when_no_solution() {
    // Same program, query with no answer: ancestor(liz, bob) has no
    // solution, so the search exhausts all choice points and the VM
    // returns RunResult::Fail rather than looping.
    let lam = compile_pipeline(
        "parent(bob, ann). parent(ann, liz). \
         ancestor(X, Y) :- parent(X, Y). \
         ancestor(X, Y) :- parent(X, Z), ancestor(Z, Y). \
         ?- ancestor(liz, bob).",
    );
    let (r, _) = run(&lam);
    assert_eq!(r, RunResult::Fail);
}
