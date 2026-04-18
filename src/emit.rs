//! `.lam` text emitter. Formats a compiled `Vec<Instr>` stream.
//!
//! Output discipline (docs/design.md §'.lam emitter output discipline'):
//! four-space indent for instructions; no trailing whitespace; atom
//! directives in order of first reference (same order compile.rs emits
//! them); labels follow the `pred_entry` / `pred_cK` / `pred_cK_body`
//! scheme. A blank line precedes each section-start label
//! (`*_body`, `*_entry`, `query`) and the initial `EXECUTE query`.
//!
//! Codegen.sno diverges from asm-spec.md in one respect: it emits a
//! fixed 10-atom preamble instead of an interned-in-order one. Per
//! plan.md §'Known risks', the Rust side follows the spec; the
//! golden fixture at tests/fixtures/ancestor.lam is the per-spec
//! reference.

use crate::compile::{Instr, MAX_INSTR};
use crate::parse::{AtomId, AtomTable};
use crate::port::BoundedArr;
use core::fmt::Write;
use thiserror::Error;

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum EmitError {
    #[error("unknown atom id in constant operand")]
    UnknownAtom,
    #[error("output buffer formatting error")]
    Fmt,
}

pub fn emit(
    instrs: &BoundedArr<Instr, MAX_INSTR>,
    atoms: &AtomTable,
) -> Result<String, EmitError> {
    let mut out = String::with_capacity(1024);
    for i in 0..instrs.len() {
        let cur = instrs.get(i).expect("index in range");
        if needs_blank(i, instrs) {
            out.push('\n');
        }
        emit_one(cur, atoms, &mut out)?;
        out.push('\n');
    }
    Ok(out)
}

fn needs_blank(i: usize, instrs: &BoundedArr<Instr, MAX_INSTR>) -> bool {
    if i == 0 {
        return false;
    }
    let prev = instrs.get(i - 1).expect("prev in range");
    let cur = instrs.get(i).expect("cur in range");
    if matches!(prev, Instr::AtomDir { .. }) && !matches!(cur, Instr::AtomDir { .. }) {
        return true;
    }
    if let Instr::Label(l) = cur {
        return is_section_label(l.as_str());
    }
    false
}

fn is_section_label(name: &str) -> bool {
    name == "query" || name.ends_with("_entry") || name.ends_with("_body")
}

fn emit_one(instr: &Instr, atoms: &AtomTable, out: &mut String) -> Result<(), EmitError> {
    match instr {
        Instr::AtomDir { id, name } => {
            wf(out, format_args!(".atom {} {}", id, name.as_str()))
        }
        Instr::Label(l) => wf(out, format_args!("{}:", l.as_str())),
        _ => emit_instr(instr, atoms, out),
    }
}

fn emit_instr(instr: &Instr, atoms: &AtomTable, out: &mut String) -> Result<(), EmitError> {
    match instr {
        Instr::PutConst { ai, atom } => emit_put_const(*ai, *atom, atoms, out),
        Instr::PutVar { ai, xi } => wf(out, format_args!("    PUT_VAR X{xi}, A{ai}")),
        Instr::PutVal { ai, xi } => wf(out, format_args!("    PUT_VAL X{xi}, A{ai}")),
        Instr::PutYVal { ai, yi } => wf(out, format_args!("    PUT_Y_VAL Y{yi}, A{ai}")),
        Instr::GetVar { ai, xi } => wf(out, format_args!("    GET_VAR X{xi}, A{ai}")),
        Instr::GetYVar { ai, yi } => wf(out, format_args!("    GET_Y_VAR Y{yi}, A{ai}")),
        Instr::GetConst { ai, atom } => emit_get_const(*ai, *atom, atoms, out),
        Instr::GetStruct { ai, atom, arity } => emit_get_struct(*ai, *atom, *arity, atoms, out),
        Instr::UnifyVar { xi } => wf(out, format_args!("    UNIFY_VAR X{xi}")),
        Instr::UnifyVal { xi } => wf(out, format_args!("    UNIFY_VAL X{xi}")),
        Instr::UnifyConst { atom } => emit_unify_const(*atom, atoms, out),
        Instr::Allocate { n } => wf(out, format_args!("    ALLOCATE {n}")),
        Instr::Deallocate => ps(out, "    DEALLOCATE"),
        Instr::Call { label } => wf(out, format_args!("    CALL {}", label.as_str())),
        Instr::Execute { label } => wf(out, format_args!("    EXECUTE {}", label.as_str())),
        Instr::Proceed => ps(out, "    PROCEED"),
        Instr::Try { label } => wf(out, format_args!("    TRY {}", label.as_str())),
        Instr::Retry { label } => wf(out, format_args!("    RETRY {}", label.as_str())),
        Instr::Trust { .. } => ps(out, "    TRUST"),
        Instr::Cut => ps(out, "    CUT"),
        Instr::Fail => ps(out, "    FAIL"),
        Instr::BWrite { ai } => wf(out, format_args!("    B_WRITE A{ai}")),
        Instr::BNl => ps(out, "    B_NL"),
        Instr::BIsAdd { .. } => ps(out, "    B_IS_ADD"),
        Instr::BIsSub { .. } => ps(out, "    B_IS_SUB"),
        Instr::BLt { .. } => ps(out, "    B_LT"),
        Instr::BGt { .. } => ps(out, "    B_GT"),
        Instr::Halt => ps(out, "    HALT"),
        Instr::AtomDir { .. } | Instr::Label(_) => Ok(()),
    }
}

fn emit_put_const(ai: u8, atom: AtomId, atoms: &AtomTable, out: &mut String) -> Result<(), EmitError> {
    let n = atom_name(atoms, atom)?;
    wf(out, format_args!("    PUT_CONST A{ai}, atom({n})"))
}

fn emit_unify_const(atom: AtomId, atoms: &AtomTable, out: &mut String) -> Result<(), EmitError> {
    let n = atom_name(atoms, atom)?;
    wf(out, format_args!("    UNIFY_CONST atom({n})"))
}

fn emit_get_const(ai: u8, atom: AtomId, atoms: &AtomTable, out: &mut String) -> Result<(), EmitError> {
    let n = atom_name(atoms, atom)?;
    wf(out, format_args!("    GET_CONST A{ai}, atom({n})"))
}

fn emit_get_struct(
    ai: u8,
    atom: AtomId,
    arity: u8,
    atoms: &AtomTable,
    out: &mut String,
) -> Result<(), EmitError> {
    let n = atom_name(atoms, atom)?;
    wf(out, format_args!("    GET_STRUCT A{ai}, {n}/{arity}"))
}

fn atom_name(atoms: &AtomTable, id: AtomId) -> Result<&str, EmitError> {
    atoms.name(id).map(|n| n.as_str()).ok_or(EmitError::UnknownAtom)
}

fn wf(out: &mut String, args: core::fmt::Arguments<'_>) -> Result<(), EmitError> {
    out.write_fmt(args).map_err(|_| EmitError::Fmt)
}

fn ps(out: &mut String, s: &str) -> Result<(), EmitError> {
    out.push_str(s);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile::{compile, Instr, LABEL_CAP};
    use crate::parse::{parse, AtomTable, NAME_CAP};
    use crate::port::BoundedStr;
    use crate::tokenize::tokenize;

    fn prog_for(src: &str) -> (BoundedArr<Instr, MAX_INSTR>, AtomTable) {
        let toks = tokenize(src).expect("tokenize ok");
        let mut atoms = AtomTable::new();
        let clauses = parse(&toks, &mut atoms).expect("parse ok");
        let prog = compile(&clauses, &atoms).expect("compile ok");
        (prog, atoms)
    }

    fn lbl(s: &str) -> BoundedStr<LABEL_CAP> {
        BoundedStr::<LABEL_CAP>::from_str(s).expect("label fits")
    }

    fn nm(s: &str) -> BoundedStr<NAME_CAP> {
        BoundedStr::<NAME_CAP>::from_str(s).expect("name fits")
    }

    fn one(i: Instr) -> String {
        let mut prog: BoundedArr<Instr, MAX_INSTR> = BoundedArr::new();
        prog.push(i).expect("one");
        let atoms = AtomTable::new();
        emit(&prog, &atoms).expect("emit ok")
    }

    #[test]
    fn format_put_var() {
        assert_eq!(one(Instr::PutVar { ai: 1, xi: 2 }), "    PUT_VAR X2, A1\n");
    }

    #[test]
    fn format_put_val() {
        assert_eq!(one(Instr::PutVal { ai: 0, xi: 0 }), "    PUT_VAL X0, A0\n");
    }

    #[test]
    fn format_get_var() {
        assert_eq!(one(Instr::GetVar { ai: 0, xi: 0 }), "    GET_VAR X0, A0\n");
    }

    #[test]
    fn format_trust_has_no_operand() {
        assert_eq!(one(Instr::Trust { label: lbl("p_c2_body") }), "    TRUST\n");
    }

    #[test]
    fn format_call_and_execute() {
        assert_eq!(one(Instr::Call { label: lbl("p_entry") }), "    CALL p_entry\n");
        assert_eq!(
            one(Instr::Execute { label: lbl("p_c1_body") }),
            "    EXECUTE p_c1_body\n"
        );
    }

    #[test]
    fn format_simple_opcodes() {
        assert_eq!(one(Instr::Proceed), "    PROCEED\n");
        assert_eq!(one(Instr::Halt), "    HALT\n");
        assert_eq!(one(Instr::BNl), "    B_NL\n");
        assert_eq!(one(Instr::Deallocate), "    DEALLOCATE\n");
    }

    #[test]
    fn format_atom_dir_and_const() {
        let mut atoms = AtomTable::new();
        let id = atoms.intern("bob").expect("intern");
        let mut prog: BoundedArr<Instr, MAX_INSTR> = BoundedArr::new();
        prog.push(Instr::AtomDir { id, name: nm("bob") }).expect("dir");
        prog.push(Instr::PutConst { ai: 0, atom: id }).expect("pc");
        let out = emit(&prog, &atoms).expect("emit ok");
        // Blank line separates atom-dir preamble from the first non-dir
        // instruction.
        assert_eq!(out, ".atom 0 bob\n\n    PUT_CONST A0, atom(bob)\n");
    }

    #[test]
    fn format_label_line_no_indent() {
        let prog = {
            let mut p: BoundedArr<Instr, MAX_INSTR> = BoundedArr::new();
            p.push(Instr::Label(lbl("parent_c2"))).expect("lbl");
            p.push(Instr::Trust { label: lbl("parent_c2_body") }).expect("t");
            p
        };
        let out = emit(&prog, &AtomTable::new()).expect("emit ok");
        // `_cK` retry label gets no blank line before it.
        assert_eq!(out, "parent_c2:\n    TRUST\n");
    }

    #[test]
    fn blank_line_before_body_and_entry_labels() {
        let mut prog: BoundedArr<Instr, MAX_INSTR> = BoundedArr::new();
        prog.push(Instr::Proceed).expect("p1");
        prog.push(Instr::Label(lbl("p_c2_body"))).expect("l1");
        prog.push(Instr::Proceed).expect("p2");
        prog.push(Instr::Label(lbl("p_entry"))).expect("l2");
        prog.push(Instr::Execute { label: lbl("p_c1_body") }).expect("e");
        let out = emit(&prog, &AtomTable::new()).expect("emit ok");
        let expected =
            "    PROCEED\n\np_c2_body:\n    PROCEED\n\np_entry:\n    EXECUTE p_c1_body\n";
        assert_eq!(out, expected);
    }

    #[test]
    fn emit_ancestor_matches_golden_fixture() {
        let (prog, atoms) = prog_for(include_str!("../examples/ancestor.pl"));
        let actual = emit(&prog, &atoms).expect("emit ok");
        let expected = include_str!("../tests/fixtures/ancestor.lam");
        assert_eq!(actual, expected);
    }

    #[test]
    fn emit_ancestor_no_trailing_whitespace() {
        let (prog, atoms) = prog_for(include_str!("../examples/ancestor.pl"));
        let out = emit(&prog, &atoms).expect("emit ok");
        for (i, line) in out.lines().enumerate() {
            assert_eq!(
                line.trim_end(),
                line,
                "line {i} has trailing whitespace: {line:?}"
            );
        }
    }

    #[test]
    fn emit_ancestor_file_ends_with_newline() {
        let (prog, atoms) = prog_for(include_str!("../examples/ancestor.pl"));
        let out = emit(&prog, &atoms).expect("emit ok");
        assert!(out.ends_with('\n'));
    }
}
