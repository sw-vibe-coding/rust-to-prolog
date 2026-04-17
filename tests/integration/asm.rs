//! Byte-diff of the Rust assembler output against a known-good binary
//! produced by `tools/lam_asm.py --raw` on the same input. Per the
//! plan.md §'Known risks' policy: `lam_asm.py` is the specification
//! authority for cell layout; the `.bin` fixture is generated from it.

use rust_to_prolog::asm::{assemble, write_flat};

const ANCESTOR_LAM: &str = include_str!("../fixtures/ancestor.lam");
const ANCESTOR_BIN: &[u8] = include_bytes!("../fixtures/ancestor.bin");

#[test]
fn ancestor_asm_byte_matches_lam_asm_py() {
    let cells = assemble(ANCESTOR_LAM).expect("assemble ok");
    let mut actual = Vec::with_capacity(ANCESTOR_BIN.len());
    write_flat(&cells, &mut actual).expect("write flat ok");
    if actual != ANCESTOR_BIN {
        eprintln!("expected {} bytes, got {} bytes", ANCESTOR_BIN.len(), actual.len());
        panic!("ancestor.bin drifted from lam_asm.py output");
    }
}
