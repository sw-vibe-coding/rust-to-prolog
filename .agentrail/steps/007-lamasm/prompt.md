Implement src/asm.rs and src/bin/lamasm.rs: a two-pass .lam assembler.

Input: .lam text. Output: a stream of 32-bit cells, written either as flat binary (u32 little-endian) or as a cor24-loadable bin+addr image.

Mirror tools/lam_asm.py in sw-cor24-prolog exactly in its cell layout. Quick reference:
- Pass 1: scan lines; resolve labels to addresses; collect .atom directives; count instruction widths (most opcodes 1 cell; some 2).
- Pass 2: emit cells. Opcode in high byte; operands in remaining bytes per asm-spec.md encoding.

Opcodes supported initially: the subset needed for ancestor.lam plus all other 24 opcodes since we already have them defined.

CLI (src/bin/lamasm.rs):
- lamasm input.lam -o output.bin
- lamasm input.lam --format flat | cor24
- --verbose prints the cell dump like lam_asm.py does.

Tests:
- Assemble tests/fixtures/ancestor.lam; compare resulting bytes against the known-good binary produced by lam_asm.py on the same input (check in as tests/fixtures/ancestor.bin).
- Unit tests per opcode encoding.

Acceptance: cargo test passes; byte match vs lam_asm.py confirmed; port-audit passes.

Commit: 'lamasm: two-pass .lam assembler (Rust)'.