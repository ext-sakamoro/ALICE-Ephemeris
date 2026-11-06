//! Directions through the horizon, against ERFA's eraAtco13 (tests/data/star-fixture.csv, written
//! directions, not catalogue stars) and against what can be worked out by hand.
//!
//! Author: Moroya Sakamoto

use alice_ephemeris::horizon::Site;
use alice_ephemeris::stars::{stars_in_sky, Star};
use alice_ephemeris::Jd2;

const DEG: f64 = core::f64::consts::PI / 180.0;
const ARCSEC: f64 = DEG / 3600.0;

/// Aberration, which this leaves out, is at most 20.5″, and UT1 is taken as UTC on both sides
const WITHIN: f64 = 25.0 * ARCSEC;

fn site(lat: f64, lon: f64) -> Site {
    Site {
        latitude: lat * DEG,
        longitude: lon * DEG,
        height: 0.0,
    }
}

#[test]
fn the_directions_agree_with_erfa() {
    let text = include_str!("data/star-fixture.csv");
    let (mut compared, mut worst, mut below, mut above) = (0, 0.0f64, 0, 0);
    for line in text.lines().skip(1) {
        let v: Vec<f64> = line
            .split(',')
            .map(|x| x.parse().expect("a number"))
            .collect();
        let star = Star {
            ra: v[0] * DEG,
            dec: v[1] * DEG,
            pm_ra: v[2] * ARCSEC,
            pm_dec: v[3] * ARCSEC,
            mag: 0.0,
        };
        let (ut1, tt) = (
            Jd2 {
                jd1: v[6],
                jd2: v[7],
            },
            Jd2 {
                jd1: v[8],
                jd2: v[9],
            },
        );
        let at = site(v[4], v[5]);
        let binding = [star];
        let h = stars_in_sky(&binding, tt, ut1, 0.0, 0.0, &at)
            .next()
            .expect("one star");
        // the angle between the two directions, so the azimuth near the zenith does not count
        let (a, b) = (v[11], h.altitude);
        let d = (a.sin() * b.sin() + a.cos() * b.cos() * (v[10] - h.azimuth).cos())
            .clamp(-1.0, 1.0)
            .acos();
        assert!(
            d < WITHIN,
            "{line}: {} arcsec, altitude {} azimuth {}",
            d / ARCSEC,
            b / DEG,
            h.azimuth / DEG
        );
        worst = worst.max(d);
        compared += 1;
        if b < 0.0 {
            below += 1;
        } else {
            above += 1;
        }
    }
    assert_eq!(compared, 60);
    assert!(
        below > 5 && above > 5,
        "only one side of the horizon: {below} {above}"
    );
    assert!(
        worst > 1.0 * ARCSEC,
        "the two agree better than aberration allows: {}",
        worst / ARCSEC
    );
}

fn j2000() -> (Jd2, Jd2) {
    (
        Jd2 {
            jd1: 2_451_545.0,
            jd2: 0.0,
        },
        Jd2 {
            jd1: 2_451_545.0,
            jd2: 0.0,
        },
    )
}

#[test]
fn the_celestial_pole_stands_at_the_site_s_latitude() {
    let (tt, ut1) = j2000();
    for lat in [36.0, -33.9, 60.0] {
        let at = site(lat, 140.0);
        let pole = [Star {
            ra: 0.0,
            dec: 90.0 * DEG,
            pm_ra: 0.0,
            pm_dec: 0.0,
            mag: 0.0,
        }];
        let h = stars_in_sky(&pole, tt, ut1, 0.0, 0.0, &at).next().unwrap();
        // geodetic and geographic latitude are the same here; the pole of date is J2000's at J2000
        assert!(
            (h.altitude / DEG - lat).abs() < 0.02,
            "{lat}: {}",
            h.altitude / DEG
        );
    }
}

#[test]
fn a_day_later_by_the_stars_it_stands_in_the_same_place() {
    // a sidereal day is 0.99726957 days
    let (tt, ut1) = j2000();
    let star = [Star {
        ra: 1.0,
        dec: 0.4,
        pm_ra: 0.0,
        pm_dec: 0.0,
        mag: 0.0,
    }];
    let at = site(36.0, 140.0);
    let a = stars_in_sky(&star, tt, ut1, 0.0, 0.0, &at).next().unwrap();
    let later = |j: Jd2| Jd2 {
        jd1: j.jd1,
        jd2: j.jd2 + 0.997_269_57,
    };
    let b = stars_in_sky(&star, later(tt), later(ut1), 0.0, 0.0, &at)
        .next()
        .unwrap();
    assert!(
        (a.altitude - b.altitude).abs() < 5.0 * ARCSEC,
        "{} {}",
        a.altitude,
        b.altitude
    );
    assert!(
        (a.azimuth - b.azimuth).abs() < 10.0 * ARCSEC,
        "{} {}",
        a.azimuth,
        b.azimuth
    );
}

#[test]
fn opposite_directions_are_on_opposite_sides_of_the_horizon() {
    let (tt, ut1) = j2000();
    let stars = [
        Star {
            ra: 1.0,
            dec: 0.4,
            pm_ra: 0.0,
            pm_dec: 0.0,
            mag: 0.0,
        },
        Star {
            ra: 1.0 + core::f64::consts::PI,
            dec: -0.4,
            pm_ra: 0.0,
            pm_dec: 0.0,
            mag: 0.0,
        },
    ];
    let at = site(36.0, 140.0);
    let v: Vec<_> = stars_in_sky(&stars, tt, ut1, 0.0, 0.0, &at).collect();
    assert_eq!(v.len(), 2);
    assert!((v[0].altitude + v[1].altitude).abs() < 5.0 * ARCSEC);
}

#[test]
fn proper_motion_moves_it_by_what_it_says() {
    // 100″ a year in declination, for 10 years, on the meridian-free difference of two runs
    let (tt, ut1) = j2000();
    let later = Jd2 {
        jd1: tt.jd1,
        jd2: 3652.5,
    };
    let at = site(36.0, 140.0);
    let still = [Star {
        ra: 1.0,
        dec: 0.4,
        pm_ra: 0.0,
        pm_dec: 0.0,
        mag: 0.0,
    }];
    let moving = [Star {
        pm_dec: 100.0 * ARCSEC,
        ..still[0]
    }];
    let (a, b) = (
        stars_in_sky(&still, later, ut1, 0.0, 0.0, &at)
            .next()
            .unwrap(),
        stars_in_sky(&moving, later, ut1, 0.0, 0.0, &at)
            .next()
            .unwrap(),
    );
    let d = (a.altitude.sin() * b.altitude.sin()
        + a.altitude.cos() * b.altitude.cos() * (a.azimuth - b.azimuth).cos())
    .clamp(-1.0, 1.0)
    .acos();
    assert!((d / ARCSEC - 1000.0).abs() < 1.0, "{}", d / ARCSEC);
}

#[test]
fn proper_motion_in_right_ascension_is_measured_along_the_sky() {
    // 100″ a year along the sky at declination 60°, for 10 years: 1000″ whatever the declination
    let (tt, ut1) = j2000();
    let later = Jd2 {
        jd1: tt.jd1,
        jd2: 3652.5,
    };
    let at = site(36.0, 140.0);
    let still = [Star {
        ra: 1.0,
        dec: 60.0 * DEG,
        pm_ra: 0.0,
        pm_dec: 0.0,
        mag: 0.0,
    }];
    let moving = [Star {
        pm_ra: 100.0 * ARCSEC,
        ..still[0]
    }];
    let (a, b) = (
        stars_in_sky(&still, later, ut1, 0.0, 0.0, &at)
            .next()
            .unwrap(),
        stars_in_sky(&moving, later, ut1, 0.0, 0.0, &at)
            .next()
            .unwrap(),
    );
    let d = (a.altitude.sin() * b.altitude.sin()
        + a.altitude.cos() * b.altitude.cos() * (a.azimuth - b.azimuth).cos())
    .clamp(-1.0, 1.0)
    .acos();
    assert!((d / ARCSEC - 1000.0).abs() < 1.0, "{}", d / ARCSEC);
}
