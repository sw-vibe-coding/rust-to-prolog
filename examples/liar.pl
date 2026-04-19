% liar.pl — "The Lion Lies on Tuesdays" logic puzzle.
%
% Classical formulation: the Lion lies Mon/Tue/Wed and tells the
% truth other days; the Unicorn lies Thu/Fri/Sat and tells the
% truth other days. Yesterday, both said "yesterday I lied."
% What day is today? (Answer: thursday.)
%
% Encoded without \+ — instead of "lion tells truth on Today" as
% '\+ lion_lies(Today)', we use an explicit lion_truth/1 fact set.
% This sidesteps the residual-choice-point issue in our current
% \+ compilation (which only handles goals G that leave at most
% one CP on the stack, whereas multi-clause facts like
% lion_lies/1 leave one dispatcher CP on every success).
%
% The puzzle reduces to: for some Today, both lion and unicorn
% can coherently say "yesterday I lied" — either they tell the
% truth today and did lie yesterday, or they lie today and did
% NOT lie yesterday (= told truth yesterday).
%
% Query: `?- day(Today), lion_says_yesterday_lied(Today),
%            unicorn_says_yesterday_lied(Today), write(Today), nl.`
% Output: "thursday\n".

day(monday).
day(tuesday).
day(wednesday).
day(thursday).
day(friday).
day(saturday).
day(sunday).

lion_lies(monday).
lion_lies(tuesday).
lion_lies(wednesday).

lion_truth(thursday).
lion_truth(friday).
lion_truth(saturday).
lion_truth(sunday).

unicorn_lies(thursday).
unicorn_lies(friday).
unicorn_lies(saturday).

unicorn_truth(sunday).
unicorn_truth(monday).
unicorn_truth(tuesday).
unicorn_truth(wednesday).

yesterday(monday, sunday).
yesterday(tuesday, monday).
yesterday(wednesday, tuesday).
yesterday(thursday, wednesday).
yesterday(friday, thursday).
yesterday(saturday, friday).
yesterday(sunday, saturday).

% Lion says "yesterday I lied":
%   Case 1: Lion tells truth today AND Lion did lie yesterday.
%   Case 2: Lion lies today AND Lion told truth yesterday.
lion_says_yesterday_lied(Today) :-
    yesterday(Today, Yday),
    lion_truth(Today),
    lion_lies(Yday).
lion_says_yesterday_lied(Today) :-
    yesterday(Today, Yday),
    lion_lies(Today),
    lion_truth(Yday).

unicorn_says_yesterday_lied(Today) :-
    yesterday(Today, Yday),
    unicorn_truth(Today),
    unicorn_lies(Yday).
unicorn_says_yesterday_lied(Today) :-
    yesterday(Today, Yday),
    unicorn_lies(Today),
    unicorn_truth(Yday).

?- day(Today),
   lion_says_yesterday_lied(Today),
   unicorn_says_yesterday_lied(Today),
   write(Today), nl.
