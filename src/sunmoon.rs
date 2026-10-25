//! The geocentric Sun and Moon: the Earth's heliocentric and barycentric
//! position and velocity (ERFA `eraEpv00`, a simplified VSOP2000 solution) and
//! the Moon's geocentric position and velocity (ERFA `eraMoon98`, the Meeus
//! 1998 series rotated into the GCRS with the IAU 2006 precession).
//!
//! Both evaluate the ERFA expressions with the same IEEE operations in the
//! same order; the sines and cosines are alice-det-math's, so every target
//! returns the same bits. They differ from ERFA built with a given libm only
//! by that libm's rounding of each sine and cosine (`tests/oracle_sunmoon.rs`
//! bounds the difference).
//!
//! ERFA's own departures from its sources are kept, since the Law is ERFA's
//! routine: the Moon's mean longitude omits the light-time correction that
//! Meeus folds into its constant (0.744″), the velocity neglects the spin of
//! the precessing ecliptic (≤ 3 mm/s), and the rate of Meeus's argument A₁ is
//! taken as `ERFA_DD2R * al1` (0.003958°/century instead of 131.849°, ≤ 2.2 mm/s
//! in the velocity).
//!
//! Accuracy of the models themselves (ERFA's notes): the Earth within 11.2 km
//! (heliocentric) and 13.4 km (barycentric) of DE405 over 1900–2100; the Moon
//! within 18.3″ in direction and 31.7 km in position of DE404 over 1950–2050
//! (2.9″ and 6.1 km RMS).

use alice_det_math::sin_cos64;

use crate::frames::{mxv, pfw06, rx, rz, IDENTITY};
use crate::jd::Jd2;
use crate::sunmoon_table::{
    E0X, E0Y, E0Z, E1X, E1Y, E1Z, E2X, E2Y, E2Z, MOON_B, MOON_LR, S0X, S0Y, S0Z, S1X, S1Y, S1Z,
    S2X, S2Y, S2Z,
};

/// Julian Date of J2000.0.
const DJ00: f64 = 2_451_545.0;
/// Days per Julian year.
const DJY: f64 = 365.25;
/// Days per Julian century.
const DJC: f64 = 36_525.0;
/// Degrees to radians (ERFA_DD2R 1.745329251994329576923691e-2, written as the
/// shortest decimal of the same `f64`).
const DD2R: f64 = 0.017_453_292_519_943_295;
/// The astronomical unit, m (IAU 2012).
const DAU: f64 = 149_597_870_700.0;

/// A position (au) and velocity (au per day of the argument's time scale).
pub type Pv = [[f64; 3]; 2];

/// The Earth's position and velocity with respect to the BCRS axes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EarthPv {
    /// Heliocentric position (au) and velocity (au/day).
    pub heliocentric: Pv,
    /// Barycentric position (au) and velocity (au/day).
    pub barycentric: Pv,
    /// The date lies within 1900–2100 (`|T| ≤ 100` Julian years from
    /// J2000.0), the span of the quoted accuracy.
    pub in_range: bool,
}

/// The Moon's position and velocity with respect to the GCRS.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MoonPv {
    /// Geocentric position (au) and velocity (au/day).
    pub geocentric: Pv,
    /// The date lies within 1950–2050 (`|T| ≤ 0.5` Julian centuries from
    /// J2000.0), the span of ERFA's quoted accuracy (ERFA's routine has no
    /// status; this flag is the only sign of a date outside it).
    pub in_range: bool,
}

impl EarthPv {
    /// The geocentric position (au) and velocity (au/day) of the Sun.
    pub fn sun_geocentric(&self) -> Pv {
        let [p, v] = self.heliocentric;
        [[-p[0], -p[1], -p[2]], [-v[0], -v[1], -v[2]]]
    }
}

/// Adds one series' contribution to a position and velocity component.
/// `power` is the power of `t` that multiplies the terms.
fn series(terms: &[[f64; 3]], power: u8, t: f64, t2: f64, xyz: &mut f64, xyzd: &mut f64) {
    for &[a, b, c] in terms {
        match power {
            0 => {
                let p = b + c * t;
                let (s, cp) = sin_cos64(p);
                *xyz += a * cp;
                *xyzd -= a * c * s;
            }
            1 => {
                let ct = c * t;
                let p = b + ct;
                let (s, cp) = sin_cos64(p);
                *xyz += a * t * cp;
                *xyzd += a * (cp - ct * s);
            }
            _ => {
                let ct = c * t;
                let p = b + ct;
                let (s, cp) = sin_cos64(p);
                *xyz += a * t2 * cp;
                *xyzd += a * t * (2.0 * cp - ct * s);
            }
        }
    }
}

/// The ecliptic-to-BCRS rotation of ERFA's model (fitted to DE405).
fn to_bcrs(v: [f64; 3]) -> [f64; 3] {
    const AM12: f64 = 0.000_000_211_284;
    const AM13: f64 = -0.000_000_091_603;
    const AM21: f64 = -0.000_000_230_286;
    const AM22: f64 = 0.917_482_137_087;
    const AM23: f64 = -0.397_776_982_902;
    const AM32: f64 = 0.397_776_982_902;
    const AM33: f64 = 0.917_482_137_087;
    let [x, y, z] = v;
    [
        x + AM12 * y + AM13 * z,
        AM21 * x + AM22 * y + AM23 * z,
        AM32 * y + AM33 * z,
    ]
}

/// The Earth's heliocentric and barycentric position and velocity at TDB
/// Julian date `tdb` (ERFA `eraEpv00`; TT may stand in for TDB).
pub fn epv00(tdb: Jd2) -> EarthPv {
    let t = ((tdb.jd1 - DJ00) + tdb.jd2) / DJY;
    let t2 = t * t;
    let earth: [[&[[f64; 3]]; 3]; 3] = [[&E0X, &E1X, &E2X], [&E0Y, &E1Y, &E2Y], [&E0Z, &E1Z, &E2Z]];
    let sun: [[&[[f64; 3]]; 3]; 3] = [[&S0X, &S1X, &S2X], [&S0Y, &S1Y, &S2Y], [&S0Z, &S1Z, &S2Z]];
    let (mut ph, mut vh, mut pb, mut vb) = ([0.0; 3], [0.0; 3], [0.0; 3], [0.0; 3]);
    for i in 0..3 {
        let (mut xyz, mut xyzd) = (0.0, 0.0);
        for (power, terms) in (0u8..).zip(earth[i]) {
            series(terms, power, t, t2, &mut xyz, &mut xyzd);
        }
        ph[i] = xyz;
        vh[i] = xyzd / DJY;
        for (power, terms) in (0u8..).zip(sun[i]) {
            series(terms, power, t, t2, &mut xyz, &mut xyzd);
        }
        pb[i] = xyz;
        vb[i] = xyzd / DJY;
    }
    EarthPv {
        heliocentric: [to_bcrs(ph), to_bcrs(vh)],
        barycentric: [to_bcrs(pb), to_bcrs(vb)],
        in_range: t.abs() <= 100.0,
    }
}

/// A fundamental argument of the Moon (deg polynomial in `t`, reduced modulo
/// 360°) in radians, and its rate in radians per Julian century.
fn argument(c: [f64; 5], t: f64) -> (f64, f64) {
    let angle = DD2R * ((c[0] + (c[1] + (c[2] + (c[3] + c[4] * t) * t) * t) * t) % 360.0);
    let rate = DD2R * (c[1] + (c[2] * 2.0 + (c[3] * 3.0 + c[4] * 4.0 * t) * t) * t);
    (angle, rate)
}

/// The E factor of a term with `m` multiples of the Sun's mean anomaly, and
/// its rate.
fn e_factor(m: i8, e: f64, de: f64, esq: f64, desq: f64) -> (f64, f64) {
    match m.unsigned_abs() {
        1 => (e, de),
        2 => (esq, desq),
        _ => (1.0, 0.0),
    }
}

/// `eraS2pv`: spherical position and rates to a position and velocity.
fn s2pv(theta: f64, phi: f64, r: f64, td: f64, pd: f64, rd: f64) -> Pv {
    let (st, ct) = sin_cos64(theta);
    let (sp, cp) = sin_cos64(phi);
    let rcp = r * cp;
    let x = rcp * ct;
    let y = rcp * st;
    let rpd = r * pd;
    let w = rpd * sp - cp * rd;
    [
        [x, y, r * sp],
        [-y * td - w * ct, x * td - w * st, rpd * cp + sp * rd],
    ]
}

/// The Moon's geocentric position (au) and velocity (au/day) with respect to
/// the GCRS at TT Julian date `tt` (ERFA `eraMoon98`), and whether the date
/// is within the span of the quoted accuracy.
pub fn moon98(tt: Jd2) -> MoonPv {
    let t = ((tt.jd1 - DJ00) + tt.jd2) / DJC;

    // Moon's mean longitude, mean elongation, Sun's mean anomaly, Moon's mean
    // anomaly, Moon's argument of latitude (Simon et al. 1994, as in Meeus)
    let (elp, delp) = argument(
        [
            218.316_654_36,
            481_267.881_234_21,
            -0.001_578_6,
            1.0 / 538_841.0,
            -1.0 / 65_194_000.0,
        ],
        t,
    );
    let (d, dd) = argument(
        [
            297.850_192_1,
            445_267.111_403_4,
            -0.001_881_9,
            1.0 / 545_868.0,
            1.0 / 113_065_000.0,
        ],
        t,
    );
    let (em, dem) = argument(
        [
            357.529_109_2,
            35_999.050_290_9,
            -0.000_153_6,
            1.0 / 24_490_000.0,
            0.0,
        ],
        t,
    );
    let (emp, demp) = argument(
        [
            134.963_396_4,
            477_198.867_505_5,
            0.008_741_4,
            1.0 / 69_699.0,
            -1.0 / 14_712_000.0,
        ],
        t,
    );
    let (f, df) = argument(
        [
            93.272_095_0,
            483_202.017_523_3,
            -0.003_653_9,
            1.0 / 3_526_000.0,
            1.0 / 863_310_000.0,
        ],
        t,
    );

    // Meeus A1 (Venus), A2 (Jupiter), A3 (sidereal motion in longitude)
    const AL1: f64 = 0.003_958;
    const AL2: f64 = 0.001_962;
    const AL3: f64 = 0.000_318;
    const A21: f64 = 479_264.290;
    const A31: f64 = 481_266.484;
    let a1 = DD2R * (119.75 + 131.849 * t);
    let da1 = DD2R * AL1;
    let a2 = DD2R * (53.09 + A21 * t);
    let da2 = DD2R * A21;
    let a3 = DD2R * (313.45 + A31 * t);
    let da3 = DD2R * A31;

    // E factor (eccentricity of the Earth's orbit) and its square
    const E1: f64 = -0.002_516;
    const E2: f64 = -0.000_007_4;
    let e = 1.0 + (E1 + E2 * t) * t;
    let de = E1 + 2.0 * E2 * t;
    let esq = e * e;
    let desq = 2.0 * e * de;

    // Meeus additive terms (deg)
    let elpmf = elp - f;
    let delpmf = delp - df;
    let (sa1, ca1) = sin_cos64(a1);
    let (selpmf, celpmf) = sin_cos64(elpmf);
    let (sa2, ca2) = sin_cos64(a2);
    let mut vel = AL1 * sa1 + AL2 * selpmf + AL3 * sa2;
    let mut vdel = AL1 * ca1 * da1 + AL2 * celpmf * delpmf + AL3 * ca2 * da2;
    let mut vr = 0.0;
    let mut vdr = 0.0;
    let a1mf = a1 - f;
    let da1mf = da1 - df;
    let a1pf = a1 + f;
    let da1pf = da1 + df;
    let dlpmp = elp - emp;
    let slpmp = elp + emp;
    const AB: [f64; 6] = [
        -0.002_235, 0.000_382, 0.000_175, 0.000_175, 0.000_127, -0.000_115,
    ];
    let (selp, celp) = sin_cos64(elp);
    let (sa3, ca3) = sin_cos64(a3);
    let (sa1mf, ca1mf) = sin_cos64(a1mf);
    let (sa1pf, ca1pf) = sin_cos64(a1pf);
    let (sdlpmp, cdlpmp) = sin_cos64(dlpmp);
    let (sslpmp, cslpmp) = sin_cos64(slpmp);
    let mut vb = AB[0] * selp
        + AB[1] * sa3
        + AB[2] * sa1mf
        + AB[3] * sa1pf
        + AB[4] * sdlpmp
        + AB[5] * sslpmp;
    let mut vdb = AB[0] * celp * delp
        + AB[1] * ca3 * da3
        + AB[2] * ca1mf * da1mf
        + AB[3] * ca1pf * da1pf
        + AB[4] * cdlpmp * (delp - demp)
        + AB[5] * cslpmp * (delp + demp);

    let argument_of = |n: [i8; 4]| {
        let (dn, emn, empn, fn_) = (
            f64::from(n[0]),
            f64::from(n[1]),
            f64::from(n[2]),
            f64::from(n[3]),
        );
        (
            dn * d + emn * em + empn * emp + fn_ * f,
            dn * dd + emn * dem + empn * demp + fn_ * df,
        )
    };

    // longitude and distance, summed from the last term to the first
    for &(n, coefl, coefr) in MOON_LR.iter().rev() {
        let (en, den) = e_factor(n[1], e, de, esq, desq);
        let (arg, darg) = argument_of(n);
        let (s, c) = sin_cos64(arg);
        vel += coefl * (s * en);
        vdel += coefl * (c * darg * en + s * den);
        vr += coefr * (c * en);
        vdr += coefr * (-s * darg * en + c * den);
    }
    const R0: f64 = 385_000_560.0;
    let el = elp + DD2R * vel;
    let del = (delp + DD2R * vdel) / DJC;
    let r = (vr + R0) / DAU;
    let dr = vdr / DAU / DJC;

    // latitude
    for &(n, coefb) in MOON_B.iter().rev() {
        let (en, den) = e_factor(n[1], e, de, esq, desq);
        let (arg, darg) = argument_of(n);
        let (s, c) = sin_cos64(arg);
        vb += coefb * (s * en);
        vdb += coefb * (c * darg * en + s * den);
    }
    let b = vb * DD2R;
    let db = vdb * DD2R / DJC;

    // mean ecliptic and equinox of date to the GCRS (IAU 2006 F-W angles)
    let pv = s2pv(el, b, r, del, db, dr);
    let fw = pfw06(tt);
    let mut rm = IDENTITY;
    rz(fw.psib, &mut rm);
    rx(-fw.phib, &mut rm);
    rz(-fw.gamb, &mut rm);
    MoonPv {
        geocentric: [mxv(&rm, pv[0]), mxv(&rm, pv[1])],
        in_range: t.abs() <= 0.5,
    }
}
