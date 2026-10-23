//! TT ↔ TDB, two named models (each its own Law).
//!
//! TDB − TT is a periodic function of about 1.7 ms amplitude. It is computed
//! in `f64` with alice-det-math's `sin64` (IEEE basic operations only, so the
//! same bits on every target), then rounded once to the nearest attosecond.
//! The inverse (TDB → TT) applies the same model at the TT estimate, three
//! times (each step shrinks the error by the series' derivative, |dΔ/dt| ≤
//! 3.4 × 10⁻¹⁰ near J2000). The series reads TT as the nearest `f64`, so a
//! round trip TT → TDB → TT returns TT exactly unless TT lies near an `f64`
//! rounding boundary, and is then off by at most 2 × |dΔ/dt| × ulp(TT)
//! (the `f64` of TT is within 1 ulp): ≤ 80 as in 2026, ≤ 320 as through
//! 2100, ≤ 2 ns at 3 × 10⁸ years from J2000 (SpiceEt; Fb1990 is the same near
//! J2000 and grows with its polynomial terms).
//!
//! Both models evaluate for any instant, but far from J2000 the T¹…T⁴ terms
//! of Fb1990 grow without bound; once |TDB − TT| reaches 1 s (about
//! 5 × 10⁶ years from J2000) the conversions return
//! [`TimeError::OutOfRange`] instead of a meaningless shift.

use alice_det_math::{round64, sin64};

use crate::fb1990_table::{BLOCKS, FAIRHD};
use crate::instant::{Duration, Instant, Tdb, Tt};
use crate::law::{self, Law};
use crate::TimeError;

/// Number of terms of [`TdbModel::Fb1990`].
pub const FB1990_TERMS: usize = crate::fb1990_table::TERMS;

/// A model of TDB − TT.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TdbModel {
    /// The NAIF SPICE formula (leapseconds kernel `naif0012.tls`, `DELTET`):
    /// `TDB − TT = K sin E`, `E = M + EB sin M`, `M = M0 + M1·t`, `t` = TT
    /// seconds past J2000. Agrees with SPICE's `unitim` to the rounding of
    /// its `f64` output; use it to compare with SPICE-based ephemerides.
    /// Its difference from [`TdbModel::Fb1990`] is at most 38 µs over
    /// 1950–2050 (measured in `tests/oracle_tdb.rs`).
    SpiceEt,
    /// The geocentric Fairhead & Bretagnon (1990) series, 787 terms plus the
    /// JPL-mass adjustment, evaluated in the order of ERFA's `eraDtdb` with
    /// zero observer offset (`u = v = 0`, no topocentric terms, which are
    /// ≤ 2 µs). ERFA states ±3 ns absolute accuracy for 1950–2050 against
    /// the numerically integrated DE405 time ephemeris.
    Fb1990,
}

/// SPICE `DELTET/K` (s).
pub const SPICE_K: f64 = 1.657e-3;
/// SPICE `DELTET/EB`.
pub const SPICE_EB: f64 = 1.671e-2;
/// SPICE `DELTET/M` (rad, rad/s).
pub const SPICE_M: [f64; 2] = [6.239996, 1.99096871e-7];

/// The 787 `[amplitude (s), frequency (rad/Julian millennium), phase (rad)]`
/// terms of [`TdbModel::Fb1990`] (from ERFA, see THIRD-PARTY-NOTICES).
pub fn fb1990_coefficients() -> &'static [[f64; 3]; FB1990_TERMS] {
    &FAIRHD
}

/// First index of the T⁰ … T⁴ blocks of [`fb1990_coefficients`], then the end.
pub const FB1990_BLOCKS: [usize; 6] = BLOCKS;

/// The JPL-mass adjustment terms of eraDtdb: `[amplitude, frequency, phase]`
/// for the four periodic terms, then the `t²` coefficient as `[c, 0, 0]`.
pub const FB1990_JPL_ADJUSTMENT: [[f64; 3]; 5] = [
    [0.00065e-6, 6069.776754, 4.021194],
    [0.00033e-6, 213.299095, 5.543132],
    [-0.00196e-6, 6208.294251, 5.696701],
    [-0.00173e-6, 74.781599, 2.435900],
    [0.03638e-6, 0.0, 0.0],
];

/// TT range (seconds since J2000 TT) of ERFA's ±3 ns statement for
/// [`TdbModel::Fb1990`]: 1950-01-01T00:00 TT to 2050-01-01T00:00 TT.
pub const FB1990_STATED_RANGE: (i64, i64) = (-1_577_880_000, 1_577_880_000);

impl TdbModel {
    /// The Law this model is.
    pub const fn law(self) -> Law {
        match self {
            Self::SpiceEt => law::TDB_SPICE_ET,
            Self::Fb1990 => law::TDB_FB1990,
        }
    }

    /// TDB − TT in seconds at TT instant `tt`.
    pub fn tdb_minus_tt(self, tt: Instant<Tt>) -> f64 {
        let s = tt.to_seconds_f64();
        match self {
            Self::SpiceEt => spice_et(s),
            Self::Fb1990 => fb1990(s / 86_400.0 / 365_250.0),
        }
    }

    /// ERFA's ±3 ns statement applies (Fb1990 within 1950–2050); `false`
    /// elsewhere and for SpiceEt, whose error is not stated by its source.
    pub fn within_stated_accuracy(self, tt: Instant<Tt>) -> bool {
        matches!(self, Self::Fb1990)
            && (FB1990_STATED_RANGE.0..FB1990_STATED_RANGE.1).contains(&tt.seconds())
    }
}

impl Instant<Tt> {
    /// TDB at this TT instant under `model`.
    pub fn to_tdb(self, model: TdbModel) -> Result<Instant<Tdb>, TimeError> {
        Ok(self
            .checked_add(to_duration(model.tdb_minus_tt(self))?)?
            .relabel())
    }
}

impl Instant<Tdb> {
    /// The TT instant whose TDB under `model` is this one (three fixed-point
    /// steps from TT = TDB; see the module documentation for the bound of a
    /// round trip).
    pub fn to_tt(self, model: TdbModel) -> Result<Instant<Tt>, TimeError> {
        let tdb: Instant<Tt> = self.relabel();
        let mut tt = tdb;
        for _ in 0..3 {
            tt = tdb.checked_sub(to_duration(model.tdb_minus_tt(tt))?)?;
        }
        Ok(tt)
    }
}

fn spice_et(t: f64) -> f64 {
    let m = SPICE_M[0] + SPICE_M[1] * t;
    let e = m + SPICE_EB * sin64(m);
    SPICE_K * sin64(e)
}

/// eraDtdb with `u = v = 0`; `t` in Julian millennia since J2000 TT.
fn fb1990(t: f64) -> f64 {
    let mut w = [0.0f64; 5];
    for (k, wk) in w.iter_mut().enumerate() {
        for j in (BLOCKS[k]..BLOCKS[k + 1]).rev() {
            let [a, f, p] = FAIRHD[j];
            *wk += a * sin64(f * t + p);
        }
    }
    let wf = t * (t * (t * (t * w[4] + w[3]) + w[2]) + w[1]) + w[0];
    let j = &FB1990_JPL_ADJUSTMENT;
    let wj = j[0][0] * sin64(j[0][1] * t + j[0][2])
        + j[1][0] * sin64(j[1][1] * t + j[1][2])
        + j[2][0] * sin64(j[2][1] * t + j[2][2])
        + j[3][0] * sin64(j[3][1] * t + j[3][2])
        + j[4][0] * t * t;
    // eraDtdb returns wt + wf + wj; wt = 0 without an observer offset
    0.0 + wf + wj
}

/// Seconds (`|s| < 1`) to a duration, rounded to the nearest attosecond.
fn to_duration(s: f64) -> Result<Duration, TimeError> {
    // a NaN fails the comparison too
    if s.abs() < 1.0 {
        Duration::from_total_attoseconds(round64(s * 1e18) as i128)
    } else {
        Err(TimeError::OutOfRange)
    }
}
