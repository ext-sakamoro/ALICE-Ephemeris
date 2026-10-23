#!/usr/bin/env bash
# Build every integration test for wasm32-wasip1 and run each under wasmtime,
# with the repository mapped at its own path (the tests read tests/golden and
# tests/data through CARGO_MANIFEST_DIR). Fails when no test binary was found,
# so an empty run is red rather than green.
set -euo pipefail
cd "$(dirname "$0")/.."
profile_flag=()
profile=debug
if [[ "${1:-}" == "--release" ]]; then
  profile_flag=(--release)
  profile=release
fi
cargo test ${profile_flag[@]+"${profile_flag[@]}"} --target wasm32-wasip1 --no-run
root=$(pwd)
ran=0
for t in oracle_utc oracle_tdb oracle_calendar_jd leap_table law_ids degenerate readme_sync; do
  # the newest build of the test (stale builds with another hash may remain)
  bin=$(ls -t "target/wasm32-wasip1/$profile/deps/${t}"-*.wasm 2>/dev/null | head -1 || true)
  if [[ -z "$bin" ]]; then
    echo "no wasm binary for test $t" >&2
    exit 1
  fi
  echo "wasmtime $bin"
  wasmtime run --dir "$root::$root" "$bin"
  ran=$((ran + 1))
done
echo "ran $ran wasm test binaries"
[[ $ran -gt 0 ]]
