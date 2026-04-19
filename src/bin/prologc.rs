//! `prologc` — read a `.pl` file and run it end-to-end on the
//! reference VM. Prints captured UART output (from `write/1` and
//! friends), then the final verdict (`HALT` or `FAIL`).
//!
//! Usage:
//!   prologc <file.pl>             run on refvm, print output
//!   prologc <file.pl> --lam       print the assembled .lam text
//!   prologc <file.pl> --cells     print the assembled 24-bit cells

use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use rust_to_prolog::asm::{assemble, Cells};
use rust_to_prolog::compile::compile;
use rust_to_prolog::emit::emit;
use rust_to_prolog::parse::{parse, AtomId, AtomTable};
use rust_to_prolog::refvm::{run_with_atoms, RunResult};
use rust_to_prolog::tokenize::tokenize;

#[derive(Parser, Debug)]
#[command(about = "Prolog-to-LAM compiler + refvm runner")]
struct Cli {
    /// Input `.pl` file.
    input: PathBuf,

    /// Print the assembled `.lam` text and exit.
    #[arg(long)]
    lam: bool,

    /// Print the assembled 24-bit cells (hex) and exit.
    #[arg(long)]
    cells: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let src = fs::read_to_string(&cli.input)
        .with_context(|| format!("reading {}", cli.input.display()))?;
    let toks = tokenize(&src).map_err(|e| anyhow::anyhow!("tokenize: {e:?}"))?;
    let mut atoms = AtomTable::new();
    let clauses = parse(&toks, &mut atoms).map_err(|e| anyhow::anyhow!("parse: {e:?}"))?;
    let instrs = compile(&clauses, &atoms).map_err(|e| anyhow::anyhow!("compile: {e:?}"))?;
    let lam = emit(&instrs, &atoms).map_err(|e| anyhow::anyhow!("emit: {e:?}"))?;

    if cli.lam {
        io::stdout().write_all(lam.as_bytes())?;
        return Ok(());
    }

    let cells = assemble(&lam).map_err(|e| anyhow::anyhow!("asm: {e:?}"))?;

    if cli.cells {
        dump_cells(&cells);
        return Ok(());
    }

    let code: Vec<u32> = (0..cells.len()).map(|i| *cells.get(i).expect("in range")).collect();
    let names: Vec<String> = (0..atoms.len() as AtomId)
        .map(|i| atoms.name(i).expect("id in range").as_str().to_string())
        .collect();
    let mut out = Vec::new();
    let verdict = run_with_atoms(code, names, &mut out);
    io::stdout().write_all(&out)?;
    match verdict {
        RunResult::Halt => eprintln!("-- HALT"),
        RunResult::Fail => eprintln!("-- FAIL (all solutions exhausted)"),
        RunResult::Error(e) => eprintln!("-- ERROR: {e}"),
    }
    Ok(())
}

fn dump_cells(cells: &Cells) {
    for i in 0..cells.len() {
        let c = *cells.get(i).expect("in range");
        println!("{:4}  0x{:06X}", i, c & 0x00FF_FFFF);
    }
}
