% Classical list append via structural recursion.
append([], L, L).
append([H|T], L, [H|R]) :- append(T, L, R).

?- append([a, b], [c, d, e], X), write(X), nl.
