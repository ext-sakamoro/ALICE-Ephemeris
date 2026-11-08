//! Directions through the horizon, against ERFA's eraAtco13 (tests/data/star-fixture.csv, written
//! directions, not catalogue stars) and against what can be worked out by hand.
//!
//! Author: Moroya Sakamoto

use alice_det_math::{acos64, cos64, max64, min64, sin_cos64};
use alice_ephemeris::horizon::Site;
use alice_ephemeris::star_table::BRIGHTEST;
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

/// The angle between two directions given by altitude and azimuth
fn angle_between(alt_a: f64, az_a: f64, alt_b: f64, az_b: f64) -> f64 {
    let ((sa, ca), (sb, cb)) = (sin_cos64(alt_a), sin_cos64(alt_b));
    acos64(min64(
        max64(sa * sb + ca * cb * cos64(az_a - az_b), -1.0),
        1.0,
    ))
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
        let d = angle_between(a, v[10], b, h.azimuth);
        assert!(
            d < WITHIN,
            "{line}: {} arcsec, altitude {} azimuth {}",
            d / ARCSEC,
            b / DEG,
            h.azimuth / DEG
        );
        worst = max64(worst, d);
        compared += 1;
        if b < 0.0 {
            below += 1;
        } else {
            above += 1;
        }
    }
    assert_eq!(compared, 160);
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
    let d = angle_between(a.altitude, a.azimuth, b.altitude, b.azimuth);
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
    let d = angle_between(a.altitude, a.azimuth, b.altitude, b.azimuth);
    assert!((d / ARCSEC - 1000.0).abs() < 1.0, "{}", d / ARCSEC);
}

// Sirius, Canopus, Arcturus, Vega, Polaris: J2000 degrees and magnitude written from memory of the usual
// published values, not read from a source. It catches a misread column or a star missing from the
// table; the real checks are the table itself and the place against ERFA.
const NAMED: [(&str, f64, f64, f32); 5] = [
    ("Sirius", 101.2872, -16.7161, -1.46),
    ("Canopus", 95.9880, -52.6957, -0.72),
    ("Arcturus", 213.9153, 19.1824, -0.04),
    ("Vega", 279.2347, 38.7837, 0.03),
    ("Polaris", 37.9546, 89.2641, 2.02),
];

#[test]
fn the_table_holds_the_named_stars_where_they_are_published() {
    let mut found = 0;
    for (name, ra, dec, mag) in NAMED {
        let s = BRIGHTEST
            .iter()
            .find(|s| {
                let c = (s.dec / DEG - dec).abs() < 0.05;
                let a = ((s.ra / DEG - ra) * cos64(dec * DEG)).abs() < 0.05;
                c && a
            })
            .unwrap_or_else(|| panic!("{name} is not in the table"));
        assert!((s.mag - mag).abs() < 0.1, "{name}: {}", s.mag);
        found += 1;
    }
    assert_eq!(found, NAMED.len());
}

#[test]
fn the_table_is_every_star_to_magnitude_six_in_order_with_sane_values() {
    // the catalogue has 5080 rows with a magnitude of 6.0 or brighter
    assert_eq!(BRIGHTEST.len(), 5080);
    assert!(BRIGHTEST.windows(2).all(|w| w[0].mag <= w[1].mag));
    assert_eq!(BRIGHTEST[0].mag, -1.46);
    assert_eq!(BRIGHTEST[5079].mag, 6.0);
    for s in &BRIGHTEST {
        assert!((0.0..core::f64::consts::TAU).contains(&s.ra));
        assert!(s.dec.abs() <= 90.0 * DEG);
        assert!(s.pm_ra.abs() < 20.0 * ARCSEC && s.pm_dec.abs() < 20.0 * ARCSEC);
    }
    // the objects with HR numbers that are not stars are not here: 47 Tuc and M 31 at their places
    for (ra, dec) in [
        (6.02417, -72.08),
        (10.67375, 41.27),
        (138.01167, -64.86),
        (132.82917, 11.81),
    ] {
        assert!(
            !BRIGHTEST
                .iter()
                .any(|s| (s.ra / DEG - ra).abs() < 0.02 && (s.dec / DEG - dec).abs() < 0.02),
            "{ra} {dec}"
        );
    }
}

/// The file a page fetches is the table: read as the generator wrote it (a column at a time: ra, dec as i32 in 1e-4
/// degree, pm_ra, pm_dec as i16 in 1e-3 arcsec a year, mag as i16 in hundredths), every star's every value is the
/// table's own f64 / f32, bit for bit, in the table's order (brightest first). Read here on its own, not through the
/// sky module
#[test]
fn the_file_a_page_fetches_is_the_table_bit_for_bit() {
    let bin = include_bytes!("../sky/stars.bin");
    let n = BRIGHTEST.len();
    assert_eq!(bin.len(), n * 14, "14 bytes a star");
    let int = |at: usize, size: usize| -> f64 {
        let b = &bin[at..at + size];
        if size == 4 {
            f64::from(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        } else {
            f64::from(i16::from_le_bytes([b[0], b[1]]))
        }
    };
    let deg = std::f64::consts::PI / 180.0;
    let arcsec = deg / 3600.0;
    let mut compared = 0;
    for (i, s) in BRIGHTEST.iter().enumerate() {
        let got = [
            (int(4 * i, 4) / 1e4) * deg,
            (int(4 * n + 4 * i, 4) / 1e4) * deg,
            (int(8 * n + 2 * i, 2) / 1e3) * arcsec,
            (int(10 * n + 2 * i, 2) / 1e3) * arcsec,
        ];
        let want = [s.ra, s.dec, s.pm_ra, s.pm_dec];
        for (g, w) in got.iter().zip(want) {
            assert_eq!(g.to_bits(), w.to_bits(), "star {i}: {g} for {w}");
            compared += 1;
        }
        let mag = (int(12 * n + 2 * i, 2) / 100.0) as f32;
        assert_eq!(
            mag.to_bits(),
            s.mag.to_bits(),
            "star {i}: magnitude {mag} for {}",
            s.mag
        );
        compared += 1;
    }
    assert_eq!(compared, 5 * n);
    assert!(n > 5000, "{n} stars");
}

#[test]
fn a_named_star_stands_where_erfa_puts_it_by_the_table_s_own_values() {
    // Sirius from the table, through this crate, and the published row of the ERFA file
    let row = include_str!("data/star-fixture.csv")
        .lines()
        .find(|l| l.starts_with("101.2872,-16.7161,") && l.contains(",36.0,140.0,"))
        .expect("a Sirius row at the site");
    let v: Vec<f64> = row.split(',').map(|x| x.parse().unwrap()).collect();
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
    let at = site(36.0, 140.0);
    let h = stars_in_sky(&BRIGHTEST[..1], tt, ut1, 0.0, 0.0, &at)
        .next()
        .unwrap();
    let (a, b) = (v[11], h.altitude);
    let d = angle_between(a, v[10], b, h.azimuth);
    assert!(d < 40.0 * ARCSEC, "{} arcsec", d / ARCSEC);
}
