#!/usr/bin/env python3
"""Write tests/data/horizons-sky.csv: JPL Horizons observer tables for the
Sun (10) and the Moon (301): airless apparent azimuth and elevation at four
sites, and the Moon's geocentric Sun-Target-Observer angle and illuminated
fraction and whether it leads or trails the Sun. The independent reference
for tests/oracle_horizon.rs.

The instants are UTC Julian dates inside the blocks of
tests/data/finals2000A-excerpt.txt, so the tests take UT1 and the pole
coordinates from the same IERS rows Horizons uses. The file is committed;
this script only documents and repeats the query (it needs the network, so CI
does not run it).

Usage:
    python3 scripts/fetch_horizons_sky.py
"""

import pathlib
import urllib.parse
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = ROOT / "tests" / "data" / "horizons-sky.csv"
API = "https://ssd.jpl.nasa.gov/api/horizons.api"

# name, east longitude (deg), geodetic latitude (deg), height (km)
SITES = [
    ("tokyo", 139.6917, 35.6895, 0.040),
    ("equator", 0.0, 0.0, 0.0),
    ("tromso", 18.9553, 69.6496, 0.010),
    ("sydney", 151.2093, -33.8688, 0.058),
]


def instants():
    """UTC Julian dates: MJD day + a fraction that walks through the day,
    inside the EOP blocks 1973-01, 2016-12/2017-01 (across the leap second),
    2026-10 and 2027-01 (predicted rows)."""
    days = [41687, 41691, 57745, 57749, 57753, 57756, 57760, 57764,
            61323, 61327, 61331, 61336, 61341, 61346, 61408, 61413, 61418, 61422]
    return [2400000.5 + d + ((k * 0.381966011250105) % 1.0) for k, d in enumerate(days)]


def query(params):
    base = {"format": "text", "EPHEM_TYPE": "OBSERVER", "TIME_TYPE": "UT", "ANG_FORMAT": "DEG",
            "EXTRA_PREC": "YES", "CSV_FORMAT": "YES", "CAL_FORMAT": "JD"}
    base.update(params)
    with urllib.request.urlopen(API + "?" + urllib.parse.urlencode(base), timeout=60) as r:
        text = r.read().decode("utf-8")
    source = next(line for line in text.splitlines() if line.startswith("Target body name"))
    body = text[text.rindex("$$SOE") + 5:text.index("$$EOE")]
    rows = [[x.strip() for x in line.split(",")] for line in body.strip().splitlines()]
    return source.split("{source:")[1].strip(" }"), rows


def main():
    tlist = " ".join(f"'{t:.9f}'" for t in instants())
    lines = ["# JPL Horizons observer tables, UTC Julian dates (scripts/fetch_horizons_sky.py)",
             "kind,body,site,jd_utc,a,b,c"]
    for target, name in (("10", "sun"), ("301", "moon")):
        for site, lon, lat, h in SITES:
            eph, rows = query({"COMMAND": f"'{target}'", "CENTER": "'coord@399'", "COORD_TYPE": "GEODETIC",
                               "SITE_COORD": f"'{lon},{lat},{h}'", "QUANTITIES": "'4'",
                               "APPARENT": "AIRLESS", "TLIST": tlist})
            if len(rows) != len(instants()):
                raise SystemExit(f"{name} {site}: {len(rows)} rows")
            for r in rows:
                # JD, solar presence, lunar presence, azimuth, elevation
                lines.append(f"azel,{name},{site},{r[0]},{r[3]},{r[4]},")
        lines.append(f"# {name} az/el: {eph}")
    eph, rows = query({"COMMAND": "'301'", "CENTER": "'500@399'", "QUANTITIES": "'10,23,24'", "TLIST": tlist})
    for r in rows:
        # JD, flags, illuminated fraction (%), S-O-T angle (deg), /T trails or /L
        # leads the Sun, S-T-O angle (deg)
        lines.append(f"phase,moon,geocentre,{r[0]},{r[3]},{r[6]},{r[5]}")
    lines.append(f"# moon phase: {eph}")
    OUT.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"wrote {OUT}: {len(lines) - 2} lines")


if __name__ == "__main__":
    main()
