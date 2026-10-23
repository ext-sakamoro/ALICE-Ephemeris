#!/usr/bin/env python3
"""Write tests/data/horizons-sunmoon.csv: the geocentric geometric position
and velocity of the Sun (10) and the Moon (301) from JPL Horizons (DE441,
ICRF, TDB, au and au/day), the independent reference for
tests/oracle_sunmoon.rs.

The file is committed; this script only documents and repeats the query (it
needs the network, so CI does not run it). Horizons answers with the
ephemeris it serves at the time, so a later run may print a newer DE and
differ in the last digits; the tests' tolerances are those of ERFA's models
(kilometres), far above that.

Usage:
    python3 scripts/fetch_horizons_sunmoon.py
"""

import pathlib
import urllib.parse
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = ROOT / "tests" / "data" / "horizons-sunmoon.csv"
API = "https://ssd.jpl.nasa.gov/api/horizons.api"


def dates():
    """TDB Julian dates: 1 January 0h of every fifth year 1900..2100, the two
    Meeus examples (1992 April 12 and October 13, 0h TD) and J2000.0."""
    out = []
    for year in range(1900, 2101, 5):
        # Julian date of 1 January 0h, proleptic Gregorian (Meeus 7.1)
        y, m = year - 1, 13
        a = y // 100
        b = 2 - a + a // 4
        out.append(int(365.25 * (y + 4716)) + int(30.6001 * (m + 1)) + 1 + b - 1524.5)
    return out + [2448724.5, 2448908.5, 2451545.0]


def query(target, tlist):
    params = {
        "format": "text",
        "COMMAND": f"'{target}'",
        "CENTER": "'500@399'",
        "EPHEM_TYPE": "VECTORS",
        "TLIST": " ".join(f"'{t:.1f}'" for t in tlist),
        "TIME_TYPE": "TDB",
        "REF_PLANE": "FRAME",
        "REF_SYSTEM": "ICRF",
        "VEC_CORR": "NONE",
        "OUT_UNITS": "AU-D",
        "CSV_FORMAT": "YES",
        "VEC_TABLE": "2",
    }
    with urllib.request.urlopen(API + "?" + urllib.parse.urlencode(params), timeout=60) as r:
        text = r.read().decode("utf-8")
    source = next(line for line in text.splitlines() if line.startswith("Target body name"))
    body = text[text.rindex("$$SOE") + 5:text.index("$$EOE")]
    rows = []
    for line in body.strip().splitlines():
        f = [x.strip() for x in line.split(",")]
        rows.append([f[0], *f[2:8]])
    if len(rows) != len(tlist):
        raise SystemExit(f"{target}: {len(rows)} rows for {len(tlist)} dates")
    return source.split("{source:")[1].strip(" }"), rows


def main():
    tlist = dates()
    lines = ["# JPL Horizons geometric geocentric states, ICRF, TDB, au and au/day (scripts/fetch_horizons_sunmoon.py)"]
    for target, name in (("10", "sun"), ("301", "moon")):
        ephemeris, rows = query(target, tlist)
        lines.append(f"# {name}: {ephemeris}")
        for r in rows:
            lines.append(",".join([name, *r]))
    lines.insert(1, "body,jd_tdb,x,y,z,vx,vy,vz")
    OUT.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"wrote {OUT}: {2 * len(tlist)} rows")


if __name__ == "__main__":
    main()
