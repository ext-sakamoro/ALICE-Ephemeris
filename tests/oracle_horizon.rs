//! Oracle: the Sun and the Moon in an observer's sky (`horizon::sky`) and the
//! Moon's phase (`horizon::moon_phase`).
//!
//! - JPL Horizons observer tables (DE441, `tests/data/horizons-sky.csv`,
//!   written by scripts/fetch_horizons_sky.py): airless apparent azimuth and
//!   elevation at four sites (Tokyo, the equator at 0° E, Tromsø, Sydney) at
//!   18 instants, and the Moon's geocentric Sun–Target–Observer angle,
//!   illuminated fraction and lead / trail flag. The instants lie inside the
//!   blocks of `tests/data/finals2000A-excerpt.txt`, and the test reaches the
//!   sky through the crate's own path: UTC → TAI → TT, UT1 and the pole
//!   coordinates from those IERS rows.
//! - ERFA's test vector for `eraGd2gc` (WGS84) from `t_erfa_c.c`.
//!
//! The Law is geometric; Horizons' directions are apparent (light time and
//! aberration). One test adds those two effects to the Law's vectors, using
//! the crate's own velocities, and compares within the models' accuracy; the
//! other compares the Law as it is, within that plus the aberration it leaves
//! out. The tolerances are derived below.
//!
//! Author: Moroya Sakamoto

mod common;

use alice_det_math::{atan2_64, max64, sin_cos64, sqrt64};
use alice_ephemeris::eop::{EopRow, EopTable};
use alice_ephemeris::frames::{c2t06a, mtxv, mxv, Mat3};
use alice_ephemeris::horizon::{moon_phase, sky, Site};
use alice_ephemeris::sunmoon::{epv00, moon98};
use alice_ephemeris::{civil_from_mjd, ExpiryPolicy, Jd2, UtcDateTime};
use common::{f64_, rows};

const EXCERPT: &str = include_str!("data/finals2000A-excerpt.txt");
const ARCSEC: f64 = core::f64::consts::PI / 648_000.0;
const DAU: f64 = 149_597_870_700.0;
/// The speed of light, au/day.
const C_AU_DAY: f64 = 299_792_458.0 * 86_400.0 / DAU;
/// The pole coordinates of the test rows stay within this (asserted).
const POLE_MAX: f64 = 0.6 * ARCSEC;

/// Aberration: the observer's speed over c, at most (30.29 km/s orbital +
/// 0.47 km/s diurnal) / 299,792.458 km/s = 21.16″.
const ABERRATION: f64 = 21.16 * ARCSEC;

/// Terms common to both bodies at a site of geodetic latitude `lat`:
/// Horizons (ITRF93 orientation) and this Law (IAU 2006 CIO with the IERS
/// pole) may refer azimuth to poles that differ by the polar motion |p|,
/// which turns an azimuth by at most |p|·sec φ; the 2027 rows' predicted UT1
/// against Horizons' own prediction (≤ 5 ms, 0.08″); the celestial pole
/// offsets the Law leaves out (≤ 0.3 mas) and the second-order aberration
/// ((v/c)² ≤ 0.002″).
fn common_tol(lat: f64) -> f64 {
    let (_, c) = sin_cos64(lat);
    POLE_MAX / c + (0.08 + 0.001 + 0.002) * ARCSEC
}

/// Beyond the common terms: the Earth model's 11.2 km at 1 au (0.016″) for
/// the Sun; for the Moon the model's worst 18.3″ geocentric, enlarged by the
/// topocentric distance ratio (≤ 1.02) to 18.7″. The light time of each body
/// is applied in the apparent comparison; the geometric comparison adds the
/// aberration and, for the Moon, its geocentric motion during the light time
/// (v⊥/c ≤ 1.08 km/s / c = 0.74″ at any distance), which for the Sun is
/// within the 21.16″ (its barycentric motion moves it ≤ 0.011″).
fn model_tol(body: &str) -> f64 {
    match body {
        "sun" => 0.016 * ARCSEC,
        "moon" => 18.7 * ARCSEC,
        other => panic!("unknown body {other}"),
    }
}

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

/// A UTC Julian date printed with nine decimals as a civil UTC date and time,
/// exactly (one nanoday is 86,400,000,000,000 attoseconds).
fn utc_from_jd(s: &str) -> UtcDateTime {
    let (whole, frac) = s.split_once('.').unwrap();
    assert_eq!(frac.len(), 9, "{s}");
    let nanodays = whole.parse::<i64>().unwrap() * 1_000_000_000 + frac.parse::<i64>().unwrap()
        - 2_400_000_500_000_000;
    let (mjd, nd) = (
        nanodays.div_euclid(1_000_000_000),
        nanodays.rem_euclid(1_000_000_000),
    );
    let atto = u128::try_from(nd).unwrap() * 86_400_000_000_000;
    let secs = u32::try_from(atto / 1_000_000_000_000_000_000).unwrap();
    let (y, m, d) = civil_from_mjd(mjd).unwrap();
    UtcDateTime::new(
        y,
        m,
        d,
        u8::try_from(secs / 3600).unwrap(),
        u8::try_from(secs / 60 % 60).unwrap(),
        u8::try_from(secs % 60).unwrap(),
        u64::try_from(atto % 1_000_000_000_000_000_000).unwrap(),
    )
    .unwrap()
}

/// TT, UT1 and the pole coordinates (radians) at a UTC Julian date, through
/// the crate's conversions and the excerpt's EOP rows.
fn orientation(jd_utc: &str, tables: &[EopTable]) -> (Jd2, Jd2, f64, f64) {
    let tai = utc_from_jd(jd_utc)
        .to_tai(ExpiryPolicy::Strict)
        .unwrap()
        .value;
    let (ut1, eop) = tables
        .iter()
        .find_map(|t| tai.to_ut1(t, ExpiryPolicy::Strict).ok())
        .unwrap_or_else(|| panic!("{jd_utc}: no EOP block"));
    let (xp, yp) = (eop.xp_arcsec * ARCSEC, eop.yp_arcsec * ARCSEC);
    assert!(sqrt64(xp * xp + yp * yp) <= POLE_MAX, "{jd_utc}: pole");
    (tai.to_tt().unwrap().to_jd2(), ut1.to_jd2(), xp, yp)
}

fn site(name: &str) -> Site {
    let (lon, lat, h) = match name {
        "tokyo" => (139.6917, 35.6895, 40.0),
        "equator" => (0.0, 0.0, 0.0),
        "tromso" => (18.9553, 69.6496, 10.0),
        "sydney" => (151.2093, -33.8688, 58.0),
        other => panic!("unknown site {other}"),
    };
    Site {
        latitude: f64::to_radians(lat),
        longitude: f64::to_radians(lon),
        height: h,
    }
}

fn add(a: [f64; 3], b: [f64; 3], k: f64) -> [f64; 3] {
    [a[0] + k * b[0], a[1] + k * b[1], a[2] + k * b[2]]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Altitude and azimuth of an ITRS direction at a site (east, north, up of
/// the ellipsoid normal), computed here independently of `horizon`.
fn local(site: &Site, d: [f64; 3]) -> (f64, f64) {
    let (sp, cp) = sin_cos64(site.latitude);
    let (sl, cl) = sin_cos64(site.longitude);
    let east = -sl * d[0] + cl * d[1];
    let north = -sp * cl * d[0] - sp * sl * d[1] + cp * d[2];
    let up = cp * cl * d[0] + cp * sl * d[1] + sp * d[2];
    let az = atan2_64(east, north);
    (
        atan2_64(up, sqrt64(east * east + north * north)),
        if az < 0.0 {
            az + core::f64::consts::TAU
        } else {
            az
        },
    )
}

/// The apparent direction (light time, then aberration to first order) of a
/// body at geocentric GCRS position `geo` with barycentric velocity `v_body`,
/// seen from the site, as altitude and azimuth.
fn apparent(
    c2t: &Mat3,
    s: &Site,
    geo: [f64; 3],
    v_body: [f64; 3],
    v_earth: [f64; 3],
) -> (f64, f64) {
    let r = s.itrs();
    let obs = mtxv(c2t, [r[0] / DAU, r[1] / DAU, r[2] / DAU]);
    // the site's rotation velocity, ω × r (au/day)
    let w = 7.292_115_146_706_98e-5 * 86_400.0;
    let v_site = mtxv(c2t, [-w * r[1] / DAU, w * r[0] / DAU, 0.0]);
    let d = add(geo, obs, -1.0);
    let tau = sqrt64(dot(d, d)) / C_AU_DAY;
    let d = add(d, v_body, -tau);
    let n = sqrt64(dot(d, d));
    let u = [d[0] / n, d[1] / n, d[2] / n];
    let v = add(v_earth, v_site, 1.0);
    let v = [v[0] / C_AU_DAY, v[1] / C_AU_DAY, v[2] / C_AU_DAY];
    let a = add(add(u, v, 1.0), u, -dot(u, v));
    local(s, mxv(c2t, a))
}

/// The great-circle angle between two (altitude, azimuth) directions.
fn separation(a: (f64, f64), b: (f64, f64)) -> f64 {
    let unit = |(alt, az): (f64, f64)| {
        let (sa, ca) = sin_cos64(alt);
        let (sz, cz) = sin_cos64(az);
        [ca * cz, ca * sz, sa]
    };
    let (u, v) = (unit(a), unit(b));
    let c = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    atan2_64(sqrt64(dot(c, c)), dot(u, v))
}

#[test]
fn sun_and_moon_agree_with_jpl_horizons() {
    let b = blocks();
    let tables: Vec<EopTable> = b.iter().map(|x| EopTable::new(x).unwrap()).collect();
    let mut compared = 0;
    let mut below_horizon = 0;
    let (mut worst_apparent, mut worst_geometric): (f64, f64) = (0.0, 0.0);
    for r in rows("../data/horizons-sky.csv")
        .into_iter()
        .filter(|r| r[0] == "azel")
    {
        let (body, s) = (r[1].as_str(), site(&r[2]));
        let (tt, ut1, xp, yp) = orientation(&r[3], &tables);
        let want = (f64_(&r[5]).to_radians(), f64_(&r[4]).to_radians());
        let tol = common_tol(s.latitude) + model_tol(body);

        // the Law as it is: geometric directions
        let law = sky(tt, ut1, xp, yp, &s);
        let h = if body == "sun" { law.sun } else { law.moon };
        assert!((0.0..core::f64::consts::TAU).contains(&h.azimuth));
        let light_time = if body == "moon" { 0.74 * ARCSEC } else { 0.0 };
        let geometric = separation((h.altitude, h.azimuth), want);
        assert!(
            geometric <= tol + ABERRATION + light_time,
            "{body} {} {}: geometric {}″",
            r[2],
            r[3],
            geometric / ARCSEC
        );

        // with light time and aberration added from the crate's velocities
        let c2t = c2t06a(tt, ut1, xp, yp);
        let e = epv00(tt);
        let v_earth = e.barycentric[1];
        let (geo, v_body) = if body == "sun" {
            let sun = e.sun_geocentric();
            (sun[0], add(e.barycentric[1], e.heliocentric[1], -1.0))
        } else {
            let m = moon98(tt).geocentric;
            (m[0], add(m[1], v_earth, 1.0))
        };
        let app = apparent(&c2t, &s, geo, v_body, v_earth);
        let d = separation(app, want);
        assert!(
            d <= tol,
            "{body} {} {}: apparent {}″ > {}″",
            r[2],
            r[3],
            d / ARCSEC,
            tol / ARCSEC
        );
        if body == "sun" {
            worst_apparent = max64(worst_apparent, d);
            worst_geometric = max64(worst_geometric, geometric);
        }
        below_horizon += usize::from(h.altitude < 0.0);
        compared += 1;
    }
    assert_eq!(compared, 2 * 4 * 18);
    // both sides of the horizon are covered; the geometric Sun shows the
    // aberration it leaves out, and adding it removes nearly all of it
    assert!(
        below_horizon > 20 && below_horizon < compared - 20,
        "{below_horizon}"
    );
    assert!(
        worst_geometric > 20.0 * ARCSEC,
        "{}″",
        worst_geometric / ARCSEC
    );
    assert!(
        worst_apparent < 2.0 * ARCSEC,
        "{}″",
        worst_apparent / ARCSEC
    );
}

/// The phase angle is the Sun–Moon–Earth angle; Horizons' S-T-O is printed
/// to 10⁻⁴° and its illuminated fraction to 10⁻⁵ %. The geometric angle
/// differs from Horizons' by the Moon's direction error (18.3″ model, 0.74″
/// light time) and the aberration of the Sun's (21.16″), so ≤ 40.2″ plus the
/// printed rounding (0.18″); the fraction `(1 + cos i)/2` then moves by at
/// most half that angle in radians. Waxing is Horizons' "/T" (the Moon trails
/// the Sun, east of it).
#[test]
fn moon_phase_agrees_with_jpl_horizons() {
    let b = blocks();
    let tables: Vec<EopTable> = b.iter().map(|x| EopTable::new(x).unwrap()).collect();
    let tol_i = (18.3 + 0.74 + 21.16 + 0.18) * ARCSEC;
    let mut compared = 0;
    let mut waxing = 0;
    for r in rows("../data/horizons-sky.csv")
        .into_iter()
        .filter(|r| r[0] == "phase")
    {
        let (tt, _, _, _) = orientation(&r[3], &tables);
        let p = moon_phase(tt);
        let want_i = f64_(&r[5]).to_radians();
        let want_k = f64_(&r[4]) / 100.0;
        assert!(
            (p.phase_angle - want_i).abs() <= tol_i,
            "{}: i {} vs {}",
            r[3],
            p.phase_angle.to_degrees(),
            r[5]
        );
        assert!(
            (p.illuminated_fraction - want_k).abs() <= tol_i / 2.0 + 5e-8,
            "{}: k {} vs {want_k}",
            r[3],
            p.illuminated_fraction
        );
        assert_eq!(p.waxing, r[6] == "/T", "{}: waxing {}", r[3], p.waxing);
        let (_, ci) = sin_cos64(p.phase_angle);
        assert_eq!(p.illuminated_fraction, (1.0 + ci) / 2.0);
        waxing += usize::from(p.waxing);
        compared += 1;
    }
    assert_eq!(compared, 18);
    assert!(waxing > 3 && waxing < 15, "{waxing}");
}

/// ERFA `t_erfa_c.c`, `t_gd2gc`: WGS84, longitude 3.1 rad, latitude −0.5 rad,
/// height 2500 m, to 10⁻⁷ m (the printed values as the shortest decimals of the
/// same `f64`).
#[test]
fn site_position_matches_the_erfa_test_vector() {
    let p = Site {
        latitude: -0.5,
        longitude: 3.1,
        height: 2500.0,
    }
    .itrs();
    let want = [
        -5_599_000.557_704_994,
        233_011.672_234_792_03,
        -3_040_909.470_698_336,
    ];
    for c in 0..3 {
        assert!((p[c] - want[c]).abs() <= 1e-7, "{c}: {}", p[c]);
    }
}

/// At a pole the horizon has no north: the azimuth is still in `[0, 2π)`; a
/// non-finite date gives non-finite directions instead of a panic.
#[test]
fn degenerate_sites_and_dates() {
    let tt = Jd2 {
        jd1: 2_451_545.0,
        jd2: 0.3,
    };
    let pole = Site {
        latitude: core::f64::consts::FRAC_PI_2,
        longitude: 0.0,
        height: 0.0,
    };
    let s = sky(tt, tt, 0.0, 0.0, &pole);
    for h in [s.sun, s.moon] {
        assert!(h.altitude.is_finite() && (0.0..core::f64::consts::TAU).contains(&h.azimuth));
    }
    // on 2000 January 1 the Sun's declination is −23.0° (Astronomical
    // Almanac): the altitude at the north pole is the declination
    assert!(
        (s.sun.altitude.to_degrees() + 23.0).abs() < 0.1,
        "{}",
        s.sun.altitude.to_degrees()
    );
    let bad = Jd2 {
        jd1: 2_451_545.0,
        jd2: f64::NAN,
    };
    let s = sky(bad, bad, 0.0, 0.0, &site("tokyo"));
    assert!(s.sun.altitude.is_nan() && s.moon.altitude.is_nan());
    assert!(moon_phase(bad).phase_angle.is_nan());
}
