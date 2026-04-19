% neq_ok.pl — negation-as-failure, success case.
%
% Companion to neq.pl, which shows the failure case. Here the two
% atoms are distinct, so X = Y fails, so \+ X = Y succeeds, so the
% query reaches write(ok) and prints it.
%
% Expected: "ok\n" then HALT.

ne(X, Y) :- \+ X = Y.

?- ne(red, blue), write(ok), nl.
