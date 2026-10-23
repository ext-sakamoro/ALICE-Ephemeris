#!/usr/bin/env bash
# Reproduce the reference tables from their sources and compare them with the
# committed ones: src/fb1990_table.rs from ERFA v2.0.1 dtdb.c, tests/golden/*.csv
# from ERFA (its expressions in IEEE binary64, and pyerfa within a derived bound)
# and CSPICE (spiceypy) with naif0012.tls. Both generators check the SHA-256 of
# their source files and the pinned tool versions.
#
# The tools live in a virtual environment under ORACLE_VENV (default
# ~/.cache/alice-ephemeris-oracle/venv-<hash of the requirements>), created from
# scripts/oracle-requirements.txt on first use; the sources are cached next to it.
set -euo pipefail
cd "$(dirname "$0")/.."
cache="${ORACLE_CACHE:-${XDG_CACHE_HOME:-$HOME/.cache}/alice-ephemeris-oracle}"
# one environment per requirements file content, so a changed pin builds a new one
req=$(python3 -c 'import hashlib,sys;print(hashlib.sha256(open(sys.argv[1],"rb").read()).hexdigest()[:16])' scripts/oracle-requirements.txt)
venv="${ORACLE_VENV:-$cache/venv-$req}"
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
fetch dat.c https://raw.githubusercontent.com/liberfa/erfa/v2.0.1/src/dat.c
fetch nut00a.c https://raw.githubusercontent.com/liberfa/erfa/v2.0.1/src/nut00a.c
fetch s06.c https://raw.githubusercontent.com/liberfa/erfa/v2.0.1/src/s06.c
fetch epv00.c https://raw.githubusercontent.com/liberfa/erfa/v2.0.1/src/epv00.c
fetch moon98.c https://raw.githubusercontent.com/liberfa/erfa/v2.0.1/src/moon98.c
"$venv/bin/python" -I scripts/gen_fb1990_table.py "$cache/src/dtdb.c" --check
"$venv/bin/python" -I scripts/gen_time_golden.py "$cache/src/naif0012.tls" "$cache/src/dtdb.c" "$cache/src/dat.c" --check
"python3" -I scripts/gen_eop_golden.py --check
"$venv/bin/python" -I scripts/gen_frames_golden.py --check
"$venv/bin/python" -I scripts/gen_iau2006_tables.py "$cache/src/nut00a.c" "$cache/src/s06.c" --check
"$venv/bin/python" -I scripts/gen_cio_golden.py "$cache/src/nut00a.c" "$cache/src/s06.c" --check
"$venv/bin/python" -I scripts/gen_sunmoon_tables.py "$cache/src/epv00.c" "$cache/src/moon98.c" --check
"$venv/bin/python" -I scripts/gen_sunmoon_golden.py "$cache/src/epv00.c" "$cache/src/moon98.c" --check
