#!/usr/bin/env python3
"""Generate tests/golden/cio.csv: IAU 2006/2000A precession-nutation, the CIO
locator s and the celestial-to-terrestrial matrix from ERFA (pyerfa).

None of the expected values comes from this crate. Each row lists its
two-part Julian dates with their exact bits.

Usage (versions pinned in scripts/oracle-requirements.txt):
    .venv/bin/python -I scripts/gen_cio_golden.py           # write
    .venv/bin/python -I scripts/gen_cio_golden.py --check   # compare
"""

import math
import pathlib
import sys

import erfa
import numpy

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = ROOT / "tests" / "golden" / "cio.csv"
ARGS_OUT = ROOT / "tests" / "golden" / "cio_args.csv"
PINS = {"pyerfa": "2.0.1.5", "numpy": "2.5.4"}


def dates():
    """TT two-part dates 1800..2200 every 211 days plus a varying fraction,
    a few far ones, and both orders of the parts."""
    out = []
    k = 0
    day = -73050
    while day <= 73050:
        out.append((2451545.0, day + (k * 0.381966011250105) % 1.0))
        day += 211
        k += 1
    out += [(2400000.5, 53736.0), (2400000.5, 50123.9999), (53736.0, 2400000.5)]
    return out


# ERFA 2.0.1 constants and the polynomial parts of eraNut00a, eraS06 (eraFa*03),
# eraPfw06 and eraObl06, evaluated one IEEE operation at a time (CPython
# rounds each operation, math.fmod is exact): the bit reference for the
# crate's arguments and precession angles. pyerfa is a cross-check only (its
# build may fuse a multiply and an add).
D2PI = 6.283185307179586476925287
DAS2R = 4.848136811095359935899141e-6
TURNAS = 1296000.0
DJ00 = 2451545.0
DJC = 36525.0


def centuries(jd1, jd2):
    return ((jd1 - DJ00) + jd2) / DJC


def fal03(t):
    return math.fmod(485868.249036 + t * (1717915923.2178 + t * (31.8792 + t * (0.051635 + t * (-0.00024470)))), TURNAS) * DAS2R


def falp03(t):
    return math.fmod(1287104.793048 + t * (129596581.0481 + t * (-0.5532 + t * (0.000136 + t * (-0.00001149)))), TURNAS) * DAS2R


def faf03(t):
    return math.fmod(335779.526232 + t * (1739527262.8478 + t * (-12.7512 + t * (-0.001037 + t * (0.00000417)))), TURNAS) * DAS2R


def fad03(t):
    return math.fmod(1072260.703692 + t * (1602961601.2090 + t * (-6.3706 + t * (0.006593 + t * (-0.00003169)))), TURNAS) * DAS2R


def faom03(t):
    return math.fmod(450160.398036 + t * (-6962890.5431 + t * (7.4722 + t * (0.007702 + t * (-0.00005939)))), TURNAS) * DAS2R


def fame03(t): return math.fmod(4.402608842 + 2608.7903141574 * t, D2PI)
def fave03(t): return math.fmod(3.176146697 + 1021.3285546211 * t, D2PI)
def fae03(t): return math.fmod(1.753470314 + 628.3075849991 * t, D2PI)
def fama03(t): return math.fmod(6.203480913 + 334.0612426700 * t, D2PI)
def faju03(t): return math.fmod(0.599546497 + 52.9690962641 * t, D2PI)
def fasa03(t): return math.fmod(0.874016757 + 21.3299104960 * t, D2PI)
def faur03(t): return math.fmod(5.481293872 + 7.4781598567 * t, D2PI)
def fapa03(t): return (0.024381750 + 0.00000538691 * t) * t


def nut00a_arguments(t):
    """eraNut00a: luni-solar l, l' (MHB2000), F, D (MHB2000), Om; planetary."""
    el = fal03(t)
    elp = math.fmod(1287104.79305 + t * (129596581.0481 + t * (-0.5532 + t * (0.000136 + t * (-0.00001149)))), TURNAS) * DAS2R
    f = faf03(t)
    d = math.fmod(1072260.70369 + t * (1602961601.2090 + t * (-6.3706 + t * (0.006593 + t * (-0.00003169)))), TURNAS) * DAS2R
    om = faom03(t)
    al = math.fmod(2.35555598 + 8328.6914269554 * t, D2PI)
    af = math.fmod(1.627905234 + 8433.466158131 * t, D2PI)
    ad = math.fmod(5.198466741 + 7771.3771468121 * t, D2PI)
    aom = math.fmod(2.18243920 - 33.757045 * t, D2PI)
    alne = math.fmod(5.321159000 + 3.8127774000 * t, D2PI)
    pl = [al, af, ad, aom, fame03(t), fave03(t), fae03(t), fama03(t), faju03(t), fasa03(t), faur03(t), alne, fapa03(t)]
    return [el, elp, f, d, om], pl


def s06_arguments(t):
    return [fal03(t), falp03(t), faf03(t), fad03(t), faom03(t), fave03(t), fae03(t), fapa03(t)]


def obl06(t):
    return (84381.406 + (-46.836769 + (-0.0001831 + (0.00200340 + (-0.000000576 + (-0.0000000434) * t) * t) * t) * t) * t) * DAS2R


def pfw06(t):
    gamb = (-0.052928 + (10.556378 + (0.4932044 + (-0.00031238 + (-0.000002788 + (0.0000000260) * t) * t) * t) * t) * t) * DAS2R
    phib = (84381.412819 + (-46.811016 + (0.0511268 + (0.00053289 + (-0.000000440 + (-0.0000000176) * t) * t) * t) * t) * t) * DAS2R
    psib = (-0.041775 + (5038.481484 + (1.5584175 + (-0.00018522 + (-0.000026452 + (-0.0000000148) * t) * t) * t) * t) * t) * DAS2R
    return [gamb, phib, psib, obl06(t)]


def cross_check(t, jd1, jd2):
    """The transcription against pyerfa: within 8 ulp of the unreduced value
    (the polynomial before fmod), which a fused multiply-add may round
    differently; a wrong coefficient or term is far outside."""
    eps = 2.220446049250313e-16
    at = abs(t)
    delaunay = (1.3e6 + 1.74e9 * at) * DAS2R
    planet = 7.0 + 8434.0 * at
    pairs = [(fal03(t), erfa.fal03(t), delaunay), (falp03(t), erfa.falp03(t), delaunay),
             (faf03(t), erfa.faf03(t), delaunay), (fad03(t), erfa.fad03(t), delaunay),
             (faom03(t), erfa.faom03(t), delaunay), (fame03(t), erfa.fame03(t), planet),
             (fave03(t), erfa.fave03(t), planet), (fae03(t), erfa.fae03(t), planet),
             (fama03(t), erfa.fama03(t), planet), (faju03(t), erfa.faju03(t), planet),
             (fasa03(t), erfa.fasa03(t), planet), (faur03(t), erfa.faur03(t), planet),
             (fapa03(t), erfa.fapa03(t), 0.03 * at), (obl06(t), erfa.obl06(jd1, jd2), 0.5)]
    pairs += list(zip(pfw06(t), erfa.pfw06(jd1, jd2), [0.03 * at + 1e-6, 0.5, 0.03 * at + 1e-6, 0.5]))
    for k, (mine, ref, mag) in enumerate(pairs):
        e = abs(mine - float(ref))
        e = min(e, D2PI - e)
        if e > 8 * eps * mag:
            sys.exit(f"transcription {k} at t={t}: {mine!r} vs {float(ref)!r} ({e:e} > {8 * eps * mag:e})")


def flat(m):
    return [float(v) for v in numpy.asarray(m).reshape(9)]


def main():
    got = {"pyerfa": erfa.__version__, "numpy": numpy.__version__}
    if got != PINS:
        sys.exit(f"tool versions {got} differ from the pins {PINS}")
    lines = [
        "# generated by scripts/gen_cio_golden.py (pyerfa 2.0.1.5)",
        "jd1,jd2,dpsi00a,deps00a,dpsi06a,deps06a,gamb,phib,psib,epsa,s06,"
        + ",".join(f"pnm{i}" for i in range(9)) + ","
        + ",".join(f"c2i{i}" for i in range(9)) + ","
        + "era_jd2,xp,yp," + ",".join(f"c2t{i}" for i in range(9)),
    ]
    for k, (jd1, jd2) in enumerate(dates()):
        dp0, de0 = erfa.nut00a(jd1, jd2)
        dp6, de6 = erfa.nut06a(jd1, jd2)
        gamb, phib, psib, epsa = erfa.pfw06(jd1, jd2)
        pnm = erfa.pnm06a(jd1, jd2)
        x, y = erfa.bpn2xy(pnm)
        s = erfa.s06(jd1, jd2, x, y)
        c2i = erfa.c2i06a(jd1, jd2)
        # UT1 ≈ TT − 69 s; pole coordinates of the size of the real ones
        ut2 = jd2 - 69.184 / 86400.0
        xp = 1.5e-6 * numpy.sin(0.3 * k)
        yp = 2.0e-6 * numpy.cos(0.2 * k)
        c2t = erfa.c2t06a(jd1, jd2, jd1, ut2, xp, yp)
        vals = [jd1, jd2, dp0, de0, dp6, de6, gamb, phib, psib, epsa, s, *flat(pnm), *flat(c2i),
                ut2, xp, yp, *flat(c2t)]
        lines.append(",".join(repr(float(v)) for v in vals))
    text = "\n".join(lines) + "\n"
    args = [
        "# generated by scripts/gen_cio_golden.py: ERFA's argument and precession polynomials in IEEE operations",
        "jd1,jd2," + ",".join(f"ls{i}" for i in range(5)) + "," + ",".join(f"pl{i}" for i in range(13)) + ","
        + ",".join(f"s{i}" for i in range(8)) + ",gamb,phib,psib,epsa",
    ]
    for jd1, jd2 in dates():
        t = centuries(jd1, jd2)
        cross_check(t, jd1, jd2)
        ls, pl = nut00a_arguments(t)
        vals = [jd1, jd2, *ls, *pl, *s06_arguments(t), *pfw06(t)]
        args.append(",".join(repr(float(v)) for v in vals))
    args_text = "\n".join(args) + "\n"
    if "--check" in sys.argv:
        if ARGS_OUT.read_text(encoding="utf-8") != args_text:
            sys.exit(f"{ARGS_OUT} differs from the regenerated table")
        if OUT.read_text(encoding="utf-8") != text:
            sys.exit(f"{OUT} differs from the regenerated table")
        print(f"{OUT}: {len(lines) - 2} rows match")
    else:
        OUT.write_text(text, encoding="utf-8")
        ARGS_OUT.write_text(args_text, encoding="utf-8")
        print(f"wrote {OUT}: {len(lines) - 2} rows")


if __name__ == "__main__":
    main()
