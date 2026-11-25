//! `CivilDate::from_ymd` and `CivilDate::days_until` against facts that do not come from the
//! implementation: the Fliegel–Van Flandern integer formula for the Julian Day Number (a different
//! formula from the Hinnant day count the crate uses), fixed calendar facts, the 400-year cycle of
//! 146 097 days, and the day steps of `Timestamp::plus_days`.

use alice_ephemeris::time::{CivilDate, Timestamp};
use alice_ephemeris::TimeError;

/// Julian Day Number of a Gregorian date, Fliegel & Van Flandern (1968), with truncating
/// division as published; valid for JDN ≥ 0 (years after −4713).
fn jdn(y: i64, m: i64, d: i64) -> i64 {
    let a = (m - 14) / 12;
    (1461 * (y + 4800 + a)) / 4 + (367 * (m - 2 - 12 * a)) / 12 - (3 * ((y + 4900 + a) / 100)) / 4
        + d
        - 32075
}

fn date(y: i64, m: u8, d: u8) -> CivilDate {
    CivilDate::from_ymd(y, m, d).unwrap_or_else(|e| panic!("{y}-{m}-{d}: {e}"))
}

/// Days in a month, from the leap-year rule written out (not the crate's table).
fn month_len(y: i64, m: u8) -> u8 {
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if leap {
                29
            } else {
                28
            }
        }
    }
}

struct Lcg(u64);
impl Lcg {
    fn next(&mut self, n: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) % n
    }
}

#[test]
fn day_differences_match_the_julian_day_number() {
    let mut r = Lcg(0x5eed_c1f1);
    let mut compared = 0;
    for _ in 0..20_000 {
        let pick = |r: &mut Lcg| {
            let y = r.next(8001) as i64 - 4000;
            let m = (r.next(12) + 1) as u8;
            let d = (r.next(u64::from(month_len(y, m))) + 1) as u8;
            (y, m, d)
        };
        let (a, b) = (pick(&mut r), pick(&mut r));
        let want =
            jdn(b.0, i64::from(b.1), i64::from(b.2)) - jdn(a.0, i64::from(a.1), i64::from(a.2));
        let got = date(a.0, a.1, a.2).days_until(date(b.0, b.1, b.2));
        assert_eq!(got, want, "{a:?} → {b:?}");
        compared += 1;
    }
    assert_eq!(compared, 20_000);
}

#[test]
fn fixed_facts() {
    assert_eq!(date(1970, 1, 1).days_until(date(2000, 1, 1)), 10_957);
    assert_eq!(date(2000, 1, 1).days_until(date(1970, 1, 1)), -10_957);
    assert_eq!(date(2026, 11, 25).days_until(date(2026, 11, 25)), 0);
    assert_eq!(date(2024, 2, 28).days_until(date(2024, 3, 1)), 2);
    assert_eq!(date(2023, 2, 28).days_until(date(2023, 3, 1)), 1);
    assert_eq!(date(2024, 2, 29).to_string(), "2024-02-29");
    assert_eq!(date(2000, 2, 29).day(), 29);
    for (y, m, d) in [
        (2023, 2, 29),
        (1900, 2, 29),
        (2026, 13, 1),
        (2026, 0, 1),
        (2026, 1, 0),
        (2026, 4, 31),
    ] {
        assert_eq!(
            CivilDate::from_ymd(y, m, d),
            Err(TimeError::InvalidDate),
            "{y}-{m}-{d}"
        );
    }
}

#[test]
fn every_day_of_every_month_is_accepted_and_the_next_is_not() {
    let mut accepted = 0;
    for y in [
        -401, -400, -1, 0, 1, 1582, 1899, 1900, 2000, 2023, 2024, 2100, 9999, 10_000,
    ] {
        for m in 1..=12u8 {
            let n = month_len(y, m);
            for d in 1..=n {
                assert!(CivilDate::from_ymd(y, m, d).is_ok(), "{y}-{m}-{d}");
                accepted += 1;
            }
            assert_eq!(
                CivilDate::from_ymd(y, m, n + 1),
                Err(TimeError::InvalidDate),
                "{y}-{m}-{}",
                n + 1
            );
        }
    }
    assert_eq!(accepted, 14 * 365 + 5); // -400, 0, 2000, 2024 and 10000 are the leap years
}

#[test]
fn the_order_agrees_with_the_day_difference() {
    let mut r = Lcg(0x0bde_57a7);
    for _ in 0..5_000 {
        let y = |r: &mut Lcg| r.next(801) as i64 + 1700;
        let (ya, yb) = (y(&mut r), y(&mut r));
        let (ma, mb) = ((r.next(12) + 1) as u8, (r.next(12) + 1) as u8);
        let a = date(ya, ma, (r.next(u64::from(month_len(ya, ma))) + 1) as u8);
        let b = date(yb, mb, (r.next(u64::from(month_len(yb, mb))) + 1) as u8);
        assert_eq!(
            a.days_until(b).signum(),
            (b.cmp(&a) as i8).into(),
            "{a} {b}"
        );
    }
}

#[test]
fn plus_days_and_days_until_agree() {
    let epoch = Timestamp::UNIX_EPOCH.civil_date_at_offset(0).unwrap();
    assert_eq!(epoch, date(1970, 1, 1));
    for n in [
        -1_000_000_i64,
        -36_525,
        -1,
        0,
        1,
        59,
        10_957,
        20_419,
        2_932_896,
    ] {
        let d = Timestamp::UNIX_EPOCH
            .plus_days(n)
            .civil_date_at_offset(0)
            .unwrap();
        assert_eq!(epoch.days_until(d), n, "{n} → {d}");
    }
}

#[test]
fn the_whole_year_range_and_its_ends() {
    // the 400-year Gregorian cycle is 146 097 days at any year, so 2·10⁹ years are 5·10⁶ cycles
    let (lo, hi) = (date(-1_000_000_000, 1, 1), date(1_000_000_000, 1, 1));
    assert_eq!(lo.days_until(hi), 5_000_000 * 146_097);
    assert_eq!(hi.days_until(lo), -5_000_000 * 146_097);
    assert!(CivilDate::from_ymd(1_000_000_000, 12, 31).is_ok());
    for y in [1_000_000_001, -1_000_000_001, i64::MAX, i64::MIN] {
        assert_eq!(
            CivilDate::from_ymd(y, 1, 1),
            Err(TimeError::InvalidDate),
            "{y}"
        );
    }
    for m in [0, 13, u8::MAX] {
        assert_eq!(
            CivilDate::from_ymd(2026, m, 1),
            Err(TimeError::InvalidDate),
            "month {m}"
        );
    }
    assert_eq!(
        CivilDate::from_ymd(2026, 1, u8::MAX),
        Err(TimeError::InvalidDate)
    );
}
