#!/usr/bin/env bash
# scripts/sky_wasm_build.sh <repo dir> <out.wasm>: the release build of sky/ with the checkout and
# the cargo home remapped out of the binary and the symbols stripped, so it carries no build path
# and the same source gives the same bytes in any directory (scripts/sky_wasm_repro.sh checks it).
set -euo pipefail
repo=$(cd "$1" && pwd -P)
out=$2
cargo_home=$(cd "${CARGO_HOME:-$HOME/.cargo}" && pwd -P)
rust_home=$(cd "$repo" && rustc --print sysroot)
# with the rust-src component installed, the standard library's paths are its files in the sysroot;
# without it they are /rustc/<commit>. Name them the second way either way, or the bytes depend on
# which components this machine has
rust_commit=$(cd "$repo" && rustc -vV | sed -n 's/^commit-hash: //p')
[ -n "$rust_commit" ] || { echo "rustc gave no commit hash" >&2; exit 2; }
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
(
  cd "$repo"
  CARGO_TARGET_DIR="$work/target" \
  RUSTFLAGS="--remap-path-prefix=$repo=/src --remap-path-prefix=$cargo_home=/cargo --remap-path-prefix=$rust_home=/rust --remap-path-prefix=$rust_home/lib/rustlib/src/rust=/rustc/$rust_commit -C strip=symbols" \
    cargo build --quiet --locked -p alice-ephemeris-sky --target wasm32-unknown-unknown --profile sky
)
cp "$work/target/wasm32-unknown-unknown/sky/alice_ephemeris_sky.wasm" "$out"
