//! WAM-style compilation: `Clause` stream to `Instr` stream.
//!
//! Algorithm for the ancestor family (facts, single-body rules,
//! multi-body recursive rules, multi-clause dispatch):
//!
//! 1. **Chunk-based variable classification.** A clause body of `n`
//!    goals has `max(1, n)` chunks: chunk 0 is `{head, body[0]}`,
//!    chunk k≥1 is `body[k]`. A variable appearing in more than one
//!    chunk is *permanent* (lives in a Y-reg inside the environment
//!    frame and survives across CALL); otherwise it is *temporary*
//!    (lives in an X-reg and may be clobbered by the callee).
//! 2. **Register assignment.** Walk head, then body in order. Assign
//!    Y-indices to permanent vars and X-indices to temporaries in
//!    order of first occurrence. A single spare X-reg (index
//!    `n_temp`) is reused as a scratch for `PUT_VAR` when a permanent
//!    var first appears as a body goal argument.
//! 3. **Code emission.** At the top of a rule body that has any
//!    permanent vars, emit `ALLOCATE n_perm`. Head args use
//!    `GET_CONST` / `GET_VAR` / `GET_Y_VAR` by role. Body goal args
//!    use `PUT_CONST` / `PUT_VAR` / `PUT_VAL` / `PUT_Y_VAL` plus the
//!    `PUT_VAR scratch, Ai; GET_Y_VAR Yj, Ai` pair for a permanent
//!    var's first occurrence in a body arg. Non-final goals emit
//!    `CALL`; the final goal emits `DEALLOCATE` (when an env frame is
//!    open) followed by `EXECUTE`.
//!
//! Labels: `pred_cK_body` per clause, `pred_entry` per predicate
//! (dispatcher `TRY` / `RETRY` / `TRUST` chain), `query` for the
//! initial goal. Atom directives are emitted in interned order.
//!
//! Deferred (later steps): `GET_STRUCT` / `UNIFY_*` for lists and
//! structures (012), `B_IS_*` / `B_LT` / `B_GT` for arithmetic (013),
//! `CUT` (014).

use crate::parse::{
    AtomId, AtomTable, Clause, ClauseKind, Term, TermIdx, VarSlot,
    MAX_ARGS, MAX_BODY, MAX_CLAUSES, MAX_CLAUSE_VARS, NAME_CAP,
};
use crate::port::{BoundedArr, BoundedStr};
use thiserror::Error;

pub const MAX_INSTR: usize = 2048;
pub const LABEL_CAP: usize = 48;

pub type LabelId = BoundedStr<LABEL_CAP>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Instr {
    PutConst { ai: u8, atom: AtomId },
    PutInt { ai: u8, value: i32 },
    PutVar { ai: u8, xi: u8 },
    PutVal { ai: u8, xi: u8 },
    PutYVal { ai: u8, yi: u8 },
    GetVar { ai: u8, xi: u8 },
    GetVal { ai: u8, xi: u8 },
    GetYVar { ai: u8, yi: u8 },
    GetConst { ai: u8, atom: AtomId },
    GetInt { ai: u8, value: i32 },
    GetStruct { ai: u8, atom: AtomId, arity: u8 },
    UnifyVar { xi: u8 },
    UnifyVal { xi: u8 },
    UnifyConst { atom: AtomId },
    UnifyInt { value: i32 },
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
    #[error("too many Y-registers in clause")]
    TooManyYRegs,
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

const REG_CAP: u8 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Class {
    Unused,
    Temp,
    Perm,
}

#[derive(Clone, Copy, Debug)]
enum VarRole {
    Unused,
    Temp { xi: u8, seen: bool },
    Perm { yi: u8, seen: bool },
}

struct RegMap {
    roles: [VarRole; MAX_CLAUSE_VARS],
    n_temp: u8,
    n_perm: u8,
    pname: BoundedStr<NAME_CAP>,
    clause_idx: u8,
    neg_ix: u8,
}

impl RegMap {
    fn empty() -> Self {
        Self {
            roles: [VarRole::Unused; MAX_CLAUSE_VARS],
            n_temp: 0,
            n_perm: 0,
            pname: BoundedStr::<NAME_CAP>::new(),
            clause_idx: 0,
            neg_ix: 0,
        }
    }

    fn fresh_neg_label(&mut self) -> Result<LabelId, CompileError> {
        let k = self.neg_ix;
        self.neg_ix = self.neg_ix.saturating_add(1);
        let mut cbuf = [0u8; 4];
        let cs = u8_to_decimal(self.clause_idx, &mut cbuf);
        let mut nbuf = [0u8; 4];
        let ns = u8_to_decimal(k, &mut nbuf);
        let pn = self.pname.as_str();
        let parts: [&str; 5] = [pn, "_c", cs, "_neg", ns];
        build_label(&parts)
    }

    fn get(&self, slot: VarSlot) -> VarRole {
        self.roles[slot as usize]
    }

    fn mark_seen(&mut self, slot: VarSlot) {
        self.roles[slot as usize] = match self.roles[slot as usize] {
            VarRole::Temp { xi, .. } => VarRole::Temp { xi, seen: true },
            VarRole::Perm { yi, .. } => VarRole::Perm { yi, seen: true },
            VarRole::Unused => VarRole::Unused,
        };
    }

    fn scratch_x(&self) -> u8 {
        self.n_temp
    }

    fn has_env(&self) -> bool {
        self.n_perm > 0
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
    let classes = classify_clause(clause, atoms)?;
    let mut rm = assign_regs(&classes, clause, atoms)?;
    rm.pname = *pname;
    rm.clause_idx = clause_idx;
    if rm.has_env() {
        push_i(out, Instr::Allocate { n: rm.n_perm })?;
    }
    emit_head(&clause.head, clause, atoms, &mut rm, out)?;
    match clause.kind {
        ClauseKind::Fact => push_i(out, Instr::Proceed),
        ClauseKind::Rule => emit_body(clause, atoms, &mut rm, out),
        ClauseKind::Query => Err(CompileError::BadHead),
    }
}

fn classify_clause(
    clause: &Clause,
    atoms: &AtomTable,
) -> Result<[Class; MAX_CLAUSE_VARS], CompileError> {
    let n_goals = clause.body.len();
    let mut goal_chunk = [0usize; MAX_BODY];
    let mut chunk_ix = 0usize;
    for i in 0..n_goals {
        goal_chunk[i] = chunk_ix;
        let is_last = i + 1 == n_goals;
        let g = *clause.body.get(i).expect("goal");
        if !is_inline_builtin(&g, atoms) && !is_last {
            chunk_ix += 1;
        }
    }
    let n_chunks = chunk_ix + 1;
    let mut hits = [[false; MAX_BODY]; MAX_CLAUSE_VARS];
    mark_vars(&clause.head, clause, &mut hits, 0)?;
    for i in 0..n_goals {
        let g = *clause.body.get(i).expect("goal");
        mark_vars(&g, clause, &mut hits, goal_chunk[i])?;
    }
    let mut classes = [Class::Unused; MAX_CLAUSE_VARS];
    for slot in 0..MAX_CLAUSE_VARS {
        let count = (0..n_chunks).filter(|&c| hits[slot][c]).count();
        classes[slot] = match count {
            0 => Class::Unused,
            1 => Class::Temp,
            _ => Class::Perm,
        };
    }
    Ok(classes)
}

fn is_inline_builtin(goal: &Term, atoms: &AtomTable) -> bool {
    match goal {
        Term::Atom(id) => match atoms.name(*id).map(|n| n.as_str()) {
            Some("nl") | Some("fail") | Some("!") => true,
            _ => false,
        },
        Term::Struct { functor, args } => match atoms.name(*functor).map(|n| n.as_str()) {
            Some("write") | Some("\\+") => args.len() == 1,
            Some("is") | Some("<") | Some(">") | Some("=") => args.len() == 2,
            _ => false,
        },
        _ => false,
    }
}

fn mark_vars(
    t: &Term,
    clause: &Clause,
    hits: &mut [[bool; MAX_BODY]; MAX_CLAUSE_VARS],
    chunk: usize,
) -> Result<(), CompileError> {
    match t {
        Term::Var(s) => {
            if (*s as usize) >= MAX_CLAUSE_VARS {
                return Err(CompileError::TooManyVars);
            }
            hits[*s as usize][chunk] = true;
            Ok(())
        }
        Term::Struct { args, .. } => {
            for i in 0..args.len() {
                let ti = *args.get(i).expect("arg in range");
                let sub = *clause.subterm(ti).ok_or(CompileError::BadSubterm)?;
                mark_vars(&sub, clause, hits, chunk)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn assign_regs(
    classes: &[Class; MAX_CLAUSE_VARS],
    clause: &Clause,
    _atoms: &AtomTable,
) -> Result<RegMap, CompileError> {
    let mut rm = RegMap::empty();
    walk_assign(&clause.head, clause, classes, &mut rm)?;
    for i in 0..clause.body.len() {
        let g = *clause.body.get(i).expect("goal in range");
        walk_assign(&g, clause, classes, &mut rm)?;
    }
    Ok(rm)
}

fn walk_assign(
    t: &Term,
    clause: &Clause,
    classes: &[Class; MAX_CLAUSE_VARS],
    rm: &mut RegMap,
) -> Result<(), CompileError> {
    match t {
        Term::Var(s) => assign_slot(*s, classes, rm),
        Term::Struct { args, .. } => {
            for i in 0..args.len() {
                let ti = *args.get(i).expect("arg in range");
                let sub = *clause.subterm(ti).ok_or(CompileError::BadSubterm)?;
                walk_assign(&sub, clause, classes, rm)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn assign_slot(
    slot: VarSlot,
    classes: &[Class; MAX_CLAUSE_VARS],
    rm: &mut RegMap,
) -> Result<(), CompileError> {
    let ix = slot as usize;
    if !matches!(rm.roles[ix], VarRole::Unused) {
        return Ok(());
    }
    match classes[ix] {
        Class::Unused => Ok(()),
        Class::Temp => {
            if rm.n_temp >= REG_CAP {
                return Err(CompileError::TooManyXRegs);
            }
            rm.roles[ix] = VarRole::Temp { xi: rm.n_temp, seen: false };
            rm.n_temp += 1;
            Ok(())
        }
        Class::Perm => {
            if rm.n_perm >= REG_CAP {
                return Err(CompileError::TooManyYRegs);
            }
            rm.roles[ix] = VarRole::Perm { yi: rm.n_perm, seen: false };
            rm.n_perm += 1;
            Ok(())
        }
    }
}

fn emit_head(
    head: &Term,
    clause: &Clause,
    atoms: &AtomTable,
    rm: &mut RegMap,
    out: &mut BoundedArr<Instr, MAX_INSTR>,
) -> Result<(), CompileError> {
    match head {
        Term::Atom(_) => Ok(()),
        Term::Struct { args, .. } => emit_head_args(args, clause, atoms, rm, out),
        _ => Err(CompileError::BadHead),
    }
}

fn emit_head_args(
    args: &BoundedArr<TermIdx, MAX_ARGS>,
    clause: &Clause,
    atoms: &AtomTable,
    rm: &mut RegMap,
    out: &mut BoundedArr<Instr, MAX_INSTR>,
) -> Result<(), CompileError> {
    for i in 0..args.len() {
        let ti = *args.get(i).expect("arg in range");
        let t = *clause.subterm(ti).ok_or(CompileError::BadSubterm)?;
        emit_head_arg(i as u8, &t, clause, atoms, rm, out)?;
    }
    Ok(())
}

fn emit_head_arg(
    ai: u8,
    arg: &Term,
    clause: &Clause,
    atoms: &AtomTable,
    rm: &mut RegMap,
    out: &mut BoundedArr<Instr, MAX_INSTR>,
) -> Result<(), CompileError> {
    match arg {
        Term::Atom(id) => push_i(out, Instr::GetConst { ai, atom: *id }),
        Term::Nil => {
            let nil = atoms.find("[]").ok_or(CompileError::UnknownAtom)?;
            push_i(out, Instr::GetConst { ai, atom: nil })
        }
        Term::Var(slot) => emit_head_var(ai, *slot, rm, out),
        Term::Int(n) => push_i(out, Instr::GetInt { ai, value: *n }),
        Term::Struct { functor, args } => {
            emit_head_struct(ai, *functor, args, clause, atoms, rm, out)
        }
    }
}

fn emit_head_struct(
    ai: u8,
    functor: AtomId,
    args: &BoundedArr<TermIdx, MAX_ARGS>,
    clause: &Clause,
    atoms: &AtomTable,
    rm: &mut RegMap,
    out: &mut BoundedArr<Instr, MAX_INSTR>,
) -> Result<(), CompileError> {
    push_i(
        out,
        Instr::GetStruct { ai, atom: functor, arity: args.len() as u8 },
    )?;
    for i in 0..args.len() {
        let ti = *args.get(i).expect("arg in range");
        let sub = *clause.subterm(ti).ok_or(CompileError::BadSubterm)?;
        emit_unify_term(&sub, clause, atoms, rm, out)?;
    }
    Ok(())
}

fn emit_unify_term(
    term: &Term,
    _clause: &Clause,
    atoms: &AtomTable,
    rm: &mut RegMap,
    out: &mut BoundedArr<Instr, MAX_INSTR>,
) -> Result<(), CompileError> {
    match term {
        Term::Atom(_) => Err(CompileError::StructArg),
        Term::Nil => {
            // Tail of cons that ends the list — handled by caller for
            // lists. Standalone nil in a unify stream position needs
            // UNIFY_CONST but we don't emit Instr::UnifyConst yet.
            let _ = atoms;
            Err(CompileError::StructArg)
        }
        Term::Var(slot) => emit_unify_var(*slot, rm, out),
        Term::Int(_) => Err(CompileError::IntArg),
        Term::Struct { .. } => Err(CompileError::StructArg),
    }
}

fn emit_unify_var(
    slot: VarSlot,
    rm: &mut RegMap,
    out: &mut BoundedArr<Instr, MAX_INSTR>,
) -> Result<(), CompileError> {
    match rm.get(slot) {
        VarRole::Unused => Err(CompileError::TooManyVars),
        VarRole::Temp { xi, seen: false } => {
            push_i(out, Instr::UnifyVar { xi })?;
            rm.mark_seen(slot);
            Ok(())
        }
        VarRole::Temp { xi, seen: true } => push_i(out, Instr::UnifyVal { xi }),
        VarRole::Perm { .. } => Err(CompileError::StructArg),
    }
}

fn emit_head_var(
    ai: u8,
    slot: VarSlot,
    rm: &mut RegMap,
    out: &mut BoundedArr<Instr, MAX_INSTR>,
) -> Result<(), CompileError> {
    match rm.get(slot) {
        VarRole::Unused => Err(CompileError::TooManyVars),
        VarRole::Temp { xi, seen: false } => {
            push_i(out, Instr::GetVar { ai, xi })?;
            rm.mark_seen(slot);
            Ok(())
        }
        VarRole::Temp { xi, seen: true } => push_i(out, Instr::GetVal { ai, xi }),
        VarRole::Perm { yi, seen: false } => {
            push_i(out, Instr::GetYVar { ai, yi })?;
            rm.mark_seen(slot);
            Ok(())
        }
        VarRole::Perm { .. } => Err(CompileError::HeadVarRepeat),
    }
}

fn emit_body(
    clause: &Clause,
    atoms: &AtomTable,
    rm: &mut RegMap,
    out: &mut BoundedArr<Instr, MAX_INSTR>,
) -> Result<(), CompileError> {
    let n = clause.body.len();
    if n == 0 {
        return Err(CompileError::EmptyBody);
    }
    for gi in 0..n {
        let goal = *clause.body.get(gi).expect("goal in range");
        let is_last = gi + 1 == n;
        emit_goal(&goal, clause, atoms, rm, is_last, out)?;
    }
    Ok(())
}

fn emit_goal(
    goal: &Term,
    clause: &Clause,
    atoms: &AtomTable,
    rm: &mut RegMap,
    is_last: bool,
    out: &mut BoundedArr<Instr, MAX_INSTR>,
) -> Result<(), CompileError> {
    if is_inline_builtin(goal, atoms) {
        emit_inline_builtin(goal, clause, atoms, rm, out)?;
        if is_last {
            if rm.has_env() {
                push_i(out, Instr::Deallocate)?;
            }
            return push_i(out, Instr::Proceed);
        }
        return Ok(());
    }
    let fid = match goal {
        Term::Atom(id) => *id,
        Term::Struct { functor, args } => {
            emit_put_args(args, clause, atoms, rm, out)?;
            *functor
        }
        _ => return Err(CompileError::BadGoal),
    };
    let pname = *atoms.name(fid).ok_or(CompileError::UnknownAtom)?;
    let entry = lbl_entry(&pname)?;
    if is_last {
        if rm.has_env() {
            push_i(out, Instr::Deallocate)?;
        }
        push_i(out, Instr::Execute { label: entry })
    } else {
        push_i(out, Instr::Call { label: entry })
    }
}

fn emit_inline_builtin(
    goal: &Term,
    clause: &Clause,
    atoms: &AtomTable,
    rm: &mut RegMap,
    out: &mut BoundedArr<Instr, MAX_INSTR>,
) -> Result<(), CompileError> {
    match goal {
        Term::Atom(id) => {
            let name = atoms.name(*id).ok_or(CompileError::UnknownAtom)?.as_str();
            match name {
                "nl" => push_i(out, Instr::BNl),
                "fail" => push_i(out, Instr::Fail),
                "!" => push_i(out, Instr::Cut),
                _ => Err(CompileError::BadGoal),
            }
        }
        Term::Struct { functor, args } => {
            let name = atoms.name(*functor).ok_or(CompileError::UnknownAtom)?.as_str();
            match (name, args.len()) {
                ("write", 1) => {
                    let ti = *args.get(0).expect("one arg");
                    let a = *clause.subterm(ti).ok_or(CompileError::BadSubterm)?;
                    emit_put_arg(0, &a, clause, atoms, rm, out)?;
                    push_i(out, Instr::BWrite { ai: 0 })
                }
                ("is", 2) => emit_is(args, clause, atoms, rm, out),
                ("<", 2) => emit_cmp(args, clause, atoms, rm, out, true),
                (">", 2) => emit_cmp(args, clause, atoms, rm, out, false),
                ("=", 2) => emit_eq(args, clause, atoms, rm, out),
                ("\\+", 1) => emit_negation(args, clause, atoms, rm, out),
                _ => Err(CompileError::BadGoal),
            }
        }
        _ => Err(CompileError::BadGoal),
    }
}

fn emit_is(
    args: &BoundedArr<TermIdx, MAX_ARGS>,
    clause: &Clause,
    atoms: &AtomTable,
    rm: &mut RegMap,
    out: &mut BoundedArr<Instr, MAX_INSTR>,
) -> Result<(), CompileError> {
    let lhs_ti = *args.get(0).expect("lhs");
    let rhs_ti = *args.get(1).expect("rhs");
    let lhs = *clause.subterm(lhs_ti).ok_or(CompileError::BadSubterm)?;
    let rhs = *clause.subterm(rhs_ti).ok_or(CompileError::BadSubterm)?;
    emit_eval_expr(&rhs, clause, atoms, rm, out)?;
    emit_bind_a0(&lhs, rm, out)
}

fn emit_eval_expr(
    expr: &Term,
    clause: &Clause,
    atoms: &AtomTable,
    rm: &mut RegMap,
    out: &mut BoundedArr<Instr, MAX_INSTR>,
) -> Result<(), CompileError> {
    match expr {
        Term::Int(n) => push_i(out, Instr::PutInt { ai: 0, value: *n }),
        Term::Var(slot) => emit_put_var(0, *slot, rm, out),
        Term::Struct { functor, args } if args.len() == 2 => {
            let name = atoms.name(*functor).ok_or(CompileError::UnknownAtom)?.as_str();
            let op = match name {
                "+" => Instr::BIsAdd { dst: 0, a: 1, b: 2 },
                "-" => Instr::BIsSub { dst: 0, a: 1, b: 2 },
                _ => return Err(CompileError::BadGoal),
            };
            let a_ti = *args.get(0).expect("lhs");
            let b_ti = *args.get(1).expect("rhs");
            let a = *clause.subterm(a_ti).ok_or(CompileError::BadSubterm)?;
            let b = *clause.subterm(b_ti).ok_or(CompileError::BadSubterm)?;
            emit_eval_leaf(1, &a, rm, out)?;
            emit_eval_leaf(2, &b, rm, out)?;
            push_i(out, op)
        }
        _ => Err(CompileError::BadGoal),
    }
}

fn emit_eval_leaf(
    ai: u8,
    term: &Term,
    rm: &mut RegMap,
    out: &mut BoundedArr<Instr, MAX_INSTR>,
) -> Result<(), CompileError> {
    match term {
        Term::Int(n) => push_i(out, Instr::PutInt { ai, value: *n }),
        Term::Var(slot) => emit_put_var(ai, *slot, rm, out),
        _ => Err(CompileError::BadGoal),
    }
}

fn emit_bind_a0(
    lhs: &Term,
    rm: &mut RegMap,
    out: &mut BoundedArr<Instr, MAX_INSTR>,
) -> Result<(), CompileError> {
    match lhs {
        Term::Int(n) => push_i(out, Instr::GetInt { ai: 0, value: *n }),
        Term::Var(slot) => match rm.get(*slot) {
            VarRole::Temp { xi, seen: false } => {
                push_i(out, Instr::GetVar { ai: 0, xi })?;
                rm.mark_seen(*slot);
                Ok(())
            }
            VarRole::Temp { xi, seen: true } => push_i(out, Instr::GetVal { ai: 0, xi }),
            VarRole::Perm { yi, seen: false } => {
                push_i(out, Instr::GetYVar { ai: 0, yi })?;
                rm.mark_seen(*slot);
                Ok(())
            }
            VarRole::Perm { .. } => Err(CompileError::HeadVarRepeat),
            VarRole::Unused => Err(CompileError::TooManyVars),
        },
        _ => Err(CompileError::BadGoal),
    }
}

fn emit_eq(
    args: &BoundedArr<TermIdx, MAX_ARGS>,
    clause: &Clause,
    atoms: &AtomTable,
    rm: &mut RegMap,
    out: &mut BoundedArr<Instr, MAX_INSTR>,
) -> Result<(), CompileError> {
    let l_ti = *args.get(0).expect("lhs");
    let r_ti = *args.get(1).expect("rhs");
    let l = *clause.subterm(l_ti).ok_or(CompileError::BadSubterm)?;
    let r = *clause.subterm(r_ti).ok_or(CompileError::BadSubterm)?;
    emit_put_arg(0, &l, clause, atoms, rm, out)?;
    let scratch = rm.scratch_x();
    if scratch >= REG_CAP {
        return Err(CompileError::TooManyXRegs);
    }
    push_i(out, Instr::GetVar { ai: 0, xi: scratch })?;
    emit_put_arg(1, &r, clause, atoms, rm, out)?;
    push_i(out, Instr::GetVal { ai: 1, xi: scratch })
}

fn emit_negation(
    args: &BoundedArr<TermIdx, MAX_ARGS>,
    clause: &Clause,
    atoms: &AtomTable,
    rm: &mut RegMap,
    out: &mut BoundedArr<Instr, MAX_INSTR>,
) -> Result<(), CompileError> {
    let alt_label = rm.fresh_neg_label()?;
    let g_ti = *args.get(0).expect("neg arg");
    let g = *clause.subterm(g_ti).ok_or(CompileError::BadSubterm)?;
    push_i(out, Instr::Try { label: alt_label })?;
    emit_neg_inner(&g, clause, atoms, rm, out)?;
    push_i(out, Instr::Trust { label: alt_label })?;
    push_i(out, Instr::Fail)?;
    push_i(out, Instr::Label(alt_label))?;
    push_i(out, Instr::Trust { label: alt_label })
}

fn emit_neg_inner(
    goal: &Term,
    clause: &Clause,
    atoms: &AtomTable,
    rm: &mut RegMap,
    out: &mut BoundedArr<Instr, MAX_INSTR>,
) -> Result<(), CompileError> {
    if is_inline_builtin(goal, atoms) {
        return emit_inline_builtin(goal, clause, atoms, rm, out);
    }
    let fid = match goal {
        Term::Atom(id) => *id,
        Term::Struct { functor, args } => {
            emit_put_args(args, clause, atoms, rm, out)?;
            *functor
        }
        _ => return Err(CompileError::BadGoal),
    };
    let pname = *atoms.name(fid).ok_or(CompileError::UnknownAtom)?;
    push_i(out, Instr::Call { label: lbl_entry(&pname)? })
}

fn emit_cmp(
    args: &BoundedArr<TermIdx, MAX_ARGS>,
    clause: &Clause,
    atoms: &AtomTable,
    rm: &mut RegMap,
    out: &mut BoundedArr<Instr, MAX_INSTR>,
    is_lt: bool,
) -> Result<(), CompileError> {
    let _ = atoms;
    let a_ti = *args.get(0).expect("lhs");
    let b_ti = *args.get(1).expect("rhs");
    let a = *clause.subterm(a_ti).ok_or(CompileError::BadSubterm)?;
    let b = *clause.subterm(b_ti).ok_or(CompileError::BadSubterm)?;
    emit_eval_leaf(0, &a, rm, out)?;
    emit_eval_leaf(1, &b, rm, out)?;
    let op = if is_lt {
        Instr::BLt { a: 0, b: 1 }
    } else {
        Instr::BGt { a: 0, b: 1 }
    };
    push_i(out, op)
}

fn emit_put_args(
    args: &BoundedArr<TermIdx, MAX_ARGS>,
    clause: &Clause,
    atoms: &AtomTable,
    rm: &mut RegMap,
    out: &mut BoundedArr<Instr, MAX_INSTR>,
) -> Result<(), CompileError> {
    for i in 0..args.len() {
        let ti = *args.get(i).expect("arg in range");
        let t = *clause.subterm(ti).ok_or(CompileError::BadSubterm)?;
        emit_put_arg(i as u8, &t, clause, atoms, rm, out)?;
    }
    Ok(())
}

fn emit_put_arg(
    ai: u8,
    arg: &Term,
    clause: &Clause,
    atoms: &AtomTable,
    rm: &mut RegMap,
    out: &mut BoundedArr<Instr, MAX_INSTR>,
) -> Result<(), CompileError> {
    match arg {
        Term::Atom(id) => push_i(out, Instr::PutConst { ai, atom: *id }),
        Term::Nil => {
            let nil = atoms.find("[]").ok_or(CompileError::UnknownAtom)?;
            push_i(out, Instr::PutConst { ai, atom: nil })
        }
        Term::Var(slot) => emit_put_var(ai, *slot, rm, out),
        Term::Int(n) => push_i(out, Instr::PutInt { ai, value: *n }),
        Term::Struct { functor, args } => {
            emit_build_list(ai, *functor, args, clause, atoms, rm, out)
        }
    }
}

fn emit_build_list(
    ai: u8,
    functor: AtomId,
    args: &BoundedArr<TermIdx, MAX_ARGS>,
    clause: &Clause,
    atoms: &AtomTable,
    rm: &mut RegMap,
    out: &mut BoundedArr<Instr, MAX_INSTR>,
) -> Result<(), CompileError> {
    let dot_id = atoms.find(".").ok_or(CompileError::UnknownAtom)?;
    let nil_id = atoms.find("[]").ok_or(CompileError::UnknownAtom)?;
    if functor != dot_id || args.len() != 2 {
        return Err(CompileError::StructArg);
    }
    let head_x = rm.scratch_x();
    let cursor_x = head_x + 1;
    if cursor_x >= REG_CAP {
        return Err(CompileError::TooManyXRegs);
    }
    push_i(out, Instr::PutVar { ai, xi: head_x })?;
    emit_list_spine(ai, args, clause, atoms, rm, cursor_x, dot_id, nil_id, out)?;
    push_i(out, Instr::PutVal { ai, xi: head_x })
}

#[allow(clippy::too_many_arguments)]
fn emit_list_spine(
    ai: u8,
    first_args: &BoundedArr<TermIdx, MAX_ARGS>,
    clause: &Clause,
    atoms: &AtomTable,
    rm: &mut RegMap,
    cursor_x: u8,
    dot_id: AtomId,
    nil_id: AtomId,
    out: &mut BoundedArr<Instr, MAX_INSTR>,
) -> Result<(), CompileError> {
    let mut cur_head_ti = *first_args.get(0).expect("cons head");
    let mut cur_tail_ti = *first_args.get(1).expect("cons tail");
    loop {
        push_i(out, Instr::GetStruct { ai, atom: dot_id, arity: 2 })?;
        let head = *clause.subterm(cur_head_ti).ok_or(CompileError::BadSubterm)?;
        let tail = *clause.subterm(cur_tail_ti).ok_or(CompileError::BadSubterm)?;
        emit_list_element(&head, atoms, rm, out)?;
        match tail {
            Term::Nil => {
                push_i(out, Instr::UnifyConst { atom: nil_id })?;
                return Ok(());
            }
            Term::Struct { functor, args: next } if functor == dot_id && next.len() == 2 => {
                push_i(out, Instr::UnifyVar { xi: cursor_x })?;
                push_i(out, Instr::PutVal { ai, xi: cursor_x })?;
                cur_head_ti = *next.get(0).expect("cons head");
                cur_tail_ti = *next.get(1).expect("cons tail");
            }
            _ => return Err(CompileError::StructArg),
        }
    }
}

fn emit_list_element(
    elem: &Term,
    atoms: &AtomTable,
    rm: &mut RegMap,
    out: &mut BoundedArr<Instr, MAX_INSTR>,
) -> Result<(), CompileError> {
    match elem {
        Term::Atom(id) => push_i(out, Instr::UnifyConst { atom: *id }),
        Term::Nil => {
            let nil = atoms.find("[]").ok_or(CompileError::UnknownAtom)?;
            push_i(out, Instr::UnifyConst { atom: nil })
        }
        Term::Var(slot) => emit_unify_var(*slot, rm, out),
        Term::Int(_) => Err(CompileError::IntArg),
        Term::Struct { .. } => Err(CompileError::StructArg),
    }
}

fn emit_put_var(
    ai: u8,
    slot: VarSlot,
    rm: &mut RegMap,
    out: &mut BoundedArr<Instr, MAX_INSTR>,
) -> Result<(), CompileError> {
    match rm.get(slot) {
        VarRole::Unused => Err(CompileError::TooManyVars),
        VarRole::Temp { xi, seen: false } => {
            push_i(out, Instr::PutVar { ai, xi })?;
            rm.mark_seen(slot);
            Ok(())
        }
        VarRole::Temp { xi, seen: true } => push_i(out, Instr::PutVal { ai, xi }),
        VarRole::Perm { yi, seen: false } => {
            let scratch = rm.scratch_x();
            if scratch >= REG_CAP {
                return Err(CompileError::TooManyXRegs);
            }
            push_i(out, Instr::PutVar { ai, xi: scratch })?;
            push_i(out, Instr::GetYVar { ai, yi })?;
            rm.mark_seen(slot);
            Ok(())
        }
        VarRole::Perm { yi, seen: true } => push_i(out, Instr::PutYVal { ai, yi }),
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
    if clause.body.is_empty() {
        return Err(CompileError::QueryShape);
    }
    let classes = classify_clause(clause, atoms)?;
    let mut rm = assign_regs(&classes, clause, atoms)?;
    rm.pname = BoundedStr::<NAME_CAP>::from_str("q")
        .map_err(|_| CompileError::LabelOverflow)?;
    rm.clause_idx = 0;
    if rm.has_env() {
        push_i(out, Instr::Allocate { n: rm.n_perm })?;
    }
    for gi in 0..clause.body.len() {
        let goal = *clause.body.get(gi).expect("goal in range");
        emit_query_goal(&goal, clause, atoms, &mut rm, out)?;
    }
    if rm.has_env() {
        push_i(out, Instr::Deallocate)?;
    }
    push_i(out, Instr::Halt)
}

fn emit_query_goal(
    goal: &Term,
    clause: &Clause,
    atoms: &AtomTable,
    rm: &mut RegMap,
    out: &mut BoundedArr<Instr, MAX_INSTR>,
) -> Result<(), CompileError> {
    if is_inline_builtin(goal, atoms) {
        return emit_inline_builtin(goal, clause, atoms, rm, out);
    }
    let fid = match goal {
        Term::Atom(id) => *id,
        Term::Struct { functor, args } => {
            emit_put_args(args, clause, atoms, rm, out)?;
            *functor
        }
        _ => return Err(CompileError::BadGoal),
    };
    let pname = *atoms.name(fid).ok_or(CompileError::UnknownAtom)?;
    push_i(out, Instr::Call { label: lbl_entry(&pname)? })
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

    /// Pinned reference for examples/ancestor.pl with the ALLOCATE /
    /// DEALLOCATE env frames that the recursive `ancestor_c2` clause
    /// needs: ancestor_c2_body grows from 9 Instr entries (pre-fix)
    /// to 12 (adds ALLOCATE, a GET_Y_VAR pair for Z, DEALLOCATE),
    /// bumping the total from 46 to 49 Instr entries.
    const ANCESTOR_INSTR_COUNT: usize = 49;

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
        match prog.get(0).expect("first") {
            Instr::AtomDir { id, .. } => assert_eq!(*id, 0),
            other => panic!("expected AtomDir, got {other:?}"),
        }
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
        let bob = atom_id(&atoms, "bob");
        let liz = atom_id(&atoms, "liz");
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
        assert_eq!(qi + 4, prog.len() - 1);
    }

    #[test]
    fn compile_ancestor_c2_body_opens_env_and_emits_y_regs() {
        let src = include_str!("../examples/ancestor.pl");
        let prog = compile_src(src);
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
        //   ALLOCATE 2
        //   GET_VAR X0, A0             ; X (temp)
        //   GET_Y_VAR Y0, A1           ; Y (perm, first-occ in head)
        //   PUT_VAL X0, A0             ; X (X-seen)
        //   PUT_VAR X1, A1             ; scratch for Z
        //   GET_Y_VAR Y1, A1           ; save fresh Z to Y1
        //   CALL parent_entry
        //   PUT_Y_VAL Y1, A0           ; Z
        //   PUT_Y_VAL Y0, A1           ; Y
        //   DEALLOCATE
        //   EXECUTE ancestor_entry
        assert!(matches!(prog.get(ci + 1), Some(Instr::Allocate { n: 2 })));
        assert!(matches!(
            prog.get(ci + 2),
            Some(Instr::GetVar { ai: 0, xi: 0 })
        ));
        assert!(matches!(
            prog.get(ci + 3),
            Some(Instr::GetYVar { ai: 1, yi: 0 })
        ));
        assert!(matches!(
            prog.get(ci + 4),
            Some(Instr::PutVal { ai: 0, xi: 0 })
        ));
        assert!(matches!(
            prog.get(ci + 5),
            Some(Instr::PutVar { ai: 1, xi: 1 })
        ));
        assert!(matches!(
            prog.get(ci + 6),
            Some(Instr::GetYVar { ai: 1, yi: 1 })
        ));
        match prog.get(ci + 7).expect("call parent_entry") {
            Instr::Call { label } => assert_eq!(label.as_str(), "parent_entry"),
            other => panic!("expected Call(parent_entry), got {other:?}"),
        }
        assert!(matches!(
            prog.get(ci + 8),
            Some(Instr::PutYVal { ai: 0, yi: 1 })
        ));
        assert!(matches!(
            prog.get(ci + 9),
            Some(Instr::PutYVal { ai: 1, yi: 0 })
        ));
        assert!(matches!(prog.get(ci + 10), Some(Instr::Deallocate)));
        match prog.get(ci + 11).expect("execute ancestor_entry") {
            Instr::Execute { label } => assert_eq!(label.as_str(), "ancestor_entry"),
            other => panic!("expected Execute(ancestor_entry), got {other:?}"),
        }
    }

    #[test]
    fn compile_ancestor_c1_body_has_no_env_frame() {
        // ancestor_c1 is `ancestor(X, Y) :- parent(X, Y).` — one body
        // goal, so no chunk boundaries, so no permanent vars, so no
        // ALLOCATE / DEALLOCATE. Sequence should be head + body
        // PUT_VAL pair + tail call with no env frame.
        let src = include_str!("../examples/ancestor.pl");
        let prog = compile_src(src);
        let mut ci = None;
        for i in 0..prog.len() {
            if matches!(
                prog.get(i),
                Some(Instr::Label(l)) if l.as_str() == "ancestor_c1_body"
            ) {
                ci = Some(i);
                break;
            }
        }
        let ci = ci.expect("ancestor_c1_body label found");
        assert!(!matches!(prog.get(ci + 1), Some(Instr::Allocate { .. })));
        assert!(matches!(
            prog.get(ci + 1),
            Some(Instr::GetVar { ai: 0, xi: 0 })
        ));
        assert!(matches!(
            prog.get(ci + 2),
            Some(Instr::GetVar { ai: 1, xi: 1 })
        ));
        match prog.get(ci + 5).expect("execute parent_entry") {
            Instr::Execute { label } => assert_eq!(label.as_str(), "parent_entry"),
            other => panic!("expected Execute(parent_entry), got {other:?}"),
        }
    }

    #[test]
    fn compile_single_fact() {
        let prog = compile_src("p(a). ?- p(a).");
        // atom dirs (p, a) + Execute(query)                = 3
        //   + p_c1_body label + GET_CONST a + PROCEED      = 3
        //   + p_entry label + EXECUTE p_c1_body            = 2
        //   + query label + PUT_CONST a + CALL p_entry + HALT = 4
        assert_eq!(prog.len(), 12);
        assert!(matches!(prog.get(0), Some(Instr::AtomDir { id: 0, .. })));
    }

    #[test]
    fn compile_single_clause_predicate_dispatcher_has_no_try() {
        let prog = compile_src("p(a). q(X) :- p(X). ?- q(a).");
        let mut saw_q_entry = false;
        for i in 0..prog.len() {
            if matches!(prog.get(i), Some(Instr::Label(l)) if l.as_str() == "q_entry") {
                saw_q_entry = true;
                match prog.get(i + 1).expect("after q_entry") {
                    Instr::Execute { label } => {
                        assert_eq!(label.as_str(), "q_c1_body");
                    }
                    other => panic!("expected Execute, got {other:?}"),
                }
                assert!(!matches!(prog.get(i + 1), Some(Instr::Try { .. })));
                break;
            }
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
