//! TAI ↔ TT (Law [`crate::law::TT_TAI`]): TT = TAI + 32.184 s exactly.

use crate::instant::{Duration, Instant, Tai, Tt};
use crate::TimeError;

/// TT − TAI: 32.184 s.
pub const TT_MINUS_TAI: Duration = match Duration::new(32, 184_000_000_000_000_000) {
    Ok(d) => d,
    Err(_) => panic!("32.184 s is a valid duration"),
};

impl Instant<Tai> {
    /// TT at this TAI instant.
    pub fn to_tt(self) -> Result<Instant<Tt>, TimeError> {
        Ok(self.checked_add(TT_MINUS_TAI)?.relabel())
    }
}

impl Instant<Tt> {
    /// TAI at this TT instant.
    pub fn to_tai(self) -> Result<Instant<Tai>, TimeError> {
        Ok(self.checked_sub(TT_MINUS_TAI)?.relabel())
    }
}
