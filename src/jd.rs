//! Julian Date (two-part, as ERFA / SOFA-style interfaces take it) and
//! Modified Julian Date (exact integer parts), on any scale (Law
//! [`crate::law::JULIAN_DATE`]).

use crate::instant::{Instant, Scale, ATTO_PER_SEC};
use crate::TimeError;

/// Julian Date of J2000.0.
pub const JD_J2000: f64 = 2_451_545.0;

/// A Julian Date split in two `f64`s: `jd1` the whole day count (here always
/// an integer + 0.0, i.e. noon), `jd2` the fraction in `[0, 1)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Jd2 {
    /// Whole part.
    pub jd1: f64,
    /// Fractional part.
    pub jd2: f64,
}

/// An exact Modified Julian Date: day number, and time of day since 00:00.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Mjd {
    /// MJD day number (MJD 51544 = 2000-01-01).
    pub day: i64,
    /// Seconds since 00:00 of that day (`0 ..= 86399`).
    pub second_of_day: u32,
    /// Attoseconds within the second.
    pub atto: u64,
}

impl<S: Scale> Instant<S> {
    /// Two-part Julian Date. `jd1` is exact (`|day| < 2⁵³`); `jd2` is the
    /// fraction of the day rounded once to `f64` (≤ 1 ulp, about
    /// 1.1 × 10⁻¹⁶ day ≈ 10 ps).
    pub fn to_jd2(self) -> Jd2 {
        let day = self.seconds().div_euclid(86_400);
        let rem = self.seconds().rem_euclid(86_400);
        let frac = (rem as f64 + self.attoseconds() as f64 * 1e-18) / 86_400.0;
        // a fraction that rounds to 1.0 belongs to the next day
        if frac >= 1.0 {
            Jd2 {
                jd1: JD_J2000 + (day + 1) as f64,
                jd2: 0.0,
            }
        } else {
            Jd2 {
                jd1: JD_J2000 + day as f64,
                jd2: frac,
            }
        }
    }

    /// The instant of a two-part Julian Date (any split of the sum). The
    /// result is as precise as the `f64` inputs: the two fractional parts
    /// are added once, then scaled to seconds once.
    pub fn from_jd2(jd: Jd2) -> Result<Self, TimeError> {
        let (d1, f1) = split(jd.jd1)?;
        let (d2, f2) = split(jd.jd2)?;
        let mut days = d1
            .checked_add(d2)
            .and_then(|d| d.checked_sub(2_451_545))
            .ok_or(TimeError::OutOfRange)?;
        let mut frac = f1 + f2;
        if frac >= 1.0 {
            days += 1;
            frac -= 1.0;
        }
        let secs = frac * 86_400.0;
        let whole = floor_i64(secs)?;
        let atto = alice_det_math::round64((secs - whole as f64) * 1e18) as i128;
        // |days| < 2^63 so days·86400 fits i128; the product with 10^18 may not
        let total = (i128::from(days) * 86_400 + i128::from(whole))
            .checked_mul(ATTO_PER_SEC as i128)
            .ok_or(TimeError::OutOfRange)?
            + atto;
        Self::from_total_attoseconds(total)
    }

    /// Exact Modified Julian Date (MJD 0 = JD 2400000.5, days begin at 00:00).
    pub fn to_mjd(self) -> Mjd {
        // J2000.0 is MJD 51544.5
        let s = i128::from(self.seconds()) + 51_544 * 86_400 + 43_200;
        Mjd {
            day: s.div_euclid(86_400) as i64,
            second_of_day: s.rem_euclid(86_400) as u32,
            atto: self.attoseconds(),
        }
    }

    /// The instant of an exact Modified Julian Date.
    pub fn from_mjd(mjd: Mjd) -> Result<Self, TimeError> {
        if mjd.second_of_day >= 86_400 {
            return Err(TimeError::InvalidTime);
        }
        let s = (i128::from(mjd.day) - 51_544) * 86_400 + i128::from(mjd.second_of_day) - 43_200;
        let sec = i64::try_from(s).map_err(|_| TimeError::OutOfRange)?;
        Self::new(sec, mjd.atto)
    }
}

/// `x = d + f` with `d` an integer and `f ∈ [0, 1)` (exact).
fn split(x: f64) -> Result<(i64, f64), TimeError> {
    let d = floor_i64(x)?;
    Ok((d, x - d as f64))
}

/// `floor(x)` for finite `|x| < 2⁶²`; only casts and comparisons.
fn floor_i64(x: f64) -> Result<i64, TimeError> {
    // a NaN fails the comparison too
    if x.abs() < 4.611_686_018_427_388e18 {
        let t = x as i64;
        Ok(if (t as f64) > x { t - 1 } else { t })
    } else {
        Err(TimeError::OutOfRange)
    }
}
