//! UTC civil time ↔ TAI (Law [`crate::law::UTC_TAI`]).
//!
//! UTC is a labelling of TAI, not a uniform scale, so it is held as a civil
//! date and time ([`UtcDateTime`]) rather than as an [`Instant`]. The
//! conversion is exact: TAI − UTC is evaluated in integer attoseconds, the
//! 1960–1971 drift term `(MJD − ref) × rate` as an exact rational rounded
//! once to the nearest attosecond (ties toward +∞).
//!
//! Valid UTC: 1960-01-01T00:00:00 up to, not including,
//! [`leap::EXPIRES_MJD`] (2027-06-28T00:00:00). On a day that ends with a
//! leap second the last second is 23:59:60; on the days of 1960–1971 the
//! length of the day in UTC seconds is 86400 plus the step of TAI − UTC at the
//! following midnight (e.g. 86400.107758 s on 1971-12-31).

use crate::calendar::{civil_from_mjd, mjd_from_civil};
use crate::instant::{Instant, Tai, ATTO_PER_SEC};
use crate::leap::{self, Segment, SEGMENTS};
use crate::TimeError;

const ATTO: i128 = ATTO_PER_SEC as i128;
const DAY_ATTO: i128 = 86_400 * ATTO;
/// 86400 s/day × 10⁷ (the rate is in 10⁻⁷ s/day).
const RATE_DEN: i128 = 864_000_000_000;
/// MJD of 2000-01-01 (J2000.0 is 12:00 on that day).
const MJD_2000: i64 = 51544;

/// What to do with a UTC date on or after the table's expiry.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ExpiryPolicy {
    /// Return [`TimeError::LeapTableExpired`] (default).
    #[default]
    Strict,
    /// Assume no leap second after the last one in the table. The result is
    /// marked [`Provenance::AssumedNoNewLeapSeconds`].
    AssumeNoNewLeapSeconds,
}

/// Whether a value comes from the table or from an assumption past it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Provenance {
    /// Inside the period the table covers.
    Table,
    /// Past the table's expiry, under [`ExpiryPolicy::AssumeNoNewLeapSeconds`].
    AssumedNoNewLeapSeconds,
}

/// A converted value and where its TAI − UTC came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Converted<T> {
    /// The value.
    pub value: T,
    /// Table, or the assumption past its expiry.
    pub provenance: Provenance,
}

/// A UTC date and time of day (proleptic Gregorian calendar).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UtcDateTime {
    /// Year.
    pub year: i64,
    /// Month, 1–12.
    pub month: u8,
    /// Day of the month.
    pub day: u8,
    /// Hour, 0–23.
    pub hour: u8,
    /// Minute, 0–59.
    pub minute: u8,
    /// Second, 0–59, or 60 during a leap second (23:59:60 only).
    pub second: u8,
    /// Fraction of the second in attoseconds (`< 10¹⁸`).
    pub atto: u64,
}

impl UtcDateTime {
    /// A UTC date and time; only the ranges of each field are checked here,
    /// whether the time exists on that day is checked by the conversions.
    pub fn new(
        year: i64,
        month: u8,
        day: u8,
        hour: u8,
        minute: u8,
        second: u8,
        atto: u64,
    ) -> Result<Self, TimeError> {
        mjd_from_civil(year, month, day)?;
        if hour > 23 || minute > 59 || second > 60 || atto >= ATTO_PER_SEC {
            return Err(TimeError::InvalidTime);
        }
        if second == 60 && (hour != 23 || minute != 59) {
            return Err(TimeError::NotALeapSecond);
        }
        Ok(Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
            atto,
        })
    }

    /// The TAI instant of this UTC label.
    pub fn to_tai(self, policy: ExpiryPolicy) -> Result<Converted<Instant<Tai>>, TimeError> {
        let check = Self::new(
            self.year,
            self.month,
            self.day,
            self.hour,
            self.minute,
            self.second,
            self.atto,
        )?;
        let mjd = mjd_from_civil(check.year, check.month, check.day)?;
        let provenance = coverage(mjd, policy)?;
        let sod = i128::from(
            u32::from(check.hour) * 3600 + u32::from(check.minute) * 60 + u32::from(check.second),
        ) * ATTO
            + i128::from(check.atto);
        let seg = segment_of(mjd)?;
        if sod >= DAY_ATTO + step_at(mjd + 1)? {
            return Err(TimeError::NotALeapSecond);
        }
        let tai = label(mjd) + sod + offset(seg, mjd, sod);
        Ok(Converted {
            value: Instant::from_total_attoseconds(tai)?,
            provenance,
        })
    }

    /// The UTC label of a TAI instant.
    pub fn from_tai(tai: Instant<Tai>, policy: ExpiryPolicy) -> Result<Converted<Self>, TimeError> {
        let t = tai.total_attoseconds();
        let i = SEGMENTS
            .iter()
            .rposition(|s| start_tai(s) <= t)
            .ok_or(TimeError::BeforeUtc)?;
        let seg = &SEGMENTS[i];
        let last = &SEGMENTS[SEGMENTS.len() - 1];
        let provenance = if t >= label(leap::EXPIRES_MJD) + offset(last, leap::EXPIRES_MJD, 0) {
            match policy {
                ExpiryPolicy::Strict => {
                    return Err(TimeError::LeapTableExpired {
                        expires_mjd: leap::EXPIRES_MJD,
                    })
                }
                ExpiryPolicy::AssumeNoNewLeapSeconds => Provenance::AssumedNoNewLeapSeconds,
            }
        } else {
            Provenance::Table
        };
        let end_mjd = SEGMENTS.get(i + 1).map_or(i64::MAX, |n| n.start_mjd);
        // the day: inside a row every UTC day lasts 86400 s or longer on the
        // TAI scale (no row has a negative rate; tests/leap_table.rs), so the
        // estimate from 86400 s days is never early, and late by at most one
        // day (TAI − UTC drifts < 0.003 s per day)
        let mjd = day_of(seg, end_mjd, t)?;
        // t − label(mjd) − delat = sod·(den + R)/den + (mjd − ref)·DAY·R/den
        let r = i128::from(seg.rate_e7);
        let y = t - label(mjd) - i128::from(seg.delat_e7) * 100_000_000_000;
        let num = y * RATE_DEN - i128::from(mjd - seg.ref_mjd) * DAY_ATTO * r;
        let sod = div_round(num, RATE_DEN + r);
        let (year, month, day) = civil_from_mjd(mjd)?;
        let whole = sod.div_euclid(ATTO);
        let atto = sod.rem_euclid(ATTO) as u64;
        // no UTC day lasts 86401 s or more (tests/leap_table.rs), so a second
        // of day ≥ 86400 is the single second 23:59:60
        let (hour, minute, second) = if whole >= 86_400 {
            (23, 59, 60)
        } else {
            (whole / 3600, whole % 3600 / 60, whole % 60)
        };
        Ok(Converted {
            value: Self {
                year,
                month,
                day,
                hour: hour as u8,
                minute: minute as u8,
                second: second as u8,
                atto,
            },
            provenance,
        })
    }
}

/// TAI − UTC in attoseconds at UTC day `mjd`, `sod` attoseconds after 00:00.
pub fn tai_minus_utc(
    mjd: i64,
    sod_atto: i128,
    policy: ExpiryPolicy,
) -> Result<Converted<i128>, TimeError> {
    let provenance = coverage(mjd, policy)?;
    Ok(Converted {
        value: offset(segment_of(mjd)?, mjd, sod_atto),
        provenance,
    })
}

fn coverage(mjd: i64, policy: ExpiryPolicy) -> Result<Provenance, TimeError> {
    if mjd < leap::FIRST_MJD {
        return Err(TimeError::BeforeUtc);
    }
    if mjd >= leap::EXPIRES_MJD {
        return match policy {
            ExpiryPolicy::Strict => Err(TimeError::LeapTableExpired {
                expires_mjd: leap::EXPIRES_MJD,
            }),
            ExpiryPolicy::AssumeNoNewLeapSeconds => Ok(Provenance::AssumedNoNewLeapSeconds),
        };
    }
    Ok(Provenance::Table)
}

fn segment_of(mjd: i64) -> Result<&'static Segment, TimeError> {
    SEGMENTS
        .iter()
        .rev()
        .find(|s| s.start_mjd <= mjd)
        .ok_or(TimeError::BeforeUtc)
}

/// TAI − UTC (attoseconds) of row `seg` at UTC day `mjd` + `sod` attoseconds.
fn offset(seg: &Segment, mjd: i64, sod: i128) -> i128 {
    let constant = i128::from(seg.delat_e7) * 100_000_000_000;
    if seg.rate_e7 == 0 {
        return constant;
    }
    let elapsed = i128::from(mjd - seg.ref_mjd) * DAY_ATTO + sod;
    constant + div_round(elapsed * i128::from(seg.rate_e7), RATE_DEN)
}

/// Step of TAI − UTC at 00:00 UTC of day `mjd` (new row minus the previous
/// row's expression at the same moment); 0 on a day without a change.
fn step_at(mjd: i64) -> Result<i128, TimeError> {
    let after = segment_of(mjd)?;
    if after.start_mjd != mjd {
        return Ok(0);
    }
    let before = segment_of(mjd - 1)?;
    Ok(offset(after, mjd, 0) - offset(before, mjd, 0))
}

/// Attoseconds since J2000.0 of the label "`mjd` 00:00:00" read as if it were
/// on the uniform scale (86400 s per day).
fn label(mjd: i64) -> i128 {
    (i128::from(mjd - MJD_2000) * 86_400 - 43_200) * ATTO
}

/// The UTC day of row `seg` (ending before day `end_mjd`) that holds TAI
/// instant `t` (attoseconds, `t` at or after the row's start).
///
/// The estimate from 86400 s days is never early and late by at most one day
/// (see the caller), so one step back finds the day. Both halves are checked
/// rather than assumed: a row that broke them is an error, not a wrong day.
fn day_of(seg: &Segment, end_mjd: i64, t: i128) -> Result<i64, TimeError> {
    let est = (t - start_tai(seg)).div_euclid(DAY_ATTO);
    let mut mjd = seg.start_mjd + i64::try_from(est).map_err(|_| TimeError::OutOfRange)?;
    mjd = mjd.min(end_mjd - 1);
    if day_start_tai(seg, mjd) > t {
        mjd -= 1;
    }
    let late = day_start_tai(seg, mjd) > t;
    let early = mjd + 1 < end_mjd && day_start_tai(seg, mjd + 1) <= t;
    if late || early {
        return Err(TimeError::OutOfRange);
    }
    Ok(mjd)
}

fn day_start_tai(seg: &Segment, mjd: i64) -> i128 {
    label(mjd) + offset(seg, mjd, 0)
}

fn start_tai(seg: &Segment) -> i128 {
    day_start_tai(seg, seg.start_mjd)
}

/// `n / d` rounded to the nearest integer, ties toward +∞ (`d > 0`).
fn div_round(n: i128, d: i128) -> i128 {
    (2 * n + d).div_euclid(2 * d)
}

#[cfg(test)]
mod tests {
    use super::{day_of, day_start_tai, start_tai, DAY_ATTO};
    use crate::leap::Segment;
    use crate::TimeError;

    /// A row whose TAI − UTC changes by `rate_e7`·10⁻⁷ s per day from MJD
    /// 40000 (no table row has such a rate; they break the invariant that
    /// the 86400 s estimate is never early and late by at most one day).
    fn row(rate_e7: i64) -> Segment {
        Segment {
            start_mjd: 40_000,
            delat_e7: 0,
            ref_mjd: 40_000,
            rate_e7,
        }
    }

    #[test]
    fn a_row_with_days_much_longer_than_86400_s_is_an_error() {
        // each UTC day lasts 11 × 86400 s of TAI: the estimate is ~11× late
        let seg = row(10 * 86_400 * 10_000_000);
        let t = start_tai(&seg) + 50 * DAY_ATTO;
        assert_eq!(day_of(&seg, i64::MAX, t), Err(TimeError::OutOfRange));
    }

    #[test]
    fn a_row_with_days_shorter_than_86400_s_is_an_error() {
        // each UTC day lasts 43200 s of TAI: the estimate is early
        let seg = row(-43_200 * 10_000_000);
        let t = start_tai(&seg) + 50 * DAY_ATTO;
        assert_eq!(day_of(&seg, i64::MAX, t), Err(TimeError::OutOfRange));
    }

    #[test]
    fn an_instant_exactly_at_the_start_of_the_next_day_is_an_error() {
        // days of 86399 s: two days in, the estimate is one day early and the
        // instant is exactly the start of the next day (it belongs to it)
        let seg = row(-10_000_000);
        let t = day_start_tai(&seg, seg.start_mjd + 2);
        assert_eq!(
            t - start_tai(&seg),
            2 * (DAY_ATTO - 1_000_000_000_000_000_000)
        );
        assert_eq!(day_of(&seg, i64::MAX, t), Err(TimeError::OutOfRange));
        // one attosecond earlier the estimate is right
        assert_eq!(day_of(&seg, i64::MAX, t - 1), Ok(seg.start_mjd + 1));
    }

    #[test]
    fn the_day_found_holds_the_instant() {
        // 1961 drift row and a constant row: the first and last instant of a day
        for seg in [
            Segment {
                start_mjd: 37_300,
                delat_e7: 14_228_180,
                ref_mjd: 37_300,
                rate_e7: 12_960,
            },
            row(0),
        ] {
            for d in [0, 1, 7, 400] {
                let start = day_start_tai(&seg, seg.start_mjd + d);
                let next = day_start_tai(&seg, seg.start_mjd + d + 1);
                assert_eq!(day_of(&seg, i64::MAX, start), Ok(seg.start_mjd + d));
                assert_eq!(day_of(&seg, i64::MAX, next - 1), Ok(seg.start_mjd + d));
            }
        }
    }
}
