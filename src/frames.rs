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

use alice_det_math::sin_cos64;

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
