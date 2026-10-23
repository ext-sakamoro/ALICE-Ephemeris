use core::fmt;

/// Why a conversion did not produce a value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TimeError {
    /// The result does not fit the representation (seconds beyond `i64`,
    /// attoseconds ≥ 10¹⁸, a non-finite Julian date).
    OutOfRange,
    /// Month or day outside the Gregorian calendar.
    InvalidDate,
    /// Hour, minute or second outside their ranges.
    InvalidTime,
    /// The time of day does not exist on that UTC day: second 60 on a day
    /// without a positive leap, or past the end of a day shortened by a
    /// negative step (1961–1971).
    NotALeapSecond,
    /// Before 1960-01-01 UTC, where UTC is not defined.
    BeforeUtc,
    /// On or after the expiry of the leap-second table
    /// ([`crate::leap::EXPIRES_MJD`]), with [`crate::ExpiryPolicy::Strict`].
    LeapTableExpired {
        /// First UTC day (MJD) the table does not cover.
        expires_mjd: i64,
    },
    /// Outside the days of the Earth orientation table (it is not
    /// extrapolated).
    EopOutOfRange {
        /// MJD of the first row.
        first_mjd: i64,
        /// MJD of the last row.
        last_mjd: i64,
    },
}

impl fmt::Display for TimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfRange => f.write_str("value out of the representable range"),
            Self::InvalidDate => f.write_str("invalid calendar date"),
            Self::InvalidTime => f.write_str("invalid time of day"),
            Self::NotALeapSecond => f.write_str("time of day does not exist on that UTC day"),
            Self::BeforeUtc => f.write_str("UTC is not defined before 1960-01-01"),
            Self::LeapTableExpired { expires_mjd } => {
                write!(
                    f,
                    "leap-second table expired (valid before MJD {expires_mjd})"
                )
            }
            Self::EopOutOfRange {
                first_mjd,
                last_mjd,
            } => write!(
                f,
                "outside the Earth orientation table (MJD {first_mjd} to {last_mjd})"
            ),
        }
    }
}

impl core::error::Error for TimeError {}
