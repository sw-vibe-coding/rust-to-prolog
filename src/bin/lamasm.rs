//! `lamasm` — two-pass `.lam` assembler CLI.
//!
//! Reads a `.lam` text file, assembles to 32-bit cells, writes flat
//! binary (little-endian u32 per cell) to a file or stdout. See
//! `rust_to_prolog::asm` for the assembler core.

use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use rust_to_prolog::asm::{assemble, dump_verbose, write_flat};

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Format {
    Flat,
}

#[derive(Parser, Debug)]
#[command(about = "Two-pass .lam assembler (mirrors tools/lam_asm.py)")]
struct Cli {
    /// Input `.lam` file.
    input: PathBuf,

    /// Output file. If absent, flat bytes are written to stdout.
    #[arg(short = 'o', long = "output")]
    output: Option<PathBuf>,

    /// Output format. Only `flat` (LE u32 per cell) is supported.
    #[arg(long = "format", value_enum, default_value_t = Format::Flat)]
    format: Format,

    /// Print a cell-by-cell dump to stderr.
    #[arg(long = "verbose")]
    verbose: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let src = fs::read_to_string(&cli.input)
        .with_context(|| format!("reading {}", cli.input.display()))?;
    let cells = assemble(&src).with_context(|| format!("assembling {}", cli.input.display()))?;

    if cli.verbose {
        let mut err = io::stderr().lock();
        dump_verbose(&cells, &mut err).context("writing verbose dump")?;
    }

    match (cli.format, &cli.output) {
        (Format::Flat, Some(path)) => {
            let mut f = fs::File::create(path)
                .with_context(|| format!("creating {}", path.display()))?;
            write_flat(&cells, &mut f).context("writing flat binary")?;
            f.flush().context("flushing output")?;
        }
        (Format::Flat, None) => {
            let mut out = io::stdout().lock();
            write_flat(&cells, &mut out).context("writing flat binary")?;
            out.flush().context("flushing stdout")?;
        }
    }
    Ok(())
}
