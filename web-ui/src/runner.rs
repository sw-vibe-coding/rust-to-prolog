//! Thin glue around the `rust_to_prolog` library. A `Session` takes a
//! `.pl` source string, runs it end-to-end through the in-process Rust
//! pipeline (tokenize → parse → compile → emit → asm → refvm), and
//! surfaces the captured output plus a verdict for the UI.
//!
//! Unlike `web-sw-cor24-snobol4`'s runner this is synchronous — our
//! refvm's `tick_limit` is bounded and programs halt in
//! milliseconds, so there's no need to batch ticks across animation
//! frames. If a future demo needs to run longer, batching can be
//! bolted on around `refvm::run_vm` the same way upstream does.

use rust_to_prolog::asm::{assemble, AsmError};
use rust_to_prolog::compile::{compile, CompileError};
use rust_to_prolog::emit::{emit, EmitError};
use rust_to_prolog::parse::{parse, AtomId, AtomTable, ParseError};
use rust_to_prolog::refvm::{run_with_atoms, RunError, RunResult};
use rust_to_prolog::tokenize::{tokenize, TokenizeError};

/// Outcome of a single run — what the UI renders into the output pane.
pub struct RunOutcome {
    /// Captured UART bytes (stdout from write/1, nl/0).
    pub output: String,
    /// Human-readable verdict ("halted", "failed", "error: …").
    pub verdict: String,
    /// Whether the run bottomed out in a VM error (tick limit, bad
    /// opcode, etc.) — used by the UI to colour the status bar.
    pub error: bool,
    /// Assembled `.lam` text, if compilation succeeded and the UI
    /// wants to show it.
    pub lam: Option<String>,
}

pub fn run_source(src: &str) -> RunOutcome {
    let lam = match compile_to_lam(src) {
        Ok(lam) => lam,
        Err(msg) => return pipeline_error(msg),
    };
    match execute(&lam) {
        Ok((out, verdict, error)) => RunOutcome {
            output: out,
            verdict,
            error,
            lam: Some(lam),
        },
        Err(msg) => RunOutcome {
            output: String::new(),
            verdict: msg,
            error: true,
            lam: Some(lam),
        },
    }
}

fn compile_to_lam(src: &str) -> Result<String, String> {
    let toks = tokenize(src).map_err(fmt_tok)?;
    let mut atoms = AtomTable::new();
    let clauses = parse(&toks, &mut atoms).map_err(fmt_parse)?;
    let instrs = compile(&clauses, &atoms).map_err(fmt_compile)?;
    emit(&instrs, &atoms).map_err(fmt_emit)
}

fn execute(lam: &str) -> Result<(String, String, bool), String> {
    let cells = assemble(lam).map_err(fmt_asm)?;
    let code: Vec<u32> = (0..cells.len())
        .map(|i| *cells.get(i).expect("cell in range"))
        .collect();
    let atoms = extract_atoms(lam)?;
    let mut out = Vec::new();
    let res = run_with_atoms(code, atoms, &mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    Ok(match res {
        RunResult::Halt => (text, "halted".into(), false),
        RunResult::Fail => (text, "failed (no more solutions)".into(), false),
        RunResult::Error(e) => (text, format!("vm error: {}", fmt_run(e)), true),
    })
}

/// Re-derive the atom name table from the `.lam`'s `.atom` directives so
/// `B_WRITE` can print `red` instead of `atom(1)`. Parsing the source
/// a second time (through tokenize+parse) would also work, but this is
/// simpler and matches what `src/bin/prologc.rs` does offline.
fn extract_atoms(lam: &str) -> Result<Vec<String>, String> {
    let mut names: Vec<String> = Vec::new();
    for line in lam.lines() {
        let line = line.split(';').next().unwrap_or("").trim();
        let rest = match line.strip_prefix(".atom") {
            Some(r) => r.trim(),
            None => continue,
        };
        let mut parts = rest.split_ascii_whitespace();
        let id_s = parts.next().ok_or("malformed .atom directive")?;
        let name = parts.next().ok_or("malformed .atom directive")?;
        let id: usize = id_s
            .parse()
            .map_err(|_| "non-numeric atom id".to_string())?;
        if names.len() <= id {
            names.resize(id + 1, String::new());
        }
        names[id] = name.to_string();
    }
    Ok(names)
}

fn pipeline_error(msg: String) -> RunOutcome {
    RunOutcome {
        output: String::new(),
        verdict: msg,
        error: true,
        lam: None,
    }
}

fn fmt_tok(e: TokenizeError) -> String {
    format!("tokenize: {:?}", e)
}

fn fmt_parse(e: ParseError) -> String {
    format!("parse: {:?}", e)
}

fn fmt_compile(e: CompileError) -> String {
    format!("compile: {:?}", e)
}

fn fmt_emit(e: EmitError) -> String {
    format!("emit: {:?}", e)
}

fn fmt_asm(e: AsmError) -> String {
    format!("asm: {:?}", e)
}

fn fmt_run(e: RunError) -> String {
    format!("{:?}", e)
}

/// Unused in the current UI but retained for parity with the SNOBOL4
/// runner's return shape.
pub fn atom_count(outcome: &RunOutcome) -> Option<usize> {
    outcome.lam.as_ref().map(|lam| {
        lam.lines()
            .filter(|l| l.trim_start().starts_with(".atom"))
            .count()
    })
}

/// For tests / debug — lets us roundtrip the compile-only path without
/// running the VM.
pub fn compile_only(src: &str) -> Result<String, String> {
    compile_to_lam(src)
}

#[allow(dead_code)]
fn _use_atom_id(_: AtomId) {}
