% color.pl — enumerate colors via backtracking, printing each.
%
% Equivalent to sw-cor24-prolog/examples/tests/retry.lam (hand-written
% .lam) but driven through the full Rust pipeline. The query
% `color(X), write(X), nl, fail.` binds X to each color in turn,
% prints it with a newline, then forces backtracking via `fail`,
% which eventually exhausts color's choice points and returns FAIL at
% the top level. Captured UART output: "red\ngreen\nblue\n".

color(red).
color(green).
color(blue).

?- color(X), write(X), nl, fail.
