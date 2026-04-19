#!/usr/bin/env bash
# Dev build — trunk compiles the Yew app to WASM and emits dist/.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
cd "$HERE"
trunk build
