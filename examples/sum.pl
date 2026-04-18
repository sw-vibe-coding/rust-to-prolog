% sum.pl — tail-recursive sum 0..N via an accumulator.
%
% sum(N, Acc, Res): walks N down to 0, adding each to Acc;
% when N hits 0, Res unifies with Acc. Because the recursive
% call is the last body goal, no ALLOCATE is needed — all vars
% stay temp and we avoid needing a `GET_VAL`-on-perm detour.
%
% Multiplication is deliberately not used; the LAM VM's
% opcode table only includes B_IS_ADD and B_IS_SUB.
%
% Query: `?- sum(3, 0, X), write(X), nl.` → "6\n".

sum(0, Acc, Acc).
sum(N, Acc, Res) :-
    N > 0,
    NewAcc is Acc + N,
    N1 is N - 1,
    sum(N1, NewAcc, Res).

?- sum(3, 0, X), write(X), nl.
