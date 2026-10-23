//! UTC ↔ TAI against ERFA (eraDtf2d / eraUtctai / eraTaiutc / eraD2dtf),
//! tests/golden/utc_tai.csv and tai_utc.csv.

mod common;
use alice_ephemeris::{ExpiryPolicy, Instant, Provenance, Tai, TimeError, UtcDateTime};
use common::{f64_, i64_, rows, u64_, u8_};

/// ERFA returns TAI as a two-part JD, so its TAI − UTC carries the rounding
/// of the day fraction (≤ about 10⁻¹¹ s); exact integers are checked in
/// leap_table.rs.
const TOL_S: f64 = 5e-11;

#[test]
fn utc_to_tai_matches_erfa() {
    let mut counts = [0usize; 4];
    for r in rows("utc_tai.csv") {
        let dt = UtcDateTime {
            year: i64_(&r[0]),
            month: u8_(&r[1]),
            day: u8_(&r[2]),
            hour: u8_(&r[3]),
            minute: u8_(&r[4]),
            second: u8_(&r[5]),
            atto: u64_(&r[6]),
        };
        let label = i128::from(i64_(&r[7])) * 1_000_000_000_000_000_000 + i128::from(dt.atto);
        let strict = dt.to_tai(ExpiryPolicy::Strict);
        let assumed = dt.to_tai(ExpiryPolicy::AssumeNoNewLeapSeconds);
        let check = |tai: Instant<Tai>| {
            let got = (tai.total_attoseconds() - label) as f64 * 1e-18;
            let want = f64_(&r[9]);
            assert!((got - want).abs() <= TOL_S, "{r:?}: got {got}, ERFA {want}");
        };
        match r[8].as_str() {
            "ok" => {
                let c = strict.unwrap_or_else(|e| panic!("{r:?}: {e}"));
                assert_eq!(c.provenance, Provenance::Table);
                check(c.value);
                assert_eq!(assumed.unwrap(), c);
                counts[0] += 1;
            }
            "expired" => {
                assert!(
                    matches!(
                        strict,
                        Err(TimeError::LeapTableExpired { expires_mjd: 61584 })
                    ),
                    "{r:?}"
                );
                let c = assumed.unwrap();
                assert_eq!(c.provenance, Provenance::AssumedNoNewLeapSeconds);
                check(c.value);
                counts[1] += 1;
            }
            "before_utc" => {
                assert_eq!(strict, Err(TimeError::BeforeUtc), "{r:?}");
                counts[2] += 1;
            }
            "not_a_leap_second" => {
                assert_eq!(strict, Err(TimeError::NotALeapSecond), "{r:?}");
                counts[3] += 1;
            }
            other => panic!("unknown outcome {other}"),
        }
    }
    // every kind of row is present and compared
    assert!(counts.iter().all(|&c| c > 0), "{counts:?}");
    assert!(counts[0] > 900, "{counts:?}");
}

#[test]
fn tai_to_utc_matches_erfa() {
    let mut n = 0;
    let mut leap = 0;
    for r in rows("tai_utc.csv") {
        let tai = Instant::<Tai>::new(i64_(&r[0]), u64_(&r[1])).unwrap();
        let c = UtcDateTime::from_tai(tai, ExpiryPolicy::Strict)
            .unwrap_or_else(|e| panic!("{r:?}: {e}"));
        let u = c.value;
        let fields = (u.year, u.month, u.day, u.hour, u.minute, u.second);
        let want = (
            i64_(&r[2]),
            u8_(&r[3]),
            u8_(&r[4]),
            u8_(&r[5]),
            u8_(&r[6]),
            u8_(&r[7]),
        );
        assert_eq!(fields, want, "{r:?}");
        // ERFA prints nanoseconds (rounded); the inputs are at quarter seconds
        let ns = i128::from(u.atto) - i128::from(i64_(&r[8])) * 1_000_000_000;
        assert!(ns.abs() <= 1_000_000_000, "{r:?}: atto {}", u.atto);
        // and the label maps back to the same TAI instant (within 1 as)
        let back = u.to_tai(ExpiryPolicy::Strict).unwrap().value;
        assert!(
            (back.total_attoseconds() - tai.total_attoseconds()).abs() <= 1,
            "{r:?}"
        );
        n += 1;
        leap += usize::from(u.second == 60);
    }
    assert!(n > 500 && leap > 50, "{n} rows, {leap} in leap seconds");
}
