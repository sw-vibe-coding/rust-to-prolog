//! Decode-and-dispatch loop for the reference LAM VM.
//!
//! Instruction layout (`sw-cor24-prolog/docs/vm-spec.md` §3):
//! cell 1 is `(opcode << 16) | (op1 << 8) | op2`; 2-cell instructions
//! follow with an immediate (label addr or tagged constant). PC
//! advances by 1 or 2 per instruction; CALL saves PC+2 to CP;
//! PROCEED sets PC = CP; EXECUTE is a tail-call (no CP save).
//!
//! Only the ancestor subset is implemented here. Opcodes outside that
//! subset return `Error::UnsupportedOpcode` with the opcode byte; they
//! land in later steps (011-builtins-io, 012-lists, 013-arithmetic,
//! 014-cut, refvm ALLOCATE/DEALLOCATE when a rule needs them).

use super::builtin::{write_nl, write_term};
use super::choice::{pop, push_choice, restore_top, top_alt, update_alt};
use super::heap::{
    alloc_unbound, fun_arity, fun_atom_id, make_fun, make_ref, make_str, payload, tag, unify,
    TAG_FUN, TAG_REF, TAG_STR,
};
use super::{EnvFrame, RunError, Step, UnifyMode, Vm};

pub const OP_NOP: u8 = 0;
pub const OP_HALT: u8 = 1;
pub const OP_CALL: u8 = 2;
pub const OP_EXECUTE: u8 = 3;
pub const OP_PROCEED: u8 = 4;
pub const OP_FAIL: u8 = 5;
pub const OP_TRY: u8 = 6;
pub const OP_RETRY: u8 = 7;
pub const OP_TRUST: u8 = 8;
pub const OP_PUT_VAR: u8 = 10;
pub const OP_PUT_VAL: u8 = 11;
pub const OP_PUT_CONST: u8 = 12;
pub const OP_PUT_Y_VAL: u8 = 13;
pub const OP_GET_VAR: u8 = 16;
pub const OP_GET_VAL: u8 = 17;
pub const OP_GET_CONST: u8 = 18;
pub const OP_GET_STRUCT: u8 = 19;
pub const OP_GET_Y_VAR: u8 = 20;
pub const OP_UNIFY_VAR: u8 = 22;
pub const OP_UNIFY_VAL: u8 = 23;
pub const OP_UNIFY_CONST: u8 = 24;
pub const OP_ALLOCATE: u8 = 28;
pub const OP_DEALLOCATE: u8 = 29;
pub const OP_B_WRITE: u8 = 32;
pub const OP_B_NL: u8 = 33;
pub const OP_B_IS_ADD: u8 = 34;
pub const OP_B_IS_SUB: u8 = 35;
pub const OP_B_LT: u8 = 36;
pub const OP_B_GT: u8 = 37;

pub fn step<W: std::io::Write>(vm: &mut Vm, out: &mut W) -> Result<Step, RunError> {
    if vm.pc >= vm.code.len() {
        return Err(RunError::PcOutOfBounds { pc: vm.pc });
    }
    let cell = vm.code[vm.pc];
    let opcode = ((cell >> 16) & 0xFF) as u8;
    let op1 = ((cell >> 8) & 0xFF) as u8;
    let op2 = (cell & 0xFF) as u8;
    match opcode {
        OP_NOP => { vm.pc += 1; Ok(Step::Continue) }
        OP_HALT => { vm.pc += 1; Ok(Step::Halt) }
        OP_PROCEED => exec_proceed(vm),
        OP_FAIL => exec_fail(vm),
        OP_CALL => exec_call(vm),
        OP_EXECUTE => exec_execute(vm),
        OP_TRY => exec_try(vm),
        OP_RETRY => exec_retry(vm),
        OP_TRUST => exec_trust(vm),
        OP_PUT_VAR => exec_put_var(vm, op1, op2),
        OP_PUT_VAL => exec_put_val(vm, op1, op2),
        OP_PUT_CONST => exec_put_const(vm, op1),
        OP_PUT_Y_VAL => exec_put_y_val(vm, op1, op2),
        OP_GET_VAR => exec_get_var(vm, op1, op2),
        OP_GET_VAL => exec_get_val(vm, op1, op2),
        OP_GET_CONST => exec_get_const(vm, op1),
        OP_GET_STRUCT => exec_get_struct(vm, op1, op2),
        OP_GET_Y_VAR => exec_get_y_var(vm, op1, op2),
        OP_UNIFY_VAR => exec_unify_var(vm, op1),
        OP_UNIFY_VAL => exec_unify_val(vm, op1),
        OP_UNIFY_CONST => exec_unify_const(vm),
        OP_ALLOCATE => exec_allocate(vm, op1),
        OP_DEALLOCATE => exec_deallocate(vm),
        OP_B_WRITE => exec_b_write(vm, op1, out),
        OP_B_NL => exec_b_nl(vm, out),
        OP_B_IS_ADD => exec_b_is_add(vm),
        OP_B_IS_SUB => exec_b_is_sub(vm),
        OP_B_LT => exec_b_lt(vm),
        OP_B_GT => exec_b_gt(vm),
        other => Err(RunError::UnsupportedOpcode { op: other, pc: vm.pc }),
    }
}

fn exec_proceed(vm: &mut Vm) -> Result<Step, RunError> {
    vm.pc = vm.cp;
    Ok(Step::Continue)
}

fn exec_fail(vm: &mut Vm) -> Result<Step, RunError> {
    match top_alt(&vm.choice) {
        Some(alt) => {
            vm.pc = alt;
            Ok(Step::Continue)
        }
        None => Ok(Step::Fail),
    }
}

fn exec_call(vm: &mut Vm) -> Result<Step, RunError> {
    let target = read_imm(vm)?;
    vm.cp = vm.pc + 2;
    vm.pc = target;
    Ok(Step::Continue)
}

fn exec_execute(vm: &mut Vm) -> Result<Step, RunError> {
    let target = read_imm(vm)?;
    vm.pc = target;
    Ok(Step::Continue)
}

fn exec_try(vm: &mut Vm) -> Result<Step, RunError> {
    let alt = read_imm(vm)?;
    push_choice(
        &mut vm.choice,
        &vm.regs,
        alt,
        vm.cp,
        vm.heap.len(),
        vm.trail.len(),
        vm.env.len(),
    );
    vm.pc += 2;
    Ok(Step::Continue)
}

fn exec_retry(vm: &mut Vm) -> Result<Step, RunError> {
    let new_alt = read_imm(vm)?;
    let env_ref = &mut vm.env;
    let saved_cp = restore_top(&vm.choice, &mut vm.regs, &mut vm.heap, &mut vm.trail, |ep| {
        env_ref.truncate(ep);
    })
    .ok_or(RunError::EmptyChoiceStack)?;
    vm.cp = saved_cp;
    update_alt(&mut vm.choice, new_alt);
    vm.pc += 2;
    Ok(Step::Continue)
}

fn exec_trust(vm: &mut Vm) -> Result<Step, RunError> {
    let env_ref = &mut vm.env;
    let saved_cp = restore_top(&vm.choice, &mut vm.regs, &mut vm.heap, &mut vm.trail, |ep| {
        env_ref.truncate(ep);
    })
    .ok_or(RunError::EmptyChoiceStack)?;
    vm.cp = saved_cp;
    pop(&mut vm.choice);
    vm.pc += 1;
    Ok(Step::Continue)
}

fn exec_put_var(vm: &mut Vm, xn: u8, ai: u8) -> Result<Step, RunError> {
    let cell = alloc_unbound(&mut vm.heap);
    let xn = reg_index(xn)?;
    let ai = reg_index(ai)?;
    vm.regs[xn] = cell;
    vm.regs[ai] = cell;
    vm.pc += 1;
    Ok(Step::Continue)
}

fn exec_put_val(vm: &mut Vm, xn: u8, ai: u8) -> Result<Step, RunError> {
    let xn = reg_index(xn)?;
    let ai = reg_index(ai)?;
    vm.regs[ai] = vm.regs[xn];
    vm.pc += 1;
    Ok(Step::Continue)
}

fn exec_put_const(vm: &mut Vm, ai: u8) -> Result<Step, RunError> {
    let imm = read_imm(vm)? as u32;
    let ai = reg_index(ai)?;
    vm.regs[ai] = imm;
    vm.pc += 2;
    Ok(Step::Continue)
}

fn exec_get_var(vm: &mut Vm, xn: u8, ai: u8) -> Result<Step, RunError> {
    let xn = reg_index(xn)?;
    let ai = reg_index(ai)?;
    vm.regs[xn] = vm.regs[ai];
    vm.pc += 1;
    Ok(Step::Continue)
}

fn exec_get_const(vm: &mut Vm, ai: u8) -> Result<Step, RunError> {
    let imm = read_imm(vm)? as u32;
    let ai_ix = reg_index(ai)?;
    let a = vm.regs[ai_ix];
    let ok = unify(a, imm, &mut vm.heap, &mut vm.trail);
    if !ok {
        return exec_fail(vm);
    }
    vm.pc += 2;
    Ok(Step::Continue)
}

fn exec_get_val(vm: &mut Vm, xi: u8, ai: u8) -> Result<Step, RunError> {
    let xi = reg_index(xi)?;
    let ai = reg_index(ai)?;
    let ok = unify(vm.regs[xi], vm.regs[ai], &mut vm.heap, &mut vm.trail);
    if !ok {
        return exec_fail(vm);
    }
    vm.pc += 1;
    Ok(Step::Continue)
}

fn exec_b_is_add(vm: &mut Vm) -> Result<Step, RunError> {
    exec_b_is_binop(vm, |a, b| a.wrapping_add(b))
}

fn exec_b_is_sub(vm: &mut Vm) -> Result<Step, RunError> {
    exec_b_is_binop(vm, |a, b| a.wrapping_sub(b))
}

fn exec_b_is_binop<F: Fn(i32, i32) -> i32>(vm: &mut Vm, op: F) -> Result<Step, RunError> {
    let a = deref_to_int(vm, 1)?;
    let b = deref_to_int(vm, 2)?;
    let r = op(a, b);
    vm.regs[0] = encode_int(r);
    vm.pc += 1;
    Ok(Step::Continue)
}

fn exec_b_lt(vm: &mut Vm) -> Result<Step, RunError> {
    exec_b_cmp(vm, |a, b| a < b)
}

fn exec_b_gt(vm: &mut Vm) -> Result<Step, RunError> {
    exec_b_cmp(vm, |a, b| a > b)
}

fn exec_b_cmp<F: Fn(i32, i32) -> bool>(vm: &mut Vm, op: F) -> Result<Step, RunError> {
    let a = deref_to_int(vm, 0)?;
    let b = deref_to_int(vm, 1)?;
    if !op(a, b) {
        return exec_fail(vm);
    }
    vm.pc += 1;
    Ok(Step::Continue)
}

fn deref_to_int(vm: &Vm, ai: u8) -> Result<i32, RunError> {
    let cell = super::heap::deref(vm.regs[ai as usize], &vm.heap);
    if tag(cell) != super::heap::TAG_INT {
        return Err(RunError::ArithNotInt { ai });
    }
    let p = payload(cell);
    let v = if p & (1 << 20) != 0 {
        (p | 0xFFE0_0000) as i32
    } else {
        p as i32
    };
    Ok(v)
}

fn encode_int(v: i32) -> u32 {
    super::heap::make(super::heap::TAG_INT, (v as u32) & 0x1F_FFFF)
}

fn exec_get_struct(vm: &mut Vm, ai: u8, arity: u8) -> Result<Step, RunError> {
    let imm = read_imm(vm)? as u32;
    let atom_id = payload(imm);
    let ai_ix = reg_index(ai)?;
    let target = super::heap::deref(vm.regs[ai_ix], &vm.heap);
    match tag(target) {
        TAG_STR => enter_read_mode(vm, target, atom_id, arity),
        TAG_REF => enter_write_mode(vm, ai_ix, target, atom_id, arity),
        _ => exec_fail(vm),
    }
}

fn enter_read_mode(vm: &mut Vm, target: u32, atom_id: u32, arity: u8) -> Result<Step, RunError> {
    let addr = payload(target) as usize;
    if addr >= vm.heap.len() {
        return exec_fail(vm);
    }
    let fun = vm.heap[addr];
    if tag(fun) != TAG_FUN || fun_atom_id(fun) != atom_id || fun_arity(fun) != arity {
        return exec_fail(vm);
    }
    vm.mode = UnifyMode::Read;
    vm.up = addr + 1;
    vm.pc += 2;
    Ok(Step::Continue)
}

fn enter_write_mode(
    vm: &mut Vm,
    ai_ix: usize,
    target: u32,
    atom_id: u32,
    arity: u8,
) -> Result<Step, RunError> {
    let fun_addr = vm.heap.len();
    vm.heap.push(make_fun(atom_id, arity));
    let str_cell = make_str(fun_addr);
    if tag(target) == TAG_REF {
        let heap_slot = payload(target) as usize;
        super::heap::bind(heap_slot, str_cell, &mut vm.heap, &mut vm.trail);
    }
    vm.regs[ai_ix] = str_cell;
    vm.mode = UnifyMode::Write;
    vm.up = fun_addr + 1;
    vm.pc += 2;
    Ok(Step::Continue)
}

fn exec_unify_var(vm: &mut Vm, xi: u8) -> Result<Step, RunError> {
    let xi_ix = reg_index(xi)?;
    match vm.mode {
        UnifyMode::Read => {
            if vm.up >= vm.heap.len() {
                return exec_fail(vm);
            }
            vm.regs[xi_ix] = vm.heap[vm.up];
            vm.up += 1;
        }
        UnifyMode::Write => {
            let slot = vm.heap.len();
            vm.heap.push(make_ref(slot));
            vm.regs[xi_ix] = make_ref(slot);
            vm.up = slot + 1;
        }
    }
    vm.pc += 1;
    Ok(Step::Continue)
}

fn exec_unify_val(vm: &mut Vm, xi: u8) -> Result<Step, RunError> {
    let xi_ix = reg_index(xi)?;
    match vm.mode {
        UnifyMode::Read => {
            if vm.up >= vm.heap.len() {
                return exec_fail(vm);
            }
            let cell = vm.heap[vm.up];
            let ok = unify(vm.regs[xi_ix], cell, &mut vm.heap, &mut vm.trail);
            if !ok {
                return exec_fail(vm);
            }
            vm.up += 1;
        }
        UnifyMode::Write => {
            vm.heap.push(vm.regs[xi_ix]);
            vm.up = vm.heap.len();
        }
    }
    vm.pc += 1;
    Ok(Step::Continue)
}

fn exec_unify_const(vm: &mut Vm) -> Result<Step, RunError> {
    let imm = read_imm(vm)? as u32;
    match vm.mode {
        UnifyMode::Read => {
            if vm.up >= vm.heap.len() {
                return exec_fail(vm);
            }
            let cell = vm.heap[vm.up];
            let ok = unify(cell, imm, &mut vm.heap, &mut vm.trail);
            if !ok {
                return exec_fail(vm);
            }
            vm.up += 1;
            vm.pc += 2;
        }
        UnifyMode::Write => {
            vm.heap.push(imm);
            vm.up = vm.heap.len();
            vm.pc += 2;
        }
    }
    Ok(Step::Continue)
}

fn exec_allocate(vm: &mut Vm, n: u8) -> Result<Step, RunError> {
    vm.env.push(EnvFrame {
        saved_cp: vm.cp,
        ys: vec![0u32; n as usize],
    });
    vm.pc += 1;
    Ok(Step::Continue)
}

fn exec_deallocate(vm: &mut Vm) -> Result<Step, RunError> {
    let frame = vm.env.pop().ok_or(RunError::EmptyEnvStack)?;
    vm.cp = frame.saved_cp;
    vm.pc += 1;
    Ok(Step::Continue)
}

fn exec_get_y_var(vm: &mut Vm, yi: u8, ai: u8) -> Result<Step, RunError> {
    let ai = reg_index(ai)?;
    let frame = vm.env.last_mut().ok_or(RunError::EmptyEnvStack)?;
    let y = yi as usize;
    if y >= frame.ys.len() {
        return Err(RunError::BadYSlot { y: yi });
    }
    frame.ys[y] = vm.regs[ai];
    vm.pc += 1;
    Ok(Step::Continue)
}

fn exec_put_y_val(vm: &mut Vm, yi: u8, ai: u8) -> Result<Step, RunError> {
    let ai = reg_index(ai)?;
    let frame = vm.env.last().ok_or(RunError::EmptyEnvStack)?;
    let y = yi as usize;
    if y >= frame.ys.len() {
        return Err(RunError::BadYSlot { y: yi });
    }
    vm.regs[ai] = frame.ys[y];
    vm.pc += 1;
    Ok(Step::Continue)
}

fn exec_b_write<W: std::io::Write>(
    vm: &mut Vm,
    ai: u8,
    out: &mut W,
) -> Result<Step, RunError> {
    let ai = reg_index(ai)?;
    write_term(vm.regs[ai], &vm.heap, &vm.atoms, out).map_err(|_| RunError::Io)?;
    vm.pc += 1;
    Ok(Step::Continue)
}

fn exec_b_nl<W: std::io::Write>(vm: &mut Vm, out: &mut W) -> Result<Step, RunError> {
    write_nl(out).map_err(|_| RunError::Io)?;
    vm.pc += 1;
    Ok(Step::Continue)
}

fn read_imm(vm: &Vm) -> Result<usize, RunError> {
    let imm_pc = vm.pc + 1;
    if imm_pc >= vm.code.len() {
        return Err(RunError::PcOutOfBounds { pc: imm_pc });
    }
    Ok(vm.code[imm_pc] as usize)
}

fn reg_index(op: u8) -> Result<usize, RunError> {
    if op as usize >= 16 {
        return Err(RunError::BadRegister { op });
    }
    Ok(op as usize)
}

#[doc(hidden)]
pub fn __reexport_ref(addr: usize) -> u32 {
    make_ref(addr)
}
