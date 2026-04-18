% member.pl — classic list membership, enumerates all solutions.
%
% member_c1 matches when X is the head of the list. member_c2
% recurses on the tail. The query `member(X, [a,b,c]), write(X),
% nl, fail.` prints each element through backtracking. Captured
% UART output: "a\nb\nc\n".

member(X, [X|_]).
member(X, [_|T]) :- member(X, T).

?- member(X, [a, b, c]), write(X), nl, fail.
