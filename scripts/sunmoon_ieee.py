"""ERFA 2.0.1 eraEpv00 and eraMoon98 evaluated one IEEE binary64 operation at
a time, with correctly rounded sines and cosines (scripts/erfa_ieee.py).

The bit reference for tests/golden/sunmoon.csv. The coefficient tables are read
from ERFA's epv00.c and moon98.c with the parser of gen_sunmoon_tables.py
(their SHA-256 pinned there); the expressions and their order follow the C.
"""

import math
import pathlib
import re
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import erfa_ieee as E  # noqa: E402
import gen_sunmoon_tables as G  # noqa: E402

DJ00 = 2451545.0
DJY = 365.25
DJC = 36525.0
DD2R = 1.745329251994329576923691e-2
DAU = 149597870.7e3

EPV = {}
MOON_LR = []
MOON_B = []


def load(epv00_c, moon98_c):
    """The tables of epv00.c and moon98.c (literals parsed by Python)."""
    epv = G.source(epv00_c, G.EPV00_SHA256)
    for name in G.EPV_ARRAYS:
        nums = [float(x) for x in re.findall(G.NUM, G.block(epv, f"static const double {name}[] = {{"))]
        EPV[name] = [nums[i:i + 3] for i in range(0, len(nums), 3)]
    moon = G.source(moon98_c, G.MOON98_SHA256)
    for row in re.findall(r"\{([^{}]*)\}", G.block(moon, "static struct termlr tlr[] = {")):
        f = [x.strip() for x in row.split(",")]
        MOON_LR.append(([int(x) for x in f[:4]], float(f[4]), float(f[5])))
    for row in re.findall(r"\{([^{}]*)\}", G.block(moon, "static struct termb tb[] = {")):
        f = [x.strip() for x in row.split(",")]
        MOON_B.append(([int(x) for x in f[:4]], float(f[4])))
    assert {k: len(v) for k, v in EPV.items()} == G.EPV_COUNTS
    assert (len(MOON_LR), len(MOON_B)) == (60, 60)


def epv_t(jd1, jd2):
    return ((jd1 - DJ00) + jd2) / DJY


def epv00(jd1, jd2):
    """(pvh, pvb) as eraEpv00 returns them (BCRS axes, au and au/day)."""
    t = epv_t(jd1, jd2)
    t2 = t * t
    ph, vh, pb, vb = [], [], [], []
    for c in "xyz":
        xyz = 0.0
        xyzd = 0.0
        for model, outp, outv in (("e", ph, vh), ("s", pb, vb)):
            for power in (0, 1, 2):
                for a, b, cc in EPV[f"{model}{power}{c}"]:
                    if power == 0:
                        p = b + cc * t
                        xyz += a * E.cos_cr(p)
                        xyzd -= a * cc * E.sin_cr(p)
                    elif power == 1:
                        ct = cc * t
                        p = b + ct
                        cp = E.cos_cr(p)
                        xyz += a * t * cp
                        xyzd += a * (cp - ct * E.sin_cr(p))
                    else:
                        ct = cc * t
                        p = b + ct
                        cp = E.cos_cr(p)
                        xyz += a * t2 * cp
                        xyzd += a * t * (2.0 * cp - ct * E.sin_cr(p))
            outp.append(xyz)
            outv.append(xyzd / DJY)

    def rot(v):
        x, y, z = v
        return [x + 0.000000211284 * y + -0.000000091603 * z,
                -0.000000230286 * x + 0.917482137087 * y + -0.397776982902 * z,
                0.397776982902 * y + 0.917482137087 * z]

    return [rot(ph), rot(vh)], [rot(pb), rot(vb)]


def _arg(c, t):
    angle = DD2R * math.fmod(c[0] + (c[1] + (c[2] + (c[3] + c[4] * t) * t) * t) * t, 360.0)
    rate = DD2R * (c[1] + (c[2] * 2.0 + (c[3] * 3.0 + c[4] * 4.0 * t) * t) * t)
    return angle, rate


def moon_t(jd1, jd2):
    return ((jd1 - DJ00) + jd2) / DJC


def moon98(jd1, jd2):
    """[p, v] as eraMoon98 returns them (GCRS, au and au/day)."""
    t = moon_t(jd1, jd2)
    elp, delp = _arg([218.31665436, 481267.88123421, -0.0015786, 1.0 / 538841.0, -1.0 / 65194000.0], t)
    d, dd = _arg([297.8501921, 445267.1114034, -0.0018819, 1.0 / 545868.0, 1.0 / 113065000.0], t)
    em, dem = _arg([357.5291092, 35999.0502909, -0.0001536, 1.0 / 24490000.0, 0.0], t)
    emp, demp = _arg([134.9633964, 477198.8675055, 0.0087414, 1.0 / 69699.0, -1.0 / 14712000.0], t)
    f, df = _arg([93.2720950, 483202.0175233, -0.0036539, 1.0 / 3526000.0, 1.0 / 863310000.0], t)
    al1, al2, al3 = 0.003958, 0.001962, 0.000318
    a1 = DD2R * (119.75 + 131.849 * t)
    da1 = DD2R * al1
    a2 = DD2R * (53.09 + 479264.290 * t)
    da2 = DD2R * 479264.290
    a3 = DD2R * (313.45 + 481266.484 * t)
    da3 = DD2R * 481266.484
    e1, e2 = -0.002516, -0.0000074
    e = 1.0 + (e1 + e2 * t) * t
    de = e1 + 2.0 * e2 * t
    esq = e * e
    desq = 2.0 * e * de
    S, C = E.sin_cr, E.cos_cr
    elpmf = elp - f
    delpmf = delp - df
    vel = al1 * S(a1) + al2 * S(elpmf) + al3 * S(a2)
    vdel = al1 * C(a1) * da1 + al2 * C(elpmf) * delpmf + al3 * C(a2) * da2
    vr = 0.0
    vdr = 0.0
    a1mf = a1 - f
    da1mf = da1 - df
    a1pf = a1 + f
    da1pf = da1 + df
    dlpmp = elp - emp
    slpmp = elp + emp
    ab = [-0.002235, 0.000382, 0.000175, 0.000175, 0.000127, -0.000115]
    vb = ab[0] * S(elp) + ab[1] * S(a3) + ab[2] * S(a1mf) + ab[3] * S(a1pf) + ab[4] * S(dlpmp) + ab[5] * S(slpmp)
    vdb = (ab[0] * C(elp) * delp + ab[1] * C(a3) * da3 + ab[2] * C(a1mf) * da1mf
           + ab[3] * C(a1pf) * da1pf + ab[4] * C(dlpmp) * (delp - demp) + ab[5] * C(slpmp) * (delp + demp))

    def efac(m):
        return {1: (e, de), 2: (esq, desq)}.get(abs(m), (1.0, 0.0))

    for n, coefl, coefr in reversed(MOON_LR):
        en, den = efac(n[1])
        dn, emn, empn, fn = (float(x) for x in n)
        arg = dn * d + emn * em + empn * emp + fn * f
        darg = dn * dd + emn * dem + empn * demp + fn * df
        farg = S(arg)
        v = farg * en
        dv = C(arg) * darg * en + farg * den
        vel += coefl * v
        vdel += coefl * dv
        farg = C(arg)
        v = farg * en
        dv = -S(arg) * darg * en + farg * den
        vr += coefr * v
        vdr += coefr * dv
    el = elp + DD2R * vel
    dl = (delp + DD2R * vdel) / DJC
    r = (vr + 385000560.0) / DAU
    dr = vdr / DAU / DJC
    for n, coefb in reversed(MOON_B):
        en, den = efac(n[1])
        dn, emn, empn, fn = (float(x) for x in n)
        arg = dn * d + emn * em + empn * emp + fn * f
        darg = dn * dd + emn * dem + empn * demp + fn * df
        farg = S(arg)
        v = farg * en
        dv = C(arg) * darg * en + farg * den
        vb += coefb * v
        vdb += coefb * dv
    b = vb * DD2R
    db = vdb * DD2R / DJC
    # eraS2pv
    st, ct, sp, cp = S(el), C(el), S(b), C(b)
    rcp = r * cp
    x = rcp * ct
    y = rcp * st
    rpd = r * db
    w = rpd * sp - cp * dr
    p = [x, y, r * sp]
    v = [-y * dl - w * ct, x * dl - w * st, rpd * cp + sp * dr]
    gamb, phib, psib, _ = E.pfw06(E.centuries(jd1, jd2))
    rm = E.ident()
    E.rz(psib, rm)
    E.rx(-phib, rm)
    E.rz(-gamb, rm)
    return [E.rxp(rm, p), E.rxp(rm, v)]
