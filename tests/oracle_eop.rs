//! Oracle: UT1 and polar motion from an IERS `finals2000A` table.
//!
//! - `tests/golden/eop.csv` (scripts/gen_eop_golden.py) interpolates the rows
//!   of tests/data/finals2000A-excerpt.txt in exact rational arithmetic with
//!   its own column slicing and TAI − UTC from the IANA leap-second list:
//!   every result, error kind and flag must match, the values within the
//!   rounding of the `f64` weights.
//! - A table written from a cubic in time is reproduced between its rows
//!   (a 4-point Lagrange polynomial is exact for degree ≤ 3).
//! - UT1 is continuous across the leap second of 2016-12-31 although
//!   UT1 − UTC jumps by 1 s there.
//!
//! The excerpt is part of the USNO `finals2000A.all` of 2026-10-23 (a work of
//! the U.S. Government), rows of 1973-01, 2016-12/2017-01 (a leap second),
//! 2026-10/11 (observed to predicted), 2026-12/2027-01 (end of dX, dY),
//! 2027-06/07 (expiry of the leap-second table) and 2027-10 (end of the
//! predictions). SHA-256 of the whole file: 95a61292c957fa74ed624bd4f7a84b1b984077e412b43ed39b99e092f38de9da.
//!
//! Author: Moroya Sakamoto

mod common;

use alice_ephemeris::eop::{EopFlag, EopParseError, EopRow, EopTable, EopTableError};
use alice_ephemeris::{ExpiryPolicy, Instant, Provenance, Tai, TimeError, Ut1, ATTO_PER_SEC};
use common::{f64_, i64_, rows, u64_};

const EXCERPT: &str = include_str!("data/finals2000A-excerpt.txt");

/// Contiguous blocks of the excerpt's rows that have UT1 − UTC.
fn blocks() -> Vec<Vec<EopRow>> {
    let mut out: Vec<Vec<EopRow>> = Vec::new();
    for line in EXCERPT.lines() {
        let Some(row) = EopRow::parse_finals2000a(line).expect("parses") else {
            continue;
        };
        match out.last_mut() {
            Some(b) if b.last().unwrap().mjd + 1 == row.mjd => b.push(row),
            _ => out.push(vec![row]),
        }
    }
    out
}

#[test]
fn the_rows_are_read_as_the_integers_the_table_prints() {
    let b = blocks();
    assert_eq!(b.len(), 6);
    // 17 1 1 57754.00 I  0.080504 ... I 0.5912821
    let r = b[1].iter().find(|r| r.mjd == 57754).unwrap();
    assert_eq!(
        (r.ut1_minus_utc_e7, r.xp_uas, r.yp_uas),
        (5_912_821, 80_504, 263_145)
    );
    assert_eq!(
        (r.ut1_flag, r.polar_flag),
        (EopFlag::Observed, EopFlag::Observed)
    );
    // 2026-10-23, first predicted row: P-0.0444801
    let r = b[2].iter().find(|r| r.mjd == 61336).unwrap();
    assert_eq!(r.ut1_minus_utc_e7, -444_801);
    assert_eq!(
        (r.ut1_flag, r.polar_flag),
        (EopFlag::Predicted, EopFlag::Predicted)
    );
    // the blank rows past the predictions are skipped, not errors
    assert_eq!(b[5].last().unwrap().mjd, 61707);
    assert!(EopRow::parse_finals2000a(EXCERPT.lines().last().unwrap())
        .unwrap()
        .is_none());
    // dX, dY end in 2027-01: the later rows have none
    assert!(b[3].iter().any(|r| r.celestial_pole_offset_uas.is_none()));
    assert!(b[3].iter().any(|r| r.celestial_pole_offset_uas.is_some()));
}

#[test]
fn interpolation_matches_exact_rationals() {
    let blocks = blocks();
    let tables: Vec<EopTable> = blocks.iter().map(|b| EopTable::new(b).unwrap()).collect();
    let mut compared = 0;
    let mut kinds = [0usize; 3];
    for r in rows("eop.csv") {
        let table = &tables[i64_(&r[0]) as usize];
        let tai = Instant::<Tai>::new(i64_(&r[1]), u64_(&r[2])).unwrap();
        let policy = match r[3].as_str() {
            "strict" => ExpiryPolicy::Strict,
            "assume" => ExpiryPolicy::AssumeNoNewLeapSeconds,
            p => panic!("{p}"),
        };
        let got = table.at(tai, policy);
        let ctx = format!("{tai:?} {}", r[3]);
        match r[4].as_str() {
            "out_of_range" => {
                assert!(
                    matches!(got, Err(TimeError::EopOutOfRange { .. })),
                    "{ctx}: {got:?}"
                );
                kinds[1] += 1;
            }
            "leap_table_expired" => {
                assert!(
                    matches!(got, Err(TimeError::LeapTableExpired { .. })),
                    "{ctx}: {got:?}"
                );
                kinds[2] += 1;
            }
            "ok" => {
                let e = got.unwrap_or_else(|err| panic!("{ctx}: {err:?}"));
                let want: i128 = r[5].parse().unwrap();
                let diff = (e.ut1_minus_tai.total_attoseconds() - want).abs();
                assert!(diff <= 1_000, "{ctx}: UT1-TAI off by {diff} as");
                for (got, col) in [(e.xp_arcsec, 6), (e.yp_arcsec, 7)] {
                    assert!(
                        (got - f64_(&r[col])).abs() <= 1e-15,
                        "{ctx} col {col}: {got}"
                    );
                }
                match e.celestial_pole_offset_arcsec {
                    Some((dx, dy)) => {
                        assert!((dx - f64_(&r[8])).abs() <= 1e-15, "{ctx}: dX {dx}");
                        assert!((dy - f64_(&r[9])).abs() <= 1e-15, "{ctx}: dY {dy}");
                    }
                    None => assert!(r[8].is_empty(), "{ctx}: dX missing"),
                }
                let flag = |c: &str| {
                    if c == "P" {
                        EopFlag::Predicted
                    } else {
                        EopFlag::Observed
                    }
                };
                assert_eq!(e.ut1_flag, flag(&r[10]), "{ctx}");
                assert_eq!(e.polar_flag, flag(&r[11]), "{ctx}");
                let leap = if r[12] == "assumed" {
                    Provenance::AssumedNoNewLeapSeconds
                } else {
                    Provenance::Table
                };
                assert_eq!(e.leap, leap, "{ctx}");
                kinds[0] += 1;
            }
            k => panic!("{k}"),
        }
        compared += 1;
    }
    assert_eq!(compared, 890);
    assert!(kinds.iter().all(|&k| k > 0), "{kinds:?}");
}

/// A table whose UT1 − UTC is the integer cubic `p(i)` (10⁻⁷ s) of the day
/// index: no leap second in 2005, so the rows are 86400 s apart and the
/// 4-point Lagrange polynomial reproduces `p` at any fraction of a day.
#[test]
fn a_cubic_table_is_reproduced_between_its_rows() {
    let p = |d: f64| 1_234_567.0 - 8_765.0 * d + 43.0 * d * d - 2.0 * d * d * d;
    let mjd0 = 53_400; // 2005-02-10
    let rows: Vec<EopRow> = (0..12)
        .map(|i| EopRow {
            mjd: mjd0 + i,
            ut1_minus_utc_e7: p(i as f64) as i32,
            ut1_flag: EopFlag::Observed,
            xp_uas: (100_000.0 + p(i as f64) / 7.0).round() as i32,
            yp_uas: 300_000,
            polar_flag: EopFlag::Observed,
            celestial_pole_offset_uas: None,
        })
        .collect();
    let table = EopTable::new(&rows).unwrap();
    // TAI − UTC is 32 s in 2005
    let t0 = (mjd0 - 51_544) * 86_400 - 43_200 + 32;
    let mut compared = 0;
    for k in 0..=1100 {
        let tai = Instant::<Tai>::new(t0 + k * 864, 0).unwrap();
        let e = table.at(tai, ExpiryPolicy::Strict).unwrap();
        let want = p(k as f64 / 100.0) * 1e-7 - 32.0;
        let got = e.ut1_minus_tai.total_attoseconds() as f64 * 1e-18;
        assert!((got - want).abs() <= 1e-14, "k={k}: {got} vs {want}");
        assert_eq!(e.yp_arcsec, 0.3, "k={k}");
        compared += 1;
    }
    assert_eq!(compared, 1101);
}

#[test]
fn ut1_is_continuous_across_the_2016_leap_second() {
    let b = blocks();
    let table = EopTable::new(&b[1]).unwrap();
    // 2016-12-31 23:59:59, 23:59:60, 2017-01-01 00:00:00 UTC in TAI
    let midnight_tai = (57_754 - 51_544) * 86_400 - 43_200 + 37;
    let mut prev: Option<i128> = None;
    for dt in -3..=2 {
        let tai = Instant::<Tai>::new(midnight_tai + dt, 0).unwrap();
        let (ut1, _) = tai.to_ut1(&table, ExpiryPolicy::Strict).unwrap();
        let u = ut1.total_attoseconds();
        if let Some(p) = prev {
            // one TAI second is one UT1 second to within 10 µs
            let step = u - p;
            assert!(
                (step - ATTO_PER_SEC as i128).abs() < 10_000_000_000_000,
                "{dt}: {step}"
            );
        }
        prev = Some(u);
    }
}

#[test]
fn ut1_round_trips_to_tai() {
    let b = blocks();
    let table = EopTable::new(&b[1]).unwrap();
    let start = (57_741 - 51_544) * 86_400 - 43_200 + 36;
    for k in 0..200 {
        let tai = Instant::<Tai>::new(
            start + k * 12_345,
            (k as u64) * 4_999_999_999_999_999 % ATTO_PER_SEC,
        )
        .unwrap();
        let (ut1, _) = tai.to_ut1(&table, ExpiryPolicy::Strict).unwrap();
        let back = ut1.to_tai(&table, ExpiryPolicy::Strict).unwrap();
        let err = back.duration_since(tai).unwrap().total_attoseconds().abs();
        assert!(err <= 100, "k={k}: {err} as");
    }
}

#[test]
fn degenerate_input_is_rejected() {
    let b = blocks();
    // fewer than 4 rows, a gap
    assert_eq!(
        EopTable::new(&b[1][..3]).unwrap_err(),
        EopTableError::TooFewRows
    );
    assert_eq!(EopTable::new(&[]).unwrap_err(), EopTableError::TooFewRows);
    let mut gap = b[1][..6].to_vec();
    gap.remove(3);
    assert_eq!(
        EopTable::new(&gap).unwrap_err(),
        EopTableError::NotConsecutive { after: gap[2].mjd }
    );
    let mut dup = b[1][..6].to_vec();
    dup[4] = dup[3];
    assert!(EopTable::new(&dup).is_err());
    // before 1960 (UTC undefined), extreme instants
    let table = EopTable::new(&b[0]).unwrap();
    for (s, a) in [(i64::MIN, 0), (i64::MAX, ATTO_PER_SEC - 1), (0, 0)] {
        let r = table.at(Instant::<Tai>::new(s, a).unwrap(), ExpiryPolicy::Strict);
        assert!(
            matches!(r, Err(TimeError::EopOutOfRange { .. })),
            "{s}: {r:?}"
        );
    }
    assert!(Instant::<Ut1>::new(i64::MAX, 0)
        .unwrap()
        .to_tai(&table, ExpiryPolicy::Strict)
        .is_err());
    // malformed lines: a column (1-based) per kind of fault
    let good = EXCERPT.lines().find(|l| l.contains("57754.00")).unwrap();
    let with = |col: usize, s: &str| {
        let mut l = good.to_owned();
        l.replace_range(col - 1..col - 1 + s.len(), s);
        l
    };
    for (line, want) in [
        (with(8, "577x4.00"), EopParseError::Field { column: 8 }),
        (with(8, "57754.50"), EopParseError::Field { column: 8 }),
        (with(17, "X"), EopParseError::Flag { column: 17 }),
        (with(58, "Q"), EopParseError::Flag { column: 58 }),
        (with(59, " 0.591282 "), EopParseError::Field { column: 59 }),
        (with(19, " 0.08050x"), EopParseError::Field { column: 19 }),
        (with(96, "Z"), EopParseError::Flag { column: 96 }),
        (String::new(), EopParseError::Field { column: 8 }),
        ("é".repeat(40), EopParseError::Field { column: 8 }),
    ] {
        assert_eq!(EopRow::parse_finals2000a(&line), Err(want), "{line:?}");
    }
    // negative values: the sign is in the first column of each field
    let neg = with(19, "-0.080504");
    let neg = neg[..37].to_owned() + "-0.263145" + &neg[46..];
    let r = EopRow::parse_finals2000a(&neg).unwrap().unwrap();
    assert_eq!((r.xp_uas, r.yp_uas), (-80_504, -263_145));
    // trailing spaces may be missing
    let trimmed = good.trim_end();
    assert_eq!(
        EopRow::parse_finals2000a(trimmed),
        EopRow::parse_finals2000a(good)
    );
}
