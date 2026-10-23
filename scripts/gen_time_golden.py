#!/usr/bin/env python3
"""Generate the reference tables tests/golden/*.csv from independent sources.

None of the expected values comes from this crate: UTC/TAI and TDB-TT
(Fairhead & Bretagnon) from ERFA through pyerfa, the SPICE TDB from CSPICE
through spiceypy with the NAIF leapseconds kernel naif0012.tls, calendar day
numbers from ERFA's eraCal2jd, and Julian-date fractions from exact rational
arithmetic (Python's fractions, rounded once by float()).

Usage (versions pinned in scripts/oracle-requirements.txt):
    python3 -m venv .venv && .venv/bin/pip install -r scripts/oracle-requirements.txt
    .venv/bin/python -I scripts/gen_time_golden.py <naif0012.tls> <dtdb.c>          # write
    .venv/bin/python -I scripts/gen_time_golden.py <naif0012.tls> <dtdb.c> --check  # compare

naif0012.tls: https://naif.jpl.nasa.gov/pub/naif/generic_kernels/lsk/naif0012.tls
dtdb.c: https://raw.githubusercontent.com/liberfa/erfa/v2.0.1/src/dtdb.c (the series' coefficients
for the 60-digit evaluation)
"""

import datetime
import hashlib
import io
import pathlib
import sys
import warnings
from fractions import Fraction

import erfa
import mpmath
import numpy
import re
import spiceypy

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = ROOT / "tests" / "golden"
NAIF0012_SHA256 = "678e32bdb5a744117a467cd9601cd6b373f0e9bc9bbde1371d5eee39600a039b"
PINS = {"pyerfa": "2.0.1.5", "spiceypy": "8.3.0", "numpy": "2.5.4", "mpmath": "1.4.1", "cspice": "CSPICE_N0067"}
DTDB_SHA256 = "228bd7c7c1c88dda44c29a076f289c968f64d77a2dc389138b03ab5d1d442105"
# the leap-second table's expiry ("File expires on 28 June 2027")
EXPIRES = datetime.date(2027, 6, 28)
J2000_DATE = datetime.date(2000, 1, 1)
ATTO = 10**18


def label_seconds(d: datetime.date, h: int, m: int, s: int) -> int:
    """Seconds of the label `d h:m:s` since 2000-01-01T12:00:00, 86400 s per day."""
    return (d - J2000_DATE).days * 86400 - 43200 + h * 3600 + m * 60 + s


def leap_days() -> list:
    """Last UTC day before each change of TAI-UTC, from leap-seconds.list."""
    days = []
    for line in (ROOT / "tests" / "data" / "leap-seconds.list").read_text(encoding="utf-8").splitlines():
        if line and not line.startswith("#"):
            ntp = int(line.split()[0])
            d = datetime.date(1900, 1, 1) + datetime.timedelta(days=ntp // 86400)
            if d.year > 1972 or (d.year == 1972 and d.month == 7):
                days.append(d - datetime.timedelta(days=1))
    return days


# changes of 1960-1971 (first day of each row, ERFA dat.c / USNO tai-utc.dat) and 1972-01-01
PRE_1972_STARTS = [
    (1961, 1), (1961, 8), (1962, 1), (1963, 11), (1964, 1), (1964, 4), (1964, 9), (1965, 1),
    (1965, 3), (1965, 7), (1965, 9), (1966, 1), (1968, 2), (1972, 1),
]


def utc_rows() -> list:
    """(y, mo, d, h, mi, s, atto) of the UTC labels to test."""
    rows = []
    fracs = [0, 250_000_000_000_000_000, 500_000_000_000_000_000, 999_999_999_000_000_000]
    for day in leap_days():
        for s, f in [(58, 0), (59, 0), (59, fracs[1]), (60, 0), (60, fracs[2]), (60, fracs[3])]:
            rows.append((day.year, day.month, day.day, 23, 59, s, f))
        nxt = day + datetime.timedelta(days=1)
        rows.append((nxt.year, nxt.month, nxt.day, 0, 0, 0, 0))
    for y, mo in PRE_1972_STARTS:
        start = datetime.date(y, mo, 1)
        prev = start - datetime.timedelta(days=1)
        for s, f in [(59, 0), (59, fracs[2]), (59, 900_000_000_000_000_000),
                     (59, 949_000_000_000_000_000), (60, 0), (60, 50_000_000_000_000_000),
                     (60, 100_000_000_000_000_000)]:
            rows.append((prev.year, prev.month, prev.day, 23, 59, s, f))
        rows.append((y, mo, 1, 0, 0, 0, 0))
        rows.append((y, mo, 1, 12, 0, 0, 0))
    d = datetime.date(1960, 1, 1)
    k = 0
    while d < datetime.date(2028, 1, 1):
        rows.append((d.year, d.month, d.day, (7 * k) % 24, (13 * k) % 60, (29 * k) % 60, fracs[k % 4]))
        d += datetime.timedelta(days=37)
        k += 1
    rows.append((1959, 12, 31, 12, 0, 0, 0))
    rows.append((2016, 12, 30, 23, 59, 60, 0))
    rows.append((2027, 6, 27, 23, 59, 59, fracs[3]))
    rows.append((2027, 6, 28, 0, 0, 0, 0))
    return rows


def utc_tai_csv() -> str:
    out = io.StringIO()
    out.write("# UTC label -> TAI - label (s), from ERFA eraDtf2d + eraUtctai\n")
    out.write("# outcome: ok | expired (on/after the table expiry; the value is ERFA's, i.e. no new leap)\n")
    out.write("#          | before_utc (before 1960-01-01) | not_a_leap_second (ERFA: time after end of day)\n")
    out.write("y,mo,d,h,mi,s,atto,label_sec,outcome,tai_minus_label\n")
    for y, mo, d, h, mi, s, f in utc_rows():
        date = datetime.date(y, mo, d)
        label = label_seconds(date, h, mi, s)
        with warnings.catch_warnings(record=True) as w:
            warnings.simplefilter("always")
            u1, u2 = erfa.dtf2d("UTC", y, mo, d, h, mi, s + f / ATTO)
            after_end = any("after end of day" in str(x.message) for x in w)
        if date < datetime.date(1960, 1, 1):
            outcome, value = "before_utc", ""
        elif after_end:
            outcome, value = "not_a_leap_second", ""
        else:
            with warnings.catch_warnings():
                warnings.simplefilter("ignore")
                a1, a2 = erfa.utctai(u1, u2)
            # TAI - UTC label: (a1 - u1) + (a2 - u2) days, then the label's own
            # seconds within the day are those of ERFA's 2-part UTC (u2 holds them)
            tai_s = (Fraction(float(a1)) - Fraction(2451545)) * 86400 + Fraction(float(a2)) * 86400
            value = repr(float(tai_s - label - Fraction(f, ATTO)))
            outcome = "expired" if date >= EXPIRES else "ok"
        out.write(f"{y},{mo},{d},{h},{mi},{s},{f},{label},{outcome},{value}\n")
    return out.getvalue()


def tai_utc_csv() -> str:
    """TAI instants -> UTC label (ERFA eraTaiutc + eraD2dtf, nanoseconds)."""
    out = io.StringIO()
    out.write("# TAI (s since J2000 TAI, integer + attoseconds) -> UTC label, ERFA eraTaiutc + eraD2dtf(9)\n")
    out.write("tai_sec,tai_atto,y,mo,d,h,mi,s,ns\n")
    points = []
    for day in leap_days():
        nxt = day + datetime.timedelta(days=1)
        base = label_seconds(nxt, 0, 0, 0)
        off = int(round(erfa.dat(nxt.year, nxt.month, nxt.day, 0.0)))
        for dt in [-1.75, -1.25, -0.75, -0.25, 0.25]:
            t = Fraction(base + off) + Fraction(dt)
            points.append(t)
    for k in range(400):
        # spread over 1960-2027, quarter-second offsets keep d2dtf away from rounding ties
        t = Fraction(-1_262_260_000 + k * 5_300_000 + (k * 7919) % 86400) + Fraction(1, 4)
        points.append(t)
    for t in points:
        sec = t.numerator // t.denominator
        atto = int((t - sec) * ATTO)
        jd1 = 2451545.0 + (sec // 86400)
        jd2 = float(Fraction(sec % 86400) / 86400 + Fraction(atto, ATTO * 86400))
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            u1, u2 = erfa.taiutc(jd1, jd2)
            y, mo, d, hmsf = erfa.d2dtf("UTC", 9, u1, u2)
        h, mi, s, ns = (int(hmsf["h"]), int(hmsf["m"]), int(hmsf["s"]), int(hmsf["f"]))
        out.write(f"{sec},{atto},{int(y)},{int(mo)},{int(d)},{h},{mi},{s},{ns}\n")
    return out.getvalue()


def tt_grid(start_year: int, end_year: int, n: int) -> list:
    lo = label_seconds(datetime.date(start_year, 1, 1), 0, 0, 0)
    hi = label_seconds(datetime.date(end_year, 1, 1), 0, 0, 0)
    pts = [lo + (hi - lo) * k // (n - 1) for k in range(n)]
    pts += [0, 1, -1, 1000, -1000, 43200]
    return pts


FB = {}  # filled in main(): "terms" (787 x 3), "blocks", "jpl" (4 x 3), "t2"


def load_dtdb(path: pathlib.Path) -> None:
    """The series' coefficients as printed in ERFA v2.0.1 dtdb.c (the same
    source as src/fb1990_table.rs), each read as the nearest f64, which is
    what the crate evaluates with."""
    src = path.read_bytes()
    if hashlib.sha256(src).hexdigest() != DTDB_SHA256:
        sys.exit("dtdb.c does not have the pinned sha256")
    text = src.decode("utf-8")
    body = text[text.index("fairhd[787][3] = {"):]
    body = body[: body.index("};")]
    num = r"([-+0-9.eE]+)"
    terms = [tuple(float(x) for x in m) for m in re.findall(r"\{\s*" + num + r"\s*,\s*" + num + r"\s*,\s*" + num + r"\s*\}", body)]
    if len(terms) != 787:
        sys.exit(f"{len(terms)} terms")
    wj = text[text.index("wj ="): text.index("t * t;") + 6]
    jpl = [tuple(float(x) for x in m) for m in re.findall(r"\(?\s*" + num + r"\s*\*\s*sin\(\s*" + num + r"\s*\*\s*t\s*\+\s*" + num + r"\)", wj)]
    t2 = re.findall(num + r"\s*\*\s*t\s*\*\s*t", wj)
    if len(jpl) != 4 or len(t2) != 1:
        sys.exit("could not read the JPL adjustment of dtdb.c")
    FB.update(terms=terms, blocks=[0, 474, 679, 764, 784, 787], jpl=jpl, t2=float(t2[0]))


def fb1990_exact(sec: int) -> str:
    """The same 787-term series evaluated in 60-digit arithmetic at the exact
    t = sec / (86400 * 365250): what the f64 evaluation approximates."""
    mpmath.mp.dps = 60
    m = mpmath.mpf
    t = m(sec) / m(86400 * 365250)
    terms, blocks = FB["terms"], FB["blocks"]
    total = m(0)
    for k in range(5):
        w = m(0)
        for a, f, p in terms[blocks[k]:blocks[k + 1]]:
            w += m(a) * mpmath.sin(m(f) * t + m(p))
        total += w * t**k
    for a, f, p in FB["jpl"]:
        total += m(a) * mpmath.sin(m(f) * t + m(p))
    total += m(FB["t2"]) * t * t
    return mpmath.nstr(total, 30, strip_zeros=False)


def tdb_fb1990_csv() -> str:
    out = io.StringIO()
    out.write("# TT (whole s since J2000 TT) -> TDB - TT (s), ERFA eraDtdb(jd1, jd2, 0, 0, 0, 0),\n")
    out.write("# and the same series in 60-digit arithmetic at the exact t (mpmath)\n")
    out.write("tt_sec,tdb_minus_tt,exact\n")
    for sec in tt_grid(1600, 2200, 1201):
        jd1 = 2451545.0 + (sec // 86400)
        jd2 = float(Fraction(sec % 86400, 86400))
        out.write(f"{sec},{repr(float(erfa.dtdb(jd1, jd2, 0.0, 0.0, 0.0, 0.0)))},{fb1990_exact(sec)}\n")
    return out.getvalue()


def tdb_fb1990_far_csv() -> str:
    """Far from J2000, where the polynomial terms bring |TDB - TT| to 1 s."""
    out = io.StringIO()
    out.write("# TT (whole s since J2000 TT, years x 365.25 days) -> TDB - TT (s), ERFA eraDtdb, far from J2000,\n")
    out.write("# and the same series in 60-digit arithmetic at the exact t (mpmath)\n")
    out.write("years,tt_sec,tdb_minus_tt,exact\n")
    years = list(range(-200_000, 200_001, 2_500)) + [-136_400, -131_600, -131_500, 130_000, 147_100, 147_200]
    for yr in sorted(set(years)):
        sec = yr * 36525 * 864  # yr * 365.25 * 86400
        jd1 = 2451545.0 + (sec // 86400)
        jd2 = float(Fraction(sec % 86400, 86400))
        out.write(f"{yr},{sec},{repr(float(erfa.dtdb(jd1, jd2, 0.0, 0.0, 0.0, 0.0)))},{fb1990_exact(sec)}\n")
    return out.getvalue()


def tdb_spice_csv() -> str:
    out = io.StringIO()
    out.write("# TT seconds (exact f64) -> SPICE unitim TDT->TDB, and the same value read as TDB -> unitim TDB->TDT\n")
    out.write("tt_sec,tdb_from_tt,tt_from_tdb_of_same_number\n")
    for sec in tt_grid(1900, 2100, 401):
        x = float(sec)
        out.write(f"{sec},{repr(float(spiceypy.unitim(x, 'TDT', 'TDB')))},{repr(float(spiceypy.unitim(x, 'TDB', 'TDT')))}\n")
    return out.getvalue()


def calendar_csv() -> str:
    out = io.StringIO()
    out.write("# Gregorian date -> MJD, ERFA eraCal2jd (djm0 + djm - 2400000.5)\n")
    out.write("y,mo,d,mjd\n")
    k = 0
    for y in list(range(-4700, 3001, 97)) + [1582, 1600, 1700, 1900, 2000, 2024, 2100]:
        for mo, d in [(1, 1), (2, 28), (2, 29 if (y % 4 == 0 and y % 100 != 0) or y % 400 == 0 else 28),
                      (3, 1), (12, 31), (1 + k % 12, 1 + (k * 7) % 28)]:
            djm0, djm = erfa.cal2jd(y, mo, d)
            out.write(f"{y},{mo},{d},{int(round(float(djm0) - 2400000.5 + float(djm)))}\n")
            k += 1
    return out.getvalue()


def jd_csv() -> str:
    out = io.StringIO()
    out.write("# instant (s + attoseconds since J2000) -> jd1, jd2 (exact rational fraction, rounded once)\n")
    out.write("sec,atto,jd1,jd2\n")
    for k in range(300):
        sec = -10**10 + k * 66_666_667 + (k * 104729) % 86400
        atto = (k * 3_141_592_653_589_793_238) % ATTO
        jd1 = 2451545 + sec // 86400
        frac = (Fraction(sec % 86400) + Fraction(atto, ATTO)) / 86400
        out.write(f"{sec},{atto},{float(jd1)!r},{float(frac)!r}\n")
    return out.getvalue()


def seconds_f64_csv() -> str:
    """Instant -> the f64 nearest to sec + atto/1e18 (exact rational, rounded once)."""
    import random
    rng = random.Random(20261023)
    out = io.StringIO()
    out.write("# instant (s + attoseconds since J2000) -> nearest f64 of the exact value (fractions, float())\n")
    out.write("sec,atto,nearest\n")
    secs = [0, 1, -1, 2**53, -(2**53), 2**53 + 1, -(2**53) - 1, 2**62, -(2**62), 2**63 - 1, -(2**63)]
    secs += [rng.randrange(-(2**63), 2**63) for _ in range(300)]
    secs += [-rng.randrange(2**53, 2**63) for _ in range(300)]
    secs += [rng.randrange(-(10**10), 10**10) for _ in range(300)]
    for sec in secs:
        for atto in (0, 1, 500_000_000_000_000_000, ATTO - 1, rng.randrange(ATTO)):
            v = float(Fraction(sec) + Fraction(atto, ATTO))
            out.write(f"{sec},{atto},{v!r}\n")
    return out.getvalue()


def header(kernel: pathlib.Path) -> str:
    iana = hashlib.sha256((ROOT / "tests" / "data" / "leap-seconds.list").read_bytes()).hexdigest()
    return (
        f"# generated by scripts/gen_time_golden.py; pyerfa {erfa.__version__} (ERFA {erfa.version.erfa_version}),"
        f" spiceypy {spiceypy.__version__} ({spiceypy.tkvrsn('TOOLKIT')}), numpy {numpy.__version__}\n"
        f"# naif0012.tls sha256 {hashlib.sha256(kernel.read_bytes()).hexdigest()}; leap-seconds.list sha256 {iana}\n"
    )


def main() -> None:
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    if len(sys.argv) < 3 or sys.argv[2].startswith("--"):
        sys.exit(__doc__)
    kernel = pathlib.Path(sys.argv[1])
    load_dtdb(pathlib.Path(sys.argv[2]))
    if hashlib.sha256(kernel.read_bytes()).hexdigest() != NAIF0012_SHA256:
        sys.exit("naif0012.tls does not have the pinned sha256")
    got = {"pyerfa": erfa.__version__, "spiceypy": spiceypy.__version__, "numpy": numpy.__version__,
           "mpmath": mpmath.__version__,
           "cspice": spiceypy.tkvrsn("TOOLKIT")}
    if got != PINS:
        sys.exit(f"versions {got} != pinned {PINS}")
    spiceypy.furnsh(str(kernel))
    files = {
        "utc_tai.csv": utc_tai_csv(),
        "tai_utc.csv": tai_utc_csv(),
        "tdb_fb1990.csv": tdb_fb1990_csv(),
        "tdb_fb1990_far.csv": tdb_fb1990_far_csv(),
        "tdb_spice.csv": tdb_spice_csv(),
        "calendar.csv": calendar_csv(),
        "jd.csv": jd_csv(),
        "seconds_f64.csv": seconds_f64_csv(),
    }
    check = "--check" in sys.argv[3:]
    bad = 0
    for name, body in files.items():
        text = header(kernel) + body
        path = OUT / name
        if check:
            if not path.exists() or path.read_text(encoding="utf-8") != text:
                print(f"{name}: differs from the generator output")
                bad += 1
        else:
            OUT.mkdir(parents=True, exist_ok=True)
            path.write_text(text, encoding="utf-8", newline="\n")
        print(f"{name}: {body.count(chr(10)) - body.count(chr(10) + '#') - 1} rows")
    if bad:
        sys.exit(1)


if __name__ == "__main__":
    main()
