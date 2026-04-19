% path.pl — transitive reachability in a small directed graph.
%
% path(X, Y) holds when there's a directed path from X to Y along
% the edge/2 facts. Two clauses: direct edge, or one hop + recurse.
%
% Same shape as grandparent.pl: recursive rule with a non-tail
% CALL followed by a tail call, so Y and Z become permanent vars
% (Y-regs) inside an ALLOCATE 2 frame.
%
% Query here checks a single reachability (yes/no with a visible
% confirmation). See path_enum.pl for the enumeration variant.
%
% Expected: "yes\n" then HALT.

edge(a, b).
edge(b, c).
edge(a, d).
edge(d, c).

path(X, Y) :- edge(X, Y).
path(X, Y) :- edge(X, Z), path(Z, Y).

?- path(a, c), write(yes), nl.
