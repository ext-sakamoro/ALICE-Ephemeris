//! Proleptic Gregorian calendar ↔ Modified Julian Date, in integers.
//!
//! The day count is H. Hinnant's `days_from_civil` / `civil_from_days`
//! (public domain), shifted from 1970-01-01 (MJD 40587) to MJD.

use crate::TimeError;

const MJD_OF_1970: i64 = 40587;

/// Days in `month` of `year` (Gregorian).
pub const fn days_in_month(year: i64, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

/// MJD of the date. Years are limited to ±10⁹ (the result then fits easily).
pub fn mjd_from_civil(year: i64, month: u8, day: u8) -> Result<i64, TimeError> {
    if !(-1_000_000_000..=1_000_000_000).contains(&year) || !(1..=12).contains(&month) {
        return Err(TimeError::InvalidDate);
    }
    if day == 0 || day > days_in_month(year, month) {
        return Err(TimeError::InvalidDate);
    }
    let (m, d) = (i64::from(month), i64::from(day));
    let y = if m <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Ok(era * 146_097 + doe - 719_468 + MJD_OF_1970)
}

/// `(year, month, day)` of an MJD.
pub fn civil_from_mjd(mjd: i64) -> Result<(i64, u8, u8), TimeError> {
    // keeps every intermediate far from i64 overflow
    if !(-400_000_000_000..=400_000_000_000).contains(&mjd) {
        return Err(TimeError::OutOfRange);
    }
    let z = mjd - MJD_OF_1970 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    Ok((y, m as u8, d as u8))
}
