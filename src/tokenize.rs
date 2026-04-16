//! Lexical analysis for the Prolog subset.
//!
//! Step 003-parse introduces the `Token` type (needed by the parser). The
//! actual tokenize function arrives in step 004-tokenize. Token spellings
//! mirror `tokenize.sno` so a SNOBOL4 port is a mechanical translation.

use crate::port::{BoundedArr, BoundedStr};

pub const MAX_TOKENS: usize = 512;
pub const ATOM_CAP: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Token {
    Atom(BoundedStr<ATOM_CAP>),
    Var(BoundedStr<ATOM_CAP>),
    Int(i32),
    LParen,
    RParen,
    LBracket,
    RBracket,
    Comma,
    Dot,
    Pipe,
    Neck,
    Cut,
    Not,
    Query,
    Eof,
}

pub fn __placeholder() {
    let _a: BoundedArr<Token, MAX_TOKENS> = BoundedArr::new();
    let _b: BoundedStr<ATOM_CAP> = BoundedStr::new();
}
