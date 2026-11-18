//! Unix time: an instant in UTC counted as POSIX time, to the nanosecond.
//!
//! [`Timestamp`] counts seconds since 1970-01-01T00:00:00Z with every day 86 400 s long (POSIX
//! time: no leap seconds; for UTC with its leap seconds see [`crate::UtcDateTime`]), plus the
//! nanoseconds within the second. The calendar is the proleptic Gregorian one
//! ([`crate::civil_from_mjd`] / [`crate::mjd_from_civil`]). The range is the years −262143 to
//! 262142.
//!
//! Everything here is `core` only; the text forms are [`core::fmt::Display`] values, so writing one
//! needs no allocation:
//!
//! | form | example | from |
//! |---|---|---|
//! | RFC 3339, `Z` (also the serde form) | `2023-11-14T22:13:20.123Z` | [`Timestamp::rfc3339_z`] |
//! | RFC 3339, `+00:00` | `2023-11-14T22:13:20.123+00:00` | [`Timestamp::rfc3339`] |
//! | RFC 3339, `Z`, 3 fraction digits | `2023-11-14T22:13:20.000Z` (truncated) | [`Timestamp::rfc3339_millis_z`] |
//! | date at an offset | `2023-11-15` (at `+09:00`) | [`Timestamp::civil_date_at_offset`], [`Timestamp::civil_date_in`] |
//! | `Display` | `2023-11-14 22:13:20.123 UTC` | `{}` |
//! | compact | `20231114T221320Z` (whole seconds) | [`Timestamp::compact`] |
//!
//! A local date takes its offset from a [`UtcOffsetAt`] (a closure, or with the `os-local` feature
//! the OS's zone, `OsLocalZone`, the only part that reads the OS).
//!
//! The fraction is the fewest of 0, 3, 6 or 9 digits that hold the nanoseconds. A year outside
//! 0..=9999 is written with its sign and at least 4 digits (`+10000`, `-0001`).
//!
//! Counts round down: [`Timestamp::unix_seconds`] and [`Timestamp::unix_millis`] of one
//! nanosecond before the epoch are −1. Differences ([`Timestamp::whole_seconds_since`],
//! [`Timestamp::whole_days_since`]) round toward zero.
//!
//! With the `std` feature, [`Timestamp::from_system_time`] converts a `std::time::SystemTime` (this
//! crate never reads the clock itself). With the `serde` feature, a timestamp serializes as its
//! [`Timestamp::rfc3339_z`] string and deserializes from any text [`Timestamp::parse_rfc3339`]
//! reads.
//!
//! ```
//! use alice_ephemeris::time::Timestamp;
//!
//! let t = Timestamp::from_unix(1_700_000_000, 123_000_000).unwrap();
//! assert_eq!(t.rfc3339_z().to_string(), "2023-11-14T22:13:20.123Z");
//! assert_eq!(t.unix_millis(), 1_700_000_000_123);
//! assert_eq!(Timestamp::parse_rfc3339("2023-11-15T07:13:20.123+09:00"), Some(t));
//! assert_eq!(t.plus_days(1).whole_days_since(t), 1);
//! ```

use core::fmt;
use core::str::FromStr;

use crate::calendar::{civil_from_mjd, days_in_month, mjd_from_civil};

const NANOS_PER_SEC: u32 = 1_000_000_000;
const SECS_PER_DAY: i64 = 86_400;
/// MJD of 1970-01-01.
const MJD_OF_1970: i64 = 40_587;
/// −262143-01-01T00:00:00Z.
const MIN_SECS: i64 = -8_334_601_228_800;
/// 262142-12-31T23:59:59Z.
const MAX_SECS: i64 = 8_210_266_876_799;

/// An instant in POSIX time: whole seconds since 1970-01-01T00:00:00Z and the nanoseconds within
/// that second (below 10⁹, also before 1970: one nanosecond before the epoch is
/// `(-1, 999_999_999)`). Ordered by the instant. See the [module documentation](self).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp {
    secs: i64,
    nanos: u32,
}

impl Timestamp {
    /// 1970-01-01T00:00:00Z.
    pub const UNIX_EPOCH: Self = Self { secs: 0, nanos: 0 };
    /// The earliest instant: −262143-01-01T00:00:00Z.
    pub const MIN: Self = Self {
        secs: MIN_SECS,
        nanos: 0,
    };
    /// The latest instant: 262142-12-31T23:59:59.999999999Z.
    pub const MAX: Self = Self {
        secs: MAX_SECS,
        nanos: NANOS_PER_SEC - 1,
    };

    /// The instant `secs` whole seconds and `nanos` nanoseconds after the epoch; `None` when
    /// `nanos` is 10⁹ or more or the instant is outside the range.
    #[must_use]
    pub const fn from_unix(secs: i64, nanos: u32) -> Option<Self> {
        if nanos >= NANOS_PER_SEC || secs < MIN_SECS || secs > MAX_SECS {
            None
        } else {
            Some(Self { secs, nanos })
        }
    }

    /// The instant of a unix millisecond count; `None` outside the range.
    #[must_use]
    pub const fn from_unix_millis(ms: i64) -> Option<Self> {
        let secs = ms.div_euclid(1000);
        // rem_euclid(1000) is in 0..1000
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let nanos = (ms.rem_euclid(1000) as u32) * 1_000_000;
        Self::from_unix(secs, nanos)
    }

    /// The instant of a `SystemTime` (held at [`Timestamp::MIN`] / [`Timestamp::MAX`] outside the
    /// range). The resolution is the platform's `SystemTime` (100 ns on Windows, 1 ns or 1 µs on
    /// unix): the nanoseconds below it are 0.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn from_system_time(t: std::time::SystemTime) -> Self {
        match t.duration_since(std::time::UNIX_EPOCH) {
            Ok(after) => i64::try_from(after.as_secs())
                .ok()
                .and_then(|s| Self::from_unix(s, after.subsec_nanos()))
                .unwrap_or(Self::MAX),
            Err(before) => {
                let d = before.duration();
                let whole = i64::try_from(d.as_secs()).ok();
                let (secs, nanos) = if d.subsec_nanos() == 0 {
                    (whole.map(|s| -s), 0)
                } else {
                    (whole.map(|s| -s - 1), NANOS_PER_SEC - d.subsec_nanos())
                };
                secs.and_then(|s| Self::from_unix(s, nanos))
                    .unwrap_or(Self::MIN)
            }
        }
    }

    /// Whole seconds since the epoch, rounded down.
    #[must_use]
    pub const fn unix_seconds(self) -> i64 {
        self.secs
    }

    /// The nanoseconds within the second (below 10⁹).
    #[must_use]
    pub const fn subsec_nanos(self) -> u32 {
        self.nanos
    }

    /// Whole milliseconds since the epoch, rounded down (the range keeps it far from overflow).
    #[must_use]
    pub const fn unix_millis(self) -> i64 {
        self.secs * 1000 + (self.nanos / 1_000_000) as i64
    }

    /// `days` days later (earlier when negative), at the same time of day; held at
    /// [`Timestamp::MIN`] / [`Timestamp::MAX`] past the range.
    #[must_use]
    pub fn plus_days(self, days: i64) -> Self {
        let secs = i128::from(self.secs) + i128::from(days) * i128::from(SECS_PER_DAY);
        match i64::try_from(secs) {
            Ok(s) if (MIN_SECS..=MAX_SECS).contains(&s) => Self {
                secs: s,
                nanos: self.nanos,
            },
            _ if secs < 0 => Self::MIN,
            _ => Self::MAX,
        }
    }

    /// `days` days later (earlier when negative), at the same time of day; `None` past the range
    /// (where [`Timestamp::plus_days`] holds at the end).
    #[must_use]
    pub fn checked_plus_days(self, days: i64) -> Option<Self> {
        self.checked_plus_seconds_i128(i128::from(days) * i128::from(SECS_PER_DAY))
    }

    /// `secs` seconds later (earlier when negative); held at [`Timestamp::MIN`] /
    /// [`Timestamp::MAX`] past the range.
    #[must_use]
    pub fn plus_seconds(self, secs: i64) -> Self {
        self.checked_plus_seconds(secs)
            .unwrap_or(if secs < 0 { Self::MIN } else { Self::MAX })
    }

    /// `secs` seconds later (earlier when negative); `None` past the range.
    #[must_use]
    pub fn checked_plus_seconds(self, secs: i64) -> Option<Self> {
        self.checked_plus_seconds_i128(i128::from(secs))
    }

    fn checked_plus_seconds_i128(self, secs: i128) -> Option<Self> {
        let s = i64::try_from(i128::from(self.secs) + secs).ok()?;
        (MIN_SECS..=MAX_SECS).contains(&s).then_some(Self {
            secs: s,
            nanos: self.nanos,
        })
    }

    /// Whole seconds from `earlier` to `self`, rounded toward zero (negative when `self` is
    /// earlier).
    #[must_use]
    // within the range the difference in seconds fits an i64
    #[allow(clippy::cast_possible_truncation)]
    pub fn whole_seconds_since(self, earlier: Self) -> i64 {
        let diff = (i128::from(self.secs) - i128::from(earlier.secs)) * i128::from(NANOS_PER_SEC)
            + (i128::from(self.nanos) - i128::from(earlier.nanos));
        (diff / i128::from(NANOS_PER_SEC)) as i64
    }

    /// Whole days (86 400 s) from `earlier` to `self`, rounded toward zero.
    #[must_use]
    pub fn whole_days_since(self, earlier: Self) -> i64 {
        self.whole_seconds_since(earlier) / SECS_PER_DAY
    }

    /// The RFC 3339 text with `Z`, the serde form: `2023-11-14T22:13:20.123Z`.
    #[must_use]
    pub const fn rfc3339_z(self) -> Rfc3339 {
        Rfc3339 {
            time: self,
            zone: Zone::Z,
            millis: false,
        }
    }

    /// The RFC 3339 text with `Z` and always 3 fraction digits, the nanoseconds past the
    /// millisecond dropped: `2023-11-14T22:13:20.000Z`, `2023-11-14T22:13:20.123Z` for
    /// `.123999999`.
    #[must_use]
    pub const fn rfc3339_millis_z(self) -> Rfc3339 {
        Rfc3339 {
            time: self,
            zone: Zone::Z,
            millis: true,
        }
    }

    /// The RFC 3339 text with `+00:00`: `2023-11-14T22:13:20.123+00:00`.
    #[must_use]
    pub const fn rfc3339(self) -> Rfc3339 {
        Rfc3339 {
            time: self,
            zone: Zone::Offset,
            millis: false,
        }
    }

    /// `YYYYMMDDTHHMMSSZ`, whole seconds: `20231114T221320Z`.
    #[must_use]
    pub const fn compact(self) -> Compact {
        Compact(self)
    }

    /// Reads RFC 3339 text: `YYYY-MM-DD`, then `T`, `t` or a space, `HH:MM:SS`, an optional
    /// fraction (digits past the ninth are dropped), then `Z`, `z`, `UTC` or `±HH:MM`. `None` for
    /// anything else, a date that does not exist, a second 60 or an instant outside the range.
    #[must_use]
    pub fn parse_rfc3339(text: &str) -> Option<Self> {
        let mut p = Cursor(text.as_bytes());
        let year = p.year()?;
        p.lit(b'-')?;
        let month = p.two()?;
        p.lit(b'-')?;
        let day = p.two()?;
        if !matches!(p.next()?, b'T' | b't' | b' ') {
            return None;
        }
        let hour = p.two()?;
        p.lit(b':')?;
        let minute = p.two()?;
        p.lit(b':')?;
        let second = p.two()?;
        let nanos = if p.peek() == Some(b'.') {
            p.next();
            p.fraction()?
        } else {
            0
        };
        let offset = p.zone()?;
        if !p.0.is_empty()
            || !(1..=12).contains(&month)
            || day == 0
            || day > days_in_month(year, month)
            || hour > 23
            || minute > 59
            || second > 59
        {
            return None;
        }
        let days = mjd_from_civil(year, month, day).ok()? - MJD_OF_1970;
        let secs = days
            .checked_mul(SECS_PER_DAY)?
            .checked_add(i64::from(hour) * 3600 + i64::from(minute) * 60 + i64::from(second))?
            .checked_sub(offset)?;
        Self::from_unix(secs, nanos)
    }

    /// The calendar date at a UTC offset of `offset_secs` seconds east (Tokyo `32_400`, New York
    /// in January `-18_000`): the date of a wall clock that far ahead of UTC at this instant.
    /// `None` for an offset of a day or more.
    #[must_use]
    pub fn civil_date_at_offset(self, offset_secs: i32) -> Option<CivilDate> {
        if offset_secs.unsigned_abs() >= 86_400 {
            return None;
        }
        let local = self.secs + i64::from(offset_secs);
        let (year, month, day) =
            civil_from_mjd(local.div_euclid(SECS_PER_DAY) + MJD_OF_1970).ok()?;
        Some(CivilDate { year, month, day })
    }

    /// The calendar date in `zone` at this instant: [`Timestamp::civil_date_at_offset`] with the
    /// offset `zone` gives for this instant (so the two sides of a daylight saving change get
    /// their own offsets). `None` when `zone` has no offset for it.
    #[must_use]
    pub fn civil_date_in(self, zone: &impl UtcOffsetAt) -> Option<CivilDate> {
        self.civil_date_at_offset(zone.utc_offset_at(self)?)
    }

    /// The date and time of day; an error only outside the calendar's range, which the type's
    /// range keeps away from.
    fn civil(self) -> Result<Civil, fmt::Error> {
        let (year, month, day) = civil_from_mjd(self.secs.div_euclid(SECS_PER_DAY) + MJD_OF_1970)
            .map_err(|_| fmt::Error)?;
        let rem = self.secs.rem_euclid(SECS_PER_DAY);
        // rem is in 0..86 400
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let (hour, minute, second) = ((rem / 3600) as u8, (rem / 60 % 60) as u8, (rem % 60) as u8);
        Ok(Civil {
            year,
            month,
            day,
            hour,
            minute,
            second,
        })
    }
}

/// A calendar date (proleptic Gregorian), from [`Timestamp::civil_date_at_offset`]. Its
/// `Display` is `YYYY-MM-DD` (a year outside 0..=9999 with its sign and at least 4 digits).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct CivilDate {
    year: i64,
    month: u8,
    day: u8,
}

impl CivilDate {
    /// The year (astronomical numbering: 0 is 1 BC).
    #[must_use]
    pub const fn year(self) -> i64 {
        self.year
    }
    /// The month, 1 to 12.
    #[must_use]
    pub const fn month(self) -> u8 {
        self.month
    }
    /// The day of the month, from 1.
    #[must_use]
    pub const fn day(self) -> u8 {
        self.day
    }
}

impl fmt::Display for CivilDate {
    /// `YYYY-MM-DD`
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        year(f, self.year)?;
        write!(f, "-{:02}-{:02}", self.month, self.day)
    }
}

/// A source of the UTC offset (seconds east) in force at an instant: a time zone. A closure
/// `|t: Timestamp| Some(offset)` is one (for tests, or a zone the caller reads itself); with the
/// `os-local` feature, `OsLocalZone` is the OS's local zone.
pub trait UtcOffsetAt {
    /// The offset in seconds east of UTC at `t`, `None` when there is none.
    fn utc_offset_at(&self, t: Timestamp) -> Option<i32>;
}

impl<F: Fn(Timestamp) -> Option<i32>> UtcOffsetAt for F {
    fn utc_offset_at(&self, t: Timestamp) -> Option<i32> {
        self(t)
    }
}

/// The local time zone of the operating system (feature `os-local`), read at each instant asked:
/// on unix the `TZ` variable or the system zone, on Windows the account's zone with the daylight
/// saving rules of the instant's year. `None` where the OS has no zone (wasm) or cannot represent
/// the instant (Windows before 1601).
#[cfg(feature = "os-local")]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct OsLocalZone;

#[cfg(feature = "os-local")]
impl UtcOffsetAt for OsLocalZone {
    fn utc_offset_at(&self, t: Timestamp) -> Option<i32> {
        alice_ephemeris_os_local::utc_offset_at_unix(t.secs)
    }
}

struct Civil {
    year: i64,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
}

impl Civil {
    fn date(&self, f: &mut fmt::Formatter<'_>, sep: &str) -> fmt::Result {
        year(f, self.year)?;
        write!(f, "{sep}{:02}{sep}{:02}", self.month, self.day)
    }
    fn time(&self, f: &mut fmt::Formatter<'_>, sep: &str) -> fmt::Result {
        write!(
            f,
            "{:02}{sep}{:02}{sep}{:02}",
            self.hour, self.minute, self.second
        )
    }
}

/// 4 digits for 0..=9999, otherwise the sign and at least 4 digits.
fn year(f: &mut fmt::Formatter<'_>, year: i64) -> fmt::Result {
    if (0..=9999).contains(&year) {
        write!(f, "{year:04}")
    } else {
        write!(f, "{year:+05}")
    }
}

/// Nothing for 0, else `.` and 3, 6 or 9 digits: the fewest that hold `nanos`.
fn fraction(f: &mut fmt::Formatter<'_>, nanos: u32) -> fmt::Result {
    if nanos == 0 {
        Ok(())
    } else if nanos % 1_000_000 == 0 {
        write!(f, ".{:03}", nanos / 1_000_000)
    } else if nanos % 1_000 == 0 {
        write!(f, ".{:06}", nanos / 1_000)
    } else {
        write!(f, ".{nanos:09}")
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Zone {
    Z,
    Offset,
}

/// The RFC 3339 text of a [`Timestamp`] ([`Timestamp::rfc3339_z`], [`Timestamp::rfc3339`],
/// [`Timestamp::rfc3339_millis_z`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rfc3339 {
    time: Timestamp,
    zone: Zone,
    /// always 3 fraction digits (truncated) instead of the fewest that hold the value
    millis: bool,
}

impl fmt::Display for Rfc3339 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let c = self.time.civil()?;
        c.date(f, "-")?;
        f.write_str("T")?;
        c.time(f, ":")?;
        if self.millis {
            write!(f, ".{:03}", self.time.nanos / 1_000_000)?;
        } else {
            fraction(f, self.time.nanos)?;
        }
        f.write_str(match self.zone {
            Zone::Z => "Z",
            Zone::Offset => "+00:00",
        })
    }
}

/// `YYYYMMDDTHHMMSSZ` ([`Timestamp::compact`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Compact(Timestamp);

impl fmt::Display for Compact {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let c = self.0.civil()?;
        c.date(f, "")?;
        f.write_str("T")?;
        c.time(f, "")?;
        f.write_str("Z")
    }
}

impl fmt::Display for Timestamp {
    /// `YYYY-MM-DD HH:MM:SS[.fraction] UTC`
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let c = self.civil()?;
        c.date(f, "-")?;
        f.write_str(" ")?;
        c.time(f, ":")?;
        fraction(f, self.nanos)?;
        f.write_str(" UTC")
    }
}

impl fmt::Debug for Timestamp {
    /// The RFC 3339 `Z` form.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.rfc3339_z(), f)
    }
}

/// [`Timestamp::parse_rfc3339`] did not read the text.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ParseTimestampError;

impl fmt::Display for ParseTimestampError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("not an RFC 3339 date and time in range")
    }
}

impl FromStr for Timestamp {
    type Err = ParseTimestampError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse_rfc3339(s).ok_or(ParseTimestampError)
    }
}

#[cfg(any(feature = "serde", test))]
/// Room for the longest text, `+262142-12-31T23:59:59.999999999+00:00` (38 bytes).
const TEXT_MAX: usize = 40;

#[cfg(any(feature = "serde", test))]
/// A fixed buffer a text form is written into (no allocation).
struct Text {
    buf: [u8; TEXT_MAX],
    len: usize,
}

#[cfg(any(feature = "serde", test))]
impl Text {
    fn of(value: impl fmt::Display) -> Result<Self, fmt::Error> {
        use fmt::Write;
        let mut text = Self {
            buf: [0; TEXT_MAX],
            len: 0,
        };
        write!(text, "{value}")?;
        Ok(text)
    }
    fn as_str(&self) -> Result<&str, fmt::Error> {
        core::str::from_utf8(&self.buf[..self.len]).map_err(|_| fmt::Error)
    }
}

#[cfg(any(feature = "serde", test))]
impl fmt::Write for Text {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let end = self.len + s.len();
        self.buf
            .get_mut(self.len..end)
            .ok_or(fmt::Error)?
            .copy_from_slice(s.as_bytes());
        self.len = end;
        Ok(())
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Timestamp {
    /// The [`Timestamp::rfc3339_z`] string.
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let text =
            Text::of(self.rfc3339_z()).map_err(|_| serde::ser::Error::custom("timestamp text"))?;
        let s = text
            .as_str()
            .map_err(|_| serde::ser::Error::custom("timestamp text"))?;
        serializer.serialize_str(s)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Timestamp {
    /// Any text [`Timestamp::parse_rfc3339`] reads.
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl serde::de::Visitor<'_> for Visitor {
            type Value = Timestamp;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an RFC 3339 date and time string")
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Timestamp, E> {
                Timestamp::parse_rfc3339(v).ok_or_else(|| E::custom(ParseTimestampError))
            }
        }
        deserializer.deserialize_str(Visitor)
    }
}

struct Cursor<'a>(&'a [u8]);

impl Cursor<'_> {
    fn peek(&self) -> Option<u8> {
        self.0.first().copied()
    }
    fn next(&mut self) -> Option<u8> {
        let (&b, rest) = self.0.split_first()?;
        self.0 = rest;
        Some(b)
    }
    fn lit(&mut self, want: u8) -> Option<()> {
        (self.next()? == want).then_some(())
    }
    fn digit(&mut self) -> Option<u8> {
        let b = self.next().filter(u8::is_ascii_digit)?;
        Some(b - b'0')
    }
    /// Exactly 2 digits.
    fn two(&mut self) -> Option<u8> {
        Some(self.digit()? * 10 + self.digit()?)
    }
    /// An optional sign, then 4 to 7 digits.
    fn year(&mut self) -> Option<i64> {
        let negative = match self.peek()? {
            b'+' => {
                self.next();
                false
            }
            b'-' => {
                self.next();
                true
            }
            _ => false,
        };
        let mut v = 0_i64;
        let mut count = 0;
        while self.peek().is_some_and(|b| b.is_ascii_digit()) {
            v = v * 10 + i64::from(self.digit()?);
            count += 1;
            if count > 7 {
                return None;
            }
        }
        if count < 4 {
            return None;
        }
        Some(if negative { -v } else { v })
    }
    /// One or more digits after the `.`, as nanoseconds (digits past the ninth dropped).
    fn fraction(&mut self) -> Option<u32> {
        let mut nanos = 0_u32;
        let mut count = 0_u32;
        while self.peek().is_some_and(|b| b.is_ascii_digit()) {
            let d = self.digit()?;
            if count < 9 {
                nanos = nanos * 10 + u32::from(d);
            }
            count += 1;
        }
        if count == 0 {
            return None;
        }
        for _ in count..9 {
            nanos *= 10;
        }
        Some(nanos)
    }
    /// `Z`, `z`, `UTC` or `±HH:MM`, as seconds east of UTC.
    fn zone(&mut self) -> Option<i64> {
        match self.next()? {
            b'Z' | b'z' => Some(0),
            b'U' => {
                self.lit(b'T')?;
                self.lit(b'C')?;
                Some(0)
            }
            sign @ (b'+' | b'-') => {
                let h = self.two()?;
                self.lit(b':')?;
                let m = self.two()?;
                if h > 23 || m > 59 {
                    return None;
                }
                let o = i64::from(h) * 3600 + i64::from(m) * 60;
                Some(if sign == b'-' { -o } else { o })
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // oracle: the type's definition (seconds rounded down, nanoseconds below 10⁹ also before 1970)
    // Offsets are whole 100 ns steps: SystemTime on Windows counts in 100 ns, and a smaller offset
    // is dropped there before it reaches the conversion
    #[cfg(feature = "std")]
    #[test]
    fn system_time_before_the_epoch_keeps_a_positive_fraction() {
        use std::time::{Duration, UNIX_EPOCH};
        let at = |s, n| Timestamp::from_unix(s, n).unwrap();
        assert_eq!(
            Timestamp::from_system_time(UNIX_EPOCH - Duration::from_nanos(100)),
            at(-1, 999_999_900)
        );
        assert_eq!(
            Timestamp::from_system_time(UNIX_EPOCH - Duration::from_secs(2)),
            at(-2, 0)
        );
        assert_eq!(
            Timestamp::from_system_time(UNIX_EPOCH + Duration::new(1_700_000_000, 500)),
            at(1_700_000_000, 500)
        );
    }

    #[test]
    fn out_of_range_inputs_are_refused_or_held() {
        assert_eq!(Timestamp::from_unix(0, 1_000_000_000), None);
        assert_eq!(Timestamp::from_unix(MAX_SECS + 1, 0), None);
        assert_eq!(Timestamp::from_unix(MIN_SECS - 1, 999_999_999), None);
        assert_eq!(Timestamp::MAX.plus_days(1), Timestamp::MAX);
        assert_eq!(Timestamp::MIN.plus_days(-1), Timestamp::MIN);
        assert_eq!(Timestamp::MAX.plus_days(i64::MAX), Timestamp::MAX);
        assert_eq!(Timestamp::MIN.plus_days(i64::MIN), Timestamp::MIN);
        assert_eq!(
            Timestamp::MAX.whole_seconds_since(Timestamp::MIN),
            MAX_SECS - MIN_SECS
        );
        assert_eq!(
            Timestamp::MIN.whole_seconds_since(Timestamp::MAX),
            MIN_SECS - MAX_SECS
        );
        assert_eq!(Timestamp::parse_rfc3339("+262143-01-01T00:00:00Z"), None);
        assert_eq!(Timestamp::parse_rfc3339("-262144-12-31T23:59:59Z"), None);
        assert_eq!(Timestamp::parse_rfc3339("+99999999-01-01T00:00:00Z"), None);
        assert_eq!(Timestamp::parse_rfc3339("2016-12-31T23:59:60Z"), None);
    }

    // oracle: the range ends are the first and last days of the years -262143 and 262142
    #[test]
    fn the_range_ends_are_the_calendar_dates() {
        assert_eq!(
            mjd_from_civil(-262_143, 1, 1).unwrap() - MJD_OF_1970,
            MIN_SECS / SECS_PER_DAY
        );
        assert_eq!(
            mjd_from_civil(262_143, 1, 1).unwrap() - MJD_OF_1970,
            (MAX_SECS + 1) / SECS_PER_DAY
        );
    }

    #[test]
    fn the_longest_text_fits() {
        let longest = Text::of(Timestamp::MAX.rfc3339()).unwrap();
        assert_eq!(
            longest.as_str().unwrap(),
            "+262142-12-31T23:59:59.999999999+00:00"
        );
        let earliest = Text::of(Timestamp::MIN.rfc3339()).unwrap();
        assert_eq!(earliest.as_str().unwrap(), "-262143-01-01T00:00:00+00:00");
    }

    #[test]
    fn debug_is_the_z_form_and_from_str_reads_it() {
        let t = Timestamp::from_unix(1_700_000_000, 123_000_000).unwrap();
        struct Dbg(Timestamp);
        impl fmt::Display for Dbg {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{:?}", self.0)
            }
        }
        assert_eq!(
            Text::of(Dbg(t)).unwrap().as_str().unwrap(),
            "2023-11-14T22:13:20.123Z"
        );
        assert_eq!("2023-11-14T22:13:20.123Z".parse::<Timestamp>(), Ok(t));
        assert_eq!("x".parse::<Timestamp>(), Err(ParseTimestampError));
    }
}
