//! Calendar ↔ MJD against ERFA eraCal2jd (tests/golden/calendar.csv), and
//! Instant ↔ two-part JD against exact rational fractions (jd.csv).

mod common;
use alice_ephemeris::{civil_from_mjd, mjd_from_civil, Instant, Jd2, Mjd, Tt};
use common::{f64_, i64_, rows, u64_, u8_, ulps};

#[test]
fn mjd_matches_erfa_cal2jd() {
    let mut n = 0;
    for r in rows("calendar.csv") {
        let (y, m, d) = (i64_(&r[0]), u8_(&r[1]), u8_(&r[2]));
        let mjd = mjd_from_civil(y, m, d).unwrap_or_else(|e| panic!("{r:?}: {e}"));
        assert_eq!(mjd, i64_(&r[3]), "{r:?}");
        assert_eq!(civil_from_mjd(mjd).unwrap(), (y, m, d), "{r:?}");
        n += 1;
    }
    assert!(n > 500);
}

#[test]
fn jd2_matches_exact_fraction() {
    let mut n = 0;
    for r in rows("jd.csv") {
        let t = Instant::<Tt>::new(i64_(&r[0]), u64_(&r[1])).unwrap();
        let jd = t.to_jd2();
        assert_eq!(jd.jd1, f64_(&r[2]), "{r:?}");
        // the fraction is (rem + atto·1e-18)/86400 in two roundings vs one
        assert!(ulps(jd.jd2, f64_(&r[3])) <= 1, "{r:?}: {:?}", jd.jd2);
        // and back: an f64 day fraction resolves ≈ 10 ps
        let back = Instant::<Tt>::from_jd2(jd).unwrap();
        let d = (back.total_attoseconds() - t.total_attoseconds()).abs();
        assert!(d < 20_000_000, "{r:?}: {d} as");
        n += 1;
    }
    assert!(n > 250);
}

#[test]
fn mjd_parts_are_exact_both_ways() {
    let mut n = 0;
    let mut s = -9_000_000_000_000i64;
    while s < 9_000_000_000_000 {
        let t = Instant::<Tt>::new(s, 987_654_321).unwrap();
        let m = t.to_mjd();
        // J2000.0 = MJD 51544.5 exactly
        let expect = i128::from(s) + 51_544 * 86_400 + 43_200;
        assert_eq!(
            i128::from(m.day) * 86_400 + i128::from(m.second_of_day),
            expect
        );
        assert_eq!(Instant::<Tt>::from_mjd(m).unwrap(), t);
        s += 77_777_777_777;
        n += 1;
    }
    assert!(n > 200);
    let j2000 = Instant::<Tt>::J2000.to_mjd();
    assert_eq!(
        j2000,
        Mjd {
            day: 51_544,
            second_of_day: 43_200,
            atto: 0
        }
    );
    assert_eq!(
        Instant::<Tt>::J2000.to_jd2(),
        Jd2 {
            jd1: 2_451_545.0,
            jd2: 0.0
        }
    );
}
