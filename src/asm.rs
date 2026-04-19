//! Two-pass LAM assembler core: `.lam` text to 32-bit cells.
//!
//! Byte-for-byte parity with `tools/lam_asm.py` in sw-cor24-prolog.
//! Pass 1 walks lines to collect `.atom` directives, bind labels to
//! instruction addresses, and validate mnemonics. Pass 2 walks lines
//! again and emits 1 or 2 u32 cells per instruction.
//!
//! Cell encoding (asm-spec.md §'Encoding Rules'):
//!   instruction cell: (opcode << 16) | (op1 << 8) | op2
//!   2-cell instrs: second cell is the immediate (label addr or tag)
//!   tagged atom: (2 << 21) | atom_id
//!   tagged int:  (1 << 21) | (n & 0x1FFFFF)
//!
//! Registers: A0..A7 → 0..7, X0..X7 → 8..15, Y0..Y7 → 0..7 (Y index
//! into the env frame, not the reg file — callers of Y-opcodes pass
//! the Y index directly).

use crate::port::{BoundedArr, BoundedStr};
use thiserror::Error;

pub const MAX_CELLS: usize = 4096;

const ATOM_CAP: usize = 48;
const LABEL_CAP: usize = 48;
const ASM_MAX_ATOMS: usize = 64;
const ASM_MAX_LABELS: usize = 128;

const TAG_INT: u32 = 1;
const TAG_ATOM: u32 = 2;
const TAG_MULT: u32 = 2_097_152;
const IMM_MASK: u32 = 0x001F_FFFF;

type AName = BoundedStr<ATOM_CAP>;
type ALbl = BoundedStr<LABEL_CAP>;

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum AsmError {
    #[error("line {line}: unknown opcode")]
    UnknownOpcode { line: u32 },
    #[error("line {line}: malformed directive")]
    BadDirective { line: u32 },
    #[error("line {line}: bad operand")]
    BadOperand { line: u32 },
    #[error("line {line}: wrong number of operands")]
    BadArity { line: u32 },
    #[error("line {line}: undefined label")]
    UndefinedLabel { line: u32 },
    #[error("line {line}: undeclared atom")]
    UndeclaredAtom { line: u32 },
    #[error("line {line}: identifier too long")]
    NameTooLong { line: u32 },
    #[error("atom table overflow")]
    AtomOverflow,
    #[error("label table overflow")]
    LabelOverflow,
    #[error("cell buffer overflow")]
    CellOverflow,
}

pub type Cells = BoundedArr<u32, MAX_CELLS>;

pub fn assemble(src: &str) -> Result<Cells, AsmError> {
    let mut atoms = AtomTab::new();
    let mut labels = LblTab::new();
    pass1(src, &mut atoms, &mut labels)?;
    pass2(src, &atoms, &labels)
}

pub fn write_flat<W: std::io::Write>(cells: &Cells, w: &mut W) -> std::io::Result<()> {
    for i in 0..cells.len() {
        let c = *cells.get(i).expect("cell in range");
        w.write_all(&c.to_le_bytes())?;
    }
    Ok(())
}

pub fn dump_verbose<W: std::io::Write>(cells: &Cells, w: &mut W) -> std::io::Result<()> {
    for i in 0..cells.len() {
        let c = *cells.get(i).expect("cell in range");
        writeln!(w, "{:04}  0x{:06X}", i, c & 0x00FF_FFFF)?;
    }
    Ok(())
}

struct AtomTab {
    data: BoundedArr<(AName, u32), ASM_MAX_ATOMS>,
}

impl AtomTab {
    fn new() -> Self {
        Self {
            data: BoundedArr::new(),
        }
    }

    fn insert(&mut self, name: &str, id: u32, line: u32) -> Result<(), AsmError> {
        let key = AName::parse_str(name).map_err(|_| AsmError::NameTooLong { line })?;
        self.data
            .push((key, id))
            .map_err(|_| AsmError::AtomOverflow)
    }

    fn get(&self, name: &str) -> Option<u32> {
        for i in 0..self.data.len() {
            let e = self.data.get(i).expect("atom index in range");
            if e.0.as_str() == name {
                return Some(e.1);
            }
        }
        None
    }
}

struct LblTab {
    data: BoundedArr<(ALbl, u32), ASM_MAX_LABELS>,
}

impl LblTab {
    fn new() -> Self {
        Self {
            data: BoundedArr::new(),
        }
    }

    fn insert(&mut self, name: &str, addr: u32, line: u32) -> Result<(), AsmError> {
        let key = ALbl::parse_str(name).map_err(|_| AsmError::NameTooLong { line })?;
        self.data
            .push((key, addr))
            .map_err(|_| AsmError::LabelOverflow)
    }

    fn get(&self, name: &str) -> Option<u32> {
        for i in 0..self.data.len() {
            let e = self.data.get(i).expect("label index in range");
            if e.0.as_str() == name {
                return Some(e.1);
            }
        }
        None
    }
}

#[derive(Clone, Copy)]
enum LineKind<'a> {
    Blank,
    AtomDir { id: u32, name: &'a str },
    Label(&'a str),
    Instr { mnem: &'a str, ops: &'a str },
}

fn strip_line(raw: &str) -> &str {
    let no_comment = match raw.find(';') {
        Some(i) => &raw[..i],
        None => raw,
    };
    no_comment.trim()
}

fn classify<'a>(body: &'a str, line: u32) -> Result<LineKind<'a>, AsmError> {
    if body.is_empty() {
        return Ok(LineKind::Blank);
    }
    if let Some(rest) = body.strip_prefix(".atom") {
        return parse_atom_dir(rest, line);
    }
    if let Some(name) = body.strip_suffix(':') {
        if is_ident(name) {
            return Ok(LineKind::Label(name));
        }
        return Err(AsmError::BadDirective { line });
    }
    let (mnem, ops) = split_mnem(body);
    if !is_mnemonic(mnem) {
        return Err(AsmError::UnknownOpcode { line });
    }
    Ok(LineKind::Instr { mnem, ops })
}

fn parse_atom_dir<'a>(rest: &'a str, line: u32) -> Result<LineKind<'a>, AsmError> {
    let rest = rest.trim_start();
    let mut parts = rest.split_ascii_whitespace();
    let id_s = parts.next().ok_or(AsmError::BadDirective { line })?;
    let name = parts.next().ok_or(AsmError::BadDirective { line })?;
    if parts.next().is_some() {
        return Err(AsmError::BadDirective { line });
    }
    let id: u32 = id_s.parse().map_err(|_| AsmError::BadDirective { line })?;
    if name.is_empty() {
        return Err(AsmError::BadDirective { line });
    }
    Ok(LineKind::AtomDir { id, name })
}

fn split_mnem(body: &str) -> (&str, &str) {
    let idx = body
        .find(|c: char| c.is_ascii_whitespace() || c == ',')
        .unwrap_or(body.len());
    (
        &body[..idx],
        body[idx..].trim_start_matches([' ', '\t', ',']),
    )
}

fn is_ident(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut bs = s.bytes();
    let first = bs.next().expect("non-empty");
    if !(first.is_ascii_alphabetic() || first == b'_') {
        return false;
    }
    for b in bs {
        if !(b.is_ascii_alphanumeric() || b == b'_') {
            return false;
        }
    }
    true
}

fn is_mnemonic(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    for b in s.bytes() {
        if !(b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_') {
            return false;
        }
    }
    true
}

fn pass1(src: &str, atoms: &mut AtomTab, labels: &mut LblTab) -> Result<(), AsmError> {
    let mut addr: u32 = 0;
    for (ix, raw) in src.lines().enumerate() {
        let line = (ix + 1) as u32;
        let body = strip_line(raw);
        match classify(body, line)? {
            LineKind::Blank => {}
            LineKind::AtomDir { id, name } => atoms.insert(name, id, line)?,
            LineKind::Label(name) => labels.insert(name, addr, line)?,
            LineKind::Instr { mnem, .. } => addr += width_of(mnem, line)?,
        }
    }
    Ok(())
}

fn pass2(src: &str, atoms: &AtomTab, labels: &LblTab) -> Result<Cells, AsmError> {
    let mut cells: Cells = BoundedArr::new();
    for (ix, raw) in src.lines().enumerate() {
        let line = (ix + 1) as u32;
        let body = strip_line(raw);
        if let LineKind::Instr { mnem, ops } = classify(body, line)? {
            emit(mnem, ops, line, atoms, labels, &mut cells)?;
        }
    }
    Ok(cells)
}

fn width_of(mnem: &str, line: u32) -> Result<u32, AsmError> {
    if opcode_of(mnem).is_none() {
        return Err(AsmError::UnknownOpcode { line });
    }
    Ok(if is_two_cell(mnem) { 2 } else { 1 })
}

fn opcode_of(mnem: &str) -> Option<u8> {
    match mnem {
        "NOP" => Some(0),
        "HALT" => Some(1),
        "CALL" => Some(2),
        "EXECUTE" => Some(3),
        "PROCEED" => Some(4),
        "FAIL" => Some(5),
        "TRY" => Some(6),
        "RETRY" => Some(7),
        "TRUST" => Some(8),
        "CUT" => Some(9),
        "PUT_VAR" => Some(10),
        "PUT_VAL" => Some(11),
        "PUT_CONST" => Some(12),
        "PUT_Y_VAL" => Some(13),
        "GET_VAR" => Some(16),
        "GET_VAL" => Some(17),
        "GET_CONST" => Some(18),
        "GET_STRUCT" => Some(19),
        "GET_Y_VAR" => Some(20),
        "UNIFY_VAR" => Some(22),
        "UNIFY_VAL" => Some(23),
        "UNIFY_CONST" => Some(24),
        "ALLOCATE" => Some(28),
        "DEALLOCATE" => Some(29),
        "B_WRITE" => Some(32),
        "B_NL" => Some(33),
        "B_IS_ADD" => Some(34),
        "B_IS_SUB" => Some(35),
        "B_LT" => Some(36),
        "B_GT" => Some(37),
        _ => None,
    }
}

fn is_two_cell(mnem: &str) -> bool {
    matches!(
        mnem,
        "CALL"
            | "EXECUTE"
            | "TRY"
            | "RETRY"
            | "PUT_CONST"
            | "GET_CONST"
            | "GET_STRUCT"
            | "UNIFY_CONST"
    )
}

#[derive(Clone, Copy)]
enum Shape {
    NoOp,
    Lbl,
    AiConst,
    AiStruct,
    XnAi,
    YiAi,
    Alloc,
    Reg1,
    ConstOnly,
}

fn shape_of(mnem: &str) -> Shape {
    match mnem {
        "NOP" | "HALT" | "PROCEED" | "FAIL" | "CUT" | "TRUST" | "DEALLOCATE" | "B_NL"
        | "B_IS_ADD" | "B_IS_SUB" | "B_LT" | "B_GT" => Shape::NoOp,
        "CALL" | "EXECUTE" | "TRY" | "RETRY" => Shape::Lbl,
        "PUT_CONST" | "GET_CONST" => Shape::AiConst,
        "GET_STRUCT" => Shape::AiStruct,
        "PUT_VAR" | "PUT_VAL" | "GET_VAR" | "GET_VAL" => Shape::XnAi,
        "PUT_Y_VAL" | "GET_Y_VAR" => Shape::YiAi,
        "ALLOCATE" => Shape::Alloc,
        "B_WRITE" | "UNIFY_VAR" | "UNIFY_VAL" => Shape::Reg1,
        "UNIFY_CONST" => Shape::ConstOnly,
        _ => Shape::NoOp,
    }
}

struct Args<'a> {
    slots: [Option<&'a str>; 4],
    count: usize,
}

fn split_args(ops: &str) -> Args<'_> {
    let mut out = Args {
        slots: [None; 4],
        count: 0,
    };
    if ops.trim().is_empty() {
        return out;
    }
    for a in ops.split(',') {
        let a = a.trim();
        if a.is_empty() {
            continue;
        }
        if out.count < 4 {
            out.slots[out.count] = Some(a);
        }
        out.count += 1;
    }
    out
}

fn emit(
    mnem: &str,
    ops: &str,
    line: u32,
    atoms: &AtomTab,
    labels: &LblTab,
    cells: &mut Cells,
) -> Result<(), AsmError> {
    let opc = opcode_of(mnem).expect("width_of already validated") as u32;
    let args = split_args(ops);
    match shape_of(mnem) {
        Shape::NoOp => emit_noop(opc, &args, line, cells),
        Shape::Lbl => emit_lbl(opc, &args, line, labels, cells),
        Shape::AiConst => emit_ai_const(opc, &args, line, atoms, cells),
        Shape::AiStruct => emit_ai_struct(opc, &args, line, atoms, cells),
        Shape::XnAi => emit_two_reg(opc, &args, line, false, cells),
        Shape::YiAi => emit_two_reg(opc, &args, line, true, cells),
        Shape::Alloc => emit_alloc(opc, &args, line, cells),
        Shape::Reg1 => emit_reg1(opc, &args, line, cells),
        Shape::ConstOnly => emit_const_only(opc, &args, line, atoms, cells),
    }
}

fn emit_const_only(
    opc: u32,
    args: &Args<'_>,
    line: u32,
    atoms: &AtomTab,
    cells: &mut Cells,
) -> Result<(), AsmError> {
    if args.count != 1 {
        return Err(AsmError::BadArity { line });
    }
    let imm = parse_const(args.slots[0].expect("count checked"), atoms, line)?;
    push_cell(cells, opc << 16)?;
    push_cell(cells, imm)
}

fn emit_noop(opc: u32, args: &Args<'_>, line: u32, cells: &mut Cells) -> Result<(), AsmError> {
    if args.count != 0 {
        return Err(AsmError::BadArity { line });
    }
    push_cell(cells, opc << 16)
}

fn emit_lbl(
    opc: u32,
    args: &Args<'_>,
    line: u32,
    labels: &LblTab,
    cells: &mut Cells,
) -> Result<(), AsmError> {
    if args.count != 1 {
        return Err(AsmError::BadArity { line });
    }
    let name = args.slots[0].expect("count checked");
    let addr = labels.get(name).ok_or(AsmError::UndefinedLabel { line })?;
    push_cell(cells, opc << 16)?;
    push_cell(cells, addr)
}

fn emit_ai_const(
    opc: u32,
    args: &Args<'_>,
    line: u32,
    atoms: &AtomTab,
    cells: &mut Cells,
) -> Result<(), AsmError> {
    if args.count != 2 {
        return Err(AsmError::BadArity { line });
    }
    let (r1, _) = parse_reg(args.slots[0].expect("count checked"), line)?;
    let imm = parse_const(args.slots[1].expect("count checked"), atoms, line)?;
    push_cell(cells, (opc << 16) | ((r1 as u32) << 8))?;
    push_cell(cells, imm)
}

fn emit_ai_struct(
    opc: u32,
    args: &Args<'_>,
    line: u32,
    atoms: &AtomTab,
    cells: &mut Cells,
) -> Result<(), AsmError> {
    if args.count != 2 {
        return Err(AsmError::BadArity { line });
    }
    let (r1, _) = parse_reg(args.slots[0].expect("count checked"), line)?;
    let raw = args.slots[1].expect("count checked");
    let (name, arity) = split_slash(raw).ok_or(AsmError::BadOperand { line })?;
    let aid = atoms.get(name).ok_or(AsmError::UndeclaredAtom { line })?;
    push_cell(cells, (opc << 16) | ((r1 as u32) << 8) | (arity as u32))?;
    push_cell(cells, (TAG_ATOM * TAG_MULT) | aid)
}

fn emit_two_reg(
    opc: u32,
    args: &Args<'_>,
    line: u32,
    first_must_be_y: bool,
    cells: &mut Cells,
) -> Result<(), AsmError> {
    if args.count != 2 {
        return Err(AsmError::BadArity { line });
    }
    let (r1, is_y) = parse_reg(args.slots[0].expect("count checked"), line)?;
    let (r2, _) = parse_reg(args.slots[1].expect("count checked"), line)?;
    if first_must_be_y && !is_y {
        return Err(AsmError::BadOperand { line });
    }
    push_cell(cells, (opc << 16) | ((r1 as u32) << 8) | (r2 as u32))
}

fn emit_alloc(opc: u32, args: &Args<'_>, line: u32, cells: &mut Cells) -> Result<(), AsmError> {
    if args.count != 1 {
        return Err(AsmError::BadArity { line });
    }
    let n: u32 = args.slots[0]
        .expect("count checked")
        .parse()
        .map_err(|_| AsmError::BadOperand { line })?;
    if n > 0xFF {
        return Err(AsmError::BadOperand { line });
    }
    push_cell(cells, (opc << 16) | (n << 8))
}

fn emit_reg1(opc: u32, args: &Args<'_>, line: u32, cells: &mut Cells) -> Result<(), AsmError> {
    if args.count != 1 {
        return Err(AsmError::BadArity { line });
    }
    let (r1, _) = parse_reg(args.slots[0].expect("count checked"), line)?;
    push_cell(cells, (opc << 16) | ((r1 as u32) << 8))
}

fn parse_reg(tok: &str, line: u32) -> Result<(u8, bool), AsmError> {
    let bytes = tok.as_bytes();
    if bytes.len() != 2 {
        return Err(AsmError::BadOperand { line });
    }
    let kind = bytes[0];
    let digit = bytes[1];
    if !digit.is_ascii_digit() {
        return Err(AsmError::BadOperand { line });
    }
    let idx = digit - b'0';
    if idx > 7 {
        return Err(AsmError::BadOperand { line });
    }
    match kind {
        b'A' => Ok((idx, false)),
        b'X' => Ok((8 + idx, false)),
        b'Y' => Ok((idx, true)),
        _ => Err(AsmError::BadOperand { line }),
    }
}

fn parse_const(tok: &str, atoms: &AtomTab, line: u32) -> Result<u32, AsmError> {
    if let Some(inner) = strip_call(tok, "atom") {
        let aid = atoms.get(inner).ok_or(AsmError::UndeclaredAtom { line })?;
        return Ok((TAG_ATOM * TAG_MULT) | aid);
    }
    if let Some(inner) = strip_call(tok, "int") {
        let n: i32 = inner.parse().map_err(|_| AsmError::BadOperand { line })?;
        let masked = (n as u32) & IMM_MASK;
        return Ok((TAG_INT * TAG_MULT) | masked);
    }
    Err(AsmError::BadOperand { line })
}

fn strip_call<'a>(tok: &'a str, name: &str) -> Option<&'a str> {
    let with_paren = tok.strip_prefix(name)?.strip_prefix('(')?;
    with_paren.strip_suffix(')')
}

fn split_slash(tok: &str) -> Option<(&str, u8)> {
    let (name, arity_s) = tok.rsplit_once('/')?;
    let arity: u8 = arity_s.parse().ok()?;
    Some((name, arity))
}

fn push_cell(cells: &mut Cells, v: u32) -> Result<(), AsmError> {
    cells.push(v).map_err(|_| AsmError::CellOverflow)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cells_of(src: &str) -> Cells {
        assemble(src).expect("assemble ok")
    }

    fn cell(c: &Cells, i: usize) -> u32 {
        *c.get(i).expect("cell in range")
    }

    #[test]
    fn noop_opcodes_encode_to_single_cell() {
        for (mnem, opc) in [
            ("NOP", 0u32),
            ("HALT", 1),
            ("PROCEED", 4),
            ("FAIL", 5),
            ("CUT", 9),
            ("TRUST", 8),
            ("DEALLOCATE", 29),
            ("B_NL", 33),
            ("B_IS_ADD", 34),
            ("B_IS_SUB", 35),
            ("B_LT", 36),
            ("B_GT", 37),
        ] {
            let src = format!("{mnem}\n");
            let c = cells_of(&src);
            assert_eq!(c.len(), 1, "{mnem}");
            assert_eq!(cell(&c, 0), opc << 16, "{mnem}");
        }
    }

    #[test]
    fn call_and_execute_encode_as_two_cells_with_label_addr() {
        let src = "foo:\n    PROCEED\nbar:\n    CALL foo\n    EXECUTE bar\n";
        let c = cells_of(src);
        // foo at 0 (PROCEED = 1 cell), bar at 1. Then CALL/EXECUTE are 2-cell.
        assert_eq!(c.len(), 5);
        assert_eq!(cell(&c, 0), 4 << 16); // PROCEED
        assert_eq!(cell(&c, 1), 2 << 16); // CALL opcode cell
        assert_eq!(cell(&c, 2), 0); // label addr of foo
        assert_eq!(cell(&c, 3), 3 << 16); // EXECUTE opcode cell
        assert_eq!(cell(&c, 4), 1); // label addr of bar
    }

    #[test]
    fn try_retry_encode_as_two_cells() {
        let src = "a:\n    PROCEED\nb:\n    TRY a\n    RETRY b\n";
        let c = cells_of(src);
        assert_eq!(c.len(), 5);
        assert_eq!(cell(&c, 1), 6 << 16);
        assert_eq!(cell(&c, 2), 0);
        assert_eq!(cell(&c, 3), 7 << 16);
        assert_eq!(cell(&c, 4), 1);
    }

    #[test]
    fn put_const_encodes_register_and_tagged_atom() {
        let src = ".atom 1 bob\n    PUT_CONST A0, atom(bob)\n";
        let c = cells_of(src);
        assert_eq!(c.len(), 2);
        assert_eq!(cell(&c, 0), (12 << 16) | (0 << 8));
        assert_eq!(cell(&c, 1), TAG_ATOM * TAG_MULT | 1);
    }

    #[test]
    fn get_const_encodes_register_and_tagged_atom() {
        let src = ".atom 2 ann\n    GET_CONST A1, atom(ann)\n";
        let c = cells_of(src);
        assert_eq!(c.len(), 2);
        assert_eq!(cell(&c, 0), (18 << 16) | (1 << 8));
        assert_eq!(cell(&c, 1), TAG_ATOM * TAG_MULT | 2);
    }

    #[test]
    fn put_const_encodes_tagged_int() {
        let src = "    PUT_CONST A2, int(42)\n";
        let c = cells_of(src);
        assert_eq!(c.len(), 2);
        assert_eq!(cell(&c, 0), (12 << 16) | (2 << 8));
        assert_eq!(cell(&c, 1), TAG_INT * TAG_MULT | 42);
    }

    #[test]
    fn put_const_negative_int_masks_to_21_bits() {
        let src = "    PUT_CONST A0, int(-1)\n";
        let c = cells_of(src);
        assert_eq!(cell(&c, 1), TAG_INT * TAG_MULT | IMM_MASK);
    }

    #[test]
    fn put_var_xn_ai_encodes_reg_codes() {
        let src = "    PUT_VAR X0, A0\n";
        let c = cells_of(src);
        // opcode 10, op1=8, op2=0
        assert_eq!(cell(&c, 0), (10 << 16) | (8 << 8));
    }

    #[test]
    fn put_val_xn_ai_encodes_reg_codes() {
        let src = "    PUT_VAL X2, A1\n";
        let c = cells_of(src);
        assert_eq!(cell(&c, 0), (11 << 16) | (10 << 8) | 1);
    }

    #[test]
    fn get_var_xn_ai_encodes_reg_codes() {
        let src = "    GET_VAR X1, A1\n";
        let c = cells_of(src);
        assert_eq!(cell(&c, 0), (16 << 16) | (9 << 8) | 1);
    }

    #[test]
    fn put_y_val_requires_first_to_be_y() {
        let src = "    PUT_Y_VAL X0, A0\n";
        let err = assemble(src).unwrap_err();
        assert!(matches!(err, AsmError::BadOperand { .. }));
    }

    #[test]
    fn put_y_val_encodes_y_then_a() {
        let src = "    PUT_Y_VAL Y3, A2\n";
        let c = cells_of(src);
        assert_eq!(cell(&c, 0), (13 << 16) | (3 << 8) | 2);
    }

    #[test]
    fn get_y_var_encodes_y_then_a() {
        let src = "    GET_Y_VAR Y0, A1\n";
        let c = cells_of(src);
        assert_eq!(cell(&c, 0), (20 << 16) | (0 << 8) | 1);
    }

    #[test]
    fn allocate_encodes_n_in_op1() {
        let src = "    ALLOCATE 3\n";
        let c = cells_of(src);
        assert_eq!(cell(&c, 0), (28 << 16) | (3 << 8));
    }

    #[test]
    fn b_write_encodes_register() {
        let src = "    B_WRITE A0\n";
        let c = cells_of(src);
        assert_eq!(cell(&c, 0), (32 << 16) | (0 << 8));
    }

    #[test]
    fn unify_var_and_unify_val_encode_register() {
        let src = "    UNIFY_VAR X0\n    UNIFY_VAL X1\n";
        let c = cells_of(src);
        assert_eq!(c.len(), 2);
        assert_eq!(cell(&c, 0), (22 << 16) | (8 << 8));
        assert_eq!(cell(&c, 1), (23 << 16) | (9 << 8));
    }

    #[test]
    fn get_struct_encodes_reg_arity_and_tagged_atom() {
        let src = ".atom 7 foo\n    GET_STRUCT A0, foo/2\n";
        let c = cells_of(src);
        assert_eq!(c.len(), 2);
        assert_eq!(cell(&c, 0), (19 << 16) | (0 << 8) | 2);
        assert_eq!(cell(&c, 1), TAG_ATOM * TAG_MULT | 7);
    }

    #[test]
    fn comments_and_blank_lines_skipped() {
        let src = "; pre\n\n    HALT  ; inline\n; trailing\n";
        let c = cells_of(src);
        assert_eq!(c.len(), 1);
        assert_eq!(cell(&c, 0), 1 << 16);
    }

    #[test]
    fn undefined_label_errors() {
        let err = assemble("    CALL missing\n").unwrap_err();
        assert!(matches!(err, AsmError::UndefinedLabel { .. }));
    }

    #[test]
    fn undeclared_atom_errors() {
        let err = assemble("    PUT_CONST A0, atom(missing)\n").unwrap_err();
        assert!(matches!(err, AsmError::UndeclaredAtom { .. }));
    }

    #[test]
    fn unknown_opcode_errors() {
        let err = assemble("    BOGUS A0\n").unwrap_err();
        assert!(matches!(err, AsmError::UnknownOpcode { .. }));
    }

    #[test]
    fn bad_arity_errors() {
        let err = assemble("    HALT A0\n").unwrap_err();
        assert!(matches!(err, AsmError::BadArity { .. }));
    }

    #[test]
    fn ancestor_assembles_to_expected_cell_count() {
        // 49 cells pre-ALLOCATE; +3 cells for ALLOCATE, GET_Y_VAR,
        // DEALLOCATE (1 each) in ancestor_c2_body once the recursive
        // rule gets its env frame. The additional GET_Y_VAR (save Z
        // to Y1) also costs 1 cell, so 49 + 4 - 1 = 52. The -1 comes
        // from the fact that PUT_VAL X1, A1 and PUT_VAL X2, A0 both
        // still collapse to 1 cell each under PUT_Y_VAL.
        let src = include_str!("../tests/fixtures/ancestor.lam");
        let c = cells_of(src);
        assert_eq!(c.len(), 52);
    }

    #[test]
    fn ancestor_first_cell_is_execute_query() {
        let src = include_str!("../tests/fixtures/ancestor.lam");
        let c = cells_of(src);
        assert_eq!(cell(&c, 0), 3 << 16);
        // query label moves from address 42 → 45 because
        // ancestor_c2_body grew by 3 cells (ALLOCATE + extra
        // GET_Y_VAR + DEALLOCATE).
        assert_eq!(cell(&c, 1), 45);
    }

    #[test]
    fn write_flat_emits_le_u32_per_cell() {
        let src = "    HALT\n";
        let c = cells_of(src);
        let mut buf = Vec::new();
        write_flat(&c, &mut buf).expect("write ok");
        assert_eq!(buf, vec![0x00, 0x00, 0x01, 0x00]);
    }
}
