Tighten scripts/port-audit.sh to enforce the full rule set from docs/design.md §'Port-aware coding rules'.

Checks to add:
- Grep src/ (excluding refvm/) for forbidden patterns: HashMap, BTreeMap, Box<, dyn, async, unsafe, f32, f64.
- Function length: parse src/**/*.rs with a simple awk/python line counter; flag any fn whose body exceeds 50 lines.
- String literal length: grep for quoted strings longer than 120 chars.
- External deps: check Cargo.toml; whitelist is thiserror + anyhow + (for bins) clap. Fail on anything else.

Fix any violations surfaced in the src/ tree (refactor long functions, swap HashMap for Vmap, etc.).

Acceptance: scripts/port-audit.sh passes on a clean tree; fails on known-bad test inputs in scripts/port-audit-tests/.

Commit: 'port-audit: strict enforcement of port-aware rules'.