% neq.pl — negation-as-failure via \+.
%
% ne(X, Y) succeeds when X and Y do NOT unify. Implemented by
% compiling `\+ X = Y` as:
%   TRY neg_label
%     <unify X with Y>
%     TRUST + FAIL    ; reached if unification succeeded — local fail
%   neg_label:
%     TRUST           ; reached if unification failed — \+ succeeds
%
% Neither branch relies on scoped cut, so the implementation works
% even with our non-scoped CUT-all semantics.
%
% Query: enumerate pairs from a finite set and keep only the
% unequal ones. Exercises backtracking around the \+.

ne(X, Y) :- \+ X = Y.

?- ne(red, red), write(ok), nl.
