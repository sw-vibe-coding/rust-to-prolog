//! Builtin predicates `B_WRITE` and `B_NL`.
//!
//! Both write to a caller-supplied `io::Write` so tests can capture
//! output without touching stdout. Integer sign-extension follows
//! `vm-spec.md` §1.3: bit 20 of the 21-bit payload is the sign.

use super::heap::{
    deref, fun_arity, fun_atom_id, payload, tag, TAG_ATOM, TAG_FUN, TAG_INT, TAG_REF, TAG_STR,
};
use std::io::{self, Write};

const INT_SIGN_BIT: u32 = 1 << 20;
const INT_SIGN_EXTEND: u32 = 0xFFE0_0000;

pub fn write_term<W: Write>(
    cell: u32,
    heap: &[u32],
    atoms: &[String],
    out: &mut W,
) -> io::Result<()> {
    let c = deref(cell, heap);
    match tag(c) {
        TAG_ATOM => write_atom(payload(c), atoms, out),
        TAG_INT => write_int(payload(c), out),
        TAG_REF => write!(out, "_G{}", payload(c)),
        TAG_STR => write_struct(payload(c) as usize, heap, atoms, out),
        _ => write!(out, "<unsupported:{:06X}>", c & 0x00FF_FFFF),
    }
}

fn write_struct<W: Write>(
    fun_addr: usize,
    heap: &[u32],
    atoms: &[String],
    out: &mut W,
) -> io::Result<()> {
    if fun_addr >= heap.len() {
        return write!(out, "<bad_str@{}>", fun_addr);
    }
    let fun = heap[fun_addr];
    if tag(fun) != TAG_FUN {
        return write!(out, "<not_fun@{}>", fun_addr);
    }
    let atom_id = fun_atom_id(fun) as usize;
    let arity = fun_arity(fun) as usize;
    let name = atoms.get(atom_id).map(|s| s.as_str()).unwrap_or("");
    if name == "." && arity == 2 {
        return write_list(fun_addr, heap, atoms, out);
    }
    if atom_id < atoms.len() {
        write!(out, "{}", atoms[atom_id])?;
    } else {
        write!(out, "atom({})", atom_id)?;
    }
    write!(out, "(")?;
    for i in 0..arity {
        if i > 0 {
            write!(out, ", ")?;
        }
        if fun_addr + 1 + i >= heap.len() {
            write!(out, "<oob>")?;
        } else {
            write_term(heap[fun_addr + 1 + i], heap, atoms, out)?;
        }
    }
    write!(out, ")")
}

fn write_list<W: Write>(
    first: usize,
    heap: &[u32],
    atoms: &[String],
    out: &mut W,
) -> io::Result<()> {
    write!(out, "[")?;
    let mut addr = first;
    let mut sep = "";
    loop {
        if addr + 2 >= heap.len() {
            break;
        }
        let head_cell = heap[addr + 1];
        write!(out, "{}", sep)?;
        write_term(head_cell, heap, atoms, out)?;
        sep = ", ";
        let tail = deref(heap[addr + 2], heap);
        match tag(tail) {
            TAG_ATOM => {
                let id = payload(tail) as usize;
                let n = atoms.get(id).map(|s| s.as_str()).unwrap_or("");
                if n == "[]" {
                    return write!(out, "]");
                }
                write!(out, " | ")?;
                write_atom(payload(tail), atoms, out)?;
                return write!(out, "]");
            }
            TAG_STR => {
                let next = payload(tail) as usize;
                if next < heap.len() {
                    let nf = heap[next];
                    if tag(nf) == TAG_FUN
                        && fun_arity(nf) == 2
                        && atoms
                            .get(fun_atom_id(nf) as usize)
                            .map(|s| s == ".")
                            .unwrap_or(false)
                    {
                        addr = next;
                        continue;
                    }
                }
                write!(out, " | ")?;
                write_term(tail, heap, atoms, out)?;
                return write!(out, "]");
            }
            _ => {
                write!(out, " | ")?;
                write_term(tail, heap, atoms, out)?;
                return write!(out, "]");
            }
        }
    }
    write!(out, "]")
}

pub fn write_nl<W: Write>(out: &mut W) -> io::Result<()> {
    writeln!(out)
}

fn write_atom<W: Write>(id: u32, atoms: &[String], out: &mut W) -> io::Result<()> {
    let ix = id as usize;
    if ix < atoms.len() {
        write!(out, "{}", atoms[ix])
    } else {
        write!(out, "atom({})", id)
    }
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
    fn writes_atom_id_when_table_empty() {
        let mut buf = Vec::new();
        write_term(make(TAG_ATOM, 5), &[], &[], &mut buf).expect("write ok");
        assert_eq!(buf, b"atom(5)");
    }

    #[test]
    fn writes_atom_name_from_table() {
        let atoms = vec!["parent".to_string(), "bob".to_string(), "ann".to_string()];
        let mut buf = Vec::new();
        write_term(make(TAG_ATOM, 1), &[], &atoms, &mut buf).expect("write ok");
        assert_eq!(buf, b"bob");
    }

    #[test]
    fn falls_back_to_atom_id_when_out_of_range() {
        let atoms = vec!["parent".to_string()];
        let mut buf = Vec::new();
        write_term(make(TAG_ATOM, 5), &[], &atoms, &mut buf).expect("write ok");
        assert_eq!(buf, b"atom(5)");
    }

    #[test]
    fn writes_positive_int() {
        let mut buf = Vec::new();
        write_term(make(TAG_INT, 42), &[], &[], &mut buf).expect("write ok");
        assert_eq!(buf, b"42");
    }

    #[test]
    fn writes_negative_int_with_sign_extension() {
        let mut buf = Vec::new();
        // -1 in 21-bit two's complement = 0x1FFFFF
        write_term(make(TAG_INT, 0x1F_FFFF), &[], &[], &mut buf).expect("write ok");
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
        write_term(r, &heap, &[], &mut buf).expect("write ok");
        assert_eq!(buf, b"_G0");
    }
}
