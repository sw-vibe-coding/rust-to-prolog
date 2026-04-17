Refresh docs/demo-plan-status.md to reflect rust-to-prolog saga state.

The existing file is a stale snapshot of the upstream SNOBOL4 project (sw-cor24-prolog); it references SNOBOL4 assembler step 013, lam_asm.sno, etc. — none of which belong to this Rust port.

Rewrite it for rust-to-prolog:
- TL;DR of where the saga is: 7 of 17 steps complete (scaffold/port-helpers/parse/tokenize/compile-ancestor/emit-lam/lamasm); next is 008-refvm-core; 98 unit + 2 integration tests green; byte-parity with lam_asm.py and the per-spec ancestor.lam fixture confirmed.
- Demo menu by step dependency: ancestor query (after 008 or 009), color backtracking (after 010), member/2 (after 011), arithmetic (after 012), liar puzzle (after 015).
- SNOBOL4/PL/SW port is NOT this saga. Steps 016-017 prep for it; the port itself is a downstream saga.
- Single progress signal: scripts/run-tests.sh pass count + port-audit clean + .lam byte-diff clean (when step 009 lands).
- Drop the stale upstream content (run-tests.sh tiers, compile-pl.sh, SNOBOL4 SB-overflow discussion).

Keep the doc terse — status page, not a retrospective.

Commit: 'docs: refresh demo-plan-status.md for rust-to-prolog saga'.