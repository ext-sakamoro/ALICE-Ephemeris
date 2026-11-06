#!/usr/bin/env python3
"""Write tests/data/star-fixture.csv: written directions (not catalogue stars) through ERFA's
eraAtco13, airless, for tests/oracle_stars.rs. Needs pyerfa (scripts/oracle-requirements.txt).

    python3 scripts/gen_star_golden.py

Author: Moroya Sakamoto
"""

import math
from pathlib import Path

import erfa

ARCSEC = math.pi / 180 / 3600
# ra deg, dec deg, pm ra·cos dec arcsec/yr, pm dec arcsec/yr
STARS = [(100.0, -20.0, -0.5, -1.0), (40.0, 85.0, 0.04, -0.01), (280.0, 40.0, 0.2, 0.3)]
# latitude, longitude degrees
SITES = [(36.0, 140.0), (-33.9, 151.2), (51.5, -0.1), (0.0, 0.0)]
# UTC, in years with 37 leap seconds
TIMES = [(2026, 11, 6, 19, 0, 0.0), (2026, 11, 7, 5, 0, 0.0), (2027, 3, 20, 12, 30, 0.0), (2028, 6, 21, 3, 15, 0.0), (2030, 12, 31, 23, 59, 0.0)]

rows = ["ra_deg,dec_deg,pm_ra_arcsec,pm_dec_arcsec,lat_deg,lon_deg,utc1,utc2,tt1,tt2,azimuth,altitude"]
for ra, dec, pa, pd in STARS:
    for lat, lon in SITES:
        for t in TIMES:
            u1, u2 = erfa.dtf2d("UTC", *t)
            a1, a2 = erfa.utctai(u1, u2)
            t1, t2 = erfa.taitt(a1, a2)
            pr = pa * ARCSEC / math.cos(math.radians(dec))
            aob, zob, *_ = erfa.atco13(
                math.radians(ra), math.radians(dec), pr, pd * ARCSEC, 0.0, 0.0, u1, u2, 0.0,
                math.radians(lon), math.radians(lat), 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.55,
            )
            rows.append(",".join(map(repr, (ra, dec, pa, pd, lat, lon, float(u1), float(u2), float(t1), float(t2), float(aob), math.pi / 2 - float(zob)))))
out = Path(__file__).resolve().parents[1] / "tests" / "data" / "star-fixture.csv"
out.write_text("\n".join(rows) + "\n", encoding="utf-8")
print(f"wrote {len(rows) - 1} rows")
