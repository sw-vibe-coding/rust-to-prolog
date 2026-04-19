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
        // Inline version — adds `write(yes), nl` to the query so the
        // UI shows a visible confirmation. examples/ancestor.pl has
        // the silent yes/no form for the byte-parity tests.
        name: "ancestor (recursion + pattern match)",
        source: "parent(bob, ann).\n\
parent(ann, liz).\n\
ancestor(X, Y) :- parent(X, Y).\n\
ancestor(X, Y) :- parent(X, Z), ancestor(Z, Y).\n\
\n\
?- ancestor(bob, liz), write(yes), nl.\n",
    },
    Demo {
        name: "append (list concatenation)",
        source: include_str!("../../examples/append.pl"),
    },
    Demo {
        name: "color (backtracking demo)",
        source: include_str!("../../examples/color.pl"),
    },
    Demo {
        name: "fib (Fibonacci with accumulator)",
        source: include_str!("../../examples/fib.pl"),
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
        name: "neq (same atoms, fails)",
        source: include_str!("../../examples/neq.pl"),
    },
    Demo {
        name: "neq (distinct atoms, succeeds)",
        source: include_str!("../../examples/neq_ok.pl"),
    },
    Demo {
        name: "path (reachability — yes/no)",
        source: include_str!("../../examples/path.pl"),
    },
    Demo {
        name: "path (print each reachable path)",
        source: include_str!("../../examples/path_show.pl"),
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
