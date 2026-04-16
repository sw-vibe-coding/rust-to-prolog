Scaffold the Rust workspace per docs/architecture.md §'Module layout'.

Deliverables:
- cargo init (binary + library layout: src/lib.rs + src/bin/prologc.rs + src/bin/lamasm.rs).
- Workspace Cargo.toml with only these deps: thiserror, anyhow. Binaries may also use clap.
- Create empty module files: src/tokenize.rs, src/parse.rs, src/compile.rs, src/emit.rs, src/asm.rs, src/port/mod.rs, src/refvm/mod.rs. Each stub exports nothing or a `pub fn __placeholder()` no-op.
- src/lib.rs wires the modules with pub mod statements.
- .gitignore for target/, build/, .DS_Store.
- scripts/port-audit.sh: stub that exits 0 and prints 'port-audit: clean (stub)'. chmod +x.
- scripts/run-tests.sh: runs 'cargo test' and 'scripts/port-audit.sh'. chmod +x.
- examples/ancestor.pl: copy from ../../sw-embed/sw-cor24-prolog/examples/ancestor/ancestor.pl (identical content so byte-diff is meaningful later).
- Update the empty README.md to a one-paragraph project description linking to docs/.

Acceptance:
- `cargo build` succeeds with zero warnings.
- `cargo test` runs and reports 0 tests, 0 failures.
- `scripts/run-tests.sh` exits 0.
- `scripts/port-audit.sh` exits 0.
- No unsafe, no external deps beyond thiserror + anyhow + (for binaries) clap.

Port-awareness: not yet enforced by audit but follow the rules in docs/design.md §'Port-aware coding rules' — bodies flat, no HashMap, etc. If in doubt, consult design.md.

Commit message suggestion: 'scaffold: cargo workspace, module stubs, scripts'. Commit before agentrail complete.