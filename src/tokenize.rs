//! Lexical analysis for the Prolog subset.
//!
//! Hand-written scanner over `&str` bytes. Token spellings mirror
//! `tokenize.sno` (ATOM, VAR, INT, NECK, LPAREN, RPAREN, COMMA, DOT, ...)
//! so the SNOBOL4 port is mechanical. Control flow is goto-shaped:
//! a top-level loop skips whitespace/comments, then `next_token`
//! dispatches by first byte to one of a small set of scan helpers.

use crate::port::{BoundedArr, BoundedStr};
use thiserror::Error;

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

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum TokenizeError {
    #[error("unterminated block comment at position {pos}")]
    UnterminatedBlockComment { pos: usize },
    #[error("invalid character {ch:?} at position {pos}")]
    InvalidChar { ch: char, pos: usize },
    #[error("bad integer at position {pos}")]
    BadInt { pos: usize },
    #[error("integer out of range at position {pos}")]
    IntOverflow { pos: usize },
    #[error("identifier too long at position {pos}")]
    IdentTooLong { pos: usize },
    #[error("too many tokens (exceeded capacity)")]
    Overflow,
}

struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<u8> {
        self.bytes.get(self.pos + offset).copied()
    }

    fn at_end(&self) -> bool {
        self.pos >= self.bytes.len()
    }
}

pub fn tokenize(src: &str) -> Result<BoundedArr<Token, MAX_TOKENS>, TokenizeError> {
    let mut cur = Cursor { bytes: src.as_bytes(), pos: 0 };
    let mut out: BoundedArr<Token, MAX_TOKENS> = BoundedArr::new();
    loop {
        skip_ws_and_comments(&mut cur)?;
        if cur.at_end() {
            break;
        }
        let tok = next_token(&mut cur)?;
        out.push(tok).map_err(|_| TokenizeError::Overflow)?;
    }
    out.push(Token::Eof).map_err(|_| TokenizeError::Overflow)?;
    Ok(out)
}

fn skip_ws_and_comments(cur: &mut Cursor) -> Result<(), TokenizeError> {
    loop {
        match cur.peek() {
            Some(b' ') | Some(b'\t') | Some(b'\n') | Some(b'\r') => {
                cur.pos += 1;
            }
            Some(b'%') => skip_line_comment(cur),
            Some(b'/') if cur.peek_at(1) == Some(b'*') => skip_block_comment(cur)?,
            _ => return Ok(()),
        }
    }
}

fn skip_line_comment(cur: &mut Cursor) {
    while let Some(b) = cur.peek() {
        cur.pos += 1;
        if b == b'\n' {
            return;
        }
    }
}

fn skip_block_comment(cur: &mut Cursor) -> Result<(), TokenizeError> {
    let start = cur.pos;
    cur.pos += 2;
    loop {
        match cur.peek() {
            None => return Err(TokenizeError::UnterminatedBlockComment { pos: start }),
            Some(b'*') if cur.peek_at(1) == Some(b'/') => {
                cur.pos += 2;
                return Ok(());
            }
            Some(_) => cur.pos += 1,
        }
    }
}

fn next_token(cur: &mut Cursor) -> Result<Token, TokenizeError> {
    let start = cur.pos;
    let c = cur.peek().expect("next_token called at end");
    if c == b':' && cur.peek_at(1) == Some(b'-') {
        cur.pos += 2;
        return Ok(Token::Neck);
    }
    if c == b'?' && cur.peek_at(1) == Some(b'-') {
        cur.pos += 2;
        return Ok(Token::Query);
    }
    if c == b'\\' && cur.peek_at(1) == Some(b'+') {
        cur.pos += 2;
        return Ok(Token::Not);
    }
    if c == b'-' && cur.peek_at(1).map(|b| b.is_ascii_digit()).unwrap_or(false) {
        return scan_int(cur);
    }
    if let Some(t) = punct_token(c) {
        cur.pos += 1;
        return Ok(t);
    }
    if c.is_ascii_digit() {
        return scan_int(cur);
    }
    if c.is_ascii_lowercase() {
        return scan_atom(cur);
    }
    if c.is_ascii_uppercase() || c == b'_' {
        return scan_var(cur);
    }
    Err(TokenizeError::InvalidChar { ch: c as char, pos: start })
}

fn punct_token(c: u8) -> Option<Token> {
    match c {
        b'(' => Some(Token::LParen),
        b')' => Some(Token::RParen),
        b'[' => Some(Token::LBracket),
        b']' => Some(Token::RBracket),
        b',' => Some(Token::Comma),
        b'.' => Some(Token::Dot),
        b'|' => Some(Token::Pipe),
        b'!' => Some(Token::Cut),
        _ => None,
    }
}

fn scan_ident_bytes<'a>(cur: &mut Cursor<'a>) -> &'a [u8] {
    let start = cur.pos;
    while let Some(b) = cur.peek() {
        if b.is_ascii_alphanumeric() || b == b'_' {
            cur.pos += 1;
        } else {
            break;
        }
    }
    &cur.bytes[start..cur.pos]
}

fn scan_atom(cur: &mut Cursor) -> Result<Token, TokenizeError> {
    let start = cur.pos;
    let bytes = scan_ident_bytes(cur);
    let s = core::str::from_utf8(bytes).expect("ascii ident is valid utf8");
    let name = BoundedStr::<ATOM_CAP>::from_str(s)
        .map_err(|_| TokenizeError::IdentTooLong { pos: start })?;
    Ok(Token::Atom(name))
}

fn scan_var(cur: &mut Cursor) -> Result<Token, TokenizeError> {
    let start = cur.pos;
    let bytes = scan_ident_bytes(cur);
    let s = core::str::from_utf8(bytes).expect("ascii ident is valid utf8");
    let name = BoundedStr::<ATOM_CAP>::from_str(s)
        .map_err(|_| TokenizeError::IdentTooLong { pos: start })?;
    Ok(Token::Var(name))
}

fn scan_int(cur: &mut Cursor) -> Result<Token, TokenizeError> {
    let start = cur.pos;
    if cur.peek() == Some(b'-') {
        cur.pos += 1;
    }
    let digits_start = cur.pos;
    while let Some(b) = cur.peek() {
        if b.is_ascii_digit() {
            cur.pos += 1;
        } else {
            break;
        }
    }
    if cur.pos == digits_start {
        return Err(TokenizeError::BadInt { pos: start });
    }
    let s = core::str::from_utf8(&cur.bytes[start..cur.pos])
        .expect("ascii digits are valid utf8");
    s.parse::<i32>()
        .map(Token::Int)
        .map_err(|_| TokenizeError::IntOverflow { pos: start })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bs(s: &str) -> BoundedStr<ATOM_CAP> {
        BoundedStr::<ATOM_CAP>::from_str(s).expect("test name fits")
    }

    fn tok_vec(src: &str) -> Vec<Token> {
        let arr = tokenize(src).expect("tokenize ok");
        arr.iter().copied().collect()
    }

    #[test]
    fn empty_source_emits_only_eof() {
        let v = tok_vec("");
        assert_eq!(v, vec![Token::Eof]);
    }

    #[test]
    fn whitespace_only() {
        let v = tok_vec("   \n\t  \r\n");
        assert_eq!(v, vec![Token::Eof]);
    }

    #[test]
    fn atoms_vars_ints() {
        let v = tok_vec("foo Bar _baz 42 -7");
        assert_eq!(
            v,
            vec![
                Token::Atom(bs("foo")),
                Token::Var(bs("Bar")),
                Token::Var(bs("_baz")),
                Token::Int(42),
                Token::Int(-7),
                Token::Eof,
            ]
        );
    }

    #[test]
    fn punctuation_each_kind() {
        let v = tok_vec("( ) [ ] , . |");
        assert_eq!(
            v,
            vec![
                Token::LParen, Token::RParen,
                Token::LBracket, Token::RBracket,
                Token::Comma, Token::Dot, Token::Pipe,
                Token::Eof,
            ]
        );
    }

    #[test]
    fn neck_query_cut_not() {
        let v = tok_vec(":- ?- ! \\+");
        assert_eq!(
            v,
            vec![Token::Neck, Token::Query, Token::Cut, Token::Not, Token::Eof]
        );
    }

    #[test]
    fn alphanumeric_identifier_with_digits_and_underscore() {
        let v = tok_vec("parent_of_2");
        assert_eq!(v, vec![Token::Atom(bs("parent_of_2")), Token::Eof]);
    }

    #[test]
    fn line_comment_is_skipped() {
        let v = tok_vec("foo % this is ignored\nbar");
        assert_eq!(
            v,
            vec![Token::Atom(bs("foo")), Token::Atom(bs("bar")), Token::Eof]
        );
    }

    #[test]
    fn block_comment_is_skipped() {
        let v = tok_vec("foo /* middle \n stuff */ bar");
        assert_eq!(
            v,
            vec![Token::Atom(bs("foo")), Token::Atom(bs("bar")), Token::Eof]
        );
    }

    #[test]
    fn fact_shape() {
        let v = tok_vec("parent(bob, ann).");
        assert_eq!(
            v,
            vec![
                Token::Atom(bs("parent")),
                Token::LParen,
                Token::Atom(bs("bob")),
                Token::Comma,
                Token::Atom(bs("ann")),
                Token::RParen,
                Token::Dot,
                Token::Eof,
            ]
        );
    }

    /// Expected token count for examples/ancestor.pl, including the final
    /// EOF sentinel. Pinned by hand-counting the reference file and cross-
    /// checked against `tokenize.sno`'s output for the same input.
    const ANCESTOR_TOKEN_COUNT: usize = 58;

    #[test]
    fn tokenize_ancestor_file_matches_reference_count() {
        let src = include_str!("../examples/ancestor.pl");
        let arr = tokenize(src).expect("tokenize ok");
        assert_eq!(arr.len(), ANCESTOR_TOKEN_COUNT);
        assert_eq!(arr.get(arr.len() - 1), Some(&Token::Eof));
        // Spot-check a few known positions to catch drift.
        assert_eq!(arr.get(0), Some(&Token::Atom(bs("parent"))));
        assert_eq!(arr.get(1), Some(&Token::LParen));
        assert_eq!(arr.get(arr.len() - 2), Some(&Token::Dot));
    }

    #[test]
    fn error_unterminated_block_comment() {
        let err = tokenize("foo /* never closed").unwrap_err();
        assert!(matches!(err, TokenizeError::UnterminatedBlockComment { .. }));
    }

    #[test]
    fn error_bad_identifier_char() {
        let err = tokenize("foo @ bar").unwrap_err();
        assert!(matches!(err, TokenizeError::InvalidChar { ch: '@', .. }));
    }

    #[test]
    fn error_standalone_minus_not_int() {
        let err = tokenize("X - 2").unwrap_err();
        assert!(matches!(err, TokenizeError::InvalidChar { ch: '-', .. }));
    }

    #[test]
    fn error_overflow_on_too_many_tokens() {
        let src = "a ".repeat(MAX_TOKENS + 8);
        let err = tokenize(&src).unwrap_err();
        assert!(matches!(err, TokenizeError::Overflow));
    }

    #[test]
    fn error_identifier_too_long() {
        let long = "a".repeat(ATOM_CAP + 1);
        let err = tokenize(&long).unwrap_err();
        assert!(matches!(err, TokenizeError::IdentTooLong { .. }));
    }
}
