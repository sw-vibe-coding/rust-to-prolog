#!/usr/bin/env bash
# scripts/run-tests.sh — the single green/red signal for the saga.
#
# Runs the fast test suite plus the port audit. Integration tests that
# shell out to `cor24-run` are gated behind `--full` (step 009 onward).
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"

cd "$ROOT"
cargo test --quiet
"$HERE/port-audit.sh"

if [[ "${1:-}" == "--full" ]]; then
    cargo test --quiet -- --ignored
fi
