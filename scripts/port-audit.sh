#!/usr/bin/env bash
# scripts/port-audit.sh — enforces docs/design.md §"Port-aware coding rules"
# so src/ stays mechanically portable to SNOBOL4 / PL/SW.
#
# Scope: src/ tree under the given root (default: repo root). The Rust-only
# reference VM at src/refvm/ and all #[cfg(test)] modules / #[test] fns are
# exempt — they don't ship in the port.
#
# Checks:
#   1. Forbidden identifiers: HashMap, BTreeMap, Box<, dyn, async, unsafe,
#      f32, f64. Any hit = fail.
#   2. String literals: any literal > 120 chars in production code fails.
#   3. Function length: any non-test fn body > 50 lines fails.
#   4. External crates: [dependencies] in Cargo.toml must be a subset of
#      { thiserror, anyhow, clap }. `clap` is further restricted to
#      src/bin/ — any other src/ file importing it fails.
#
# Usage:
#   scripts/port-audit.sh               # audit the repo
#   scripts/port-audit.sh <root>        # audit a directory (used by tests)
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="${1:-$(cd "$HERE/.." && pwd)}"

cd "$ROOT"

FAIL=0
SRC="src"
if [ ! -d "$SRC" ]; then
    echo "port-audit: no src/ directory under $ROOT" >&2
    exit 2
fi

# 1. Forbidden identifier patterns in src/ (excluding src/refvm/).
check_forbidden() {
    local pattern="$1" label="$2"
    local hits
    hits=$(grep -rnE --include='*.rs' "$pattern" "$SRC" 2>/dev/null \
        | grep -v "^${SRC}/refvm/" || true)
    if [ -n "$hits" ]; then
        echo "port-audit: forbidden $label:"
        echo "$hits" | sed 's/^/  /'
        FAIL=1
    fi
}

check_forbidden '\bHashMap\b'      'HashMap (use port::Vmap)'
check_forbidden '\bBTreeMap\b'     'BTreeMap (use port::Vmap)'
check_forbidden '\bBox<'           'Box<T> (no heap allocation for AST)'
check_forbidden '\bdyn [A-Z_]'     'dyn trait object (static dispatch only)'
check_forbidden '\basync +(fn|\{)' 'async fn/block'
check_forbidden '\bunsafe +(fn|\{|impl)' 'unsafe'
check_forbidden '\bf32\b'          'f32 (integers only)'
check_forbidden '\bf64\b'          'f64 (integers only)'

# 2. String literals > 120 chars.
STRING_HITS=$(grep -rnE --include='*.rs' '"[^"]{121,}"' "$SRC" 2>/dev/null \
    | grep -v "^${SRC}/refvm/" || true)
if [ -n "$STRING_HITS" ]; then
    echo "port-audit: string literal > 120 chars:"
    echo "$STRING_HITS" | sed 's/^/  /'
    FAIL=1
fi

# 3. Function body > 50 lines (non-test, non-refvm).
FN_REPORT=$(python3 - "$SRC" <<'PY'
import os, re, sys
src = sys.argv[1]
bad = []
for dirpath, _, files in os.walk(src):
    if dirpath.startswith(os.path.join(src, "refvm")):
        continue
    for fn in files:
        if not fn.endswith(".rs"):
            continue
        path = os.path.join(dirpath, fn)
        lines = open(path).readlines()
        i, n, in_tests = 0, len(lines), 0
        while i < n:
            line = lines[i]
            stripped = line.strip()
            # Enter #[cfg(test)] mod tests — skip whole module.
            if stripped.startswith("#[cfg(test)]"):
                j = i + 1
                while j < n and lines[j].strip() == "":
                    j += 1
                if j < n and re.match(r'\s*mod\s+\w+\s*\{', lines[j]):
                    depth = 0
                    k = j
                    while k < n:
                        depth += lines[k].count("{") - lines[k].count("}")
                        if depth == 0 and "{" in lines[k]:
                            # single-line — done.
                            break
                        if depth == 0:
                            break
                        k += 1
                    i = k + 1
                    continue
            # Skip #[test] fns.
            if stripped == "#[test]":
                # advance past attrs to the fn line
                j = i + 1
                while j < n and lines[j].strip().startswith("#["):
                    j += 1
                if j < n and re.match(r'\s*(pub\s+)?fn\s+', lines[j]):
                    # walk until matching close brace.
                    depth = 0
                    k = j
                    opened = False
                    while k < n:
                        depth += lines[k].count("{") - lines[k].count("}")
                        if "{" in lines[k]:
                            opened = True
                        if opened and depth == 0:
                            break
                        k += 1
                    i = k + 1
                    continue
            # Ordinary fn signature.
            if re.match(r'\s*(pub(\(.+?\))?\s+)?(const\s+)?fn\s+', line):
                start = i
                depth = 0
                opened = False
                while i < n:
                    depth += lines[i].count("{") - lines[i].count("}")
                    if "{" in lines[i]:
                        opened = True
                    if opened and depth == 0:
                        break
                    i += 1
                body = i - start - 1
                if body > 50:
                    bad.append((path, start + 1, body))
            i += 1

for p, ln, body in bad:
    print(f"{p}:{ln}: fn body {body} lines (> 50)")
sys.exit(1 if bad else 0)
PY
) && FN_OK=1 || FN_OK=0
if [ "$FN_OK" = "0" ]; then
    echo "port-audit: function body > 50 lines:"
    echo "$FN_REPORT" | sed 's/^/  /'
    FAIL=1
fi

# 4. External deps whitelist. thiserror, anyhow, clap only.
if [ -f Cargo.toml ]; then
    DEPS_REPORT=$(python3 - <<'PY'
import re, sys
text = open("Cargo.toml").read()
whitelist = {"thiserror", "anyhow", "clap"}
# grab every [dependencies] or [dev-dependencies] block in the top-level
# package. dev-dependencies is fine to loosen — it only affects tests —
# so we only enforce on [dependencies].
m = re.search(r'^\[dependencies\]\s*\n(.*?)(?=^\[|\Z)', text, re.DOTALL | re.MULTILINE)
deps = []
if m:
    for line in m.group(1).splitlines():
        line = line.split("#", 1)[0].strip()
        if not line or "=" not in line:
            continue
        name = line.split("=", 1)[0].strip()
        deps.append(name)
bad = sorted(set(d for d in deps if d not in whitelist))
for d in bad:
    print(f"Cargo.toml [dependencies]: {d} not in whitelist (thiserror, anyhow, clap)")
sys.exit(1 if bad else 0)
PY
) && DEP_OK=1 || DEP_OK=0
    if [ "$DEP_OK" = "0" ]; then
        echo "port-audit: forbidden external dep:"
        echo "$DEPS_REPORT" | sed 's/^/  /'
        FAIL=1
    fi
fi

# clap confined to src/bin/ (never library code).
CLAP_LIB=$(grep -rnE --include='*.rs' '^\s*use\s+clap(::|$|;)' "$SRC" 2>/dev/null \
    | grep -v "^${SRC}/bin/" || true)
if [ -n "$CLAP_LIB" ]; then
    echo "port-audit: clap imported outside src/bin/ (library code must not depend on clap):"
    echo "$CLAP_LIB" | sed 's/^/  /'
    FAIL=1
fi

if [ "$FAIL" -eq 0 ]; then
    echo "port-audit: clean"
fi
exit $FAIL
