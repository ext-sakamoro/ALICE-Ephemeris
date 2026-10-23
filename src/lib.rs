//! Deterministic astronomical time scales.
//!
//! - **UTC ↔ TAI** ([`UtcDateTime`]): the TAI − UTC table from 1960-01-01,
//!   including the 1960–1971 drift expressions and every leap second
//!   (23:59:60), exact to the attosecond. Valid until the table's expiry
//!   ([`leap::EXPIRES_MJD`], 2027-06-28); later dates return
//!   [`TimeError::LeapTableExpired`] unless the caller opts in to
//!   [`ExpiryPolicy::AssumeNoNewLeapSeconds`], which marks the result.
//! - **TAI ↔ TT**: TT = TAI + 32.184 s exactly.
//! - **TT ↔ TDB** ([`TdbModel`]): the SPICE formula, or the Fairhead &
//!   Bretagnon (1990) series (±3 ns for 1950–2050 as stated by ERFA).
//! - **Julian dates** ([`Jd2`], [`Mjd`]): two-part JD for interoperability,
//!   exact MJD.
//!
//! Instants ([`Instant`]) are integer seconds since J2000.0 plus attoseconds
//! on a typed scale ([`Tai`], [`Tt`], [`Tdb`]): no `f64` accumulates error.
//! The only `f64` arithmetic is the TDB − TT series and the JD fraction, and
//! every transcendental there is alice-det-math's, so each function returns
//! the same bits on every target (checked by a golden hash on Linux, macOS,
//! Windows and wasm32). Each conversion is a [`law::Law`] with its own
//! semantics identifier.
//!
//! ```
//! use alice_ephemeris::{ExpiryPolicy, TdbModel, UtcDateTime};
//!
//! let utc = UtcDateTime::new(2016, 12, 31, 23, 59, 60, 0)?; // a leap second
//! let tai = utc.to_tai(ExpiryPolicy::Strict)?.value;
//! let tt = tai.to_tt()?;
//! let tdb = tt.to_tdb(TdbModel::Fb1990)?; // Instant<Tdb>
//! assert_eq!(UtcDateTime::from_tai(tai, ExpiryPolicy::Strict)?.value, utc);
//! assert!(TdbModel::Fb1990.tdb_minus_tt(tt).abs() < 2e-3);
//! assert_eq!(tdb.to_tt(TdbModel::Fb1990)?, tt);
//! # Ok::<(), alice_ephemeris::TimeError>(())
//! ```

#![no_std]
#![forbid(unsafe_code)]

mod calendar;
pub mod eop;
mod error;
mod fb1990_table;
pub mod frames;
mod iau2006_table;
mod instant;
mod jd;
pub mod law;
pub mod leap;
pub mod tdb;
mod tt;
mod utc;

pub use calendar::{civil_from_mjd, days_in_month, mjd_from_civil};
pub use error::TimeError;
pub use instant::{Duration, Instant, Scale, Tai, Tdb, Tt, Ut1, ATTO_PER_SEC};
pub use jd::{Jd2, Mjd, JD_J2000};
pub use tdb::TdbModel;
pub use tt::TT_MINUS_TAI;
pub use utc::{tai_minus_utc, Converted, ExpiryPolicy, Provenance, UtcDateTime};
