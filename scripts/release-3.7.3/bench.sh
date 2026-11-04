#!/usr/bin/env bash
# Benchmarks for the 3.7.3 release, one at a time (they measure wall time).
# usage: scripts/release-3.7.3/bench.sh <lib dir to test> [<3.7.2 lib dir>] [<3.6.0 lib dir>]
# The lib dirs are `lib/` of a checkout with a RELEASE build (`npm run build`),
# or `node_modules/phyron-skia-canvas` of an npm install. Dev builds render
# about 6x slower and are not comparable.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
lib=$(cd "$1" && pwd)
run() { echo "== $1"; shift; node --expose-gc "$@"; }
cd "$here"
run "F16 loop"                       studioLoopRepro.mjs   "$lib" 30 64
run "8-bit loop"                     studioLoop8bit.mjs    "$lib" 30 64
run "F16 loop, toImage + release"    studioLoopToImage.mjs "$lib" 30 64
SKIA_CANVAS_CACHE_MB=256 run "F16 loop, 256 MB cache" studioLoopRepro.mjs "$lib" 30 64
echo "== orphaned canvases";         node orphans-async.js "$lib" test
if [[ $# -ge 2 ]]; then
  ref=$(cd "$2" && pwd)
  echo "== pixel deltas vs reference";  node deltas.js "$ref" "$lib"
  echo "== rounding probe";             node rounding.js "$lib"
  echo "== F16 store probe";            node f16store.js "$lib"
fi
if [[ $# -ge 3 ]]; then
  run "3.6.0 8-bit loop" studioLoop8bit.mjs "$(cd "$3" && pwd)" 30 64
fi
