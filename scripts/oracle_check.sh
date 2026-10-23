#!/usr/bin/env bash
# Reproduce the reference tables from their sources and compare them with the
# committed ones: src/fb1990_table.rs from ERFA v2.0.1 dtdb.c, tests/golden/*.csv
# from ERFA (pyerfa) and CSPICE (spiceypy) with naif0012.tls. Both generators
# check the SHA-256 of their source file and the pinned tool versions.
#
# The tools live in a virtual environment under ORACLE_VENV (default
# ~/.cache/alice-ephemeris-oracle/venv), created from
# scripts/oracle-requirements.txt on first use; the sources are cached next to it.
set -euo pipefail
cd "$(dirname "$0")/.."
cache="${ORACLE_CACHE:-${XDG_CACHE_HOME:-$HOME/.cache}/alice-ephemeris-oracle}"
venv="${ORACLE_VENV:-$cache/venv}"
mkdir -p "$cache/src"
if [[ ! -x "$venv/bin/python" ]]; then
  python3 -m venv "$venv"
  "$venv/bin/python" -m pip install --quiet -r scripts/oracle-requirements.txt
fi
fetch() {
  [[ -s "$cache/src/$1" ]] || curl -fsSL --retry 3 -o "$cache/src/$1" "$2"
}
fetch naif0012.tls https://naif.jpl.nasa.gov/pub/naif/generic_kernels/lsk/naif0012.tls
fetch dtdb.c https://raw.githubusercontent.com/liberfa/erfa/v2.0.1/src/dtdb.c
"$venv/bin/python" -I scripts/gen_fb1990_table.py "$cache/src/dtdb.c" --check
"$venv/bin/python" -I scripts/gen_time_golden.py "$cache/src/naif0012.tls" --check
