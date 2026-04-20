% Classic Hello World. Uses the bareword atom `hello_world` because
% our subset doesn't support single-quoted atoms (`'Hello World!'`).
% See docs/limitations.md §"Quoted atoms and strings" for the gap
% and the plan to lift it.

hello :- write(hello_world), nl.

?- hello.
