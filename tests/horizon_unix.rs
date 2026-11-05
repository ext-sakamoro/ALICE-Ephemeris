//! `horizon::sky_at_unix_ms` against references outside its own code path.
//!
//! - At the March equinox of 2026 (2026-03-20 14:46 UTC) the Sun's declination is about 0, so at
//!   the subsolar point (latitude 0, longitude −(15°/h · UT − 180° + the equation of time, −7.5
//!   min)) it stands within a degree of the zenith, and of the nadir at the antipode. This
//!   catches a wrong unit, a longitude of the wrong sign or a wrong day.
//! - In 2026 TT − UTC is 37 s + 32.184 s (the published leap second count, not the crate's
//!   table), and UT1 is taken as UTC: the result equals `horizon::sky` at those two dates.
//! - A time before UTC (1960) is an error, not a sky.
//!
//! Author: Moroya Sakamoto

use alice_ephemeris::horizon::{sky, sky_at_unix_ms, Site};
use alice_ephemeris::Jd2;

const DEG: f64 = core::f64::consts::PI / 180.0;

/// 2026-03-20 14:46:00 UTC in Unix milliseconds (20 532 days after 1970-01-01, plus 14:46)
const EQUINOX_MS: i64 = 20_532 * 86_400_000 + (14 * 3600 + 46 * 60) * 1000;

#[test]
fn at_the_equinox_the_sun_is_overhead_at_the_subsolar_point() {
    let ut_hours = 14.0 + 46.0 / 60.0;
    let lon = -(15.0 * ut_hours - 180.0 - 7.5 / 4.0);
    let at = |lat: f64, lon: f64| {
        let site = Site {
            latitude: lat * DEG,
            longitude: lon * DEG,
            height: 0.0,
        };
        sky_at_unix_ms(EQUINOX_MS, &site)
            .expect("a time in the table")
            .sun
            .altitude
            / DEG
    };
    let up = at(0.0, lon);
    assert!(up > 89.0, "the Sun at the subsolar point: altitude {up}°");
    let down = at(0.0, lon + 180.0);
    assert!(down < -89.0, "the Sun at the antipode: altitude {down}°");
}

#[test]
fn tt_is_utc_plus_69_184_s_and_ut1_is_utc_in_2026() {
    let site = Site {
        latitude: 35.0 * DEG,
        longitude: 139.0 * DEG,
        height: 0.0,
    };
    // 2026-11-05 12:34:56.789 UTC
    let ms = 20_762 * 86_400_000 + (12 * 3600 + 34 * 60 + 56) * 1000 + 789;
    let mjd = 40_587.0 + 20_762.0;
    let frac = ((12 * 3600 + 34 * 60 + 56) as f64 + 0.789) / 86_400.0;
    let ut1 = Jd2 {
        jd1: 2_400_000.5 + mjd,
        jd2: frac,
    };
    let tt = Jd2 {
        jd1: 2_400_000.5 + mjd,
        jd2: frac + 69.184 / 86_400.0,
    };
    let want = sky(tt, ut1, 0.0, 0.0, &site);
    let got = sky_at_unix_ms(ms, &site).expect("a time in the table");
    for (name, a, b) in [
        ("sun altitude", got.sun.altitude, want.sun.altitude),
        ("sun azimuth", got.sun.azimuth, want.sun.azimuth),
        ("moon altitude", got.moon.altitude, want.moon.altitude),
        ("moon azimuth", got.moon.azimuth, want.moon.azimuth),
    ] {
        assert!((a - b).abs() < 1e-11, "{name}: {a} vs {b}");
    }
}

#[test]
fn a_time_before_utc_is_an_error() {
    // UTC begins in 1960; 1900-01-01 and the earliest i64 are before it
    let site = Site {
        latitude: 0.0,
        longitude: 0.0,
        height: 0.0,
    };
    for ms in [i64::MIN, -2_208_988_800_000] {
        assert_eq!(
            sky_at_unix_ms(ms, &site).err(),
            Some(alice_ephemeris::TimeError::BeforeUtc),
            "{ms}"
        );
    }
    // 1960-01-01 is in it
    assert!(sky_at_unix_ms(-315_619_200_000, &site).is_ok());
}
