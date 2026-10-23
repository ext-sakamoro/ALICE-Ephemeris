//! Instants on a uniform time scale and signed durations, held as integer
//! seconds plus attoseconds, so that adding and subtracting never rounds.

use core::fmt;
use core::marker::PhantomData;

use crate::TimeError;

/// Attoseconds per second (10¹⁸).
pub const ATTO_PER_SEC: u64 = 1_000_000_000_000_000_000;

const ATTO_I128: i128 = ATTO_PER_SEC as i128;

mod sealed {
    pub trait Sealed {}
}

/// A uniform time scale an [`Instant`] is measured on.
pub trait Scale: sealed::Sealed + Copy + Ord + fmt::Debug + core::hash::Hash {
    /// Short name of the scale (`"TAI"`, `"TT"`, `"TDB"`).
    const NAME: &'static str;
}

macro_rules! scale {
    ($(#[$doc:meta])* $ty:ident, $name:literal) => {
        $(#[$doc])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum $ty {}
        impl sealed::Sealed for $ty {}
        impl Scale for $ty {
            const NAME: &'static str = $name;
        }
    };
}

scale!(
    /// International Atomic Time.
    Tai,
    "TAI"
);
scale!(
    /// Terrestrial Time, TT = TAI + 32.184 s exactly.
    Tt,
    "TT"
);
scale!(
    /// Barycentric Dynamical Time (the time argument of JPL ephemerides).
    Tdb,
    "TDB"
);

/// An instant on scale `S`: seconds since J2000.0 (2000-01-01 12:00:00 on
/// that same scale) plus a fraction in attoseconds.
///
/// Range: `sec` is any `i64` (about ±2.9 × 10¹¹ years) and the resolution is
/// exactly 10⁻¹⁸ s. Arithmetic is integer and checked: it never rounds and
/// returns [`TimeError::OutOfRange`] instead of wrapping.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Instant<S: Scale> {
    sec: i64,
    atto: u64,
    scale: PhantomData<S>,
}

impl<S: Scale> fmt::Debug for Instant<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} J2000{:+}s+{:018}as", S::NAME, self.sec, self.atto)
    }
}

impl<S: Scale> Instant<S> {
    /// J2000.0 on this scale.
    pub const J2000: Self = Self {
        sec: 0,
        atto: 0,
        scale: PhantomData,
    };

    /// `sec` seconds and `atto` attoseconds after J2000.0 (`atto < 10¹⁸`).
    pub const fn new(sec: i64, atto: u64) -> Result<Self, TimeError> {
        if atto >= ATTO_PER_SEC {
            return Err(TimeError::OutOfRange);
        }
        Ok(Self {
            sec,
            atto,
            scale: PhantomData,
        })
    }

    /// Whole seconds since J2000.0 (floor: the fraction is always ≥ 0).
    pub const fn seconds(self) -> i64 {
        self.sec
    }

    /// Fraction of the second, in attoseconds (`0 ..= 10¹⁸ − 1`).
    pub const fn attoseconds(self) -> u64 {
        self.atto
    }

    /// Attoseconds since J2000.0.
    pub const fn total_attoseconds(self) -> i128 {
        self.sec as i128 * ATTO_I128 + self.atto as i128
    }

    /// The instant `total` attoseconds after J2000.0.
    pub fn from_total_attoseconds(total: i128) -> Result<Self, TimeError> {
        let sec = total.div_euclid(ATTO_I128);
        let atto = total.rem_euclid(ATTO_I128);
        let sec = i64::try_from(sec).map_err(|_| TimeError::OutOfRange)?;
        Ok(Self {
            sec,
            atto: atto as u64,
            scale: PhantomData,
        })
    }

    /// `self + d`.
    pub fn checked_add(self, d: Duration) -> Result<Self, TimeError> {
        Self::from_total_attoseconds(self.total_attoseconds() + d.total_attoseconds())
    }

    /// `self − d`.
    pub fn checked_sub(self, d: Duration) -> Result<Self, TimeError> {
        Self::from_total_attoseconds(self.total_attoseconds() - d.total_attoseconds())
    }

    /// `self − earlier`.
    pub fn duration_since(self, earlier: Self) -> Result<Duration, TimeError> {
        Duration::from_total_attoseconds(self.total_attoseconds() - earlier.total_attoseconds())
    }

    /// Seconds since J2000.0 as an `f64`, within 1 ulp of the exact value
    /// (IEEE operations only, so the same bits on every target; an `f64` of
    /// the magnitude of 2026 resolves about 1.2 × 10⁻⁷ s).
    pub fn to_seconds_f64(self) -> f64 {
        // split toward zero so that the integer and the fraction have the
        // same sign and cannot cancel: −1 s + 0.9999 s would lose the low bits
        if self.sec < 0 && self.atto > 0 {
            (self.sec + 1) as f64 - (ATTO_PER_SEC - self.atto) as f64 * 1e-18
        } else {
            self.sec as f64 + self.atto as f64 * 1e-18
        }
    }

    pub(crate) const fn relabel<T: Scale>(self) -> Instant<T> {
        Instant {
            sec: self.sec,
            atto: self.atto,
            scale: PhantomData,
        }
    }
}

/// A signed length of time, integer seconds plus attoseconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Duration {
    sec: i64,
    atto: u64,
}

impl Duration {
    /// Zero.
    pub const ZERO: Self = Self { sec: 0, atto: 0 };

    /// `sec` seconds plus `atto` attoseconds (`atto < 10¹⁸`; a negative
    /// duration has `sec < 0` and a non-negative fraction, e.g. −0.25 s is
    /// `(-1, 750_000_000_000_000_000)`).
    pub const fn new(sec: i64, atto: u64) -> Result<Self, TimeError> {
        if atto >= ATTO_PER_SEC {
            return Err(TimeError::OutOfRange);
        }
        Ok(Self { sec, atto })
    }

    /// Whole seconds (floor).
    pub const fn seconds(self) -> i64 {
        self.sec
    }

    /// Fraction of the second in attoseconds (`0 ..= 10¹⁸ − 1`).
    pub const fn attoseconds(self) -> u64 {
        self.atto
    }

    /// Length in attoseconds.
    pub const fn total_attoseconds(self) -> i128 {
        self.sec as i128 * ATTO_I128 + self.atto as i128
    }

    /// The duration of `total` attoseconds.
    pub fn from_total_attoseconds(total: i128) -> Result<Self, TimeError> {
        let sec = i64::try_from(total.div_euclid(ATTO_I128)).map_err(|_| TimeError::OutOfRange)?;
        Ok(Self {
            sec,
            atto: total.rem_euclid(ATTO_I128) as u64,
        })
    }
}
