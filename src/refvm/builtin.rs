//! Builtin predicates `B_WRITE` and `B_NL`.
//!
//! Both write to a caller-supplied `io::Write` so tests can capture
//! output without touching stdout. Integer sign-extension follows
//! `vm-spec.md` §1.3: bit 20 of the 21-bit payload is the sign.

use super::heap::{deref, payload, tag, TAG_ATOM, TAG_INT, TAG_REF};
use std::io::{self, Write};

const INT_SIGN_BIT: u32 = 1 << 20;
const INT_SIGN_EXTEND: u32 = 0xFFE0_0000;

pub fn write_term<W: Write>(cell: u32, heap: &[u32], out: &mut W) -> io::Result<()> {
    let c = deref(cell, heap);
    match tag(c) {
        TAG_ATOM => write_atom(payload(c), out),
        TAG_INT => write_int(payload(c), out),
        TAG_REF => write!(out, "_G{}", payload(c)),
        _ => write!(out, "<unsupported:{:06X}>", c & 0x00FF_FFFF),
    }
}

pub fn write_nl<W: Write>(out: &mut W) -> io::Result<()> {
    writeln!(out)
}

fn write_atom<W: Write>(id: u32, out: &mut W) -> io::Result<()> {
    write!(out, "atom({})", id)
}

fn write_int<W: Write>(p: u32, out: &mut W) -> io::Result<()> {
    let value = if p & INT_SIGN_BIT != 0 {
        (p | INT_SIGN_EXTEND) as i32
    } else {
        p as i32
    };
    write!(out, "{}", value)
}

#[cfg(test)]
mod tests {
    use super::super::heap::{alloc_unbound, make, TAG_ATOM, TAG_INT};
    use super::*;

    #[test]
    fn writes_atom_as_atom_id_for_now() {
        let mut buf = Vec::new();
        write_term(make(TAG_ATOM, 5), &[], &mut buf).expect("write ok");
        assert_eq!(buf, b"atom(5)");
    }

    #[test]
    fn writes_positive_int() {
        let mut buf = Vec::new();
        write_term(make(TAG_INT, 42), &[], &mut buf).expect("write ok");
        assert_eq!(buf, b"42");
    }

    #[test]
    fn writes_negative_int_with_sign_extension() {
        let mut buf = Vec::new();
        // -1 in 21-bit two's complement = 0x1FFFFF
        write_term(make(TAG_INT, 0x1F_FFFF), &[], &mut buf).expect("write ok");
        assert_eq!(buf, b"-1");
    }

    #[test]
    fn writes_newline() {
        let mut buf = Vec::new();
        write_nl(&mut buf).expect("write ok");
        assert_eq!(buf, b"\n");
    }

    #[test]
    fn writes_unbound_ref_as_named_placeholder() {
        let mut heap: Vec<u32> = Vec::new();
        let r = alloc_unbound(&mut heap);
        let mut buf = Vec::new();
        write_term(r, &heap, &mut buf).expect("write ok");
        assert_eq!(buf, b"_G0");
    }
}
