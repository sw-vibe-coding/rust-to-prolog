% path_show.pl — print each reachable path as it's traversed.
%
% Companion to path.pl. Instead of a yes/no reachability check,
% show_path/2 prints every node along the way, using the tail
% recursion to walk the graph. The fail-driven query forces
% backtracking so both paths from `a` to `c` are printed.
%
% Why not use `?- path(a, c, Route), write(Route), nl.`?
% Building Route as a list requires either a list pattern in the
% head (e.g. `path(X, Y, [X, Y]) :- edge(X, Y).`) or a body-side
% `Route = [X, Y]` unification. Both paths try to unify a cons
% cell where the list elements are permanent vars (X, Y appear in
% multiple chunks of the clause), and our compiler currently
% errors with `StructArg` on permanent vars inside any UNIFY_*
% stream. See docs/limitations.md §"Permanent vars in UNIFY
% streams" for the full story and the classical WAM fix.
%
% Expected output (first proof, then second via backtracking):
%   a
%   b
%   c
%   done        — path a → b → c
%   a
%   d
%   c
%   done        — path a → d → c
% then FAIL (all solutions exhausted).

edge(a, b).
edge(b, c).
edge(a, d).
edge(d, c).

show_path(X, X) :- write(X), nl.
show_path(X, Y) :- edge(X, Z), write(X), nl, show_path(Z, Y).

?- show_path(a, c), write(done), nl, fail.
