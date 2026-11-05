#!/usr/bin/env bash
# Build browser/ (the Law identifier checks of tests/law_ids*.rs) for
# wasm32-unknown-unknown, the target a browser loads, and run every check
# under node (scripts/run_browser_wasm.mjs). Debug, then release.
set -euo pipefail
cd "$(dirname "$0")/../browser"
for profile in dev release; do
  cargo build --locked --target wasm32-unknown-unknown --profile "$profile"
  dir=$([[ "$profile" == dev ]] && echo debug || echo release)
  node ../scripts/run_browser_wasm.mjs \
    "${CARGO_TARGET_DIR:-target}/wasm32-unknown-unknown/$dir/alice_ephemeris_browser.wasm"
done
