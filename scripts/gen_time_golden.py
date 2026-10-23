#!/usr/bin/env python3
"""Generate the reference tables tests/golden/*.csv from independent sources.

None of the expected values comes from this crate: UTC/TAI and TDB-TT
(Fairhead & Bretagnon) from ERFA, the SPICE TDB from CSPICE
through spiceypy with the NAIF leapseconds kernel naif0012.tls, calendar day
numbers from ERFA's eraCal2jd, and Julian-date fractions from exact rational
arithmetic (Python's fractions, rounded once by float()).

The float columns from ERFA (TAI - UTC, TDB - TT) are ERFA v2.0.1's own
expressions evaluated here in IEEE binary64 with correctly rounded sines, so the
tables are the same on every machine: a compiled pyerfa may fuse a·b + c (its
arm64 wheel does), which changes the last bits. pyerfa is run on every row too
and must agree within a bound derived from the arithmetic. The other columns
taken from the compiled libraries are integers or SPICE values compared within
2 ulp; `--check` reproduces every table bit for bit on arm64 and x86_64.

Usage (versions pinned in scripts/oracle-requirements.txt):
    python3 -m venv .venv && .venv/bin/pip install -r scripts/oracle-requirements.txt
    .venv/bin/python -I scripts/gen_time_golden.py <naif0012.tls> <dtdb.c> <dat.c>          # write
    .venv/bin/python -I scripts/gen_time_golden.py <naif0012.tls> <dtdb.c> <dat.c> --check  # compare

naif0012.tls: https://naif.jpl.nasa.gov/pub/naif/generic_kernels/lsk/naif0012.tls
dtdb.c: https://raw.githubusercontent.com/liberfa/erfa/v2.0.1/src/dtdb.c (the series' coefficients
for the 60-digit evaluation and the IEEE one)
dat.c: https://raw.githubusercontent.com/liberfa/erfa/v2.0.1/src/dat.c (eraDat's table, for the
IEEE evaluation of eraDtf2d and eraUtctai)
"""

import datetime
import hashlib
import io
import math
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
DAT_SHA256 = "09e3377ff0c770372c759dc16af12aadc70975d3c549d9524e5c3dd44e24fa88"
# pyerfa's TAI - UTC against the IEEE evaluation: the two-part UTC and TAI keep
# their day fraction (and the day numbers' difference) below 2 days, where one
# rounding is at most 2^-52 day; eraDtf2d, eraDat and eraUtctai round at most
# 8 times on that path in a way a fused multiply-add can change
UTCTAI_BOUND_S = Fraction(8 * 86400, 2**52)
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


def tai_minus_label(a1: float, a2: float, label: int, f: int) -> Fraction:
    """TAI - UTC label (s) from ERFA's two-part TAI, exactly."""
    tai_s = (Fraction(a1) - Fraction(2451545)) * 86400 + Fraction(a2) * 86400
    return tai_s - label - Fraction(f, ATTO)


def utc_tai_csv() -> str:
    out = io.StringIO()
    out.write("# UTC label -> TAI - label (s), from ERFA eraDtf2d + eraUtctai (IEEE binary64, pyerfa within 8 ulp of a day)\n")
    out.write("# outcome: ok | expired (on/after the table expiry; the value is ERFA's, i.e. no new leap)\n")
    out.write("#          | before_utc (before 1960-01-01) | not_a_leap_second (ERFA: time after end of day)\n")
    out.write("y,mo,d,h,mi,s,atto,label_sec,outcome,tai_minus_label\n")
    for y, mo, d, h, mi, s, f in utc_rows():
        date = datetime.date(y, mo, d)
        label = label_seconds(date, h, mi, s)
        with warnings.catch_warnings(record=True) as w:
            warnings.simplefilter("always")
            p1, p2 = erfa.dtf2d("UTC", y, mo, d, h, mi, s + f / ATTO)
            after_end = any("after end of day" in str(x.message) for x in w)
        if date < datetime.date(1960, 1, 1):
            outcome, value = "before_utc", ""
        else:
            u1, u2, ours_after_end = ieee_dtf2d_utc(y, mo, d, h, mi, s + f / ATTO)
            if ours_after_end != after_end:
                sys.exit(f"{y}-{mo}-{d} {h}:{mi}:{s}: end of day differs from pyerfa")
            if after_end:
                outcome, value = "not_a_leap_second", ""
            else:
                a1, a2 = ieee_utctai(u1, u2)
                with warnings.catch_warnings():
                    warnings.simplefilter("ignore")
                    q1, q2 = erfa.utctai(p1, p2)
                # TAI - UTC label: (a1 - u1) + (a2 - u2) days, then the label's own
                # seconds within the day are those of ERFA's 2-part UTC (u2 holds them)
                ours = tai_minus_label(a1, a2, label, f)
                theirs = tai_minus_label(float(q1), float(q2), label, f)
                if not abs(ours - theirs) <= UTCTAI_BOUND_S:
                    sys.exit(f"{y}-{mo}-{d} {h}:{mi}:{s}: pyerfa {float(theirs)!r}"
                             f" vs the IEEE evaluation {float(ours)!r}")
                STATS["utctai"] = max(STATS.get("utctai", 0), abs(ours - theirs))
                value = repr(float(ours))
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


# ---------------------------------------------------------------------------
# ERFA's expressions in IEEE binary64, each operation rounded once
#
# A pyerfa wheel is compiled C, and the compiler may contract a·b + c into one
# fused multiply-add (the arm64 wheel of pyerfa 2.0.1.5 does): its float results
# then depend on the machine that generated the table. The float columns that
# come from ERFA are therefore evaluated here, from ERFA v2.0.1's own source
# expressions in Python floats (IEEE binary64, round to nearest, no fused
# operation), with every sine correctly rounded (mpmath), so the same table comes
# out on every machine. pyerfa itself is still run on every row and must agree
# within a bound derived from the arithmetic (`check_against_pyerfa`).

DAT = {}  # filled in main(): "changes" [(year, month, delat)], "drift" [(mjd0, rate)]


def load_dat(path: pathlib.Path) -> None:
    """eraDat's table as printed in ERFA v2.0.1 dat.c."""
    src = path.read_bytes()
    if hashlib.sha256(src).hexdigest() != DAT_SHA256:
        sys.exit("dat.c does not have the pinned sha256")
    text = src.decode("utf-8")
    drift = text[text.index("drift[][2] = {"):]
    drift = drift[: drift.index("};")]
    changes = text[text.index("_changes[] = {"):]
    changes = changes[: changes.index("};")]
    num = r"([-+0-9.eE]+)"
    DAT["drift"] = [(float(a), float(b)) for a, b in re.findall(r"\{\s*" + num + r"\s*,\s*" + num + r"\s*\}", drift)]
    DAT["changes"] = [(int(y), int(m), float(v)) for y, m, v in
                      re.findall(r"\{\s*(\d+)\s*,\s*(\d+)\s*,\s*" + num + r"\s*\}", changes)]
    if len(DAT["drift"]) != 14 or len(DAT["changes"]) != 42 or DAT["changes"][-1][:2] != (2017, 1):
        sys.exit("could not read the table of dat.c")


def c_div(a: int, b: int) -> int:
    """C's integer division (truncation toward zero)."""
    q = abs(a) // abs(b)
    return q if (a >= 0) == (b >= 0) else -q


def dnint(a: float) -> float:
    """ERFA_DNINT."""
    if abs(a) < 0.5:
        return 0.0
    return float(math.ceil(a - 0.5)) if a < 0.0 else float(math.floor(a + 0.5))


def ieee_cal2jd(iy: int, im: int, id_: int) -> tuple:
    my = c_div(im - 14, 12)
    iypmy = iy + my
    djm = float(c_div(1461 * (iypmy + 4800), 4) + c_div(367 * (im - 2 - 12 * my), 12)
                - c_div(3 * c_div(iypmy + 4900, 100), 4) + id_ - 2432076)
    return 2400000.5, djm


def ieee_jd2cal(dj1: float, dj2: float) -> tuple:
    d = dnint(dj1)
    f1 = dj1 - d
    jd = int(d)
    d = dnint(dj2)
    f2 = dj2 - d
    jd += int(d)
    s, cs = 0.5, 0.0
    for x in (f1, f2):
        t = s + x
        cs += (s - t) + x if abs(s) >= abs(x) else (x - t) + s
        s = t
        if s >= 1.0:
            jd += 1
            s -= 1.0
    f = s + cs
    cs = f - s
    if f < 0.0:
        f = s + 1.0
        cs += (1.0 - f) + s
        s = f
        f = s + cs
        cs = f - s
        jd -= 1
    eps = sys.float_info.epsilon
    if (f - 1.0) >= -eps / 4.0:
        t = s - 1.0
        cs += (s - t) - 1.0
        s = t
        f = s + cs
        if -eps / 2.0 < f:
            jd += 1
            f = max(f, 0.0)
    l = jd + 68569
    n = c_div(4 * l, 146097)
    l -= c_div(146097 * n + 3, 4)
    i = c_div(4000 * (l + 1), 1461001)
    l -= c_div(1461 * i, 4) - 31
    k = c_div(80 * l, 2447)
    id_ = l - c_div(2447 * k, 80)
    l = c_div(k, 11)
    im = k + 2 - 12 * l
    iy = 100 * (n - 49) + i + l
    return iy, im, id_, f


def ieee_dat(iy: int, im: int, id_: int, fd: float) -> float:
    """eraDat for a valid date from 1960 on (the rows the table needs)."""
    _, djm = ieee_cal2jd(iy, im, id_)
    m = 12 * iy + im
    changes = DAT["changes"]
    i = max(k for k, (y, mo, _) in enumerate(changes) if m >= 12 * y + mo)
    da = changes[i][2]
    if i < len(DAT["drift"]):
        d0, rate = DAT["drift"][i]
        da += (djm + fd - d0) * rate
    return da


def ieee_dtf2d_utc(iy: int, im: int, id_: int, ihr: int, imn: int, sec: float) -> tuple:
    """eraDtf2d("UTC", ...): (d1, d2, after_end_of_day)."""
    dj, w = ieee_cal2jd(iy, im, id_)
    dj += w
    day = 86400.0
    seclim = 60.0
    dat0 = ieee_dat(iy, im, id_, 0.0)
    dat12 = ieee_dat(iy, im, id_, 0.5)
    iy2, im2, id2, _ = ieee_jd2cal(dj, 1.5)
    dat24 = ieee_dat(iy2, im2, id2, 0.0)
    dleap = dat24 - (2.0 * dat12 - dat0)
    day += dleap
    if ihr == 23 and imn == 59:
        seclim += dleap
    time = (60.0 * float(60 * ihr + imn) + sec) / day
    return dj, time, sec >= seclim


def ieee_utctai(utc1: float, utc2: float) -> tuple:
    big1 = abs(utc1) >= abs(utc2)
    u1, u2 = (utc1, utc2) if big1 else (utc2, utc1)
    iy, im, id_, fd = ieee_jd2cal(u1, u2)
    dat0 = ieee_dat(iy, im, id_, 0.0)
    dat12 = ieee_dat(iy, im, id_, 0.5)
    iyt, imt, idt, _ = ieee_jd2cal(u1 + 1.5, u2 - fd)
    dat24 = ieee_dat(iyt, imt, idt, 0.0)
    dlod = 2.0 * (dat12 - dat0)
    dleap = dat24 - (dat0 + dlod)
    fd *= (86400.0 + dleap) / 86400.0
    fd *= (86400.0 + dlod) / 86400.0
    z1, z2 = ieee_cal2jd(iy, im, id_)
    a2 = z1 - u1
    a2 += z2
    a2 += fd + dat0 / 86400.0
    return (u1, a2) if big1 else (a2, u1)


def sin_cr(x: float) -> float:
    """sin(x) correctly rounded to binary64 (x is exact in mpmath)."""
    with mpmath.workprec(256):
        return float(mpmath.sin(mpmath.mpf(x)))


def ieee_dtdb(date1: float, date2: float) -> float:
    """eraDtdb(date1, date2, 0, 0, 0, 0): no topocentric terms (u = v = 0
    makes each of them +0 or -0, and their sum +0)."""
    t = ((date1 - 2451545.0) + date2) / 365250.0
    terms, blocks = FB["terms"], FB["blocks"]
    w = []
    for k in range(5):
        acc = 0.0
        for a, f, p in reversed(terms[blocks[k]:blocks[k + 1]]):
            acc += a * sin_cr(f * t + p)
        w.append(acc)
    wf = t * (t * (t * (t * w[4] + w[3]) + w[2]) + w[1]) + w[0]
    j = FB["jpl"]
    wj = (j[0][0] * sin_cr(j[0][1] * t + j[0][2]) + j[1][0] * sin_cr(j[1][1] * t + j[1][2])
          + j[2][0] * sin_cr(j[2][1] * t + j[2][2]) + j[3][0] * sin_cr(j[3][1] * t + j[3][2])
          + FB["t2"] * t * t)
    return 0.0 + wf + wj


def fb1990_rounding_bound(t: float) -> float:
    """The bound on the rounding error of an f64 evaluation of the series at t
    (the same derivation as in tests/oracle_tdb.rs: sine within 2 ulp, each
    product and sum rounded once, Horner for the powers of t). Two evaluations
    that each obey it are within twice of it of each other."""
    u = 2.0 ** -53
    at = abs(t)
    terms, blocks = FB["terms"], FB["blocks"]
    total = horner = size = 0.0
    for k in range(5):
        e = w = 0.0
        for a, f, p in terms[blocks[k]:blocks[k + 1]]:
            ft = abs(f * t)
            e += abs(a) * (ft * 5.0 * u + (ft + abs(p)) * u + 5.0 * u)
            w += abs(a)
        e += (blocks[k + 1] - blocks[k]) * u * w
        tk = at ** k
        total += tk * (e + k * 4.0 * u * w)
        horner += w * tk
        size += w * tk
    total += 8.0 * u * horner
    ej = 0.0
    for a, f, p in FB["jpl"]:
        ft = abs(f * t)
        ej += abs(a) * (ft * 5.0 * u + (ft + abs(p)) * u + 5.0 * u)
        size += abs(a)
    c = abs(FB["t2"]) * at * at
    ej += c * 10.0 * u + 5.0 * u * c
    size += c
    return total + ej + u * size


def dtdb_row(sec: int) -> str:
    """TDB - TT at whole TT second `sec`: ERFA's expression in IEEE binary64,
    checked against pyerfa within twice the rounding bound."""
    jd1 = 2451545.0 + (sec // 86400)
    jd2 = float(Fraction(sec % 86400, 86400))
    ours = ieee_dtdb(jd1, jd2)
    theirs = float(erfa.dtdb(jd1, jd2, 0.0, 0.0, 0.0, 0.0))
    t = ((jd1 - 2451545.0) + jd2) / 365250.0
    if not abs(ours - theirs) <= 2.0 * fb1990_rounding_bound(t):
        sys.exit(f"pyerfa dtdb at {sec}: {theirs!r} vs the IEEE evaluation {ours!r}")
    STATS["dtdb"] = max(STATS.get("dtdb", 0.0), abs(ours - theirs) / fb1990_rounding_bound(t))
    return repr(ours)


STATS = {}


def tdb_fb1990_csv() -> str:
    out = io.StringIO()
    out.write("# TT (whole s since J2000 TT) -> TDB - TT (s), ERFA eraDtdb(jd1, jd2, 0, 0, 0, 0) (IEEE binary64,\n")
    out.write("# correctly rounded sines, pyerfa within twice the rounding bound),\n")
    out.write("# and the same series in 60-digit arithmetic at the exact t (mpmath)\n")
    out.write("tt_sec,tdb_minus_tt,exact\n")
    for sec in tt_grid(1600, 2200, 1201):
        out.write(f"{sec},{dtdb_row(sec)},{fb1990_exact(sec)}\n")
    return out.getvalue()


def tdb_fb1990_far_csv() -> str:
    """Far from J2000, where the polynomial terms bring |TDB - TT| to 1 s."""
    out = io.StringIO()
    out.write("# TT (whole s since J2000 TT, years x 365.25 days) -> TDB - TT (s), ERFA eraDtdb (IEEE binary64,\n")
    out.write("# correctly rounded sines, pyerfa within twice the rounding bound), far from J2000,\n")
    out.write("# and the same series in 60-digit arithmetic at the exact t (mpmath)\n")
    out.write("years,tt_sec,tdb_minus_tt,exact\n")
    years = list(range(-200_000, 200_001, 2_500)) + [-136_400, -131_600, -131_500, 130_000, 147_100, 147_200]
    for yr in sorted(set(years)):
        sec = yr * 36525 * 864  # yr * 365.25 * 86400
        out.write(f"{yr},{sec},{dtdb_row(sec)},{fb1990_exact(sec)}\n")
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
        "# no column depends on the machine that generated it (--check reproduces it on arm64 and x86_64)\n"
    )


def main() -> None:
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    if len(sys.argv) < 4 or any(a.startswith("--") for a in sys.argv[1:4]):
        sys.exit(__doc__)
    kernel = pathlib.Path(sys.argv[1])
    load_dtdb(pathlib.Path(sys.argv[2]))
    load_dat(pathlib.Path(sys.argv[3]))
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
    check = "--check" in sys.argv[4:]
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
    print(f"pyerfa vs IEEE: dtdb within {STATS['dtdb']:.3f} of the rounding bound,"
          f" TAI - UTC within {float(STATS['utctai']):.3e} s")
    if bad:
        sys.exit(1)


if __name__ == "__main__":
    main()
