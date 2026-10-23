"""ERFA 2.0.1 routines evaluated one IEEE binary64 operation at a time.

The bit reference for the Earth-orientation tables: CPython rounds every
operation and math.fmod is exact, and every sine, cosine and arctangent is
correctly rounded (mpmath at 256 bits, rounded once), so the result is the same
on every machine. A compiled ERFA may fuse a·b + c and uses the platform libm,
so pyerfa is only a cross-check within derived bounds. The expressions and
their order follow ERFA's C source; the nutation and s series are read from
ERFA's nut00a.c and s06.c (their SHA-256 pinned by the callers).
"""

import functools
import math
import re

import mpmath

D2PI = 6.283185307179586476925287
DAS2R = 4.848136811095359935899141e-6
DS2R = 7.272205216643039903848712e-5
TURNAS = 1296000.0
DJ00 = 2451545.0
DJC = 36525.0
DAYSEC = 86400.0


def sin_cr(x):
    with mpmath.workprec(256):
        return float(mpmath.sin(mpmath.mpf(x)))


def cos_cr(x):
    with mpmath.workprec(256):
        return float(mpmath.cos(mpmath.mpf(x)))


def atan2_cr(y, x):
    with mpmath.workprec(256):
        return float(mpmath.atan2(mpmath.mpf(y), mpmath.mpf(x)))


def atan_cr(x):
    with mpmath.workprec(256):
        return float(mpmath.atan(mpmath.mpf(x)))


def anp(a):
    w = math.fmod(a, D2PI)
    return w + D2PI if w < 0 else w


def ordered(jd1, jd2):
    return (jd1, jd2) if jd1 < jd2 else (jd2, jd1)


def era00(jd1, jd2):
    d1, d2 = ordered(jd1, jd2)
    t = d1 + (d2 - DJ00)
    f = math.fmod(d1, 1.0) + math.fmod(d2, 1.0)
    return anp(D2PI * (f + 0.7790572732640 + 0.00273781191135448 * t))


def gmst82(jd1, jd2):
    a = 24110.54841 - DAYSEC / 2.0
    b, c, d = 8640184.812866, 0.093104, -6.2e-6
    d1, d2 = ordered(jd1, jd2)
    t = (d1 + (d2 - DJ00)) / DJC
    f = DAYSEC * (math.fmod(d1, 1.0) + math.fmod(d2, 1.0))
    return anp(DS2R * ((a + (b + (c + d * t) * t) * t) + f))


def sp00(jd1, jd2):
    t = ((jd1 - DJ00) + jd2) / DJC
    return -47e-6 * t * DAS2R


def ident():
    return [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]


def rx(phi, r):
    s, c = sin_cr(phi), cos_cr(phi)
    a10 = c * r[1][0] + s * r[2][0]
    a11 = c * r[1][1] + s * r[2][1]
    a12 = c * r[1][2] + s * r[2][2]
    a20 = - s * r[1][0] + c * r[2][0]
    a21 = - s * r[1][1] + c * r[2][1]
    a22 = - s * r[1][2] + c * r[2][2]
    r[1] = [a10, a11, a12]
    r[2] = [a20, a21, a22]


def ry(theta, r):
    s, c = sin_cr(theta), cos_cr(theta)
    a00 = c * r[0][0] - s * r[2][0]
    a01 = c * r[0][1] - s * r[2][1]
    a02 = c * r[0][2] - s * r[2][2]
    a20 = s * r[0][0] + c * r[2][0]
    a21 = s * r[0][1] + c * r[2][1]
    a22 = s * r[0][2] + c * r[2][2]
    r[0] = [a00, a01, a02]
    r[2] = [a20, a21, a22]


def rz(psi, r):
    s, c = sin_cr(psi), cos_cr(psi)
    a00 = c * r[0][0] + s * r[1][0]
    a01 = c * r[0][1] + s * r[1][1]
    a02 = c * r[0][2] + s * r[1][2]
    a10 = - s * r[0][0] + c * r[1][0]
    a11 = - s * r[0][1] + c * r[1][1]
    a12 = - s * r[0][2] + c * r[1][2]
    r[0] = [a00, a01, a02]
    r[1] = [a10, a11, a12]


def rxr(a, b):
    """eraRxr: the sum over k accumulated from 0, as ERFA does."""
    out = [[0.0] * 3 for _ in range(3)]
    for i in range(3):
        for j in range(3):
            w = 0.0
            for k in range(3):
                w += a[i][k] * b[k][j]
            out[i][j] = w
    return out


def rxp(m, p):
    out = []
    for i in range(3):
        w = 0.0
        for k in range(3):
            w += m[i][k] * p[k]
        out.append(w)
    return out


def pom00(xp, yp, sp):
    r = ident()
    rz(sp, r)
    ry(-xp, r)
    rx(-yp, r)
    return r


# ---- IAU 2006/2000A -----------------------------------------------------------

XLS, XPL, SP, S_TERMS = [], [], [], []


def load_series(nut00a_c, s06_c):
    """The coefficient tables of nut00a.c and s06.c (literals parsed by Python)."""
    t = open(nut00a_c, encoding="utf-8").read()
    a = t.index("} xls[] = {")
    for row in re.findall(r"\{([^{}]*)\}", t[a + 10:t.index("};", a)]):
        f = [x for x in row.replace("\n", " ").split(",") if x.strip()]
        XLS.append(([int(x) for x in f[:5]], [float(x) for x in f[5:]]))
    a = t.index("} xpl[] = {")
    for row in re.findall(r"\{([^{}]*)\}", t[a + 10:t.index("};", a)]):
        f = [x for x in row.split(",") if x.strip()]
        XPL.append(([int(x) for x in f[:13]], [int(x) for x in f[13:]]))
    s = open(s06_c, encoding="utf-8").read()
    a = s.index("static const double sp[] = {")
    SP.extend(float(x) for x in re.findall(r"[-+0-9.]+e-6", s[a:s.index("};", a)]))
    for n in ("s0", "s1", "s2", "s3", "s4"):
        a = s.index(f"static const TERM {n}[] = {{")
        rows = re.findall(r"\{\{([^}]*)\},\s*([^,}]+),\s*([^}]+)\}", s[a:s.index("};", a)])
        S_TERMS.append([([int(x) for x in nfa.split(",")], float(sv), float(cv)) for nfa, sv, cv in rows])
    assert (len(XLS), len(XPL), len(SP), [len(x) for x in S_TERMS]) == (678, 687, 6, [33, 3, 25, 4, 1])


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


def centuries(jd1, jd2):
    return ((jd1 - DJ00) + jd2) / DJC


def nut00a_arguments(t):
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


@functools.lru_cache(maxsize=None)
def nut00a(jd1, jd2):
    u2r = DAS2R / 1e7
    t = centuries(jd1, jd2)
    ls, pl = nut00a_arguments(t)
    dp = de = 0.0
    for n, c in reversed(XLS):
        arg = math.fmod(float(n[0]) * ls[0] + float(n[1]) * ls[1] + float(n[2]) * ls[2]
                        + float(n[3]) * ls[3] + float(n[4]) * ls[4], D2PI)
        sarg, carg = sin_cr(arg), cos_cr(arg)
        dp += (c[0] + c[1] * t) * sarg + c[2] * carg
        de += (c[3] + c[4] * t) * carg + c[5] * sarg
    dpsils, depsls = dp * u2r, de * u2r
    dp = de = 0.0
    for n, c in reversed(XPL):
        arg = float(n[0]) * pl[0]
        for k in range(1, 13):
            arg += float(n[k]) * pl[k]
        arg = math.fmod(arg, D2PI)
        sarg, carg = sin_cr(arg), cos_cr(arg)
        dp += float(c[0]) * sarg + float(c[1]) * carg
        de += float(c[2]) * sarg + float(c[3]) * carg
    return dpsils + dp * u2r, depsls + de * u2r


def nut06a(jd1, jd2):
    t = centuries(jd1, jd2)
    fj2 = -2.7774e-6 * t
    dp, de = nut00a(jd1, jd2)
    return dp + dp * (0.4697e-6 + fj2), de + de * fj2


def obl06(t):
    return (84381.406 + (-46.836769 + (-0.0001831 + (0.00200340 + (-0.000000576 + (-0.0000000434) * t) * t) * t) * t) * t) * DAS2R


def pfw06(t):
    gamb = (-0.052928 + (10.556378 + (0.4932044 + (-0.00031238 + (-0.000002788 + (0.0000000260) * t) * t) * t) * t) * t) * DAS2R
    phib = (84381.412819 + (-46.811016 + (0.0511268 + (0.00053289 + (-0.000000440 + (-0.0000000176) * t) * t) * t) * t) * t) * DAS2R
    psib = (-0.041775 + (5038.481484 + (1.5584175 + (-0.00018522 + (-0.000026452 + (-0.0000000148) * t) * t) * t) * t) * t) * DAS2R
    return gamb, phib, psib, obl06(t)


def fw2m(gamb, phib, psi, eps):
    r = ident()
    rz(gamb, r)
    rx(phib, r)
    rz(-psi, r)
    rx(-eps, r)
    return r


def pnm06a(jd1, jd2):
    gamb, phib, psib, epsa = pfw06(centuries(jd1, jd2))
    dp, de = nut06a(jd1, jd2)
    return fw2m(gamb, phib, psib + dp, epsa + de)


@functools.lru_cache(maxsize=None)
def s06(jd1, jd2, x, y):
    t = centuries(jd1, jd2)
    fa = s06_arguments(t)
    w = list(SP)
    for k in range(5):
        for nfa, sv, cv in reversed(S_TERMS[k]):
            a = 0.0
            for j in range(8):
                a += float(nfa[j]) * fa[j]
            w[k] += sv * sin_cr(a) + cv * cos_cr(a)
    return (w[0] + (w[1] + (w[2] + (w[3] + (w[4] + w[5] * t) * t) * t) * t) * t) * DAS2R - x * y / 2.0


def c2ixys(x, y, s):
    r2 = x * x + y * y
    e = atan2_cr(y, x) if r2 > 0.0 else 0.0
    d = atan_cr(math.sqrt(r2 / (1.0 - r2)))
    r = ident()
    rz(e, r)
    ry(d, r)
    rz(-(e + s), r)
    return r


def c2i06a(jd1, jd2):
    rbpn = pnm06a(jd1, jd2)
    x, y = rbpn[2][0], rbpn[2][1]
    return c2ixys(x, y, s06(jd1, jd2, x, y))


def c2t06a(tta, ttb, uta, utb, xp, yp):
    rc2i = c2i06a(tta, ttb)
    era = era00(uta, utb)
    rpom = pom00(xp, yp, sp00(tta, ttb))
    r = [row[:] for row in rc2i]
    rz(era, r)
    return rxr(rpom, r)
