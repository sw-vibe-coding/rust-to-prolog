//! Tagged heap cells, dereference, and unification.
//!
//! Per `sw-cor24-prolog/docs/vm-spec.md` §1: every value is a 24-bit
//! tagged cell. Top 3 bits are the tag, low 21 bits are the payload.
//! Cells live in registers, in the heap, and as immediates in 2-cell
//! instructions; all share one encoding.

pub const TAG_REF: u32 = 0;
pub const TAG_INT: u32 = 1;
pub const TAG_ATOM: u32 = 2;
pub const TAG_STR: u32 = 3;
pub const TAG_LIST: u32 = 4;
pub const TAG_FUN: u32 = 5;

pub const TAG_SHIFT: u32 = 21;
pub const PAYLOAD_MASK: u32 = 0x001F_FFFF;

pub fn tag(cell: u32) -> u32 {
    (cell >> TAG_SHIFT) & 0x7
}

pub fn payload(cell: u32) -> u32 {
    cell & PAYLOAD_MASK
}

pub fn make(t: u32, p: u32) -> u32 {
    (t << TAG_SHIFT) | (p & PAYLOAD_MASK)
}

pub fn make_ref(addr: usize) -> u32 {
    make(TAG_REF, addr as u32)
}

pub fn make_str(addr: usize) -> u32 {
    make(TAG_STR, addr as u32)
}

pub fn make_fun(atom_id: u32, arity: u8) -> u32 {
    make(TAG_FUN, (atom_id << 5) | (arity as u32 & 0x1F))
}

pub fn fun_atom_id(fun_cell: u32) -> u32 {
    (payload(fun_cell) >> 5) & 0xFFFF
}

pub fn fun_arity(fun_cell: u32) -> u8 {
    (payload(fun_cell) & 0x1F) as u8
}

pub fn is_unbound(cell: u32, heap: &[u32]) -> bool {
    if tag(cell) != TAG_REF {
        return false;
    }
    let addr = payload(cell) as usize;
    addr < heap.len() && heap[addr] == cell
}

pub fn deref(cell: u32, heap: &[u32]) -> u32 {
    let mut c = cell;
    loop {
        if tag(c) != TAG_REF {
            return c;
        }
        let addr = payload(c) as usize;
        if addr >= heap.len() {
            return c;
        }
        let next = heap[addr];
        if next == c {
            return c;
        }
        c = next;
    }
}

pub fn alloc_unbound(heap: &mut Vec<u32>) -> u32 {
    let h = heap.len();
    heap.push(make_ref(h));
    make_ref(h)
}

pub fn bind(addr: usize, value: u32, heap: &mut [u32], trail: &mut Vec<u32>) {
    heap[addr] = value;
    trail.push(addr as u32);
}

pub fn unwind_trail(target_len: usize, heap: &mut [u32], trail: &mut Vec<u32>) {
    while trail.len() > target_len {
        let addr = trail.pop().expect("checked length") as usize;
        heap[addr] = make_ref(addr);
    }
}

pub fn unify(a: u32, b: u32, heap: &mut Vec<u32>, trail: &mut Vec<u32>) -> bool {
    let da = deref(a, heap);
    let db = deref(b, heap);
    if da == db {
        return true;
    }
    let ta = tag(da);
    let tb = tag(db);
    if ta == TAG_REF {
        bind(payload(da) as usize, db, heap, trail);
        return true;
    }
    if tb == TAG_REF {
        bind(payload(db) as usize, da, heap, trail);
        return true;
    }
    if ta == TAG_ATOM && tb == TAG_ATOM {
        return payload(da) == payload(db);
    }
    if ta == TAG_INT && tb == TAG_INT {
        return payload(da) == payload(db);
    }
    if ta == TAG_STR && tb == TAG_STR {
        return unify_struct(payload(da) as usize, payload(db) as usize, heap, trail);
    }
    false
}

fn unify_struct(
    a_addr: usize,
    b_addr: usize,
    heap: &mut Vec<u32>,
    trail: &mut Vec<u32>,
) -> bool {
    if a_addr >= heap.len() || b_addr >= heap.len() {
        return false;
    }
    let fa = heap[a_addr];
    let fb = heap[b_addr];
    if tag(fa) != TAG_FUN || tag(fb) != TAG_FUN {
        return false;
    }
    if payload(fa) != payload(fb) {
        return false;
    }
    let arity = fun_arity(fa) as usize;
    for i in 0..arity {
        let ea = *heap.get(a_addr + 1 + i).unwrap_or(&0);
        let eb = *heap.get(b_addr + 1 + i).unwrap_or(&0);
        if !unify(ea, eb, heap, trail) {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_and_payload_roundtrip() {
        let c = make(TAG_ATOM, 42);
        assert_eq!(tag(c), TAG_ATOM);
        assert_eq!(payload(c), 42);
    }

    #[test]
    fn deref_follows_ref_chain() {
        let mut heap = vec![make_ref(0); 4];
        heap[0] = make_ref(1);
        heap[1] = make_ref(2);
        heap[2] = make(TAG_ATOM, 7);
        heap[3] = make_ref(3);
        assert_eq!(deref(make_ref(0), &heap), make(TAG_ATOM, 7));
        assert_eq!(deref(make_ref(3), &heap), make_ref(3));
    }

    #[test]
    fn alloc_unbound_returns_self_ref() {
        let mut heap: Vec<u32> = Vec::new();
        let r = alloc_unbound(&mut heap);
        assert_eq!(heap.len(), 1);
        assert_eq!(tag(r), TAG_REF);
        assert_eq!(heap[0], r);
        assert!(is_unbound(r, &heap));
    }

    #[test]
    fn unify_atom_with_atom() {
        let mut heap: Vec<u32> = Vec::new();
        let mut trail: Vec<u32> = Vec::new();
        let a = make(TAG_ATOM, 1);
        let b = make(TAG_ATOM, 1);
        let c = make(TAG_ATOM, 2);
        assert!(unify(a, b, &mut heap, &mut trail));
        assert!(!unify(a, c, &mut heap, &mut trail));
    }

    #[test]
    fn unify_unbound_with_atom_binds() {
        let mut heap: Vec<u32> = Vec::new();
        let mut trail: Vec<u32> = Vec::new();
        let r = alloc_unbound(&mut heap);
        let a = make(TAG_ATOM, 3);
        assert!(unify(r, a, &mut heap, &mut trail));
        assert_eq!(deref(r, &heap), a);
        assert_eq!(trail.len(), 1);
    }

    #[test]
    fn unwind_trail_restores_unbound() {
        let mut heap: Vec<u32> = Vec::new();
        let mut trail: Vec<u32> = Vec::new();
        let r = alloc_unbound(&mut heap);
        let a = make(TAG_ATOM, 4);
        assert!(unify(r, a, &mut heap, &mut trail));
        assert_eq!(deref(r, &heap), a);
        unwind_trail(0, &mut heap, &mut trail);
        assert!(is_unbound(r, &heap));
        assert_eq!(trail.len(), 0);
    }

    #[test]
    fn unify_two_unbound_binds_one_to_other() {
        let mut heap: Vec<u32> = Vec::new();
        let mut trail: Vec<u32> = Vec::new();
        let a = alloc_unbound(&mut heap);
        let b = alloc_unbound(&mut heap);
        assert!(unify(a, b, &mut heap, &mut trail));
        let atom = make(TAG_ATOM, 9);
        assert!(unify(a, atom, &mut heap, &mut trail));
        assert_eq!(deref(a, &heap), atom);
        assert_eq!(deref(b, &heap), atom);
    }
}
