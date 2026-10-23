//! The TAI − UTC table (a Law: [`crate::law::UTC_TAI`]).
//!
//! UTC began on 1960-01-01. Until 1972-01-01 TAI − UTC was a linear function
//! of the date ("rubber seconds"); since then it changes only by whole leap
//! seconds at the end of a UTC day.
//!
//! - 1960–1971: the expressions of USNO `tai-utc.dat`, as tabulated in ERFA
//!   v2.0.1 `src/dat.c` (BSD-3-Clause, see THIRD-PARTY-NOTICES). Each row is
//!   `TAI − UTC = delat + (MJD − ref) × rate` with `delat` in 10⁻⁷ s,
//!   `rate` in 10⁻⁷ s/day and MJD the UTC date including the fraction of
//!   the day.
//! - 1972 onwards: the IERS leap seconds, as published in the IETF/IANA
//!   `leap-seconds.list` (updated through IERS Bulletin C; a copy is in
//!   `tests/data/leap-seconds.list` and a test checks this table against it
//!   and against the file's own SHA-1).
//!
//! The table is valid for UTC dates before [`EXPIRES_MJD`] (the file's
//! expiry, 2027-06-28). After it a leap second may have been announced that
//! this table does not know, so the conversions return
//! [`crate::TimeError::LeapTableExpired`] unless the caller opts in to
//! [`crate::ExpiryPolicy::AssumeNoNewLeapSeconds`].

/// One row of the table: from UTC day `start_mjd` (00:00 UTC) until the next
/// row, `TAI − UTC = delat_e7·10⁻⁷ + (MJD − ref_mjd) × rate_e7·10⁻⁷` seconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Segment {
    /// First UTC day (Modified Julian Date) the row applies to.
    pub start_mjd: i64,
    /// Constant part of TAI − UTC, in units of 10⁻⁷ s.
    pub delat_e7: i64,
    /// Reference date of the drift term (MJD); 0 when `rate_e7` is 0.
    pub ref_mjd: i64,
    /// Drift of TAI − UTC, in units of 10⁻⁷ s per day; 0 from 1972 on.
    pub rate_e7: i64,
}

impl Segment {
    const fn drift(start_mjd: i64, delat_e7: i64, ref_mjd: i64, rate_e7: i64) -> Self {
        Self {
            start_mjd,
            delat_e7,
            ref_mjd,
            rate_e7,
        }
    }

    const fn leap(start_mjd: i64, tai_minus_utc: i64) -> Self {
        Self {
            start_mjd,
            delat_e7: tai_minus_utc * 10_000_000,
            ref_mjd: 0,
            rate_e7: 0,
        }
    }
}

/// First UTC day of the table: 1960-01-01.
pub const FIRST_MJD: i64 = 36934;

/// First day of the leap-second era: 1972-01-01.
pub const LEAP_ERA_MJD: i64 = 41317;

/// The table is valid for UTC dates strictly before this day (2027-06-28,
/// "File expires on 28 June 2027" in `leap-seconds.list`).
pub const EXPIRES_MJD: i64 = 61584;

/// `#$` line of `leap-seconds.list`: last update, NTP seconds.
pub const SOURCE_UPDATED_NTP: u64 = 3_992_312_697;

/// `#@` line of `leap-seconds.list`: expiry, NTP seconds.
pub const SOURCE_EXPIRES_NTP: u64 = 4_023_129_600;

/// `#h` line of `leap-seconds.list`: SHA-1 over its data, as published.
pub const SOURCE_SHA1: [u32; 5] = [
    0xa9ba_d145,
    0x84c3_1c70,
    0x7584_02aa,
    0xb37b_fd54,
    0x5923_836a,
];

/// The rows, in date order.
#[rustfmt::skip]
pub const SEGMENTS: [Segment; 42] = [
    // 1960–1971 (ERFA dat.c, from USNO tai-utc.dat)
    Segment::drift(36934, 14_178_180, 37300, 12960), // 1960-01-01
    Segment::drift(37300, 14_228_180, 37300, 12960), // 1961-01-01
    Segment::drift(37512, 13_728_180, 37300, 12960), // 1961-08-01
    Segment::drift(37665, 18_458_580, 37665, 11232), // 1962-01-01
    Segment::drift(38334, 19_458_580, 37665, 11232), // 1963-11-01
    Segment::drift(38395, 32_401_300, 38761, 12960), // 1964-01-01
    Segment::drift(38486, 33_401_300, 38761, 12960), // 1964-04-01
    Segment::drift(38639, 34_401_300, 38761, 12960), // 1964-09-01
    Segment::drift(38761, 35_401_300, 38761, 12960), // 1965-01-01
    Segment::drift(38820, 36_401_300, 38761, 12960), // 1965-03-01
    Segment::drift(38942, 37_401_300, 38761, 12960), // 1965-07-01
    Segment::drift(39004, 38_401_300, 38761, 12960), // 1965-09-01
    Segment::drift(39126, 43_131_700, 39126, 25920), // 1966-01-01
    Segment::drift(39887, 42_131_700, 39126, 25920), // 1968-02-01
    // 1972 onwards (leap-seconds.list)
    Segment::leap(41317, 10), // 1972-01-01
    Segment::leap(41499, 11), // 1972-07-01
    Segment::leap(41683, 12), // 1973-01-01
    Segment::leap(42048, 13), // 1974-01-01
    Segment::leap(42413, 14), // 1975-01-01
    Segment::leap(42778, 15), // 1976-01-01
    Segment::leap(43144, 16), // 1977-01-01
    Segment::leap(43509, 17), // 1978-01-01
    Segment::leap(43874, 18), // 1979-01-01
    Segment::leap(44239, 19), // 1980-01-01
    Segment::leap(44786, 20), // 1981-07-01
    Segment::leap(45151, 21), // 1982-07-01
    Segment::leap(45516, 22), // 1983-07-01
    Segment::leap(46247, 23), // 1985-07-01
    Segment::leap(47161, 24), // 1988-01-01
    Segment::leap(47892, 25), // 1990-01-01
    Segment::leap(48257, 26), // 1991-01-01
    Segment::leap(48804, 27), // 1992-07-01
    Segment::leap(49169, 28), // 1993-07-01
    Segment::leap(49534, 29), // 1994-07-01
    Segment::leap(50083, 30), // 1996-01-01
    Segment::leap(50630, 31), // 1997-07-01
    Segment::leap(51179, 32), // 1999-01-01
    Segment::leap(53736, 33), // 2006-01-01
    Segment::leap(54832, 34), // 2009-01-01
    Segment::leap(56109, 35), // 2012-07-01
    Segment::leap(57204, 36), // 2015-07-01
    Segment::leap(57754, 37), // 2017-01-01
];
