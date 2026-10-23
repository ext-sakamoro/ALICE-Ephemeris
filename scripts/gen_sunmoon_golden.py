#!/usr/bin/env python3
"""Generate tests/golden/sunmoon.csv: the Earth's heliocentric and barycentric
position and velocity (ERFA eraEpv00) and the Moon's geocentric position and
velocity (ERFA eraMoon98), as ERFA 2.0.1 computes them.

None of the expected values comes from this crate. Every value written is the
same on every machine: ERFA's expressions evaluated one IEEE binary64
operation at a time with correctly rounded sines and cosines
(scripts/sunmoon_ieee.py, the tables read from ERFA's epv00.c and moon98.c).

Each row also carries two bounds derived from the arithmetic, one for the
Earth and one for the Moon, on the difference from a build of the same
expressions whose sines and cosines are within 1 ulp (alice-det-math's are
musl's, < 1 ulp): `tests/oracle_sunmoon.rs` holds the crate to them. pyerfa
(whose build may also fuse a*b + c) is run on every row as a cross-check
within the same derivation with fused rounding added; a row outside it, or too
few values compared, fails.

Usage (versions pinned in scripts/oracle-requirements.txt):
    .venv/bin/python -I scripts/gen_sunmoon_golden.py <epv00.c> <moon98.c>           # write
    .venv/bin/python -I scripts/gen_sunmoon_golden.py <epv00.c> <moon98.c> --check   # compare
"""

import math
import pathlib
import sys
import warnings

import erfa
import numpy

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import sunmoon_ieee as M  # noqa: E402

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = ROOT / "tests" / "golden" / "sunmoon.csv"
PINS = {"pyerfa": "2.0.1.5", "numpy": "2.5.4"}
U = 2.0 ** -53
# |sin or cos within 1 ulp − correctly rounded| ≤ 1 ulp + ½ ulp ≤ 3U (|value| ≤ 1)
TRIG = 3 * U
ulp = math.ulp


def dates():
    """TDB (= TT here) two-part dates 1900..2100 every 173 days with a varying
    fraction, five dates out to 1600 and 2400, ERFA's test-vector dates, and
    both orders of the parts."""
    out = []
    k = 0
    day = -36525
    while day <= 36525:
        out.append((2451545.0, day + (k * 0.381966011250105) % 1.0))
        day += 173
        k += 1
    out += [(2451545.0, d + 0.25) for d in (-146100.0, -73050.0, 73050.0, 146100.0, 0.0)]
    out += [(2400000.5, 53411.52501161), (2400000.5, 43999.9), (53411.52501161, 2400000.5)]
    return out


def epv_bound(jd1, jd2, fused):
    """Bounds on |Δ position| (au) and |Δ velocity| (au/day), per component of
    the heliocentric and barycentric vectors (the barycentric sum continues
    the heliocentric one, so its bound does too)."""
    t = M.epv_t(jd1, jd2)
    at = abs(t)
    out_p, out_v = [], []
    for c in "xyz":
        bp = bv = 0.0
        mp = mv = 0.0
        n = 0
        per_model = []
        for model in "es":
            for power in (0, 1, 2):
                for a, b, cc in M.EPV[f"{model}{power}{c}"]:
                    ct = abs(cc) * at
                    dp = 1.5 * ulp(max(ct, abs(b + cc * t))) if fused else 0.0
                    d = TRIG + dp
                    tk = at ** power
                    bp += abs(a) * tk * d + 2 * ulp(abs(a) * tk)
                    mp += abs(a) * tk
                    if power == 0:
                        vel = abs(a * cc)
                    elif power == 1:
                        vel = abs(a) * (1 + ct)
                    else:
                        vel = abs(a) * at * (2 + ct)
                    bv += vel * d + 3 * ulp(vel)
                    mv += vel
                    n += 1
            # the running sums never exceed the sums of the magnitudes
            per_model.append((bp + 2 * n * ulp(mp), (bv + 2 * n * ulp(mv)) / M.DJY + ulp(mv / M.DJY), mp, mv / M.DJY))
        out_p.append([per_model[0][0], per_model[1][0]])
        out_v.append([per_model[0][1], per_model[1][1]])
        out_p[-1].append((per_model[0][2], per_model[1][2]))
        out_v[-1].append((per_model[0][3], per_model[1][3]))
    am = [[1.0, 0.000000211284, 0.000000091603],
          [0.000000230286, 0.917482137087, 0.397776982902],
          [0.0, 0.397776982902, 0.917482137087]]
    res = []
    for model in (0, 1):
        for comp in (out_p, out_v):
            b = [sum(am[i][j] * comp[j][model] for j in range(3))
                 + 4 * ulp(sum(am[i][j] * comp[j][2][model] for j in range(3))) for i in range(3)]
            res.append(max(b))
    return res  # helio pos, helio vel, bary pos, bary vel


def moon_bound(jd1, jd2, fused):
    """Bounds on |Δ position| (au) and |Δ velocity| (au/day), per component."""
    t = M.moon_t(jd1, jd2)
    at = abs(t)
    pmax = 360.0 + 483202.1 * at + 0.01 * at * at + at ** 3 + at ** 4
    dfund = M.DD2R * 6 * ulp(pmax) if fused else 0.0
    drate = M.DD2R * 4 * ulp(483202.1 + 0.02 * at + 3 * at * at) if fused else 0.0
    rate = M.DD2R * (483202.1 + 0.02 * at + 3 * at * at)
    de = 0.002516 + 0.0000148 * at
    e = 1 + de * at
    dfac = 4 * U if fused else 0.0

    def arg_err(n):
        k = sum(abs(x) for x in n)
        da = k * dfund + (3 * ulp(k * 2 * math.pi) if fused else 0.0)
        dd = k * drate + (3 * ulp(k * rate) if fused else 0.0)
        return da, dd, k * rate

    def sums(rows):
        b = bd = m = md = 0.0
        for n, coef in rows:
            en, den = {1: (e, de), 2: (e * e, 2 * e * de)}.get(abs(n[1]), (1.0, 0.0))
            da, dd, rk = arg_err(n)
            b += abs(coef) * (en * (TRIG + da) + dfac) + 2 * ulp(abs(coef))
            bd += abs(coef) * (rk * en * (TRIG + da) + en * dd + den * (TRIG + da) + 3 * ulp(rk) + dfac * rk) + 3 * ulp(abs(coef) * rk)
            m += abs(coef) * en
            md += abs(coef) * (rk * en + den)
        return b + 2 * len(rows) * ulp(m), bd + 2 * len(rows) * ulp(md), m, md

    # additive terms: arguments a1, a2, a3 (rounding of a + b·t under fusion) and the fundamentals
    dadd = (2 * M.DD2R * ulp(53.09 + 481266.484 * at) if fused else 0.0) + dfund
    add_l = 0.006238 * (TRIG + dadd)
    add_b = 0.001209 * (TRIG + dadd)
    add_ld = 0.006238 * (rate * (TRIG + dadd) + drate)
    bl, bld, ml, mld = sums([(n, cl) for n, cl, _ in M.MOON_LR])
    br, brd, mr, mrd = sums([(n, cr) for n, _, cr in M.MOON_LR])
    bb, bbd, mb, mbd = sums(M.MOON_B)
    del_ = M.DD2R * (bl + add_l + 64 * ulp(ml)) + dfund + 2 * ulp(8.0)
    db = M.DD2R * (bb + add_b + 64 * ulp(mb)) + 2 * ulp(0.1)
    r = (385000560.0 + mr) / M.DAU
    dr = br / M.DAU + 2 * ulp(r)
    td = (rate + M.DD2R * mld) / M.DJC
    dtd = (drate + M.DD2R * (bld + add_ld + 64 * ulp(mld))) / M.DJC + 3 * ulp(td)
    pd = M.DD2R * mbd / M.DJC
    dpd = M.DD2R * (bbd + 64 * ulp(mbd)) / M.DJC + 3 * ulp(pd + 1e-30)
    rd = mrd / M.DAU / M.DJC
    drd = brd / M.DAU / M.DJC + 3 * ulp(rd)
    # s2pv
    pos = dr + r * (2 * TRIG + del_ + db) + 3 * ulp(r)
    w = r * pd + rd
    dw = dr * pd + r * dpd + r * pd * (TRIG + db) + rd * (TRIG + db) + drd + 3 * ulp(w)
    vmax = r * td + w
    vel = (dr + r * (2 * TRIG + del_ + db)) * td + r * dtd + dw + w * (TRIG + del_) + 4 * ulp(vmax)
    # three rotations by the F-W angles (each ≤ 0.41 rad; fused polynomials move them ≤ 6 ulp)
    dang = 6 * ulp(0.5) if fused else 0.0
    em = 3 * (TRIG + dang + 4 * U)
    pos = 3 * em * r + 3 * pos + 3 * ulp(r)
    vel = 3 * em * vmax + 3 * vel + 3 * ulp(vmax)
    return pos, vel


class CrossCheck:
    def __init__(self):
        self.compared = 0
        self.worst = 0.0

    def vector(self, ours, theirs, tol, what):
        for o, t in zip(ours, numpy.asarray(theirs).reshape(-1)):
            e = abs(o - float(t))
            if e > tol:
                sys.exit(f"pyerfa {what}: {o!r} vs {float(t)!r} ({e:e} > {tol:e})")
            self.worst = max(self.worst, e / tol)
            self.compared += 1


def main():
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    got = {"pyerfa": erfa.__version__, "numpy": numpy.__version__}
    if got != PINS:
        sys.exit(f"tool versions {got} differ from the pins {PINS}")
    M.load(sys.argv[1], sys.argv[2])
    # epv00 flags the dates outside 1900-2100 that the table includes on purpose
    warnings.simplefilter("ignore", erfa.ErfaWarning)
    cc = CrossCheck()
    lines = [
        "# generated by scripts/gen_sunmoon_golden.py (ERFA 2.0.1 expressions in IEEE binary64, correctly rounded sines)",
        "jd1,jd2," + ",".join(f"h{i}" for i in range(6)) + "," + ",".join(f"b{i}" for i in range(6)) + ","
        + ",".join(f"m{i}" for i in range(6))
        + ",bound_hp,bound_hv,bound_bp,bound_bv,bound_mp,bound_mv",
    ]
    for jd1, jd2 in dates():
        pvh, pvb = M.epv00(jd1, jd2)
        moon = M.moon98(jd1, jd2)
        theirs = erfa.epv00(jd1, jd2)
        fh = epv_bound(jd1, jd2, True)
        cc.vector(pvh[0], theirs[0][0], fh[0], "epv00 heliocentric position")
        cc.vector(pvh[1], theirs[0][1], fh[1], "epv00 heliocentric velocity")
        cc.vector(pvb[0], theirs[1][0], fh[2], "epv00 barycentric position")
        cc.vector(pvb[1], theirs[1][1], fh[3], "epv00 barycentric velocity")
        fm = moon_bound(jd1, jd2, True)
        rm = erfa.moon98(jd1, jd2)
        cc.vector(moon[0], rm[0], fm[0], "moon98 position")
        cc.vector(moon[1], rm[1], fm[1], "moon98 velocity")
        vals = [jd1, jd2, *pvh[0], *pvh[1], *pvb[0], *pvb[1], *moon[0], *moon[1],
                *epv_bound(jd1, jd2, False), *moon_bound(jd1, jd2, False)]
        lines.append(",".join(repr(float(v)) for v in vals))
    # per date: 4 Earth vectors and 2 Moon vectors of 3
    if cc.compared != 18 * len(dates()):
        sys.exit(f"the pyerfa cross-check compared {cc.compared} values, not {18 * len(dates())}")
    text = "\n".join(lines) + "\n"
    if "--check" in sys.argv[3:]:
        if OUT.read_text(encoding="utf-8") != text:
            sys.exit(f"{OUT} differs from the regenerated table")
        print(f"{OUT.name}: {len(lines) - 2} rows match ({cc.compared} pyerfa cross-checks, worst {cc.worst:.3f} of the bound)")
    else:
        OUT.write_text(text, encoding="utf-8")
        print(f"wrote {OUT.name}: {len(lines) - 2} rows ({cc.compared} pyerfa cross-checks, worst {cc.worst:.3f} of the bound)")


if __name__ == "__main__":
    main()
