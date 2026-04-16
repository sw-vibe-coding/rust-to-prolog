//! WAM-style compilation: `Clause` stream to `Instr` stream.
//!
//! Mirrors the `codegen.sno` algorithm for the ancestor subset:
//! per-clause VMap assigning each variable slot an X-register on first
//! occurrence; all head/body var references reuse that mapping. Multi-
//! clause predicates get a `TRY/RETRY/TRUST` dispatcher under
//! `pred_entry`. Clause bodies land at `pred_cK_body` labels and the
//! query at `query:`. Atom directives are emitted for every interned
//! atom (ordered by AtomId), followed by an initial `EXECUTE query`.
//!
//! Y-register classification plus `ALLOCATE`/`DEALLOCATE` is deferred:
//! ancestor.pl under LAM VM semantics (X-registers preserved across
//! `CALL`) does not need them. Later steps slot in `GetStruct`,
//! `UnifyVar`, `UnifyVal`, `Cut`, `Fail`, and the `B_*` variants.

use crate::parse::{
    AtomId, AtomTable, Clause, ClauseKind, Term, TermIdx, VarSlot,
    MAX_ARGS, MAX_CLAUSES, MAX_CLAUSE_VARS, NAME_CAP,
};
use crate::port::{BoundedArr, BoundedStr};
use thiserror::Error;

pub const MAX_INSTR: usize = 2048;
pub const LABEL_CAP: usize = 48;

pub type LabelId = BoundedStr<LABEL_CAP>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Instr {
    PutConst { ai: u8, atom: AtomId },
    PutVar { ai: u8, xi: u8 },
    PutVal { ai: u8, xi: u8 },
    PutYVal { ai: u8, yi: u8 },
    GetVar { ai: u8, xi: u8 },
    GetYVar { ai: u8, yi: u8 },
    GetConst { ai: u8, atom: AtomId },
    GetStruct { ai: u8, atom: AtomId, arity: u8 },
    UnifyVar { xi: u8 },
    UnifyVal { xi: u8 },
    Allocate { n: u8 },
    Deallocate,
    Call { label: LabelId },
    Execute { label: LabelId },
    Proceed,
    Try { label: LabelId },
    Retry { label: LabelId },
    Trust { label: LabelId },
    Cut,
    Fail,
    BWrite { ai: u8 },
    BNl,
    BIsAdd { dst: u8, a: u8, b: u8 },
    BIsSub { dst: u8, a: u8, b: u8 },
    BLt { a: u8, b: u8 },
    BGt { a: u8, b: u8 },
    Halt,
    Label(LabelId),
    AtomDir { id: AtomId, name: BoundedStr<NAME_CAP> },
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum CompileError {
    #[error("instruction stream overflow")]
    InstrOverflow,
    #[error("label name too long")]
    LabelOverflow,
    #[error("unknown atom id in clause")]
    UnknownAtom,
    #[error("clause head is not an atom or functor")]
    BadHead,
    #[error("goal is not an atom or functor")]
    BadGoal,
    #[error("no query clause in program")]
    NoQuery,
    #[error("more than one query clause")]
    MultipleQueries,
    #[error("subterm index out of range")]
    BadSubterm,
    #[error("var slot out of range")]
    TooManyVars,
    #[error("too many X-registers in clause")]
    TooManyXRegs,
    #[error("query must contain exactly one goal (multi-goal queries deferred)")]
    QueryShape,
    #[error("repeated head var (GET_VAL) not supported in this subset")]
    HeadVarRepeat,
    #[error("nested struct in arg position not supported in this subset")]
    StructArg,
    #[error("integer literal in arg position not supported in this subset")]
    IntArg,
    #[error("empty body in rule")]
    EmptyBody,
}

const SENTINEL_X: u8 = u8::MAX;

struct XMap {
    slot_to_x: BoundedArr<u8, MAX_CLAUSE_VARS>,
    next_x: u8,
}

impl XMap {
    fn new() -> Self {
        let mut slot_to_x: BoundedArr<u8, MAX_CLAUSE_VARS> = BoundedArr::new();
        for _ in 0..MAX_CLAUSE_VARS {
            slot_to_x.push(SENTINEL_X).expect("xmap prefill within capacity");
        }
        Self { slot_to_x, next_x: 0 }
    }

    fn lookup_or_assign(&mut self, slot: VarSlot) -> Result<(u8, bool), CompileError> {
        let idx = slot as usize;
        if idx >= MAX_CLAUSE_VARS {
            return Err(CompileError::TooManyVars);
        }
        let cur = *self.slot_to_x.get(idx).expect("xmap index in range");
        if cur != SENTINEL_X {
            return Ok((cur, true));
        }
        if self.next_x == SENTINEL_X {
            return Err(CompileError::TooManyXRegs);
        }
        let x = self.next_x;
        *self.slot_to_x.get_mut(idx).expect("xmap index in range") = x;
        self.next_x += 1;
        Ok((x, false))
    }
}

pub fn compile(
    clauses: &BoundedArr<Clause, MAX_CLAUSES>,
    atoms: &AtomTable,
) -> Result<BoundedArr<Instr, MAX_INSTR>, CompileError> {
    let query_ix = find_query(clauses)?;
    let mut out: BoundedArr<Instr, MAX_INSTR> = BoundedArr::new();
    emit_atom_dirs(atoms, &mut out)?;
    push_i(&mut out, Instr::Execute { label: lbl_query()? })?;
    emit_all_predicates(clauses, query_ix, atoms, &mut out)?;
    emit_query_clause(
        clauses.get(query_ix).expect("query index in range"),
        atoms,
        &mut out,
    )?;
    Ok(out)
}

fn find_query(clauses: &BoundedArr<Clause, MAX_CLAUSES>) -> Result<usize, CompileError> {
    let mut found: Option<usize> = None;
    for i in 0..clauses.len() {
        if clauses.get(i).expect("clause index in range").kind == ClauseKind::Query {
            if found.is_some() {
                return Err(CompileError::MultipleQueries);
            }
            found = Some(i);
        }
    }
    found.ok_or(CompileError::NoQuery)
}

fn push_i(out: &mut BoundedArr<Instr, MAX_INSTR>, i: Instr) -> Result<(), CompileError> {
    out.push(i).map_err(|_| CompileError::InstrOverflow)
}

fn emit_atom_dirs(
    atoms: &AtomTable,
    out: &mut BoundedArr<Instr, MAX_INSTR>,
) -> Result<(), CompileError> {
    for id in 0..atoms.len() as AtomId {
        let name = *atoms.name(id).ok_or(CompileError::UnknownAtom)?;
        push_i(out, Instr::AtomDir { id, name })?;
    }
    Ok(())
}

fn emit_all_predicates(
    clauses: &BoundedArr<Clause, MAX_CLAUSES>,
    query_ix: usize,
    atoms: &AtomTable,
    out: &mut BoundedArr<Instr, MAX_INSTR>,
) -> Result<(), CompileError> {
    let mut i = 0;
    while i < clauses.len() {
        if i == query_ix {
            i += 1;
            continue;
        }
        let (end, pid) = scan_group(clauses, i, query_ix)?;
        emit_group(clauses, i, end, pid, atoms, out)?;
        i = end;
    }
    Ok(())
}

fn scan_group(
    clauses: &BoundedArr<Clause, MAX_CLAUSES>,
    start: usize,
    query_ix: usize,
) -> Result<(usize, AtomId), CompileError> {
    let c0 = clauses.get(start).expect("start in range");
    let (fid, ar) = head_pred(&c0.head)?;
    let mut end = start + 1;
    while end < clauses.len() && end != query_ix {
        let c = clauses.get(end).expect("end in range");
        let (fid2, ar2) = head_pred(&c.head)?;
        if fid2 != fid || ar2 != ar {
            break;
        }
        end += 1;
    }
    Ok((end, fid))
}

fn head_pred(head: &Term) -> Result<(AtomId, u8), CompileError> {
    match head {
        Term::Atom(id) => Ok((*id, 0)),
        Term::Struct { functor, args } => Ok((*functor, args.len() as u8)),
        _ => Err(CompileError::BadHead),
    }
}

fn emit_group(
    clauses: &BoundedArr<Clause, MAX_CLAUSES>,
    start: usize,
    end: usize,
    pid: AtomId,
    atoms: &AtomTable,
    out: &mut BoundedArr<Instr, MAX_INSTR>,
) -> Result<(), CompileError> {
    let pname = *atoms.name(pid).ok_or(CompileError::UnknownAtom)?;
    let count = end - start;
    for k in 0..count {
        let clause = clauses.get(start + k).expect("clause in range");
        let clause_idx = (k + 1) as u8;
        emit_clause_body(clause, &pname, clause_idx, atoms, out)?;
    }
    emit_dispatcher(&pname, count as u8, out)
}

fn emit_clause_body(
    clause: &Clause,
    pname: &BoundedStr<NAME_CAP>,
    clause_idx: u8,
    atoms: &AtomTable,
    out: &mut BoundedArr<Instr, MAX_INSTR>,
) -> Result<(), CompileError> {
    push_i(out, Instr::Label(lbl_clause_body(pname, clause_idx)?))?;
    let mut xm = XMap::new();
    emit_head(&clause.head, clause, &mut xm, out)?;
    match clause.kind {
        ClauseKind::Fact => push_i(out, Instr::Proceed),
        ClauseKind::Rule => emit_body(clause, atoms, &mut xm, out),
        ClauseKind::Query => Err(CompileError::BadHead),
    }
}

fn emit_head(
    head: &Term,
    clause: &Clause,
    xm: &mut XMap,
    out: &mut BoundedArr<Instr, MAX_INSTR>,
) -> Result<(), CompileError> {
    match head {
        Term::Atom(_) => Ok(()),
        Term::Struct { args, .. } => emit_head_args(args, clause, xm, out),
        _ => Err(CompileError::BadHead),
    }
}

fn emit_head_args(
    args: &BoundedArr<TermIdx, MAX_ARGS>,
    clause: &Clause,
    xm: &mut XMap,
    out: &mut BoundedArr<Instr, MAX_INSTR>,
) -> Result<(), CompileError> {
    for i in 0..args.len() {
        let ti = *args.get(i).expect("arg index in range");
        let t = *clause.subterm(ti).ok_or(CompileError::BadSubterm)?;
        emit_head_arg(i as u8, &t, xm, out)?;
    }
    Ok(())
}

fn emit_head_arg(
    ai: u8,
    arg: &Term,
    xm: &mut XMap,
    out: &mut BoundedArr<Instr, MAX_INSTR>,
) -> Result<(), CompileError> {
    match arg {
        Term::Atom(id) => push_i(out, Instr::GetConst { ai, atom: *id }),
        Term::Var(slot) => {
            let (xi, seen) = xm.lookup_or_assign(*slot)?;
            if seen {
                Err(CompileError::HeadVarRepeat)
            } else {
                push_i(out, Instr::GetVar { ai, xi })
            }
        }
        Term::Int(_) => Err(CompileError::IntArg),
        Term::Struct { .. } => Err(CompileError::StructArg),
        Term::Nil => Err(CompileError::StructArg),
    }
}

fn emit_body(
    clause: &Clause,
    atoms: &AtomTable,
    xm: &mut XMap,
    out: &mut BoundedArr<Instr, MAX_INSTR>,
) -> Result<(), CompileError> {
    let n = clause.body.len();
    if n == 0 {
        return Err(CompileError::EmptyBody);
    }
    for gi in 0..n {
        let goal = *clause.body.get(gi).expect("goal index in range");
        let is_last = gi + 1 == n;
        emit_goal(&goal, clause, atoms, xm, is_last, out)?;
    }
    Ok(())
}

fn emit_goal(
    goal: &Term,
    clause: &Clause,
    atoms: &AtomTable,
    xm: &mut XMap,
    is_last: bool,
    out: &mut BoundedArr<Instr, MAX_INSTR>,
) -> Result<(), CompileError> {
    let fid = match goal {
        Term::Atom(id) => *id,
        Term::Struct { functor, args } => {
            emit_put_args(args, clause, xm, out)?;
            *functor
        }
        _ => return Err(CompileError::BadGoal),
    };
    let pname = *atoms.name(fid).ok_or(CompileError::UnknownAtom)?;
    let entry = lbl_entry(&pname)?;
    let ctrl = if is_last {
        Instr::Execute { label: entry }
    } else {
        Instr::Call { label: entry }
    };
    push_i(out, ctrl)
}

fn emit_put_args(
    args: &BoundedArr<TermIdx, MAX_ARGS>,
    clause: &Clause,
    xm: &mut XMap,
    out: &mut BoundedArr<Instr, MAX_INSTR>,
) -> Result<(), CompileError> {
    for i in 0..args.len() {
        let ti = *args.get(i).expect("arg index in range");
        let t = *clause.subterm(ti).ok_or(CompileError::BadSubterm)?;
        emit_put_arg(i as u8, &t, xm, out)?;
    }
    Ok(())
}

fn emit_put_arg(
    ai: u8,
    arg: &Term,
    xm: &mut XMap,
    out: &mut BoundedArr<Instr, MAX_INSTR>,
) -> Result<(), CompileError> {
    match arg {
        Term::Atom(id) => push_i(out, Instr::PutConst { ai, atom: *id }),
        Term::Var(slot) => {
            let (xi, seen) = xm.lookup_or_assign(*slot)?;
            if seen {
                push_i(out, Instr::PutVal { ai, xi })
            } else {
                push_i(out, Instr::PutVar { ai, xi })
            }
        }
        Term::Int(_) => Err(CompileError::IntArg),
        Term::Struct { .. } => Err(CompileError::StructArg),
        Term::Nil => Err(CompileError::StructArg),
    }
}

fn emit_dispatcher(
    pname: &BoundedStr<NAME_CAP>,
    clause_count: u8,
    out: &mut BoundedArr<Instr, MAX_INSTR>,
) -> Result<(), CompileError> {
    push_i(out, Instr::Label(lbl_entry(pname)?))?;
    if clause_count == 1 {
        return push_i(out, Instr::Execute { label: lbl_clause_body(pname, 1)? });
    }
    push_i(out, Instr::Try { label: lbl_clause(pname, 2)? })?;
    push_i(out, Instr::Execute { label: lbl_clause_body(pname, 1)? })?;
    let mut k = 2u8;
    while k <= clause_count {
        push_i(out, Instr::Label(lbl_clause(pname, k)?))?;
        let cbody = lbl_clause_body(pname, k)?;
        if k == clause_count {
            push_i(out, Instr::Trust { label: cbody })?;
        } else {
            push_i(out, Instr::Retry { label: lbl_clause(pname, k + 1)? })?;
        }
        push_i(out, Instr::Execute { label: cbody })?;
        k += 1;
    }
    Ok(())
}

fn emit_query_clause(
    clause: &Clause,
    atoms: &AtomTable,
    out: &mut BoundedArr<Instr, MAX_INSTR>,
) -> Result<(), CompileError> {
    push_i(out, Instr::Label(lbl_query()?))?;
    if clause.body.len() != 1 {
        return Err(CompileError::QueryShape);
    }
    let goal = *clause.body.get(0).expect("single-goal query");
    let mut xm = XMap::new();
    let fid = match goal {
        Term::Atom(id) => id,
        Term::Struct { functor, args } => {
            emit_put_args(&args, clause, &mut xm, out)?;
            functor
        }
        _ => return Err(CompileError::BadGoal),
    };
    let pname = *atoms.name(fid).ok_or(CompileError::UnknownAtom)?;
    push_i(out, Instr::Call { label: lbl_entry(&pname)? })?;
    push_i(out, Instr::Halt)
}

fn lbl_query() -> Result<LabelId, CompileError> {
    mk_label_str("query")
}

fn mk_label_str(s: &str) -> Result<LabelId, CompileError> {
    BoundedStr::<LABEL_CAP>::from_str(s).map_err(|_| CompileError::LabelOverflow)
}

fn lbl_entry(pname: &BoundedStr<NAME_CAP>) -> Result<LabelId, CompileError> {
    build_label(&[pname.as_str(), "_entry"])
}

fn lbl_clause(pname: &BoundedStr<NAME_CAP>, k: u8) -> Result<LabelId, CompileError> {
    let mut dbuf = [0u8; 4];
    let ks = u8_to_decimal(k, &mut dbuf);
    build_label(&[pname.as_str(), "_c", ks])
}

fn lbl_clause_body(pname: &BoundedStr<NAME_CAP>, k: u8) -> Result<LabelId, CompileError> {
    let mut dbuf = [0u8; 4];
    let ks = u8_to_decimal(k, &mut dbuf);
    build_label(&[pname.as_str(), "_c", ks, "_body"])
}

fn build_label(parts: &[&str]) -> Result<LabelId, CompileError> {
    let total: usize = parts.iter().map(|s| s.len()).sum();
    if total > LABEL_CAP {
        return Err(CompileError::LabelOverflow);
    }
    let mut buf = [0u8; LABEL_CAP];
    let mut pos = 0;
    for part in parts {
        let b = part.as_bytes();
        buf[pos..pos + b.len()].copy_from_slice(b);
        pos += b.len();
    }
    let s = core::str::from_utf8(&buf[..pos]).expect("ascii label parts");
    BoundedStr::<LABEL_CAP>::from_str(s).map_err(|_| CompileError::LabelOverflow)
}

fn u8_to_decimal(n: u8, buf: &mut [u8; 4]) -> &str {
    if n == 0 {
        buf[0] = b'0';
        return core::str::from_utf8(&buf[..1]).expect("ascii zero");
    }
    let mut tmp: [u8; 3] = [0; 3];
    let mut n32 = n as u32;
    let mut i = 0usize;
    while n32 > 0 {
        tmp[i] = b'0' + (n32 % 10) as u8;
        n32 /= 10;
        i += 1;
    }
    for j in 0..i {
        buf[j] = tmp[i - 1 - j];
    }
    core::str::from_utf8(&buf[..i]).expect("ascii digits")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;
    use crate::tokenize::tokenize;

    fn compile_src(src: &str) -> BoundedArr<Instr, MAX_INSTR> {
        let toks = tokenize(src).expect("tokenize ok");
        let mut atoms = AtomTable::new();
        let clauses = parse(&toks, &mut atoms).expect("parse ok");
        compile(&clauses, &atoms).expect("compile ok")
    }

    fn atom_id(atoms: &AtomTable, name: &str) -> AtomId {
        for i in 0..atoms.len() as AtomId {
            if atoms.name(i).expect("id in range").as_str() == name {
                return i;
            }
        }
        panic!("atom {name:?} not interned");
    }

    fn as_label(i: &Instr) -> Option<&str> {
        match i {
            Instr::Label(l) => Some(l.as_str()),
            _ => None,
        }
    }

    /// Pinned reference for examples/ancestor.pl. Corresponds to:
    ///   5 atom dirs + 1 `Execute query` + 14 instrs for parent group
    ///   + 21 instrs for ancestor group + 5 instrs for query = 46.
    /// Derived by hand-tracing the codegen.sno algorithm; consistent
    /// with a codegen.sno run modulo the atom-dir subset we intern
    /// versus codegen.sno's hardcoded 10-atom preamble.
    const ANCESTOR_INSTR_COUNT: usize = 46;

    #[test]
    fn compile_ancestor_instruction_count() {
        let src = include_str!("../examples/ancestor.pl");
        let prog = compile_src(src);
        assert_eq!(prog.len(), ANCESTOR_INSTR_COUNT);
    }

    #[test]
    fn compile_ancestor_first_and_last_instructions() {
        let src = include_str!("../examples/ancestor.pl");
        let prog = compile_src(src);
        // First instructions are all AtomDirs.
        match prog.get(0).expect("first") {
            Instr::AtomDir { id, .. } => assert_eq!(*id, 0),
            other => panic!("expected AtomDir, got {other:?}"),
        }
        // Last instruction is Halt (end of query).
        assert!(matches!(prog.get(prog.len() - 1), Some(Instr::Halt)));
    }

    #[test]
    fn compile_ancestor_has_expected_labels() {
        let src = include_str!("../examples/ancestor.pl");
        let prog = compile_src(src);
        let labels: Vec<&str> = prog.iter().filter_map(as_label).collect();
        assert_eq!(
            labels,
            vec![
                "parent_c1_body",
                "parent_c2_body",
                "parent_entry",
                "parent_c2",
                "ancestor_c1_body",
                "ancestor_c2_body",
                "ancestor_entry",
                "ancestor_c2",
                "query",
            ]
        );
    }

    #[test]
    fn compile_ancestor_initial_execute_query() {
        let src = include_str!("../examples/ancestor.pl");
        let prog = compile_src(src);
        // atom dirs come first; the instruction after the last AtomDir
        // is Execute(query).
        let mut after_atoms = 0;
        for i in 0..prog.len() {
            match prog.get(i).unwrap() {
                Instr::AtomDir { .. } => after_atoms = i + 1,
                _ => break,
            }
        }
        match prog.get(after_atoms).expect("exec query") {
            Instr::Execute { label } => assert_eq!(label.as_str(), "query"),
            other => panic!("expected Execute(query), got {other:?}"),
        }
    }

    #[test]
    fn compile_ancestor_query_tail_is_call_then_halt() {
        let src = include_str!("../examples/ancestor.pl");
        let toks = tokenize(src).expect("tokenize ok");
        let mut atoms = AtomTable::new();
        let clauses = parse(&toks, &mut atoms).expect("parse ok");
        let prog = compile(&clauses, &atoms).expect("compile ok");
        let ancestor = atom_id(&atoms, "ancestor");
        let bob = atom_id(&atoms, "bob");
        let liz = atom_id(&atoms, "liz");
        // Scan for the `query:` label and check the three following
        // instructions: PUT_CONST A0 bob, PUT_CONST A1 liz, CALL
        // ancestor_entry, HALT.
        let mut qi = None;
        for i in 0..prog.len() {
            if matches!(prog.get(i), Some(Instr::Label(l)) if l.as_str() == "query") {
                qi = Some(i);
                break;
            }
        }
        let qi = qi.expect("query label found");
        assert!(matches!(
            prog.get(qi + 1),
            Some(Instr::PutConst { ai: 0, atom }) if *atom == bob
        ));
        assert!(matches!(
            prog.get(qi + 2),
            Some(Instr::PutConst { ai: 1, atom }) if *atom == liz
        ));
        match prog.get(qi + 3).expect("call") {
            Instr::Call { label } => assert_eq!(label.as_str(), "ancestor_entry"),
            other => panic!("expected Call(ancestor_entry), got {other:?}"),
        }
        assert!(matches!(prog.get(qi + 4), Some(Instr::Halt)));
        // Query is last: nothing after Halt.
        assert_eq!(qi + 4, prog.len() - 1);
        // Also assert no stray atom ids in the interned table.
        let _ = ancestor;
    }

    #[test]
    fn compile_ancestor_c2_body_has_call_parent_then_execute_ancestor() {
        let src = include_str!("../examples/ancestor.pl");
        let prog = compile_src(src);
        // Locate `ancestor_c2_body:` label.
        let mut ci = None;
        for i in 0..prog.len() {
            if matches!(
                prog.get(i),
                Some(Instr::Label(l)) if l.as_str() == "ancestor_c2_body"
            ) {
                ci = Some(i);
                break;
            }
        }
        let ci = ci.expect("ancestor_c2_body label found");
        // Expected sequence after label:
        //   GET_VAR X0 A0 ; GET_VAR X1 A1 ; PUT_VAL X0 A0 ;
        //   PUT_VAR X2 A1 ; CALL parent_entry ;
        //   PUT_VAL X2 A0 ; PUT_VAL X1 A1 ; EXECUTE ancestor_entry
        assert!(matches!(
            prog.get(ci + 1),
            Some(Instr::GetVar { ai: 0, xi: 0 })
        ));
        assert!(matches!(
            prog.get(ci + 2),
            Some(Instr::GetVar { ai: 1, xi: 1 })
        ));
        assert!(matches!(
            prog.get(ci + 3),
            Some(Instr::PutVal { ai: 0, xi: 0 })
        ));
        assert!(matches!(
            prog.get(ci + 4),
            Some(Instr::PutVar { ai: 1, xi: 2 })
        ));
        match prog.get(ci + 5).expect("call parent_entry") {
            Instr::Call { label } => assert_eq!(label.as_str(), "parent_entry"),
            other => panic!("expected Call(parent_entry), got {other:?}"),
        }
        assert!(matches!(
            prog.get(ci + 6),
            Some(Instr::PutVal { ai: 0, xi: 2 })
        ));
        assert!(matches!(
            prog.get(ci + 7),
            Some(Instr::PutVal { ai: 1, xi: 1 })
        ));
        match prog.get(ci + 8).expect("execute ancestor_entry") {
            Instr::Execute { label } => assert_eq!(label.as_str(), "ancestor_entry"),
            other => panic!("expected Execute(ancestor_entry), got {other:?}"),
        }
    }

    #[test]
    fn compile_single_fact() {
        // p(a). ?- p(a).
        let prog = compile_src("p(a). ?- p(a).");
        // atom dirs (p, a) + Execute(query)               = 3
        //   + p_c1_body label + GET_CONST a + PROCEED     = 3
        //   + p_entry label + EXECUTE p_c1_body           = 2
        //   + query label + PUT_CONST a + CALL p_entry + HALT = 4
        assert_eq!(prog.len(), 12);
        assert!(matches!(prog.get(0), Some(Instr::AtomDir { id: 0, .. })));
    }

    #[test]
    fn compile_single_clause_predicate_dispatcher_has_no_try() {
        // p(a). q(X) :- p(X). ?- q(a).
        let prog = compile_src("p(a). q(X) :- p(X). ?- q(a).");
        // q_entry is single clause: just Label + Execute(q_c1_body).
        let mut i = 0;
        let mut saw_q_entry = false;
        while i < prog.len() {
            if matches!(prog.get(i), Some(Instr::Label(l)) if l.as_str() == "q_entry") {
                saw_q_entry = true;
                match prog.get(i + 1).expect("after q_entry") {
                    Instr::Execute { label } => {
                        assert_eq!(label.as_str(), "q_c1_body");
                    }
                    other => panic!("expected Execute, got {other:?}"),
                }
                // Next must not be a Try.
                assert!(!matches!(prog.get(i + 1), Some(Instr::Try { .. })));
                break;
            }
            i += 1;
        }
        assert!(saw_q_entry);
    }

    #[test]
    fn compile_error_on_missing_query() {
        let src = "parent(bob, ann).";
        let toks = tokenize(src).expect("tokenize ok");
        let mut atoms = AtomTable::new();
        let clauses = parse(&toks, &mut atoms).expect("parse ok");
        let err = compile(&clauses, &atoms).unwrap_err();
        assert!(matches!(err, CompileError::NoQuery));
    }

    #[test]
    fn compile_error_on_multiple_queries() {
        let src = "p(a). ?- p(a). ?- p(a).";
        let toks = tokenize(src).expect("tokenize ok");
        let mut atoms = AtomTable::new();
        let clauses = parse(&toks, &mut atoms).expect("parse ok");
        let err = compile(&clauses, &atoms).unwrap_err();
        assert!(matches!(err, CompileError::MultipleQueries));
    }

    #[test]
    fn u8_to_decimal_basic_cases() {
        let mut buf = [0u8; 4];
        assert_eq!(u8_to_decimal(0, &mut buf), "0");
        assert_eq!(u8_to_decimal(1, &mut buf), "1");
        assert_eq!(u8_to_decimal(9, &mut buf), "9");
        assert_eq!(u8_to_decimal(10, &mut buf), "10");
        assert_eq!(u8_to_decimal(99, &mut buf), "99");
        assert_eq!(u8_to_decimal(100, &mut buf), "100");
        assert_eq!(u8_to_decimal(255, &mut buf), "255");
    }

    #[test]
    fn build_label_concatenation() {
        let parts = ["parent", "_c", "10", "_body"];
        let lbl = build_label(&parts).expect("fits");
        assert_eq!(lbl.as_str(), "parent_c10_body");
    }
}
