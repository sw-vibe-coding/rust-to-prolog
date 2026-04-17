//! Byte-diff of the Rust pipeline's emitted .lam against a
//! human-verified golden fixture. Per plan.md §'Known risks': the
//! fixture is per-spec (asm-spec.md), not a literal codegen.sno
//! capture — codegen.sno hardcodes a 10-atom preamble that the Rust
//! side deliberately does not reproduce.

use rust_to_prolog::compile::compile;
use rust_to_prolog::emit::emit;
use rust_to_prolog::parse::{parse, AtomTable};
use rust_to_prolog::tokenize::tokenize;

const ANCESTOR_SRC: &str = include_str!("../../examples/ancestor.pl");
const ANCESTOR_GOLDEN: &str = include_str!("../fixtures/ancestor.lam");

fn compile_ancestor_lam() -> String {
    let toks = tokenize(ANCESTOR_SRC).expect("tokenize ok");
    let mut atoms = AtomTable::new();
    let clauses = parse(&toks, &mut atoms).expect("parse ok");
    let prog = compile(&clauses, &atoms).expect("compile ok");
    emit(&prog, &atoms).expect("emit ok")
}

#[test]
fn ancestor_emit_byte_matches_golden() {
    let actual = compile_ancestor_lam();
    if actual != ANCESTOR_GOLDEN {
        eprintln!("--- expected ({} bytes) ---", ANCESTOR_GOLDEN.len());
        eprintln!("{ANCESTOR_GOLDEN}");
        eprintln!("--- actual ({} bytes) ---", actual.len());
        eprintln!("{actual}");
        panic!("ancestor.lam drifted from golden fixture");
    }
}
