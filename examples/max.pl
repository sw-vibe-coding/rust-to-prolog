% max.pl — max(A, B, M) with a cut on the first clause.
%
% If A > B, the first clause binds M = A and cuts, committing
% to that choice. Otherwise backtracking falls through to the
% second clause where M = B. The query uses `fail` to force
% backtracking — if cut works, we only see one answer; without
% cut we would see both clauses fire.

max(X, Y, X) :- X > Y, !.
max(_, Y, Y).

?- max(5, 3, M), write(M), nl, fail.
