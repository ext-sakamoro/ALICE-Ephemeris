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
//! SpiceEt evaluates for any instant. The Fb1990 series is only meaningful
//! around 1950–2050; far from J2000 its T¹…T⁴ terms grow without bound (as
//! for ERFA's eraDtdb, whose |TDB − TT| first reaches 1 s at −123,365 and
//! +126,978 years, scanned at 0.005-year steps). Its domain is
//! |t| ≤ [`FB1990_DOMAIN_MILLENNIA`] Julian millennia from J2000 TT (t as
//! the series reads it, from the nearest `f64` of TT). Inside it
//! |TDB − TT| < 1 s is proven, not sampled: every sine is at most 1, so the
//! value is at most Σₖ Wₖ·|t|ᵏ plus the JPL terms (Wₖ the sum of |amplitude|
//! of the Tᵏ block), and with the rounding bound below that is 0.920 s at
//! 110 millennia. The domain is not wider because this bound reaches 1 s
//! before 115 millennia, so 120 millennia, where ERFA is still below 1 s,
//! could not be proven this way (`tests/oracle_tdb.rs`). Outside the domain
//! the series is not evaluated (there its arguments need the slow reduction
//! of `sin64` for huge values, up to 23 times the cost per call, measured in
//! a debug build), [`TdbModel::tdb_minus_tt`] returns NaN, and the
//! conversions return [`TimeError::OutOfRange`], as they do for any shift of
//! 1 s or more; TDB → TT is OutOfRange when the TT it finds is outside.
//!
//! Accuracy of the `f64` evaluation of Fb1990: within a bound derived from
//! its arithmetic (t to 4 u relative, each argument f·t + p to
//! 5 u·|f·t| + u·|f·t + p|, sin64 to 2 ulp, one rounding per product and
//! per sum, Horner's γ₈ for the powers of t; u = 2⁻⁵³) of the same series
//! evaluated exactly — measured at most 0.37 of that bound over 1600–2200.
//! The bound, and so the agreement with any other evaluation such as ERFA's,
//! degrades as t⁴ outside 1950–2050 (`tests/oracle_tdb.rs`).

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

/// Half-width of the [`TdbModel::Fb1990`] domain, in Julian millennia from
/// J2000 TT (110,000 Julian years; the module documentation says why).
pub const FB1990_DOMAIN_MILLENNIA: f64 = 110.0;

impl TdbModel {
    /// The Law this model is.
    pub const fn law(self) -> Law {
        match self {
            Self::SpiceEt => law::TDB_SPICE_ET,
            Self::Fb1990 => law::TDB_FB1990,
        }
    }

    /// TDB − TT in seconds at TT instant `tt`; NaN for [`TdbModel::Fb1990`]
    /// outside its domain ([`FB1990_DOMAIN_MILLENNIA`]).
    pub fn tdb_minus_tt(self, tt: Instant<Tt>) -> f64 {
        let s = tt.to_seconds_f64();
        match self {
            Self::SpiceEt => spice_et(s),
            Self::Fb1990 => {
                let t = s / 86_400.0 / 365_250.0;
                if in_fb1990_domain(t) {
                    fb1990(t)
                } else {
                    f64::NAN
                }
            }
        }
    }

    /// [`TdbModel::tdb_minus_tt`] with t held at the ends of the Fb1990
    /// domain, for the estimates of TDB → TT: an estimate within 1 s of an
    /// end may lie beyond it although the TT sought does not, and an estimate
    /// far beyond it is not evaluated at its slow huge arguments (its result
    /// is OutOfRange anyway). Equal to it inside the domain.
    fn tdb_minus_tt_held(self, tt: Instant<Tt>) -> f64 {
        let s = tt.to_seconds_f64();
        match self {
            Self::SpiceEt => spice_et(s),
            Self::Fb1990 => {
                let t = s / 86_400.0 / 365_250.0;
                fb1990(t.clamp(-FB1990_DOMAIN_MILLENNIA, FB1990_DOMAIN_MILLENNIA))
            }
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
    /// round trip). [`TimeError::OutOfRange`] if that TT is outside the
    /// domain of the model.
    pub fn to_tt(self, model: TdbModel) -> Result<Instant<Tt>, TimeError> {
        let tdb: Instant<Tt> = self.relabel();
        let mut tt = tdb;
        for _ in 0..3 {
            tt = tdb.checked_sub(to_duration(model.tdb_minus_tt_held(tt))?)?;
        }
        if model == TdbModel::Fb1990
            && !in_fb1990_domain(tt.to_seconds_f64() / 86_400.0 / 365_250.0)
        {
            return Err(TimeError::OutOfRange);
        }
        Ok(tt)
    }
}

/// `t` (Julian millennia from J2000 TT, as the series reads it) is inside
/// the Fb1990 domain.
fn in_fb1990_domain(t: f64) -> bool {
    t.abs() <= FB1990_DOMAIN_MILLENNIA
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

#[cfg(test)]
mod tests {
    use super::to_duration;
    use crate::{Duration, Instant, TdbModel, TimeError, Tt};

    #[test]
    fn shifts_of_one_second_or_more_are_out_of_range() {
        // |TDB − TT| < 1 s is the documented domain; ±1.0 itself is outside
        for s in [1.0, -1.0, 1.5, f64::NAN, f64::INFINITY] {
            assert_eq!(to_duration(s), Err(TimeError::OutOfRange), "{s}");
        }
        let d = to_duration(-0.25).unwrap();
        assert_eq!(
            (d.seconds(), d.attoseconds()),
            (-1, 750_000_000_000_000_000)
        );
        assert!(to_duration(0.999_999_999_999_999_9).is_ok());
    }

    #[test]
    fn fb1990_domain_ends_at_110_millennia() {
        // 110 Julian millennia is 3_471_210_000_000 s, and t is exact there;
        // half a second is far more than 1 ulp of the f64 seconds (2⁻¹¹ s)
        let end = 110 * 365_250 * 86_400;
        let half = 500_000_000_000_000_000;
        for (sec, atto, inside) in [
            (end, 0, true),
            (end, half, false),
            (end + 1, 0, false),
            (end - 1, half, true),
            (-end, 0, true),
            (-end - 1, half, false),
            (-end - 1, 0, false),
            (-end, half, true),
        ] {
            let tt = Instant::<Tt>::new(sec, atto).unwrap();
            let d = TdbModel::Fb1990.tdb_minus_tt(tt);
            assert_eq!(d.is_nan(), !inside, "{sec} {atto}");
            assert_eq!(tt.to_tdb(TdbModel::Fb1990).is_ok(), inside, "{sec} {atto}");
            if inside {
                // the domain ends before the shift reaches 1 s
                assert!(d.abs() < 0.99, "{sec} {atto}: {d}");
            }
        }
        // TT at either end goes to TDB and back; a TDB whose TT would lie
        // beyond an end is OutOfRange
        for sec in [end, -end] {
            let tt = Instant::<Tt>::new(sec, 0).unwrap();
            let tdb = tt.to_tdb(TdbModel::Fb1990).unwrap();
            assert_eq!(tdb.to_tt(TdbModel::Fb1990), Ok(tt), "{sec}");
            let step =
                Duration::from_total_attoseconds(sec.signum() as i128 * 1_000_000_000_000_000_000)
                    .unwrap();
            let beyond = tdb.checked_add(step).unwrap();
            assert_eq!(
                beyond.to_tt(TdbModel::Fb1990),
                Err(TimeError::OutOfRange),
                "{sec}"
            );
        }
        // the other model has no such end
        let far = Instant::<Tt>::new(end + 1, 0).unwrap();
        assert!(far.to_tdb(TdbModel::SpiceEt).is_ok());
    }
}
