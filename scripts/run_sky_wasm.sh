#!/usr/bin/env bash
# Build sky/ (the one export a web page loads, sun_alt_az) for wasm32-unknown-unknown as a page
# gets it and check it: scripts/sky_wasm_build.sh; under node (scripts/run_sky_wasm.mjs) only
# sun_alt_az and memory exported, the native bits over sky/expected.txt, the Sun overhead at the
# subsolar point of the 2026 March equinox; no build path in the bytes (scripts/wasm_path_scan.py).
set -euo pipefail
cd "$(dirname "$0")/.."
wasm=$(mktemp -d)/sky.wasm
scripts/sky_wasm_build.sh . "$wasm"
node scripts/run_sky_wasm.mjs "$wasm" sky/expected.txt
python3 scripts/wasm_path_scan.py "$wasm"
