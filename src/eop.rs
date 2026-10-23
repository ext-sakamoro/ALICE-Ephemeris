//! Earth orientation parameters: UT1 and polar motion from an IERS table
//! (the IERS Rapid Service `finals2000A` format, Bulletin A columns).
//!
//! The table is the caller's ([`EopTable`] borrows parsed [`EopRow`]s): it is
//! reissued every week, so it is data, not part of the Law. The Law is the
//! reading of a row and the interpolation:
//!
//! - every value is kept as the integer the table prints (UT1 − UTC in
//!   10⁻⁷ s, angles in microarcseconds), nothing is rounded on reading;
//! - UT1 − TAI, not UT1 − UTC, is interpolated: UT1 − UTC jumps by 1 s at a
//!   leap second, UT1 − TAI is smooth. Each row's UT1 − TAI is exact in
//!   attoseconds (`UT1 − UTC − (TAI − UTC)` at 00:00 UTC of its day);
//! - the interpolation is the 4-point Lagrange polynomial of the IERS
//!   routine `INTERP` (two rows on each side, the window shifted inward at the
//!   ends of the table) in the TAI time of each row, so that a day of 86401 s
//!   is a longer interval and not a jump. The diurnal and semidiurnal ocean
//!   tide and libration terms of the IERS Conventions (2010) §5.5 (up to
//!   about 0.1 ms in UT1 and 0.1 mas in the pole) are not added.
//!
//! Each result says whether its window used predicted rows ([`EopFlag`]),
//! and the leap-second table's [`Provenance`] of the row times. A time
//! outside the rows is [`TimeError::EopOutOfRange`]: the table is never
//! extrapolated.
//!
//! The arithmetic is integer except the Lagrange weights and the weighted
//! sum, which are IEEE `f64` operations (no transcendental), so the result is
//! the same bits on every target.

use core::fmt;

use crate::instant::{Duration, Instant, Tai, Ut1, ATTO_PER_SEC};
use crate::utc::{tai_minus_utc, ExpiryPolicy, Provenance};
use crate::TimeError;

const ATTO: i128 = ATTO_PER_SEC as i128;
/// MJD of 2000-01-01 (J2000.0 is 12:00 on that day).
const MJD_2000: i64 = 51544;
/// Attoseconds per 10⁻⁷ s.
const ATTO_PER_E7: i128 = 100_000_000_000;
/// Rows in an interpolation window.
const WINDOW: usize = 4;

/// Whether a value was measured or predicted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EopFlag {
    /// Final or rapid value from observations (`I` in the table).
    Observed,
    /// Prediction (`P` in the table).
    Predicted,
}

impl EopFlag {
    fn parse(c: u8, column: usize) -> Result<Self, EopParseError> {
        match c {
            b'I' => Ok(Self::Observed),
            b'P' => Ok(Self::Predicted),
            _ => Err(EopParseError::Flag { column }),
        }
    }

    /// Predicted if either is.
    fn worst(self, other: Self) -> Self {
        if self == Self::Predicted || other == Self::Predicted {
            Self::Predicted
        } else {
            Self::Observed
        }
    }
}

/// One day of the table, at 00:00 UTC, as the integers it prints.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EopRow {
    /// MJD of the day (the value applies at 00:00 UTC).
    pub mjd: i64,
    /// UT1 − UTC in units of 10⁻⁷ s.
    pub ut1_minus_utc_e7: i32,
    /// Whether UT1 − UTC is observed or predicted.
    pub ut1_flag: EopFlag,
    /// Pole coordinate x in microarcseconds.
    pub xp_uas: i32,
    /// Pole coordinate y in microarcseconds.
    pub yp_uas: i32,
    /// Whether the pole coordinates are observed or predicted.
    pub polar_flag: EopFlag,
    /// Celestial pole offsets dX, dY (IAU 2000A) in microarcseconds, when the
    /// row has them.
    pub celestial_pole_offset_uas: Option<(i32, i32)>,
}

/// Why a `finals2000A` line could not be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EopParseError {
    /// The field starting at this column (1-based) is not a number in the
    /// format of the table.
    Field {
        /// First column of the field.
        column: usize,
    },
    /// The flag at this column is neither `I` nor `P`.
    Flag {
        /// Column of the flag.
        column: usize,
    },
}

impl fmt::Display for EopParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Field { column } => write!(f, "malformed field at column {column}"),
            Self::Flag { column } => write!(f, "flag at column {column} is not I or P"),
        }
    }
}

impl core::error::Error for EopParseError {}

/// Columns `first..=last` (1-based) of `line`, empty past its end.
fn columns(line: &[u8], first: usize, last: usize) -> &[u8] {
    let end = last.min(line.len());
    line.get(first - 1..end).unwrap_or(&[])
}

/// A decimal number printed with exactly `decimals` digits after the point,
/// as an integer in units of 10^−decimals (`-0.0444801` with 7 → −444801).
/// Leading spaces, an optional sign and an empty integer part are allowed.
fn fixed(field: &[u8], decimals: usize, column: usize) -> Result<i64, EopParseError> {
    let bad = EopParseError::Field { column };
    let s = field.trim_ascii();
    let (negative, s) = match s.split_first() {
        Some((b'-', rest)) => (true, rest),
        Some((b'+', rest)) => (false, rest),
        _ => (false, s),
    };
    let point = s.iter().position(|&c| c == b'.').ok_or(bad)?;
    let (int, frac) = (&s[..point], &s[point + 1..]);
    if frac.len() != decimals || int.len() + frac.len() > 15 || frac.is_empty() {
        return Err(bad);
    }
    let mut v: i64 = 0;
    for &c in int.iter().chain(frac) {
        if !c.is_ascii_digit() {
            return Err(bad);
        }
        v = v * 10 + i64::from(c - b'0');
    }
    Ok(if negative { -v } else { v })
}

fn fixed_i32(field: &[u8], decimals: usize, column: usize) -> Result<i32, EopParseError> {
    i32::try_from(fixed(field, decimals, column)?).map_err(|_| EopParseError::Field { column })
}

impl EopRow {
    /// Reads one line of `finals2000A.all` / `finals2000A.daily` (IERS
    /// Rapid Service format, Bulletin A columns: MJD 8–15, pole flag 17,
    /// x 19–27, y 38–46 arcsec, UT1 flag 58, UT1 − UTC 59–68 s, nutation flag
    /// 96, dX 98–106, dY 117–125 mas).
    ///
    /// `Ok(None)` for a dated line without UT1 − UTC (the table lists days
    /// past the end of its predictions with empty fields). Trailing spaces
    /// may be missing.
    pub fn parse_finals2000a(line: &str) -> Result<Option<Self>, EopParseError> {
        let b = line.as_bytes();
        let mjd = fixed(columns(b, 8, 15), 2, 8)?;
        if mjd % 100 != 0 {
            return Err(EopParseError::Field { column: 8 });
        }
        let mjd = mjd / 100;
        let ut1_flag = match columns(b, 58, 58) {
            [] | [b' '] => return Ok(None),
            [c] => EopFlag::parse(*c, 58)?,
            _ => unreachable!("one column"),
        };
        let polar_flag = EopFlag::parse(*columns(b, 17, 17).first().unwrap_or(&b' '), 17)?;
        let offset = match columns(b, 96, 96) {
            [] | [b' '] => None,
            [c] => {
                EopFlag::parse(*c, 96)?;
                Some((
                    fixed_i32(columns(b, 98, 106), 3, 98)?,
                    fixed_i32(columns(b, 117, 125), 3, 117)?,
                ))
            }
            _ => unreachable!("one column"),
        };
        Ok(Some(Self {
            mjd,
            ut1_minus_utc_e7: fixed_i32(columns(b, 59, 68), 7, 59)?,
            ut1_flag,
            xp_uas: fixed_i32(columns(b, 19, 27), 6, 19)?,
            yp_uas: fixed_i32(columns(b, 38, 46), 6, 38)?,
            polar_flag,
            celestial_pole_offset_uas: offset,
        }))
    }
}

/// Why rows do not form a table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EopTableError {
    /// Fewer than 4 rows (one interpolation window).
    TooFewRows,
    /// The row after MJD `after` is not the next day.
    NotConsecutive {
        /// MJD of the row before the gap.
        after: i64,
    },
}

impl fmt::Display for EopTableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooFewRows => f.write_str("an EOP table needs at least 4 rows"),
            Self::NotConsecutive { after } => {
                write!(f, "the row after MJD {after} is not the next day")
            }
        }
    }
}

impl core::error::Error for EopTableError {}

/// Consecutive daily rows (at least 4).
#[derive(Clone, Copy, Debug)]
pub struct EopTable<'a> {
    rows: &'a [EopRow],
}

/// Earth orientation at one instant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Eop {
    /// UT1 − TAI (the Lagrange value in attoseconds, truncated toward zero).
    pub ut1_minus_tai: Duration,
    /// Pole coordinate x in arcseconds.
    pub xp_arcsec: f64,
    /// Pole coordinate y in arcseconds.
    pub yp_arcsec: f64,
    /// Celestial pole offsets dX, dY in arcseconds, when every row of the
    /// window has them.
    pub celestial_pole_offset_arcsec: Option<(f64, f64)>,
    /// Predicted if any row of the window has a predicted UT1 − UTC.
    pub ut1_flag: EopFlag,
    /// Predicted if any row of the window has predicted pole coordinates.
    pub polar_flag: EopFlag,
    /// Whether the TAI − UTC of the rows' days comes from the leap-second
    /// table or from the assumption past its expiry.
    pub leap: Provenance,
}

/// Seconds of a non-negative attosecond count as `f64` (integer part and
/// fraction converted separately, then added once).
fn seconds_f64(atto: i128) -> f64 {
    atto.div_euclid(ATTO) as f64 + atto.rem_euclid(ATTO) as f64 * 1e-18
}

/// Lagrange weights of the nodes `x` at `u`.
fn weights(x: &[f64; WINDOW], u: f64) -> [f64; WINDOW] {
    let mut w = [1.0; WINDOW];
    for (j, wj) in w.iter_mut().enumerate() {
        for m in 0..WINDOW {
            if m != j {
                *wj *= (u - x[m]) / (x[j] - x[m]);
            }
        }
    }
    w
}

/// `value − first` as `f64` (exact: below 2³³).
fn relative(value: i32, first: i32) -> f64 {
    (i64::from(value) - i64::from(first)) as f64
}

/// Arcseconds of `first + delta` microarcseconds.
fn uas_to_arcsec(first: i32, delta: f64) -> f64 {
    (f64::from(first) + delta) * 1e-6
}

fn dot(w: &[f64; WINDOW], y: [f64; WINDOW]) -> f64 {
    w[0] * y[0] + w[1] * y[1] + w[2] * y[2] + w[3] * y[3]
}

impl<'a> EopTable<'a> {
    /// A table of consecutive daily rows.
    pub fn new(rows: &'a [EopRow]) -> Result<Self, EopTableError> {
        if rows.len() < WINDOW {
            return Err(EopTableError::TooFewRows);
        }
        for pair in rows.windows(2) {
            if pair[0].mjd.checked_add(1) != Some(pair[1].mjd) {
                return Err(EopTableError::NotConsecutive { after: pair[0].mjd });
            }
        }
        Ok(Self { rows })
    }

    /// The rows.
    pub fn rows(&self) -> &'a [EopRow] {
        self.rows
    }

    fn out_of_range(&self) -> TimeError {
        TimeError::EopOutOfRange {
            first_mjd: self.rows[0].mjd,
            last_mjd: self.rows[self.rows.len() - 1].mjd,
        }
    }

    /// TAI (attoseconds since J2000.0) at 00:00 UTC of row `i`, its exact
    /// UT1 − TAI, and the leap table's provenance.
    fn node(&self, i: usize, policy: ExpiryPolicy) -> Result<(i128, i128, Provenance), TimeError> {
        let mjd = self.rows[i].mjd;
        let off = tai_minus_utc(mjd, 0, policy)?;
        let label = i128::from(mjd - MJD_2000)
            .checked_mul(86_400 * ATTO)
            .ok_or(TimeError::OutOfRange)?
            - 43_200 * ATTO;
        let ut1_minus_tai = i128::from(self.rows[i].ut1_minus_utc_e7) * ATTO_PER_E7 - off.value;
        Ok((label + off.value, ut1_minus_tai, off.provenance))
    }

    /// Index `k` of the row with `t_k ≤ t < t_(k+1)` (or the last row when
    /// `t` is exactly its time). The row times past the leap table's expiry
    /// are those of no new leap second; whether that is allowed is checked
    /// on the window's rows.
    fn bracket(&self, t: i128) -> Result<usize, TimeError> {
        let policy = ExpiryPolicy::AssumeNoNewLeapSeconds;
        let n = self.rows.len();
        let (first, _, _) = self.node(0, policy)?;
        if t < first {
            return Err(self.out_of_range());
        }
        // a day of UTC is 86400 s of TAI except at a leap second, so the day
        // count is right or one too high
        let days = (t - first).div_euclid(86_400 * ATTO);
        let mut k = usize::try_from(days).map_err(|_| self.out_of_range())?;
        if k >= n {
            if k > n {
                return Err(self.out_of_range());
            }
            k = n - 1;
        }
        if self.node(k, policy)?.0 > t {
            k -= 1;
        }
        if k == n - 1 && self.node(k, policy)?.0 != t {
            return Err(self.out_of_range());
        }
        Ok(k)
    }

    /// Earth orientation at TAI instant `tai`.
    pub fn at(&self, tai: Instant<Tai>, policy: ExpiryPolicy) -> Result<Eop, TimeError> {
        let t = tai.total_attoseconds();
        let k = self.bracket(t)?;
        let s = k.saturating_sub(1).min(self.rows.len() - WINDOW);
        let mut x = [0.0; WINDOW];
        let mut dut = [0.0; WINDOW];
        let mut xp = [0.0; WINDOW];
        let mut yp = [0.0; WINDOW];
        let mut dx = [0.0; WINDOW];
        let mut dy = [0.0; WINDOW];
        let mut has_offset = true;
        let (t0, base, mut leap) = self.node(s, policy)?;
        let first = &self.rows[s];
        let first_offset = first.celestial_pole_offset_uas.unwrap_or((0, 0));
        let mut ut1_flag = EopFlag::Observed;
        let mut polar_flag = EopFlag::Observed;
        // every value relative to the window's first row (exact integer
        // differences), so that a constant is reproduced exactly
        for j in 0..WINDOW {
            let (tj, uj, pj) = self.node(s + j, policy)?;
            let row = &self.rows[s + j];
            x[j] = seconds_f64(tj - t0);
            // exact in f64 to the attosecond up to 9 ms
            dut[j] = (uj - base) as f64;
            xp[j] = relative(row.xp_uas, first.xp_uas);
            yp[j] = relative(row.yp_uas, first.yp_uas);
            match row.celestial_pole_offset_uas {
                Some((a, b)) => {
                    dx[j] = relative(a, first_offset.0);
                    dy[j] = relative(b, first_offset.1);
                }
                None => has_offset = false,
            }
            ut1_flag = ut1_flag.worst(row.ut1_flag);
            polar_flag = polar_flag.worst(row.polar_flag);
            if pj == Provenance::AssumedNoNewLeapSeconds {
                leap = pj;
            }
        }
        let w = weights(&x, seconds_f64(t - t0));
        let ut1_minus_tai = Duration::from_total_attoseconds(base + dot(&w, dut) as i128)?;
        Ok(Eop {
            ut1_minus_tai,
            xp_arcsec: uas_to_arcsec(first.xp_uas, dot(&w, xp)),
            yp_arcsec: uas_to_arcsec(first.yp_uas, dot(&w, yp)),
            celestial_pole_offset_arcsec: has_offset.then(|| {
                (
                    uas_to_arcsec(first_offset.0, dot(&w, dx)),
                    uas_to_arcsec(first_offset.1, dot(&w, dy)),
                )
            }),
            ut1_flag,
            polar_flag,
            leap,
        })
    }
}

impl Instant<Tai> {
    /// UT1 at this TAI instant, from `table`, with the Earth orientation used.
    pub fn to_ut1(
        self,
        table: &EopTable<'_>,
        policy: ExpiryPolicy,
    ) -> Result<(Instant<Ut1>, Eop), TimeError> {
        let eop = table.at(self, policy)?;
        Ok((self.checked_add(eop.ut1_minus_tai)?.relabel(), eop))
    }
}

impl Instant<Ut1> {
    /// TAI at this UT1 instant, from `table`: three fixed-point steps of
    /// `TAI = UT1 − (UT1 − TAI)(TAI)`. The error shrinks by the rate of
    /// UT1 − TAI per step, which is below 10⁻⁷ s/s (an excess length of day
    /// below 8.6 ms), from at most 40 s: within 4 × 10⁻²⁰ s after three steps,
    /// below the rounding of the interpolation.
    pub fn to_tai(
        self,
        table: &EopTable<'_>,
        policy: ExpiryPolicy,
    ) -> Result<Instant<Tai>, TimeError> {
        let guess: Instant<Tai> = self.relabel();
        let mut tai = guess;
        for _ in 0..3 {
            tai = guess.checked_sub(table.at(tai, policy)?.ut1_minus_tai)?;
        }
        Ok(tai)
    }
}
