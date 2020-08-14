#!/usr/bin/env bash
# Re-vendor the xterm.js bundles in crates/termprobe/assets/xtermjs.
#
#   vendor-xtermjs.sh            re-fetch the versions pinned.json already names
#   vendor-xtermjs.sh --latest   bump pinned.json to the latest releases first
#
# The bundles are checked into the repository so that building termprobe needs
# neither Node nor a network. This script is how they are updated.
set -euo pipefail

assets="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/crates/termprobe/assets/xtermjs"
pinned="$assets/pinned.json"

if [ "${1:-}" = "--latest" ]; then
  headless="$(npm view @xterm/headless version)"
