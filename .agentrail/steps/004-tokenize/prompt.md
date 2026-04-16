Implement src/tokenize.rs.

NOTE: this step was originally planned BEFORE 003-parse. The saga numbering is off because an earlier add command failed on a Unicode character. When this step is current, do it; when 003-parse is current and tokenize is already done, just proceed normally.

Input: &str (Prolog source). Output: BoundedArr<Token, 512>. Expose via fn tokenize(src: &str) -> Result<BoundedArr<Token, 512>, TokenizeError>.

Token enum: Atom(BoundedStr<32>), Var(BoundedStr<32>), Int(i32), LParen, RParen, LBracket, RBracket, Comma, Dot, Pipe, Neck, Cut, Not, Eof.

Lexical rules (per tokenize.sno in sw-cor24-prolog):
- Atoms: lowercase-first alphanumeric_underscore.
- Vars: uppercase-first or leading underscore, alphanumeric_underscore.
- Ints: decimal, optional leading minus.
- Neck: two-char ':-'.
- Cut: single-char '!'. Not: two-char backslash-plus.
- Comments: '%' to end of line; /* ... */ block comments.
- Whitespace: spaces, tabs, newlines.

Tests:
- Hand-written token-expected pairs for each token kind.
- Tokenize examples/ancestor.pl completely; assert token count matches reference (run tokenize.sno to obtain the expected count and pin it as a constant).
- Error cases: unterminated block comment, bad identifier, overflow (more than 512 tokens).

Constraints:
- No regex crates. Hand-written scanner with a cursor.
- Function bodies under 50 lines. Flat control flow.
- Spelling of Token variants mirrors tokenize.sno names (ATOM, VAR, INT, NECK, LPAREN, RPAREN, COMMA, DOT) rendered in Rust case.

Acceptance: cargo test passes; scripts/port-audit.sh passes.

Commit: 'tokenize: lexer for Prolog source'.