Ship the demo: examples/liar.pl solves 'The Lion Lies on Tuesdays.'

Puzzle statement (standard): The Lion lies on Mondays, Tuesdays, and Wednesdays, and tells the truth on other days. The Unicorn lies on Thursdays, Fridays, and Saturdays, and tells the truth on other days. Yesterday, both said: 'Yesterday I lied.' What day is today?

Encoding in liar.pl (sketch):
    day(monday). day(tuesday). day(wednesday). day(thursday). day(friday). day(saturday). day(sunday).
    lion_lies(monday). lion_lies(tuesday). lion_lies(wednesday).
    unicorn_lies(thursday). unicorn_lies(friday). unicorn_lies(saturday).
    yesterday(monday, sunday). yesterday(tuesday, monday). yesterday(wednesday, tuesday). ... (all 7)
    lion_told_truth_yesterday_about_yesterday_lying(Today) :-
      yesterday(Today, Yesterday), \\+ lion_lies(Today), lion_lies(Yesterday).
    lion_lied_about_yesterday_lying(Today) :-
      yesterday(Today, Yesterday), lion_lies(Today), \\+ lion_lies(Yesterday).
    lion_says_yesterday_lied(Today) :-
      lion_told_truth_yesterday_about_yesterday_lying(Today) ; lion_lied_about_yesterday_lying(Today).
    (same for unicorn)
    ?- day(Today), lion_says_yesterday_lied(Today), unicorn_says_yesterday_lied(Today), write(Today), nl.

NOTE: if \\+ disjunction (;) is needed, it must be implemented. If that is out of scope for this step, reformulate the puzzle to avoid disjunction (pair of clauses instead of ';'). Document the decision in the commit.

Tests:
- refvm runs liar.pl and prints 'thursday' (standard answer).
- Real VM integration test produces same output.

Acceptance: cargo test passes including the liar test.

Commit: 'liar: Lion Lies on Tuesdays end-to-end'.