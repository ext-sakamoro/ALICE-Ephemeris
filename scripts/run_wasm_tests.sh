#!/usr/bin/env bash
# Build every integration test (tests/*.rs) for wasm32-wasip1 and run each
# under wasmtime, with the repository mapped at its own path (the tests read
# tests/golden and tests/data through CARGO_MANIFEST_DIR). The test binaries
# are taken from cargo's own build output (so CARGO_TARGET_DIR is honoured),
# and the run fails unless every tests/*.rs file produced one binary that ran:
# a new test file cannot be skipped on wasm, and an empty run is red.
set -euo pipefail
cd "$(dirname "$0")/.."
flags=(--target wasm32-wasip1 --tests --no-run --message-format=json)
if [[ "${1:-}" == "--release" ]]; then
  flags+=(--release)
fi
root=$(pwd)
expected=$(find tests -maxdepth 1 -name '*.rs' | wc -l | tr -d ' ')
# name and executable of each integration test target, from cargo's JSON
bins=$(cargo test "${flags[@]}" | python3 -c '
import json, sys
for line in sys.stdin:
    try:
        m = json.loads(line)
    except ValueError:
        continue
    t = m.get("target", {})
    if m.get("reason") == "compiler-artifact" and m.get("executable") \
            and t.get("kind") == ["test"] and "/tests/" in t.get("src_path", ""):
        print(t["name"], m["executable"])
')
ran=0
while read -r name bin; do
  [[ -z "${name:-}" ]] && continue
  echo "wasmtime $name ($bin)"
  wasmtime run --dir "$root::$root" "$bin"
  ran=$((ran + 1))
done <<< "$bins"
echo "ran $ran of $expected integration test files under wasmtime"
if [[ "$ran" -eq 0 || "$ran" -ne "$expected" ]]; then
  echo "expected one wasm run per tests/*.rs file" >&2
  exit 1
fi
