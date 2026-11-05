#!/usr/bin/env bash
# scripts/sky_wasm_build.sh <repo dir> <out.wasm>: the release build of sky/ with the checkout and
# the cargo home remapped out of the binary and the symbols stripped, so it carries no build path
# and the same source gives the same bytes in any directory (scripts/sky_wasm_repro.sh checks it).
set -euo pipefail
repo=$(cd "$1" && pwd -P)
out=$2
cargo_home=$(cd "${CARGO_HOME:-$HOME/.cargo}" && pwd -P)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
(
  cd "$repo"
  CARGO_TARGET_DIR="$work/target" \
  RUSTFLAGS="--remap-path-prefix=$repo=/src --remap-path-prefix=$cargo_home=/cargo -C strip=symbols" \
    cargo build --quiet --locked -p alice-ephemeris-sky --target wasm32-unknown-unknown --profile sky
)
cp "$work/target/wasm32-unknown-unknown/sky/alice_ephemeris_sky.wasm" "$out"
