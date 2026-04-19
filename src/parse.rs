//! Clause parser: turns a token stream into `Clause` structures.
//!
//! Grammar (subset for ancestor.pl, color.pl, liar puzzle):
//!   clause := term ('.' | ':-' body '.') | '?-' body '.'
//!   body   := goal (',' goal)*
//!   goal   := term | '\+' goal | '!'
//!   term   := atom | var | int | atom '(' term (',' term)* ')' | list
//!   list   := '[]' | '[' term (',' term)* ( '|' term )? ']'
//!
//! Lists desugar to Struct('.'/2, [H, T]); Nil is the empty list.
//!
//! Port shape: subterms of a clause live in a flat arena
//! (`BoundedArr<Term, MAX_SUBTERMS>`) addressed by `TermIdx` so that
//! SNOBOL4 can mirror the layout with a single ARRAY per clause.

use crate::port::{BoundedArr, BoundedStr};
use crate::tokenize::{Token, MAX_TOKENS};
use core::mem::discriminant;
use thiserror::Error;

pub type AtomId = u16;
pub type VarSlot = u8;
pub type TermIdx = u8;

pub const MAX_ATOMS: usize = 50;
pub const MAX_CLAUSE_VARS: usize = 16;
pub const MAX_SUBTERMS: usize = 64;
pub const MAX_BODY: usize = 16;
pub const MAX_CLAUSES: usize = 64;
pub const MAX_ARGS: usize = 8;
pub const NAME_CAP: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Term {
    Atom(AtomId),
    Var(VarSlot),
    Int(i32),
    Struct { functor: AtomId, args: BoundedArr<TermIdx, MAX_ARGS> },
    Nil,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClauseKind {
    Fact,
    Rule,
    Query,
}

#[derive(Clone, Copy, Debug)]
pub struct AtomTable {
    names: BoundedArr<BoundedStr<NAME_CAP>, MAX_ATOMS>,
}

impl AtomTable {
    pub const fn new() -> Self {
        Self { names: BoundedArr::new() }
    }

    pub fn intern(&mut self, s: &str) -> Result<AtomId, ParseError> {
        let key = BoundedStr::<NAME_CAP>::from_str(s).map_err(|_| ParseError::AtomOverflow)?;
        for i in 0..self.names.len() {
            if self.names.get(i).map(|n| *n == key).unwrap_or(false) {
                return Ok(i as AtomId);
            }
        }
        let id = self.names.len() as AtomId;
        self.names.push(key).map_err(|_| ParseError::AtomOverflow)?;
        Ok(id)
    }

    pub fn name(&self, id: AtomId) -> Option<&BoundedStr<NAME_CAP>> {
        self.names.get(id as usize)
    }

    pub fn find(&self, s: &str) -> Option<AtomId> {
        for i in 0..self.names.len() {
            let n = self.names.get(i).expect("in range");
            if n.as_str() == s {
                return Some(i as AtomId);
            }
        }
        None
    }

    pub fn len(&self) -> usize {
        self.names.len()
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}

impl Default for AtomTable {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct VarTable {
    names: BoundedArr<BoundedStr<NAME_CAP>, MAX_CLAUSE_VARS>,
}

impl VarTable {
    pub const fn new() -> Self {
        Self { names: BoundedArr::new() }
    }

    pub fn slot(&mut self, s: &str) -> Result<VarSlot, ParseError> {
        let key = BoundedStr::<NAME_CAP>::from_str(s).map_err(|_| ParseError::VarOverflow)?;
        for i in 0..self.names.len() {
            if self.names.get(i).map(|n| *n == key).unwrap_or(false) {
                return Ok(i as VarSlot);
            }
        }
        let id = self.names.len() as VarSlot;
        self.names.push(key).map_err(|_| ParseError::VarOverflow)?;
        Ok(id)
    }

    pub fn name(&self, slot: VarSlot) -> Option<&BoundedStr<NAME_CAP>> {
        self.names.get(slot as usize)
    }

    pub fn len(&self) -> usize {
        self.names.len()
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}

impl Default for VarTable {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Clause {
    pub kind: ClauseKind,
    pub head: Term,
    pub body: BoundedArr<Term, MAX_BODY>,
    pub subterms: BoundedArr<Term, MAX_SUBTERMS>,
    pub vars: VarTable,
}

impl Clause {
    pub const fn new() -> Self {
        Self {
            kind: ClauseKind::Fact,
            head: Term::Nil,
            body: BoundedArr::new(),
            subterms: BoundedArr::new(),
            vars: VarTable::new(),
        }
    }

    pub fn subterm(&self, idx: TermIdx) -> Option<&Term> {
        self.subterms.get(idx as usize)
    }
}

impl Default for Clause {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    #[error("unexpected end of input at token {0}")]
    UnexpectedEof(usize),
    #[error("unexpected token at position {0}")]
    UnexpectedToken(usize),
    #[error("missing '.' at position {0}")]
    MissingDot(usize),
    #[error("mismatched '(' at position {0}")]
    MismatchedParen(usize),
    #[error("mismatched '[' at position {0}")]
    MismatchedBracket(usize),
    #[error("bad operator at position {0}")]
    BadOperator(usize),
    #[error("empty struct argument list at position {0}")]
    EmptyArgs(usize),
    #[error("atom table overflow")]
    AtomOverflow,
    #[error("var table overflow")]
    VarOverflow,
    #[error("subterm arena overflow")]
    SubtermOverflow,
    #[error("too many args for struct")]
    TooManyArgs,
    #[error("too many body goals")]
    TooManyGoals,
    #[error("too many clauses")]
    TooManyClauses,
}

struct Parser<'a> {
    toks: &'a BoundedArr<Token, MAX_TOKENS>,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(toks: &'a BoundedArr<Token, MAX_TOKENS>) -> Self {
        Self { toks, pos: 0 }
    }

    fn peek(&self) -> Option<Token> {
        self.toks.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<Token> {
        let t = self.toks.get(self.pos).copied()?;
        self.pos += 1;
        Some(t)
    }

    fn at_end(&self) -> bool {
        matches!(self.peek(), None | Some(Token::Eof))
    }
}

pub fn parse(
    toks: &BoundedArr<Token, MAX_TOKENS>,
    atoms: &mut AtomTable,
) -> Result<BoundedArr<Clause, MAX_CLAUSES>, ParseError> {
    let mut p = Parser::new(toks);
    let mut clauses: BoundedArr<Clause, MAX_CLAUSES> = BoundedArr::new();
    while !p.at_end() {
        let clause = parse_clause(&mut p, atoms)?;
        clauses.push(clause).map_err(|_| ParseError::TooManyClauses)?;
    }
    Ok(clauses)
}

fn parse_clause(p: &mut Parser, atoms: &mut AtomTable) -> Result<Clause, ParseError> {
    let mut clause = Clause::new();
    if matches!(p.peek(), Some(Token::Query)) {
        p.bump();
        clause.kind = ClauseKind::Query;
        clause.head = Term::Nil;
        parse_body(p, atoms, &mut clause)?;
        expect_kind(p, &Token::Dot, ParseError::MissingDot(p.pos))?;
        return Ok(clause);
    }
    clause.head = parse_term(p, atoms, &mut clause.subterms, &mut clause.vars)?;
    match p.peek() {
        Some(Token::Dot) => {
            p.bump();
            clause.kind = ClauseKind::Fact;
        }
        Some(Token::Neck) => {
            p.bump();
            clause.kind = ClauseKind::Rule;
            parse_body(p, atoms, &mut clause)?;
            expect_kind(p, &Token::Dot, ParseError::MissingDot(p.pos))?;
        }
        Some(_) => return Err(ParseError::MissingDot(p.pos)),
        None => return Err(ParseError::UnexpectedEof(p.pos)),
    }
    Ok(clause)
}

fn parse_body(p: &mut Parser, atoms: &mut AtomTable, clause: &mut Clause) -> Result<(), ParseError> {
    loop {
        let goal = parse_goal_expr(p, atoms, &mut clause.subterms, &mut clause.vars)?;
        clause.body.push(goal).map_err(|_| ParseError::TooManyGoals)?;
        if matches!(p.peek(), Some(Token::Comma)) {
            p.bump();
            continue;
        }
        break;
    }
    Ok(())
}

fn parse_goal_expr(
    p: &mut Parser,
    atoms: &mut AtomTable,
    subs: &mut BoundedArr<Term, MAX_SUBTERMS>,
    vars: &mut VarTable,
) -> Result<Term, ParseError> {
    let left = parse_cmp_expr(p, atoms, subs, vars)?;
    if is_infix_atom(p, "is") {
        p.bump();
        let right = parse_add_expr(p, atoms, subs, vars)?;
        return build_infix(atoms, subs, "is", left, right);
    }
    Ok(left)
}

fn parse_cmp_expr(
    p: &mut Parser,
    atoms: &mut AtomTable,
    subs: &mut BoundedArr<Term, MAX_SUBTERMS>,
    vars: &mut VarTable,
) -> Result<Term, ParseError> {
    let left = parse_add_expr(p, atoms, subs, vars)?;
    match p.peek() {
        Some(Token::Lt) => {
            p.bump();
            let right = parse_add_expr(p, atoms, subs, vars)?;
            build_infix(atoms, subs, "<", left, right)
        }
        Some(Token::Gt) => {
            p.bump();
            let right = parse_add_expr(p, atoms, subs, vars)?;
            build_infix(atoms, subs, ">", left, right)
        }
        Some(Token::Eq) => {
            p.bump();
            let right = parse_add_expr(p, atoms, subs, vars)?;
            build_infix(atoms, subs, "=", left, right)
        }
        _ => Ok(left),
    }
}

fn parse_add_expr(
    p: &mut Parser,
    atoms: &mut AtomTable,
    subs: &mut BoundedArr<Term, MAX_SUBTERMS>,
    vars: &mut VarTable,
) -> Result<Term, ParseError> {
    let mut left = parse_goal(p, atoms, subs, vars)?;
    loop {
        let op = match p.peek() {
            Some(Token::Plus) => "+",
            Some(Token::Minus) => "-",
            _ => return Ok(left),
        };
        p.bump();
        let right = parse_goal(p, atoms, subs, vars)?;
        left = build_infix(atoms, subs, op, left, right)?;
    }
}

fn is_infix_atom(p: &Parser, name: &str) -> bool {
    match p.peek() {
        Some(Token::Atom(s)) => s.as_str() == name,
        _ => false,
    }
}

fn build_infix(
    atoms: &mut AtomTable,
    subs: &mut BoundedArr<Term, MAX_SUBTERMS>,
    op: &str,
    left: Term,
    right: Term,
) -> Result<Term, ParseError> {
    let id = atoms.intern(op)?;
    let l_idx = push_sub(subs, left)?;
    let r_idx = push_sub(subs, right)?;
    let mut args: BoundedArr<TermIdx, MAX_ARGS> = BoundedArr::new();
    args.push(l_idx).map_err(|_| ParseError::TooManyArgs)?;
    args.push(r_idx).map_err(|_| ParseError::TooManyArgs)?;
    Ok(Term::Struct { functor: id, args })
}

fn parse_goal(
    p: &mut Parser,
    atoms: &mut AtomTable,
    subs: &mut BoundedArr<Term, MAX_SUBTERMS>,
    vars: &mut VarTable,
) -> Result<Term, ParseError> {
    match p.peek() {
        Some(Token::Cut) => {
            p.bump();
            let id = atoms.intern("!")?;
            Ok(Term::Atom(id))
        }
        Some(Token::Not) => {
            p.bump();
            let inner = parse_cmp_expr(p, atoms, subs, vars)?;
            let id = atoms.intern("\\+")?;
            let idx = push_sub(subs, inner)?;
            let mut args: BoundedArr<TermIdx, MAX_ARGS> = BoundedArr::new();
            args.push(idx).map_err(|_| ParseError::TooManyArgs)?;
            Ok(Term::Struct { functor: id, args })
        }
        Some(_) => parse_term(p, atoms, subs, vars),
        None => Err(ParseError::UnexpectedEof(p.pos)),
    }
}

fn parse_term(
    p: &mut Parser,
    atoms: &mut AtomTable,
    subs: &mut BoundedArr<Term, MAX_SUBTERMS>,
    vars: &mut VarTable,
) -> Result<Term, ParseError> {
    let start = p.pos;
    let tok = p.bump().ok_or(ParseError::UnexpectedEof(start))?;
    match tok {
        Token::Atom(name) => parse_atom_or_struct(p, atoms, subs, vars, &name),
        Token::Var(name) => {
            let slot = vars.slot(name.as_str())?;
            Ok(Term::Var(slot))
        }
        Token::Int(n) => Ok(Term::Int(n)),
        Token::LBracket => parse_list(p, atoms, subs, vars),
        Token::Cut | Token::Not => Err(ParseError::BadOperator(start)),
        _ => Err(ParseError::UnexpectedToken(start)),
    }
}

fn parse_atom_or_struct(
    p: &mut Parser,
    atoms: &mut AtomTable,
    subs: &mut BoundedArr<Term, MAX_SUBTERMS>,
    vars: &mut VarTable,
    name: &BoundedStr<NAME_CAP>,
) -> Result<Term, ParseError> {
    if !matches!(p.peek(), Some(Token::LParen)) {
        let id = atoms.intern(name.as_str())?;
        return Ok(Term::Atom(id));
    }
    p.bump();
    if matches!(p.peek(), Some(Token::RParen)) {
        return Err(ParseError::EmptyArgs(p.pos));
    }
    let id = atoms.intern(name.as_str())?;
    let mut args: BoundedArr<TermIdx, MAX_ARGS> = BoundedArr::new();
    loop {
        let arg = parse_term(p, atoms, subs, vars)?;
        let idx = push_sub(subs, arg)?;
        args.push(idx).map_err(|_| ParseError::TooManyArgs)?;
        match p.peek() {
            Some(Token::Comma) => {
                p.bump();
                continue;
            }
            Some(Token::RParen) => {
                p.bump();
                break;
            }
            Some(_) => return Err(ParseError::MismatchedParen(p.pos)),
            None => return Err(ParseError::UnexpectedEof(p.pos)),
        }
    }
    Ok(Term::Struct { functor: id, args })
}

fn parse_list(
    p: &mut Parser,
    atoms: &mut AtomTable,
    subs: &mut BoundedArr<Term, MAX_SUBTERMS>,
    vars: &mut VarTable,
) -> Result<Term, ParseError> {
    let _ = atoms.intern("[]")?;
    if matches!(p.peek(), Some(Token::RBracket)) {
        p.bump();
        return Ok(Term::Nil);
    }
    let mut elements: BoundedArr<Term, MAX_SUBTERMS> = BoundedArr::new();
    loop {
        let e = parse_term(p, atoms, subs, vars)?;
        elements.push(e).map_err(|_| ParseError::SubtermOverflow)?;
        if matches!(p.peek(), Some(Token::Comma)) {
            p.bump();
            continue;
        }
        break;
    }
    let tail = if matches!(p.peek(), Some(Token::Pipe)) {
        p.bump();
        parse_term(p, atoms, subs, vars)?
    } else {
        Term::Nil
    };
    expect_kind(p, &Token::RBracket, ParseError::MismatchedBracket(p.pos))?;
    let dot_id = atoms.intern(".")?;
    let mut current = tail;
    for i in (0..elements.len()).rev() {
        let h = *elements.get(i).expect("list fold: element index in range");
        let h_idx = push_sub(subs, h)?;
        let t_idx = push_sub(subs, current)?;
        let mut args: BoundedArr<TermIdx, MAX_ARGS> = BoundedArr::new();
        args.push(h_idx).map_err(|_| ParseError::TooManyArgs)?;
        args.push(t_idx).map_err(|_| ParseError::TooManyArgs)?;
        current = Term::Struct { functor: dot_id, args };
    }
    Ok(current)
}

fn push_sub(
    subs: &mut BoundedArr<Term, MAX_SUBTERMS>,
    t: Term,
) -> Result<TermIdx, ParseError> {
    let idx = subs.len();
    if idx >= u8::MAX as usize {
        return Err(ParseError::SubtermOverflow);
    }
    subs.push(t).map_err(|_| ParseError::SubtermOverflow)?;
    Ok(idx as TermIdx)
}

fn expect_kind(p: &mut Parser, expected: &Token, err: ParseError) -> Result<(), ParseError> {
    match p.peek() {
        Some(t) if discriminant(&t) == discriminant(expected) => {
            p.bump();
            Ok(())
        }
        Some(_) => Err(err),
        None => Err(ParseError::UnexpectedEof(p.pos)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tstream(arr: &[Token]) -> BoundedArr<Token, MAX_TOKENS> {
        let mut out: BoundedArr<Token, MAX_TOKENS> = BoundedArr::new();
        for t in arr {
            out.push(*t).expect("test stream fits");
        }
        out.push(Token::Eof).expect("eof fits");
        out
    }

    fn a(s: &str) -> Token {
        Token::Atom(BoundedStr::<NAME_CAP>::from_str(s).expect("atom name fits"))
    }

    fn v(s: &str) -> Token {
        Token::Var(BoundedStr::<NAME_CAP>::from_str(s).expect("var name fits"))
    }

    fn name_of(atoms: &AtomTable, id: AtomId) -> &str {
        atoms.name(id).expect("atom id in range").as_str()
    }

    #[test]
    fn parse_single_fact() {
        // parent(bob, ann).
        let toks = tstream(&[
            a("parent"), Token::LParen, a("bob"), Token::Comma, a("ann"),
            Token::RParen, Token::Dot,
        ]);
        let mut atoms = AtomTable::new();
        let clauses = parse(&toks, &mut atoms).expect("parse ok");
        assert_eq!(clauses.len(), 1);
        let c = clauses.get(0).unwrap();
        assert_eq!(c.kind, ClauseKind::Fact);
        let (name, arity) = match c.head {
            Term::Struct { functor, args } => (name_of(&atoms, functor), args.len()),
            _ => panic!("head is not struct"),
        };
        assert_eq!(name, "parent");
        assert_eq!(arity, 2);
        assert!(c.body.is_empty());
    }

    #[test]
    fn parse_rule_with_single_body_goal() {
        // ancestor(X, Y) :- parent(X, Y).
        let toks = tstream(&[
            a("ancestor"), Token::LParen, v("X"), Token::Comma, v("Y"), Token::RParen,
            Token::Neck,
            a("parent"), Token::LParen, v("X"), Token::Comma, v("Y"), Token::RParen,
            Token::Dot,
        ]);
        let mut atoms = AtomTable::new();
        let clauses = parse(&toks, &mut atoms).expect("parse ok");
        assert_eq!(clauses.len(), 1);
        let c = clauses.get(0).unwrap();
        assert_eq!(c.kind, ClauseKind::Rule);
        assert_eq!(c.body.len(), 1);
        match c.head {
            Term::Struct { functor, args } => {
                assert_eq!(name_of(&atoms, functor), "ancestor");
                assert_eq!(args.len(), 2);
            }
            _ => panic!("head shape wrong"),
        }
        match c.body.get(0).copied().unwrap() {
            Term::Struct { functor, args } => {
                assert_eq!(name_of(&atoms, functor), "parent");
                assert_eq!(args.len(), 2);
            }
            _ => panic!("body[0] shape wrong"),
        }
        // Var table shared between head+body: X=0, Y=1.
        assert_eq!(c.vars.len(), 2);
    }

    #[test]
    fn parse_query_directive() {
        // ?- ancestor(bob, liz).
        let toks = tstream(&[
            Token::Query,
            a("ancestor"), Token::LParen, a("bob"), Token::Comma, a("liz"),
            Token::RParen, Token::Dot,
        ]);
        let mut atoms = AtomTable::new();
        let clauses = parse(&toks, &mut atoms).expect("parse ok");
        assert_eq!(clauses.len(), 1);
        let c = clauses.get(0).unwrap();
        assert_eq!(c.kind, ClauseKind::Query);
        assert!(matches!(c.head, Term::Nil));
        assert_eq!(c.body.len(), 1);
    }

    #[test]
    fn parse_full_ancestor_program() {
        // parent(bob, ann).
        // parent(ann, liz).
        // ancestor(X, Y) :- parent(X, Y).
        // ancestor(X, Y) :- parent(X, Z), ancestor(Z, Y).
        // ?- ancestor(bob, liz).
        let toks = tstream(&[
            // parent(bob, ann).
            a("parent"), Token::LParen, a("bob"), Token::Comma, a("ann"),
            Token::RParen, Token::Dot,
            // parent(ann, liz).
            a("parent"), Token::LParen, a("ann"), Token::Comma, a("liz"),
            Token::RParen, Token::Dot,
            // ancestor(X, Y) :- parent(X, Y).
            a("ancestor"), Token::LParen, v("X"), Token::Comma, v("Y"), Token::RParen,
            Token::Neck,
            a("parent"), Token::LParen, v("X"), Token::Comma, v("Y"), Token::RParen,
            Token::Dot,
            // ancestor(X, Y) :- parent(X, Z), ancestor(Z, Y).
            a("ancestor"), Token::LParen, v("X"), Token::Comma, v("Y"), Token::RParen,
            Token::Neck,
            a("parent"), Token::LParen, v("X"), Token::Comma, v("Z"), Token::RParen,
            Token::Comma,
            a("ancestor"), Token::LParen, v("Z"), Token::Comma, v("Y"), Token::RParen,
            Token::Dot,
            // ?- ancestor(bob, liz).
            Token::Query,
            a("ancestor"), Token::LParen, a("bob"), Token::Comma, a("liz"),
            Token::RParen, Token::Dot,
        ]);
        let mut atoms = AtomTable::new();
        let clauses = parse(&toks, &mut atoms).expect("parse ok");
        assert_eq!(clauses.len(), 5);
        let kinds: Vec<ClauseKind> = clauses.iter().map(|c| c.kind).collect();
        assert_eq!(
            kinds,
            vec![
                ClauseKind::Fact, ClauseKind::Fact, ClauseKind::Rule,
                ClauseKind::Rule, ClauseKind::Query,
            ]
        );
        let heads: Vec<&str> = clauses
            .iter()
            .map(|c| match c.head {
                Term::Struct { functor, .. } => name_of(&atoms, functor),
                Term::Nil => "",
                _ => "?",
            })
            .collect();
        assert_eq!(heads, vec!["parent", "parent", "ancestor", "ancestor", ""]);
        // Body of rule 2 has two goals.
        assert_eq!(clauses.get(3).unwrap().body.len(), 2);
    }

    #[test]
    fn list_sugar_three_elements() {
        // foo([a, b, c]).
        let toks = tstream(&[
            a("foo"), Token::LParen,
            Token::LBracket,
            a("a"), Token::Comma, a("b"), Token::Comma, a("c"),
            Token::RBracket,
            Token::RParen, Token::Dot,
        ]);
        let mut atoms = AtomTable::new();
        let clauses = parse(&toks, &mut atoms).expect("parse ok");
        let c = clauses.get(0).unwrap();
        // head = foo(X) where X is a list struct
        let (functor, args) = match c.head {
            Term::Struct { functor, args } => (functor, args),
            _ => panic!("head not struct"),
        };
        assert_eq!(name_of(&atoms, functor), "foo");
        assert_eq!(args.len(), 1);
        // Walk the cons list: '.'(a, '.'(b, '.'(c, []))).
        let mut cur_idx = *args.get(0).unwrap();
        let expected = ["a", "b", "c"];
        for want in expected {
            let node = c.subterm(cur_idx).copied().expect("cons node present");
            let (f, a2) = match node {
                Term::Struct { functor, args } => (functor, args),
                _ => panic!("expected cons, got {:?}", node),
            };
            assert_eq!(name_of(&atoms, f), ".");
            assert_eq!(a2.len(), 2);
            let head_idx = *a2.get(0).unwrap();
            let tail_idx = *a2.get(1).unwrap();
            let head = c.subterm(head_idx).copied().unwrap();
            match head {
                Term::Atom(aid) => assert_eq!(name_of(&atoms, aid), want),
                _ => panic!("list head not atom"),
            }
            cur_idx = tail_idx;
        }
        // Final tail should be Nil.
        let end = c.subterm(cur_idx).copied().unwrap();
        assert!(matches!(end, Term::Nil));
    }

    #[test]
    fn list_with_tail_var() {
        // foo([H|T]).
        let toks = tstream(&[
            a("foo"), Token::LParen,
            Token::LBracket, v("H"), Token::Pipe, v("T"), Token::RBracket,
            Token::RParen, Token::Dot,
        ]);
        let mut atoms = AtomTable::new();
        let clauses = parse(&toks, &mut atoms).expect("parse ok");
        let c = clauses.get(0).unwrap();
        let args = match c.head {
            Term::Struct { args, .. } => args,
            _ => panic!("head not struct"),
        };
        let list_idx = *args.get(0).unwrap();
        let cons = c.subterm(list_idx).copied().unwrap();
        let (f, a2) = match cons {
            Term::Struct { functor, args } => (functor, args),
            _ => panic!("list not cons"),
        };
        assert_eq!(name_of(&atoms, f), ".");
        assert_eq!(a2.len(), 2);
        let h = c.subterm(*a2.get(0).unwrap()).copied().unwrap();
        let t = c.subterm(*a2.get(1).unwrap()).copied().unwrap();
        assert!(matches!(h, Term::Var(_)));
        assert!(matches!(t, Term::Var(_)));
    }

    #[test]
    fn empty_list_is_nil() {
        // foo([]).
        let toks = tstream(&[
            a("foo"), Token::LParen,
            Token::LBracket, Token::RBracket,
            Token::RParen, Token::Dot,
        ]);
        let mut atoms = AtomTable::new();
        let clauses = parse(&toks, &mut atoms).expect("parse ok");
        let c = clauses.get(0).unwrap();
        let args = match c.head {
            Term::Struct { args, .. } => args,
            _ => panic!("head not struct"),
        };
        let inside = c.subterm(*args.get(0).unwrap()).copied().unwrap();
        assert!(matches!(inside, Term::Nil));
    }

    #[test]
    fn cut_in_body_becomes_atom() {
        // p :- !, q.
        let toks = tstream(&[
            a("p"), Token::Neck,
            Token::Cut, Token::Comma, a("q"),
            Token::Dot,
        ]);
        let mut atoms = AtomTable::new();
        let clauses = parse(&toks, &mut atoms).expect("parse ok");
        let c = clauses.get(0).unwrap();
        assert_eq!(c.body.len(), 2);
        match c.body.get(0).copied().unwrap() {
            Term::Atom(id) => assert_eq!(name_of(&atoms, id), "!"),
            other => panic!("expected cut atom, got {:?}", other),
        }
    }

    #[test]
    fn not_goal_wraps_in_neg() {
        // p :- \+ q.
        let toks = tstream(&[
            a("p"), Token::Neck,
            Token::Not, a("q"),
            Token::Dot,
        ]);
        let mut atoms = AtomTable::new();
        let clauses = parse(&toks, &mut atoms).expect("parse ok");
        let c = clauses.get(0).unwrap();
        assert_eq!(c.body.len(), 1);
        match c.body.get(0).copied().unwrap() {
            Term::Struct { functor, args } => {
                assert_eq!(name_of(&atoms, functor), "\\+");
                assert_eq!(args.len(), 1);
                let inner = c.subterm(*args.get(0).unwrap()).copied().unwrap();
                match inner {
                    Term::Atom(id) => assert_eq!(name_of(&atoms, id), "q"),
                    _ => panic!("inner not atom"),
                }
            }
            _ => panic!("body[0] not struct"),
        }
    }

    #[test]
    fn int_literal() {
        // foo(42).
        let toks = tstream(&[
            a("foo"), Token::LParen, Token::Int(42), Token::RParen, Token::Dot,
        ]);
        let mut atoms = AtomTable::new();
        let clauses = parse(&toks, &mut atoms).expect("parse ok");
        let c = clauses.get(0).unwrap();
        let args = match c.head {
            Term::Struct { args, .. } => args,
            _ => panic!("head not struct"),
        };
        let arg = c.subterm(*args.get(0).unwrap()).copied().unwrap();
        assert!(matches!(arg, Term::Int(42)));
    }

    #[test]
    fn atom_interning_reuses_ids() {
        // parent(bob, ann). parent(ann, liz).
        let toks = tstream(&[
            a("parent"), Token::LParen, a("bob"), Token::Comma, a("ann"),
            Token::RParen, Token::Dot,
            a("parent"), Token::LParen, a("ann"), Token::Comma, a("liz"),
            Token::RParen, Token::Dot,
        ]);
        let mut atoms = AtomTable::new();
        let _ = parse(&toks, &mut atoms).expect("parse ok");
        // "parent", "bob", "ann", "liz" — 4 distinct atoms total.
        assert_eq!(atoms.len(), 4);
    }

    #[test]
    fn error_missing_dot() {
        // parent(bob, ann)
        let toks = tstream(&[
            a("parent"), Token::LParen, a("bob"), Token::Comma, a("ann"),
            Token::RParen,
        ]);
        let mut atoms = AtomTable::new();
        let err = parse(&toks, &mut atoms).unwrap_err();
        assert!(matches!(err, ParseError::MissingDot(_) | ParseError::UnexpectedEof(_)));
    }

    #[test]
    fn error_mismatched_paren() {
        // parent(bob, ann.
        let toks = tstream(&[
            a("parent"), Token::LParen, a("bob"), Token::Comma, a("ann"),
            Token::Dot,
        ]);
        let mut atoms = AtomTable::new();
        let err = parse(&toks, &mut atoms).unwrap_err();
        assert!(matches!(err, ParseError::MismatchedParen(_)));
    }

    #[test]
    fn error_mismatched_bracket() {
        // foo([a, b.
        let toks = tstream(&[
            a("foo"), Token::LParen,
            Token::LBracket, a("a"), Token::Comma, a("b"),
            Token::Dot,
        ]);
        let mut atoms = AtomTable::new();
        let err = parse(&toks, &mut atoms).unwrap_err();
        assert!(matches!(
            err,
            ParseError::MismatchedBracket(_) | ParseError::UnexpectedToken(_)
        ));
    }

    #[test]
    fn error_bad_operator() {
        // :- p.   (starts with a bare operator, no head)
        let toks = tstream(&[
            Token::Neck, a("p"), Token::Dot,
        ]);
        let mut atoms = AtomTable::new();
        let err = parse(&toks, &mut atoms).unwrap_err();
        assert!(matches!(err, ParseError::UnexpectedToken(_)));
    }

    #[test]
    fn error_cut_as_term() {
        // foo(!).   cut cannot stand as a term argument
        let toks = tstream(&[
            a("foo"), Token::LParen, Token::Cut, Token::RParen, Token::Dot,
        ]);
        let mut atoms = AtomTable::new();
        let err = parse(&toks, &mut atoms).unwrap_err();
        assert!(matches!(err, ParseError::BadOperator(_)));
    }

    #[test]
    fn error_empty_struct_args() {
        // foo().
        let toks = tstream(&[
            a("foo"), Token::LParen, Token::RParen, Token::Dot,
        ]);
        let mut atoms = AtomTable::new();
        let err = parse(&toks, &mut atoms).unwrap_err();
        assert!(matches!(err, ParseError::EmptyArgs(_)));
    }
}
