#!/usr/bin/env bash
# Hot-reload dev server on port 4011 (from Trunk.toml).
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
cd "$HERE"
trunk serve
