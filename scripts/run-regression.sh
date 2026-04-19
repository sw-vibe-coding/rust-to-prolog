#!/usr/bin/env bash
# scripts/run-regression.sh — CLI regression harness for prologc demos.
#
# Runs every `r2p_*` baseline under reg-rs/ and reports pass/fail.
# Each baseline captures stdout+stderr of one `examples/*.pl` run through
# the full tokenize → parse → compile → emit → asm → refvm pipeline.
#
# Usage:
#   scripts/run-regression.sh           # summary + failure details
#   scripts/run-regression.sh -vv       # full diffs on failure
#   scripts/run-regression.sh -q        # exit code only
#
# Rebaseline after an intentional output change:
#   REG_RS_DATA_DIR="$(pwd)/reg-rs" reg-rs rebase -p r2p_<name>
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"

cd "$ROOT"

if ! command -v reg-rs &>/dev/null; then
    echo "ERROR: reg-rs not found in PATH." >&2
    echo "Install from https://github.com/softwarewrighter/reg-rs" >&2
    exit 1
fi

cargo build --quiet --bin prologc

export REG_RS_DATA_DIR="$ROOT/reg-rs"
exec reg-rs run -p r2p_ --parallel "$@"
