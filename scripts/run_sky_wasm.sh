#!/usr/bin/env bash
# Build sky/ (the one export a web page loads, sun_alt_az) for wasm32-unknown-unknown and check it
# under node (scripts/run_sky_wasm.mjs): only sun_alt_az and memory exported, the native bits over
# sky/expected.txt, the Sun overhead at the subsolar point of the 2026 March equinox. Release only
# (the page loads the release build); prints its size.
set -euo pipefail
cd "$(dirname "$0")/../sky"
cargo build --locked --target wasm32-unknown-unknown --release
node ../scripts/run_sky_wasm.mjs \
  "${CARGO_TARGET_DIR:-target}/wasm32-unknown-unknown/release/alice_ephemeris_sky.wasm" \
  expected.txt
