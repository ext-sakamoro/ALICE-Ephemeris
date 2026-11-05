#!/usr/bin/env bash
# The module a page loads must not depend on where it was built: build the committed tree of sky/
# in two directories at different depths and require the same bytes (and no build path in them).
set -euo pipefail
cd "$(dirname "$0")/.."
root=$(cd "$(mktemp -d)" && pwd -P)
trap 'rm -rf "$root"' EXIT
mkdir -p "$root/a" "$root/deeper/path/b"
for d in "$root/a" "$root/deeper/path/b"; do
  git archive HEAD | tar -x -C "$d"
  scripts/sky_wasm_build.sh "$d" "$d/sky.wasm"
done
cmp "$root/a/sky.wasm" "$root/deeper/path/b/sky.wasm"
python3 scripts/wasm_path_scan.py "$root/a/sky.wasm" "$root/deeper/path/b/sky.wasm"
echo "sky module: the same bytes in 2 directories ($(wc -c < "$root/a/sky.wasm" | tr -d ' ') bytes)"
