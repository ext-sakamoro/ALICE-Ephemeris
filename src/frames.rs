//! Earth rotation and the terrestrial frames: the Earth rotation angle
//! (IAU 2000), Greenwich mean sidereal time (IAU 1982), polar motion, and the
//! TEME → ITRS rotation used with SGP4 states.
//!
//! The angle functions take two-part Julian dates and evaluate the ERFA
//! expressions with the same IEEE operations in the same order (`era00`,
//! `gmst82`, `sp00`, `anp`), so they return the same bits as ERFA 2.0.1 on
//! every target. Rotations use alice-det-math's sine and cosine.
//!
//! TEME → ITRS follows Vallado, Crawford, Hujsak & Kelso (AIAA 2006-6753):
//! TEME to the pseudo-Earth-fixed frame by GMST (1982) alone, then polar
//! motion; the velocity loses the Earth's rotation `ω × r`, with
//! `ω = 7.292115146706979 × 10⁻⁵ rad/s × (1 − LOD/86400 s)`.

use alice_det_math::{atan2_64, atan64, sin_cos64, sqrt64};

use crate::instant::{Instant, Ut1};
use crate::jd::Jd2;

/// A 3 × 3 rotation matrix, row-major.
pub type Mat3 = [[f64; 3]; 3];

/// 2π (ERFA_D2PI 6.283185307179586476925287 rounds to the same `f64`).
const D2PI: f64 = core::f64::consts::TAU;
/// Seconds of time to radians (ERFA_DS2R 7.272205216643039903848712e-5,
/// written as the shortest decimal of the same `f64`).
const DS2R: f64 = 7.272_205_216_643_04e-5;
/// Arcseconds to radians (ERFA_DAS2R 4.848136811095359935899141e-6, written
/// as the shortest decimal of the same `f64`).
const DAS2R: f64 = 4.848_136_811_095_36e-6;
/// Seconds per day.
const DAYSEC: f64 = 86_400.0;
/// Days per Julian century.
const DJC: f64 = 36_525.0;
/// Julian Date of J2000.0.
const DJ00: f64 = 2_451_545.0;
/// The Earth's mean rotation rate (Vallado 2006), rad/s.
pub const EARTH_ROTATION_RATE: f64 = 7.292_115_146_706_98e-5;

/// The identity matrix.
pub const IDENTITY: Mat3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// `a` reduced to `[0, 2π)` (ERFA `eraAnp`; NaN for a non-finite angle).
fn anp(a: f64) -> f64 {
    let w = a % D2PI;
    if w < 0.0 {
        w + D2PI
    } else {
        w
    }
}

/// The two parts with the smaller first (as ERFA orders them).
fn ordered(jd: Jd2) -> (f64, f64) {
    if jd.jd1 < jd.jd2 {
        (jd.jd1, jd.jd2)
    } else {
        (jd.jd2, jd.jd1)
    }
}

/// Earth rotation angle (IAU 2000) at UT1 Julian date `ut1`, in `[0, 2π)`.
pub fn era00(ut1: Jd2) -> f64 {
    let (d1, d2) = ordered(ut1);
    let t = d1 + (d2 - DJ00);
    let f = d1 % 1.0 + d2 % 1.0;
    anp(D2PI * (f + 0.779_057_273_264_0 + 0.002_737_811_911_354_48 * t))
}

/// Greenwich mean sidereal time (IAU 1982) at UT1 Julian date `ut1`, in
/// `[0, 2π)`.
pub fn gmst82(ut1: Jd2) -> f64 {
    let a = 24_110.548_41 - DAYSEC / 2.0;
    let b = 8_640_184.812_866;
    let c = 0.093_104;
    let d = -6.2e-6;
    let (d1, d2) = ordered(ut1);
    let t = (d1 + (d2 - DJ00)) / DJC;
    let f = DAYSEC * (d1 % 1.0 + d2 % 1.0);
    anp(DS2R * ((a + (b + (c + d * t) * t) * t) + f))
}

/// The TIO locator s′ (IERS Conventions 2003) at TT Julian date `tt`, in
/// radians.
pub fn sp00(tt: Jd2) -> f64 {
    let t = ((tt.jd1 - DJ00) + tt.jd2) / DJC;
    -47e-6 * t * DAS2R
}

/// Rotates `r` about the x axis by `phi` (ERFA `eraRx`: `r ← Rx(phi) · r`).
pub fn rx(phi: f64, r: &mut Mat3) {
    let (s, c) = sin_cos64(phi);
    let (a1, a2) = (r[1], r[2]);
    r[1] = core::array::from_fn(|k| c * a1[k] + s * a2[k]);
    r[2] = core::array::from_fn(|k| -s * a1[k] + c * a2[k]);
}

/// Rotates `r` about the y axis by `theta` (ERFA `eraRy`).
pub fn ry(theta: f64, r: &mut Mat3) {
    let (s, c) = sin_cos64(theta);
    let (a0, a2) = (r[0], r[2]);
    r[0] = core::array::from_fn(|k| c * a0[k] - s * a2[k]);
    r[2] = core::array::from_fn(|k| s * a0[k] + c * a2[k]);
}

/// Rotates `r` about the z axis by `psi` (ERFA `eraRz`).
pub fn rz(psi: f64, r: &mut Mat3) {
    let (s, c) = sin_cos64(psi);
    let (a0, a1) = (r[0], r[1]);
    r[0] = core::array::from_fn(|k| c * a0[k] + s * a1[k]);
    r[1] = core::array::from_fn(|k| -s * a0[k] + c * a1[k]);
}

/// Polar motion matrix (ERFA `eraPom00`) from the pole coordinates `xp`,
/// `yp` and the TIO locator `sp`, all in radians: `V(ITRS) = W · V(TIRS)`.
pub fn pom00(xp: f64, yp: f64, sp: f64) -> Mat3 {
    let mut w = IDENTITY;
    rz(sp, &mut w);
    ry(-xp, &mut w);
    rx(-yp, &mut w);
    w
}

/// `m · p`.
pub fn mxv(m: &Mat3, p: [f64; 3]) -> [f64; 3] {
    let row = |i: usize| m[i][0] * p[0] + m[i][1] * p[1] + m[i][2] * p[2];
    [row(0), row(1), row(2)]
}

/// `mᵀ · p`.
pub fn mtxv(m: &Mat3, p: [f64; 3]) -> [f64; 3] {
    let col = |j: usize| m[0][j] * p[0] + m[1][j] * p[1] + m[2][j] * p[2];
    [col(0), col(1), col(2)]
}

/// `a · b`.
pub fn mxm(a: &Mat3, b: &Mat3) -> Mat3 {
    let mut c = [[0.0; 3]; 3];
    for (i, row) in c.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j];
        }
    }
    c
}

impl Instant<Ut1> {
    /// Earth rotation angle (IAU 2000) at this instant, in `[0, 2π)`.
    pub fn earth_rotation_angle(self) -> f64 {
        era00(self.to_jd2())
    }
}

/// The rotation from TEME (the frame of SGP4 states) to ITRS at one instant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TemeToItrs {
    /// GMST (1982), radians.
    pub gmst: f64,
    /// TEME → pseudo-Earth-fixed: `Rz(gmst)`.
    pub teme_to_pef: Mat3,
    /// Polar motion, pseudo-Earth-fixed → ITRS.
    pub polar_motion: Mat3,
    /// The product, TEME → ITRS.
    pub matrix: Mat3,
}

impl TemeToItrs {
    /// The rotation at UT1 Julian date `ut1` (GMST) and TT Julian date `tt`
    /// (s′), with pole coordinates `xp`, `yp` in radians.
    pub fn new(ut1: Jd2, tt: Jd2, xp: f64, yp: f64) -> Self {
        let gmst = gmst82(ut1);
        let mut teme_to_pef = IDENTITY;
        rz(gmst, &mut teme_to_pef);
        let polar_motion = pom00(xp, yp, sp00(tt));
        Self {
            gmst,
            teme_to_pef,
            polar_motion,
            matrix: mxm(&polar_motion, &teme_to_pef),
        }
    }

    /// An ITRS position from a TEME position.
    pub fn position(&self, r: [f64; 3]) -> [f64; 3] {
        mxv(&self.matrix, r)
    }

    /// A TEME position from an ITRS position.
    pub fn position_to_teme(&self, r: [f64; 3]) -> [f64; 3] {
        mtxv(&self.matrix, r)
    }

    /// An ITRS position and velocity from a TEME state; `lod` is the excess
    /// length of day in seconds (0 for the mean rate).
    pub fn state(&self, r: [f64; 3], v: [f64; 3], lod: f64) -> ([f64; 3], [f64; 3]) {
        let w = EARTH_ROTATION_RATE * (1.0 - lod / DAYSEC);
        let rp = mxv(&self.teme_to_pef, r);
        let vp = mxv(&self.teme_to_pef, v);
        // v_pef = Rz·v − ω × r_pef, ω along z
        let vp = [vp[0] + w * rp[1], vp[1] - w * rp[0], vp[2]];
        (mxv(&self.polar_motion, rp), mxv(&self.polar_motion, vp))
    }

    /// A TEME position and velocity from an ITRS state (the inverse of
    /// [`Self::state`]).
    pub fn state_to_teme(&self, r: [f64; 3], v: [f64; 3], lod: f64) -> ([f64; 3], [f64; 3]) {
        let w = EARTH_ROTATION_RATE * (1.0 - lod / DAYSEC);
        let rp = mtxv(&self.polar_motion, r);
        let vp = mtxv(&self.polar_motion, v);
        let vp = [vp[0] - w * rp[1], vp[1] + w * rp[0], vp[2]];
        (mtxv(&self.teme_to_pef, rp), mtxv(&self.teme_to_pef, vp))
    }
}

// ---- IAU 2006/2000A precession-nutation, CIO based --------------------------
//
// The functions below follow ERFA 2.0.1 (eraNut00a, eraNut06a, eraPfw06,
// eraObl06, eraFw2m, eraPnm06a, eraBpn2xy, eraS06, eraC2ixys, eraC2i06a,
// eraC2tcio, eraC2t06a and the fundamental arguments eraFa*03) operation for
// operation; only sine, cosine, arctangent and square root are
// alice-det-math's. The series are in `iau2006_table`.

/// Arcseconds in a full circle.
const TURNAS: f64 = 1_296_000.0;

/// Julian centuries of TT since J2000.0, as ERFA forms them.
fn centuries(tt: Jd2) -> f64 {
    ((tt.jd1 - DJ00) + tt.jd2) / DJC
}

/// Delaunay and planetary arguments (IERS Conventions 2003), radians.
fn fal03(t: f64) -> f64 {
    ((485_868.249_036
        + t * (1_717_915_923.217_8 + t * (31.879_2 + t * (0.051_635 + t * (-0.000_244_70)))))
        % TURNAS)
        * DAS2R
}
fn falp03(t: f64) -> f64 {
    ((1_287_104.793_048
        + t * (129_596_581.048_1 + t * (-0.553_2 + t * (0.000_136 + t * (-0.000_011_49)))))
        % TURNAS)
        * DAS2R
}
fn faf03(t: f64) -> f64 {
    ((335_779.526_232
        + t * (1_739_527_262.847_8 + t * (-12.751_2 + t * (-0.001_037 + t * 0.000_004_17))))
        % TURNAS)
        * DAS2R
}
fn fad03(t: f64) -> f64 {
    ((1_072_260.703_692
        + t * (1_602_961_601.209_0 + t * (-6.370_6 + t * (0.006_593 + t * (-0.000_031_69)))))
        % TURNAS)
        * DAS2R
}
fn faom03(t: f64) -> f64 {
    ((450_160.398_036
        + t * (-6_962_890.543_1 + t * (7.472_2 + t * (0.007_702 + t * (-0.000_059_39)))))
        % TURNAS)
        * DAS2R
}
fn fame03(t: f64) -> f64 {
    (4.402_608_842 + 2_608.790_314_157_4 * t) % D2PI
}
fn fave03(t: f64) -> f64 {
    (3.176_146_697 + 1_021.328_554_621_1 * t) % D2PI
}
fn fae03(t: f64) -> f64 {
    (1.753_470_314 + 628.307_584_999_1 * t) % D2PI
}
fn fama03(t: f64) -> f64 {
    (6.203_480_913 + 334.061_242_670_0 * t) % D2PI
}
fn faju03(t: f64) -> f64 {
    (0.599_546_497 + 52.969_096_264_1 * t) % D2PI
}
fn fasa03(t: f64) -> f64 {
    (0.874_016_757 + 21.329_910_496_0 * t) % D2PI
}
fn faur03(t: f64) -> f64 {
    (5.481_293_872 + 7.478_159_856_7 * t) % D2PI
}
fn fapa03(t: f64) -> f64 {
    (0.024_381_750 + 0.000_005_386_91 * t) * t
}

/// Nutation IAU 2000A at TT `tt`: (Δψ, Δε) in radians (ERFA `eraNut00a`).
pub fn nut00a(tt: Jd2) -> (f64, f64) {
    use crate::iau2006_table::{NUT_LUNI_SOLAR, NUT_PLANETARY};
    let u2r = DAS2R / 1e7;
    let t = centuries(tt);
    // luni-solar: l, l′ (MHB2000 form of l′ and D), F, D, Ω
    let el = fal03(t);
    let elp = ((1_287_104.793_05
        + t * (129_596_581.048_1 + t * (-0.553_2 + t * (0.000_136 + t * (-0.000_011_49)))))
        % TURNAS)
        * DAS2R;
    let f = faf03(t);
    let d = ((1_072_260.703_69
        + t * (1_602_961_601.209_0 + t * (-6.370_6 + t * (0.006_593 + t * (-0.000_031_69)))))
        % TURNAS)
        * DAS2R;
    let om = faom03(t);
    let (mut dp, mut de) = (0.0, 0.0);
    for (n, c) in NUT_LUNI_SOLAR.iter().rev() {
        let arg = (f64::from(n[0]) * el
            + f64::from(n[1]) * elp
            + f64::from(n[2]) * f
            + f64::from(n[3]) * d
            + f64::from(n[4]) * om)
            % D2PI;
        let (sarg, carg) = sin_cos64(arg);
        dp += (c[0] + c[1] * t) * sarg + c[2] * carg;
        de += (c[3] + c[4] * t) * carg + c[5] * sarg;
    }
    let (dpsils, depsls) = (dp * u2r, de * u2r);
    // planetary: MHB2000 linear forms of l, F, D, Ω, Neptune, then IERS 2003
    let al = (2.355_555_98 + 8_328.691_426_955_4 * t) % D2PI;
    let af = (1.627_905_234 + 8_433.466_158_131 * t) % D2PI;
    let ad = (5.198_466_741 + 7_771.377_146_812_1 * t) % D2PI;
    let aom = (2.182_439_20 - 33.757_045 * t) % D2PI;
    let apa = fapa03(t);
    let alme = fame03(t);
    let alve = fave03(t);
    let alea = fae03(t);
    let alma = fama03(t);
    let alju = faju03(t);
    let alsa = fasa03(t);
    let alur = faur03(t);
    let alne = (5.321_159_000 + 3.812_777_400_0 * t) % D2PI;
    let args = [
        al, af, ad, aom, alme, alve, alea, alma, alju, alsa, alur, alne, apa,
    ];
    let (mut dp, mut de) = (0.0, 0.0);
    for (n, c) in NUT_PLANETARY.iter().rev() {
        let mut arg = f64::from(n[0]) * args[0];
        for k in 1..13 {
            arg += f64::from(n[k]) * args[k];
        }
        let (sarg, carg) = sin_cos64(arg % D2PI);
        dp += f64::from(c[0]) * sarg + f64::from(c[1]) * carg;
        de += f64::from(c[2]) * sarg + f64::from(c[3]) * carg;
    }
    (dpsils + dp * u2r, depsls + de * u2r)
}

/// Nutation IAU 2000A with the P03 adjustments for IAU 2006 precession
/// (ERFA `eraNut06a`): (Δψ, Δε) in radians.
pub fn nut06a(tt: Jd2) -> (f64, f64) {
    let t = centuries(tt);
    let fj2 = -2.7774e-6 * t;
    let (dp, de) = nut00a(tt);
    (dp + dp * (0.4697e-6 + fj2), de + de * fj2)
}

/// Mean obliquity of the ecliptic, IAU 2006 (ERFA `eraObl06`), radians.
pub fn obl06(tt: Jd2) -> f64 {
    let t = centuries(tt);
    (84_381.406
        + (-46.836_769
            + (-0.000_183_1 + (0.002_003_40 + (-0.000_000_576 + (-0.000_000_043_4) * t) * t) * t)
                * t)
            * t)
        * DAS2R
}

/// Fukushima-Williams precession angles (IAU 2006), radians.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pfw06 {
    /// F-W angle γ̄.
    pub gamb: f64,
    /// F-W angle φ̄.
    pub phib: f64,
    /// F-W angle ψ̄.
    pub psib: f64,
    /// Mean obliquity ε_A.
    pub epsa: f64,
}

/// Precession angles, IAU 2006, Fukushima-Williams 4-angle formulation
/// (ERFA `eraPfw06`).
pub fn pfw06(tt: Jd2) -> Pfw06 {
    let t = centuries(tt);
    let gamb = (-0.052_928
        + (10.556_378
            + (0.493_204_4 + (-0.000_312_38 + (-0.000_002_788 + 0.000_000_026_0 * t) * t) * t)
                * t)
            * t)
        * DAS2R;
    let phib = (84_381.412_819
        + (-46.811_016
            + (0.051_126_8 + (0.000_532_89 + (-0.000_000_440 + (-0.000_000_017_6) * t) * t) * t)
                * t)
            * t)
        * DAS2R;
    let psib = (-0.041_775
        + (5_038.481_484
            + (1.558_417_5 + (-0.000_185_22 + (-0.000_026_452 + (-0.000_000_014_8) * t) * t) * t)
                * t)
            * t)
        * DAS2R;
    Pfw06 {
        gamb,
        phib,
        psib,
        epsa: obl06(tt),
    }
}

/// Rotation matrix from Fukushima-Williams angles (ERFA `eraFw2m`).
pub fn fw2m(gamb: f64, phib: f64, psi: f64, eps: f64) -> Mat3 {
    let mut r = IDENTITY;
    rz(gamb, &mut r);
    rx(phib, &mut r);
    rz(-psi, &mut r);
    rx(-eps, &mut r);
    r
}

/// Bias-precession-nutation matrix, IAU 2006/2000A (ERFA `eraPnm06a`):
/// `V(true) = M · V(GCRS)`.
pub fn pnm06a(tt: Jd2) -> Mat3 {
    let f = pfw06(tt);
    let (dp, de) = nut06a(tt);
    fw2m(f.gamb, f.phib, f.psib + dp, f.epsa + de)
}

/// The CIP coordinates X, Y from a bias-precession-nutation matrix (ERFA
/// `eraBpn2xy`).
pub fn bpn2xy(rbpn: &Mat3) -> (f64, f64) {
    (rbpn[2][0], rbpn[2][1])
}

/// The CIO locator s (IAU 2006, ERFA `eraS06`) at TT `tt`, given the CIP
/// coordinates `x`, `y`; radians.
pub fn s06(tt: Jd2, x: f64, y: f64) -> f64 {
    use crate::iau2006_table::{S06_POLYNOMIAL, S06_T0, S06_T1, S06_T2, S06_T3, S06_T4};
    let t = centuries(tt);
    let fa = [
        fal03(t),
        falp03(t),
        faf03(t),
        fad03(t),
        faom03(t),
        fave03(t),
        fae03(t),
        fapa03(t),
    ];
    let series = |terms: &[([i8; 8], f64, f64)], mut w: f64| {
        for (nfa, s, c) in terms.iter().rev() {
            let mut a = 0.0;
            for j in 0..8 {
                a += f64::from(nfa[j]) * fa[j];
            }
            let (sa, ca) = sin_cos64(a);
            w += s * sa + c * ca;
        }
        w
    };
    let w0 = series(&S06_T0, S06_POLYNOMIAL[0]);
    let w1 = series(&S06_T1, S06_POLYNOMIAL[1]);
    let w2 = series(&S06_T2, S06_POLYNOMIAL[2]);
    let w3 = series(&S06_T3, S06_POLYNOMIAL[3]);
    let w4 = series(&S06_T4, S06_POLYNOMIAL[4]);
    let w5 = S06_POLYNOMIAL[5];
    (w0 + (w1 + (w2 + (w3 + (w4 + w5 * t) * t) * t) * t) * t) * DAS2R - x * y / 2.0
}

/// Celestial-to-intermediate matrix from the CIP X, Y and the CIO locator s
/// (ERFA `eraC2ixys`). NaN when `X² + Y² ≥ 1` (no such pole).
pub fn c2ixys(x: f64, y: f64, s: f64) -> Mat3 {
    let r2 = x * x + y * y;
    let e = if r2 > 0.0 { atan2_64(y, x) } else { 0.0 };
    let d = atan64(sqrt64(r2 / (1.0 - r2)));
    let mut r = IDENTITY;
    rz(e, &mut r);
    ry(d, &mut r);
    rz(-(e + s), &mut r);
    r
}

/// Celestial-to-intermediate matrix, IAU 2006/2000A, CIO based (ERFA
/// `eraC2i06a`): `V(CIRS) = M · V(GCRS)`.
pub fn c2i06a(tt: Jd2) -> Mat3 {
    let rbpn = pnm06a(tt);
    let (x, y) = bpn2xy(&rbpn);
    c2ixys(x, y, s06(tt, x, y))
}

/// Celestial-to-terrestrial matrix from its CIO-based components (ERFA
/// `eraC2tcio`).
pub fn c2tcio(rc2i: &Mat3, era: f64, rpom: &Mat3) -> Mat3 {
    let mut r = *rc2i;
    rz(era, &mut r);
    mxm(rpom, &r)
}

/// Celestial-to-terrestrial matrix, IAU 2006/2000A, CIO based (ERFA
/// `eraC2t06a`): `V(ITRS) = M · V(GCRS)`, given TT, UT1 and the pole
/// coordinates in radians.
pub fn c2t06a(tt: Jd2, ut1: Jd2, xp: f64, yp: f64) -> Mat3 {
    let rc2i = c2i06a(tt);
    let era = era00(ut1);
    let rpom = pom00(xp, yp, sp00(tt));
    c2tcio(&rc2i, era, &rpom)
}
