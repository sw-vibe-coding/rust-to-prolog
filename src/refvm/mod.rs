//! Reference LAM VM for fast unit tests.
//!
//! The PL/SW VM in `sw-cor24-prolog` is authoritative; any mismatch
//! with this module is a bug here. Scope for step 009-refvm-core:
//! opcodes needed for the ancestor subset plus the `B_WRITE` / `B_NL`
//! builtins (wired to an `io::Write` so tests can capture output).
//! Unimplemented opcodes surface as `RunError::UnsupportedOpcode` and
//! land in later steps.
//!
//! ## Port-audit exception
//!
//! Per `docs/plan.md` step 009, this module is a Rust-only test aid
//! and will not be ported to SNOBOL4 / PL/SW. The port-aware coding
//! rules (no `Vec`, no `std::io`, 50-line-function cap, etc.) are
//! relaxed here: the heap/trail/choice-stack are `Vec<u32>` and
//! builtins use `io::Write`. All other modules remain port-friendly.
//!
//! ## Known caveat (hand-off to step 010-integration-ancestor)
//!
//! `src/compile.rs` currently omits `ALLOCATE`/`DEALLOCATE` for
//! recursive rules. That leaves the second clause of `ancestor/2`
//! unable to preserve `CP` across `CALL parent_entry` before the
//! tail-call `EXECUTE ancestor_entry`, and the program will loop on
//! a correct VM. Step 010 fixes the compiler; refvm tests in this
//! step use hand-crafted `.lam` that already matches the spec
//! (same pattern the upstream self-tests use).

pub mod builtin;
pub mod choice;
pub mod dispatch;
pub mod heap;

use thiserror::Error;

pub const DEFAULT_TICK_LIMIT: u64 = 1_000_000;

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum RunError {
    #[error("pc out of bounds: {pc}")]
    PcOutOfBounds { pc: usize },
    #[error("unsupported opcode {op} at pc {pc}")]
    UnsupportedOpcode { op: u8, pc: usize },
    #[error("bad register index {op}")]
    BadRegister { op: u8 },
    #[error("empty choice stack on restore")]
    EmptyChoiceStack,
    #[error("empty env stack (ALLOCATE/DEALLOCATE imbalance)")]
    EmptyEnvStack,
    #[error("bad Y-slot index {y}")]
    BadYSlot { y: u8 },
    #[error("arithmetic operand in A{ai} is not an integer")]
    ArithNotInt { ai: u8 },
    #[error("tick limit exceeded (runaway guard)")]
    TickLimit,
    #[error("io error writing builtin output")]
    Io,
}

#[derive(Debug, PartialEq, Eq)]
pub enum RunResult {
    Halt,
    Fail,
    Error(RunError),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Continue,
    Halt,
    Fail,
}

#[derive(Clone, Debug)]
pub struct EnvFrame {
    pub saved_cp: usize,
    pub ys: Vec<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnifyMode {
    Read,
    Write,
}

pub struct Vm {
    pub code: Vec<u32>,
    pub heap: Vec<u32>,
    pub trail: Vec<u32>,
    pub regs: [u32; 16],
    pub pc: usize,
    pub cp: usize,
    pub choice: Vec<choice::ChoicePt>,
    /// Env frames. Grows monotonically during forward execution;
    /// `DEALLOCATE` and choice-point restore only move `ep` rather
    /// than popping, so frames protected by active choice points
    /// aren't destroyed and come back into scope on backtrack.
    pub env: Vec<EnvFrame>,
    /// Logical env-stack top. `env[ep - 1]` is the current frame.
    pub ep: usize,
    pub atoms: Vec<String>,
    pub mode: UnifyMode,
    pub up: usize,
    pub tick_limit: u64,
}

impl Vm {
    pub fn new(code: Vec<u32>) -> Self {
        Self {
            code,
            heap: Vec::new(),
            trail: Vec::new(),
            regs: [0u32; 16],
            pc: 0,
            cp: 0,
            choice: Vec::new(),
            env: Vec::new(),
            ep: 0,
            atoms: Vec::new(),
            mode: UnifyMode::Read,
            up: 0,
            tick_limit: DEFAULT_TICK_LIMIT,
        }
    }
}

pub fn run(code: Vec<u32>) -> RunResult {
    let mut sink = std::io::sink();
    run_with(code, &mut sink)
}

pub fn run_with<W: std::io::Write>(code: Vec<u32>, out: &mut W) -> RunResult {
    let mut vm = Vm::new(code);
    run_vm(&mut vm, out)
}

pub fn run_with_atoms<W: std::io::Write>(
    code: Vec<u32>,
    atoms: Vec<String>,
    out: &mut W,
) -> RunResult {
    let mut vm = Vm::new(code);
    vm.atoms = atoms;
    run_vm(&mut vm, out)
}

pub fn run_vm<W: std::io::Write>(vm: &mut Vm, out: &mut W) -> RunResult {
    let mut ticks: u64 = 0;
    loop {
        if ticks >= vm.tick_limit {
            return RunResult::Error(RunError::TickLimit);
        }
        ticks += 1;
        match dispatch::step(vm, out) {
            Ok(Step::Continue) => {}
            Ok(Step::Halt) => return RunResult::Halt,
            Ok(Step::Fail) => return RunResult::Fail,
            Err(e) => return RunResult::Error(e),
        }
    }
}
