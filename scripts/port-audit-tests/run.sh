#!/usr/bin/env bash
# scripts/port-audit-tests/run.sh — verifies port-audit.sh fires on each
# documented violation and stays quiet on the positive control.
#
# Each subdirectory of this folder is a fixture:
#   clean/            must exit 0
#   <everything-else> must exit non-zero, with stdout containing a phrase
#                     returned by the expect_phrase() helper below.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
AUDIT="$(cd "$HERE/.." && pwd)/port-audit.sh"

# Phrase each violation fixture must print. Kept as a case-statement so the
# script runs on macOS's bash 3.2 (no associative arrays).
expect_phrase() {
    case "$1" in
        forbidden-hashmap) echo "HashMap" ;;
        forbidden-box)     echo "Box<T>" ;;
        forbidden-dyn)     echo "dyn trait" ;;
        forbidden-float)   echo "f64" ;;
        forbidden-unsafe)  echo "unsafe" ;;
        long-fn)           echo "fn body" ;;
        long-string)       echo "string literal" ;;
        bad-deps)          echo "forbidden external dep" ;;
        bad-clap-lib)      echo "clap imported outside src/bin/" ;;
        *) echo "" ;;
    esac
}

pass=0
fail=0
for case_dir in "$HERE"/*/; do
    name="$(basename "$case_dir")"
    set +e
    body="$("$AUDIT" "$case_dir" 2>&1)"
    exit_code=$?
    set -e

    if [ "$name" = "clean" ]; then
        if [ "$exit_code" -eq 0 ]; then
            echo "  PASS  clean                 - exit 0 as expected"
            pass=$((pass + 1))
        else
            echo "  FAIL  clean                 - expected exit 0, got $exit_code"
            echo "$body" | sed 's/^/        /'
            fail=$((fail + 1))
        fi
        continue
    fi

    expect="$(expect_phrase "$name")"
    if [ -z "$expect" ]; then
        echo "  SKIP  $name                - no expectation registered"
        continue
    fi

    if [ "$exit_code" -ne 1 ]; then
        echo "  FAIL  $name - expected exit 1, got $exit_code"
        echo "$body" | sed 's/^/        /'
        fail=$((fail + 1))
        continue
    fi
    if ! printf '%s\n' "$body" | grep -qF "$expect"; then
        echo "  FAIL  $name - exit 1 but missing phrase '$expect' in output"
        echo "$body" | sed 's/^/        /'
        fail=$((fail + 1))
        continue
    fi
    printf "  PASS  %-22s- exit 1, matched '%s'\n" "$name" "$expect"
    pass=$((pass + 1))
done

total=$((pass + fail))
echo "port-audit tests: $pass/$total passed"
if [ "$fail" -eq 0 ]; then exit 0; else exit 1; fi
