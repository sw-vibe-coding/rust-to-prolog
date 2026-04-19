//! Bundled `.pl` demos, baked into the WASM binary at build time.
//!
//! Each `Demo` pairs a display name with the raw Prolog source from
//! `../examples/*.pl` (or an inline string for programs that don't
//! have a checked-in example file). The UI renders these in the
//! dropdown in alphabetical order.

pub struct Demo {
    pub name: &'static str,
    pub source: &'static str,
}

pub static DEMOS: &[Demo] = &[
    Demo {
        name: "ancestor (recursion + pattern match)",
        source: include_str!("../../examples/ancestor.pl"),
    },
    Demo {
        name: "append (list concatenation)",
        source: "% Classical list append via structural recursion.\n\
append([], L, L).\n\
append([H|T], L, [H|R]) :- append(T, L, R).\n\
\n\
?- append([a, b], [c, d, e], X), write(X), nl.\n",
    },
    Demo {
        name: "color (backtracking demo)",
        source: include_str!("../../examples/color.pl"),
    },
    Demo {
        name: "fib (Fibonacci with accumulator)",
        source: "% Tail-recursive Fibonacci via a pair-accumulator.\n\
% fib(N, A, B, R): A = fib(k), B = fib(k+1), iterate N times.\n\
% Call with fib(N, 0, 1, R) to get R = fib(N).\n\
\n\
fib(0, A, _, A).\n\
fib(N, A, B, R) :-\n    \
N > 0,\n    \
NewB is A + B,\n    \
N1 is N - 1,\n    \
fib(N1, B, NewB, R).\n\
\n\
?- fib(10, 0, 1, F), write(F), nl.\n",
    },
    Demo {
        name: "liar (Lion Lies on Tuesdays)",
        source: include_str!("../../examples/liar.pl"),
    },
    Demo {
        name: "max (cut commitment)",
        source: include_str!("../../examples/max.pl"),
    },
    Demo {
        name: "member (list membership)",
        source: include_str!("../../examples/member.pl"),
    },
    Demo {
        name: "neq (negation-as-failure)",
        source: include_str!("../../examples/neq.pl"),
    },
    Demo {
        name: "sum (tail-recursive arithmetic)",
        source: include_str!("../../examples/sum.pl"),
    },
];

pub fn default_index() -> usize {
    // Lead with the liar puzzle — the headline demo. Its name starts
    // with 'l' so the alphabetical order pushes it past ancestor /
    // append / color / fib in the dropdown.
    DEMOS
        .iter()
        .position(|d| d.name.starts_with("liar"))
        .unwrap_or(0)
}
