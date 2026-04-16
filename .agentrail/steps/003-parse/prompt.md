Implement src/parse.rs.

Input: BoundedArr<Token, 512> (from tokenize). Output: BoundedArr<Clause, 64>.

Types (in src/parse.rs):
- enum Term { Atom(AtomId), Var(VarSlot), Int(i32), Struct { functor: AtomId, args: BoundedArr<Term, 8> }, Nil }
- struct Clause { head: Term, body: BoundedArr<Term, 16> }
- AtomId is a u16 interned index into a per-parse AtomTable (another Vmap-style struct).
- VarSlot is a u8 index into a clause-local table of variable names.

Grammar (subset sufficient for ancestor.pl, color.pl, and the liar puzzle):
- clause := term ('.' | ':-' body '.')
- body := goal (',' goal)*
- goal := term | '\\+' goal | '!'
- term := atom | var | int | atom '(' term (',' term)* ')' | list
- list := '[]' | '[' term (',' term)* ( '|' term )? ']'

Desugaring:
- [H|T] -> Struct('./2', [H, T])
- [a,b,c] -> Struct('./2', [a, Struct('./2', [b, Struct('./2', [c, Nil])])])
- '!' -> Atom(intern('!'))   (compiler treats specially)
- '\\+' Goal -> Struct('\\+'/1, [Goal])

Tests:
- Parse each example file and assert clause count + head-functor names.
- Handful of AST-shape assertions (e.g., list desugaring).
- Error cases: mismatched parens, missing dot, bad operator.

Acceptance: cargo test passes; scripts/port-audit.sh passes.

Commit: 'parse: Prolog AST + list desugaring'.