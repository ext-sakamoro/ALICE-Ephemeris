//! Degenerate and boundary inputs: each returns the error (or value) written
//! next to it, never a panic and never a wrapped value.

use alice_ephemeris::leap;
use alice_ephemeris::{
    civil_from_mjd, mjd_from_civil, Duration, ExpiryPolicy, Instant, Jd2, Mjd, Provenance, Tai,
    Tdb, TdbModel, TimeError, Tt, UtcDateTime, ATTO_PER_SEC,
};

#[test]
fn instant_and_duration_reject_a_full_second_of_attoseconds() {
    assert_eq!(
        Instant::<Tt>::new(0, ATTO_PER_SEC),
        Err(TimeError::OutOfRange)
    );
    assert_eq!(Duration::new(0, ATTO_PER_SEC), Err(TimeError::OutOfRange));
    assert!(Instant::<Tt>::new(0, ATTO_PER_SEC - 1).is_ok());
}

#[test]
fn arithmetic_at_the_ends_of_i64_is_an_error_not_a_wrap() {
    let max = Instant::<Tai>::new(i64::MAX, ATTO_PER_SEC - 1).unwrap();
    let min = Instant::<Tai>::new(i64::MIN, 0).unwrap();
    let one_as = Duration::new(0, 1).unwrap();
    assert_eq!(max.checked_add(one_as), Err(TimeError::OutOfRange));
    assert_eq!(min.checked_sub(one_as), Err(TimeError::OutOfRange));
    // TT = TAI + 32.184 s does not fit after the largest TAI
    assert_eq!(max.to_tt(), Err(TimeError::OutOfRange));
    assert_eq!(
        Instant::<Tt>::new(i64::MIN, 0).unwrap().to_tai(),
        Err(TimeError::OutOfRange)
    );
    assert_eq!(max.duration_since(min), Err(TimeError::OutOfRange));
    // negative durations keep a non-negative fraction: −0.25 s
    let d = Instant::<Tt>::J2000
        .duration_since(Instant::<Tt>::new(0, 250_000_000_000_000_000).unwrap())
        .unwrap();
    assert_eq!(
        (d.seconds(), d.attoseconds()),
        (-1, 750_000_000_000_000_000)
    );
}

#[test]
fn tdb_at_the_ends_of_i64_is_an_error_or_a_value() {
    for model in [TdbModel::SpiceEt, TdbModel::Fb1990] {
        // the series is evaluated far outside any stated range: the value is
        // finite, and the conversion either succeeds or reports OutOfRange
        // (Fb1990's polynomial terms exceed 1 s millions of years out)
        let big = Instant::<Tt>::new(i64::MAX, 0).unwrap();
        assert!(model.tdb_minus_tt(big).is_finite(), "{model:?}");
        assert!(!model.within_stated_accuracy(big));
        let r = big.to_tdb(model);
        assert!(
            r.is_ok() || r == Err(TimeError::OutOfRange),
            "{model:?}: {r:?}"
        );
        let small = Instant::<Tdb>::new(i64::MIN, 0).unwrap();
        let r = small.to_tt(model);
        assert!(
            r.is_ok() || r == Err(TimeError::OutOfRange),
            "{model:?}: {r:?}"
        );
    }
    let tt = Instant::<Tt>::J2000;
    assert!(TdbModel::Fb1990.within_stated_accuracy(tt));
    assert!(!TdbModel::SpiceEt.within_stated_accuracy(tt));
}

#[test]
fn invalid_civil_fields() {
    assert_eq!(
        UtcDateTime::new(2020, 13, 1, 0, 0, 0, 0),
        Err(TimeError::InvalidDate)
    );
    assert_eq!(
        UtcDateTime::new(2020, 0, 1, 0, 0, 0, 0),
        Err(TimeError::InvalidDate)
    );
    assert_eq!(
        UtcDateTime::new(2019, 2, 29, 0, 0, 0, 0),
        Err(TimeError::InvalidDate)
    );
    assert!(UtcDateTime::new(2020, 2, 29, 0, 0, 0, 0).is_ok());
    assert_eq!(
        UtcDateTime::new(1900, 2, 29, 0, 0, 0, 0),
        Err(TimeError::InvalidDate)
    );
    assert_eq!(
        UtcDateTime::new(2020, 1, 1, 24, 0, 0, 0),
        Err(TimeError::InvalidTime)
    );
    assert_eq!(
        UtcDateTime::new(2020, 1, 1, 0, 60, 0, 0),
        Err(TimeError::InvalidTime)
    );
    assert_eq!(
        UtcDateTime::new(2020, 1, 1, 0, 0, 61, 0),
        Err(TimeError::InvalidTime)
    );
    assert_eq!(
        UtcDateTime::new(2020, 1, 1, 0, 0, 0, ATTO_PER_SEC),
        Err(TimeError::InvalidTime)
    );
    // second 60 only as 23:59:60
    assert_eq!(
        UtcDateTime::new(2016, 12, 31, 12, 0, 60, 0),
        Err(TimeError::NotALeapSecond)
    );
    // a hand-built value with bad fields is checked again by the conversion
    let bad = UtcDateTime {
        year: 2016,
        month: 12,
        day: 31,
        hour: 23,
        minute: 59,
        second: 61,
        atto: 0,
    };
    assert_eq!(
        bad.to_tai(ExpiryPolicy::Strict),
        Err(TimeError::InvalidTime)
    );
    assert_eq!(mjd_from_civil(i64::MAX, 1, 1), Err(TimeError::InvalidDate));
    assert_eq!(civil_from_mjd(i64::MIN), Err(TimeError::OutOfRange));
}

#[test]
fn utc_range_and_expiry() {
    let first = UtcDateTime::new(1960, 1, 1, 0, 0, 0, 0).unwrap();
    assert!(first.to_tai(ExpiryPolicy::Strict).is_ok());
    let before = UtcDateTime::new(1959, 12, 31, 23, 59, 59, ATTO_PER_SEC - 1).unwrap();
    assert_eq!(
        before.to_tai(ExpiryPolicy::Strict),
        Err(TimeError::BeforeUtc)
    );
    assert_eq!(
        before.to_tai(ExpiryPolicy::AssumeNoNewLeapSeconds),
        Err(TimeError::BeforeUtc)
    );
    // the last representable instant before the expiry, then the expiry
    let last = UtcDateTime::new(2027, 6, 27, 23, 59, 59, ATTO_PER_SEC - 1).unwrap();
    let c = last.to_tai(ExpiryPolicy::Strict).unwrap();
    assert_eq!(c.provenance, Provenance::Table);
    let exp = UtcDateTime::new(2027, 6, 28, 0, 0, 0, 0).unwrap();
    let expired = Err(TimeError::LeapTableExpired {
        expires_mjd: leap::EXPIRES_MJD,
    });
    assert_eq!(exp.to_tai(ExpiryPolicy::Strict), expired);
    let assumed = exp.to_tai(ExpiryPolicy::AssumeNoNewLeapSeconds).unwrap();
    assert_eq!(assumed.provenance, Provenance::AssumedNoNewLeapSeconds);
    // 37 s, as since 2017
    let label = (i128::from(mjd_from_civil(2027, 6, 28).unwrap() - 51_544) * 86_400 - 43_200)
        * i128::from(ATTO_PER_SEC);
    assert_eq!(
        assumed.value.total_attoseconds() - label,
        37 * i128::from(ATTO_PER_SEC)
    );
    // and the reverse direction applies the same policy
    assert_eq!(
        UtcDateTime::from_tai(assumed.value, ExpiryPolicy::Strict),
        Err(TimeError::LeapTableExpired {
            expires_mjd: leap::EXPIRES_MJD
        })
    );
    let back = UtcDateTime::from_tai(assumed.value, ExpiryPolicy::AssumeNoNewLeapSeconds).unwrap();
    assert_eq!(
        (back.value, back.provenance),
        (exp, Provenance::AssumedNoNewLeapSeconds)
    );
    // TAI before UTC began, and far beyond the calendar range
    let early = Instant::<Tai>::new(-1_300_000_000, 0).unwrap();
    assert_eq!(
        UtcDateTime::from_tai(early, ExpiryPolicy::Strict),
        Err(TimeError::BeforeUtc)
    );
    let far = Instant::<Tai>::new(i64::MAX, 0).unwrap();
    assert_eq!(
        UtcDateTime::from_tai(far, ExpiryPolicy::AssumeNoNewLeapSeconds),
        Err(TimeError::OutOfRange)
    );
}

#[test]
fn leap_second_boundaries_are_monotone_and_one_second_apart() {
    let at = |s, atto| {
        UtcDateTime::new(2016, 12, 31, 23, 59, s, atto)
            .unwrap()
            .to_tai(ExpiryPolicy::Strict)
            .unwrap()
            .value
            .total_attoseconds()
    };
    let next = UtcDateTime::new(2017, 1, 1, 0, 0, 0, 0)
        .unwrap()
        .to_tai(ExpiryPolicy::Strict)
        .unwrap()
        .value
        .total_attoseconds();
    let a = i128::from(ATTO_PER_SEC);
    assert_eq!(at(60, 0) - at(59, 0), a);
    assert_eq!(next - at(60, 0), a);
    assert_eq!(at(60, ATTO_PER_SEC - 1) + 1, next);
    // a day without a leap second has no 23:59:60
    assert_eq!(
        UtcDateTime::new(2016, 12, 30, 23, 59, 60, 0)
            .unwrap()
            .to_tai(ExpiryPolicy::Strict),
        Err(TimeError::NotALeapSecond)
    );
    // 1961-07-31 ended 0.05 s early (TAI − UTC stepped down by 0.05 s)
    let short = |atto| {
        UtcDateTime::new(1961, 7, 31, 23, 59, 59, atto)
            .unwrap()
            .to_tai(ExpiryPolicy::Strict)
    };
    assert!(short(949_999_999_999_999_999).is_ok());
    assert_eq!(
        short(950_000_000_000_000_000),
        Err(TimeError::NotALeapSecond)
    );
}

#[test]
fn julian_dates_reject_non_finite_and_huge_values() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1e300] {
        let r = Instant::<Tt>::from_jd2(Jd2 { jd1: bad, jd2: 0.0 });
        assert_eq!(r, Err(TimeError::OutOfRange), "{bad}");
        let r = Instant::<Tt>::from_jd2(Jd2 {
            jd1: 2_451_545.0,
            jd2: bad,
        });
        assert_eq!(r, Err(TimeError::OutOfRange), "{bad}");
    }
    // a day count whose seconds do not fit i64
    let r = Instant::<Tt>::from_jd2(Jd2 {
        jd1: 4e17,
        jd2: 0.0,
    });
    assert_eq!(r, Err(TimeError::OutOfRange));
    // a fraction of the day of exactly 1 belongs to the next day
    let t = Instant::<Tt>::from_jd2(Jd2 {
        jd1: 2_451_545.0,
        jd2: 1.0,
    })
    .unwrap();
    assert_eq!((t.seconds(), t.attoseconds()), (86_400, 0));
    // negative fractions and an arbitrary split
    let t = Instant::<Tt>::from_jd2(Jd2 {
        jd1: 2_451_546.0,
        jd2: -0.75,
    })
    .unwrap();
    assert_eq!((t.seconds(), t.attoseconds()), (21_600, 0));
    let bad_mjd = Mjd {
        day: 0,
        second_of_day: 86_400,
        atto: 0,
    };
    assert_eq!(
        Instant::<Tt>::from_mjd(bad_mjd),
        Err(TimeError::InvalidTime)
    );
    let huge = Mjd {
        day: i64::MAX,
        second_of_day: 0,
        atto: 0,
    };
    assert_eq!(Instant::<Tt>::from_mjd(huge), Err(TimeError::OutOfRange));
}

#[test]
fn display_and_debug_name_the_value() {
    assert_eq!(
        TimeError::BeforeUtc.to_string(),
        "UTC is not defined before 1960-01-01"
    );
    assert_eq!(
        TimeError::LeapTableExpired {
            expires_mjd: 61_584
        }
        .to_string(),
        "leap-second table expired (valid before MJD 61584)"
    );
    assert_eq!(
        TimeError::OutOfRange.to_string(),
        "value out of the representable range"
    );
    assert_eq!(TimeError::InvalidDate.to_string(), "invalid calendar date");
    assert_eq!(TimeError::InvalidTime.to_string(), "invalid time of day");
    assert_eq!(
        TimeError::NotALeapSecond.to_string(),
        "time of day does not exist on that UTC day"
    );
    let t = Instant::<Tdb>::new(-5, 7).unwrap();
    assert_eq!(format!("{t:?}"), "TDB J2000-5s+000000000000000007as");
}

#[test]
fn positive_durations_keep_their_seconds() {
    let a = Instant::<Tt>::new(10, 0).unwrap();
    let b = Instant::<Tt>::new(13, 500).unwrap();
    let d = b.duration_since(a).unwrap();
    assert_eq!((d.seconds(), d.attoseconds()), (3, 500));
    assert_eq!(a.checked_add(d).unwrap(), b);
}

#[test]
fn a_day_fraction_that_rounds_to_one_moves_to_the_next_day() {
    // 23:59:59.999… after J2000 noon + 0 days: the fraction rounds to 1.0
    let t = Instant::<Tt>::new(86_399, ATTO_PER_SEC - 1).unwrap();
    assert_eq!(
        t.to_jd2(),
        Jd2 {
            jd1: 2_451_546.0,
            jd2: 0.0
        }
    );
    let t = Instant::<Tt>::new(-1, ATTO_PER_SEC - 1).unwrap();
    assert_eq!(
        t.to_jd2(),
        Jd2 {
            jd1: 2_451_545.0,
            jd2: 0.0
        }
    );
}

#[test]
fn the_last_tai_instant_before_the_expiry_still_has_a_label() {
    let last = UtcDateTime::new(2027, 6, 27, 23, 59, 59, ATTO_PER_SEC - 1).unwrap();
    let tai = last.to_tai(ExpiryPolicy::Strict).unwrap().value;
    let back = UtcDateTime::from_tai(tai, ExpiryPolicy::Strict).unwrap();
    assert_eq!((back.value, back.provenance), (last, Provenance::Table));
    let one_as = Duration::new(0, 1).unwrap();
    assert!(matches!(
        UtcDateTime::from_tai(tai.checked_add(one_as).unwrap(), ExpiryPolicy::Strict),
        Err(TimeError::LeapTableExpired { .. })
    ));
}

#[test]
fn every_day_of_the_drift_era_round_trips_at_its_last_moments() {
    // 1960–1971: TAI − UTC drifts, so the day of a TAI instant near the end
    // of a UTC day is found by stepping back from the 86400 s estimate
    let mut n = 0;
    for mjd in leap::FIRST_MJD..leap::LEAP_ERA_MJD + 2 {
        let (y, m, d) = civil_from_mjd(mjd).unwrap();
        for (s, atto) in [
            (0, 0),
            (59, 0),
            (59, 940_000_000_000_000_000),
            (59, 999_999_999_999_999_999),
        ] {
            let (h, mi) = if s == 0 { (0, 0) } else { (23, 59) };
            let Ok(dt) = UtcDateTime::new(y, m, d, h, mi, s, atto) else {
                continue;
            };
            let Ok(tai) = dt.to_tai(ExpiryPolicy::Strict) else {
                continue;
            };
            let back = UtcDateTime::from_tai(tai.value, ExpiryPolicy::Strict)
                .unwrap()
                .value;
            let again = back.to_tai(ExpiryPolicy::Strict).unwrap().value;
            let diff = again.total_attoseconds() - tai.value.total_attoseconds();
            assert!(diff.abs() <= 1, "{dt:?} -> {back:?}");
            assert_eq!(
                (back.year, back.month, back.day),
                (y, m, d),
                "{dt:?} -> {back:?}"
            );
            n += 1;
        }
    }
    assert!(n > 15_000, "{n}");
}
