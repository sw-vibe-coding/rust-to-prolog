% Tail-recursive Fibonacci via a pair-accumulator.
% fib(N, A, B, R): A = fib(k), B = fib(k+1), iterate N times.
% Call with fib(N, 0, 1, R) to get R = fib(N).

fib(0, A, _, A).
fib(N, A, B, R) :-
    N > 0,
    NewB is A + B,
    N1 is N - 1,
    fib(N1, B, NewB, R).

?- fib(10, 0, 1, F), write(F), nl.
