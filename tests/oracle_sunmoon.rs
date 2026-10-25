//! Oracle: the Earth's position and velocity (ERFA `eraEpv00`) and the Moon's
//! (ERFA `eraMoon98`).
//!
//! - `tests/golden/sunmoon.csv` (scripts/gen_sunmoon_golden.py): ERFA 2.0.1's
//!   expressions evaluated one IEEE binary64 operation at a time with
//!   correctly rounded sines and cosines (the same table on every machine;
//!   the generator checks pyerfa against it within derived bounds). The crate
//!   uses alice-det-math's sines and cosines (< 1 ulp, not correctly rounded),
//!   so each row carries the bound, derived from the arithmetic, on the
//!   difference that 1-ulp sines and cosines can make.
//! - Independent of ERFA: Meeus, *Astronomical Algorithms* (2nd ed., 1998),
//!   Example 47.a (the Moon on 1992 April 12, 0h TD) and Example 25.b (the Sun
//!   on 1992 October 13, 0h TD, from the full VSOP87), to the printed digits
//!   and the stated accuracy of the simplified Earth model.
//! - The velocities against central differences of the positions.
//!
//! Author: Moroya Sakamoto

mod common;

use alice_det_math::{atan2_64, max64, sqrt64};
use alice_ephemeris::frames::{mtxv, pfw06, rx, rz, Mat3, IDENTITY};
use alice_ephemeris::sunmoon::{epv00, moon98};
use alice_ephemeris::Jd2;
use common::{f64_, rows};

fn jd(jd1: f64, jd2: f64) -> Jd2 {
    Jd2 { jd1, jd2 }
}

#[test]
fn earth_and_moon_match_the_erfa_expressions_within_the_derived_bounds() {
    let mut compared = 0;
    let mut differed = 0;
    let mut worst: f64 = 0.0;
    for r in rows("sunmoon.csv") {
        let d = jd(f64_(&r[0]), f64_(&r[1]));
        let want: Vec<f64> = r[2..20].iter().map(|s| f64_(s)).collect();
        let bound: Vec<f64> = r[20..26].iter().map(|s| f64_(s)).collect();
        let e = epv00(d);
        let m = moon98(d).geocentric;
        let got = [
            e.heliocentric[0],
            e.heliocentric[1],
            e.barycentric[0],
            e.barycentric[1],
            m[0],
            m[1],
        ];
        for (v, (vec, b)) in got.iter().zip(bound.iter()).enumerate() {
            assert!(*b > 0.0 && *b < 1e-11, "{d:?}: bound {b:e}");
            for c in 0..3 {
                let diff = (vec[c] - want[3 * v + c]).abs();
                assert!(
                    diff <= *b,
                    "{d:?} vector {v} component {c}: {diff:e} > {b:e}"
                );
                worst = max64(worst, diff / b);
                differed += usize::from(diff > 0.0);
                compared += 1;
            }
        }
    }
    assert_eq!(compared, 431 * 18);
    // musl's sines and cosines round some arguments differently from the
    // correctly rounded reference: a comparison that never saw one tested
    // nothing about the bound
    assert!(differed > 0, "no row differed from the reference at all");
    assert!(worst > 0.0);
}

/// The rotation from the mean ecliptic and equinox of date to the GCRS that
/// `moon98` applies (IAU 2006 Fukushima-Williams angles).
fn ecliptic_to_gcrs(tt: Jd2) -> Mat3 {
    let fw = pfw06(tt);
    let mut rm = IDENTITY;
    rz(fw.psib, &mut rm);
    rx(-fw.phib, &mut rm);
    rz(-fw.gamb, &mut rm);
    rm
}

/// Longitude (deg, `[0, 360)`), latitude (deg) and distance of a vector.
fn spherical(p: [f64; 3]) -> (f64, f64, f64) {
    let rxy = sqrt64(p[0] * p[0] + p[1] * p[1]);
    let r = sqrt64(rxy * rxy + p[2] * p[2]);
    let lon = atan2_64(p[1], p[0]).to_degrees().rem_euclid(360.0);
    let lat = atan2_64(p[2], rxy).to_degrees();
    (lon, lat, r)
}

const AU_KM: f64 = 149_597_870.7;

/// Meeus Example 47.a: 1992 April 12, 0h TD (JDE 2448724.5): λ = 133.162655°,
/// β = −3.229126°, Δ = 368409.7 km (mean ecliptic and equinox of date, before
/// nutation). eraMoon98 is the same series except that it omits the light-time
/// correction folded into Meeus's constant of the mean longitude (ERFA's note
/// 2): its L′₀ = 218.31665436° (Simon et al. 1994) against Meeus's 218.3164477°,
/// so its longitude is larger by exactly that 0.00020666° (0.744″, the Moon's
/// motion in 1.28 s); the other four arguments have Meeus's constants.
#[test]
fn moon_reproduces_meeus_example_47a() {
    let d = jd(2_448_724.5, 0.0);
    let p = mtxv(&ecliptic_to_gcrs(d), moon98(d).geocentric[0]);
    let (lon, lat, r) = spherical(p);
    let light_time = 218.316_654_36 - 218.316_447_7;
    assert!((lon - light_time - 133.162_655).abs() <= 1e-6, "λ {lon}");
    assert!((lat - -3.229_126).abs() <= 1e-6, "β {lat}");
    assert!((r * AU_KM - 368_409.7).abs() <= 0.05, "Δ {} km", r * AU_KM);
}

/// JPL Horizons (DE441, `tests/data/horizons-sunmoon.csv`, written by
/// scripts/fetch_horizons_sunmoon.py): geometric geocentric states of the Sun
/// and the Moon every five years 1900–2100. ERFA's comparisons bound the
/// models: the Earth within 11.2 km and 5.0 mm/s of DE405 over 1900–2100, the
/// Moon within 31.7 km and 172 mm/s of ELP/MPP02 over 1950–2100 (the Moon's
/// rows before 1950 are held to the same bound, which they meet).
#[test]
fn sun_and_moon_agree_with_jpl_horizons_within_the_stated_accuracy() {
    let km = 1.0 / AU_KM;
    let mm_s = 1e-6 * km * 86_400.0;
    let mut compared = 0;
    let (mut worst_sun, mut worst_moon): (f64, f64) = (0.0, 0.0);
    for r in rows("../data/horizons-sunmoon.csv") {
        let d = jd(f64_(&r[1]), 0.0);
        let want: Vec<f64> = r[2..8].iter().map(|s| f64_(s)).collect();
        let (got, bound_p, bound_v) = match r[0].as_str() {
            "sun" => (epv00(d).sun_geocentric(), 11.2 * km, 5.0 * mm_s),
            "moon" => (moon98(d).geocentric, 31.7 * km, 172.0 * mm_s),
            other => panic!("unknown body {other}"),
        };
        let dist = |a: [f64; 3], b: &[f64]| {
            let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
            sqrt64(d[0] * d[0] + d[1] * d[1] + d[2] * d[2])
        };
        let (dp, dv) = (dist(got[0], &want[0..3]), dist(got[1], &want[3..6]));
        assert!(dp <= bound_p, "{} {d:?}: {} km", r[0], dp / km);
        assert!(dv <= bound_v, "{} {d:?}: {} mm/s", r[0], dv / mm_s);
        if r[0] == "sun" {
            worst_sun = max64(worst_sun, dp / km);
        } else {
            worst_moon = max64(worst_moon, dp / km);
        }
        compared += 1;
    }
    assert_eq!(compared, 88);
    // a comparison against a copy of the model itself would read 0
    assert!(
        worst_sun > 0.1 && worst_moon > 0.1,
        "{worst_sun} {worst_moon}"
    );
}

/// The velocities are the derivatives of the positions: the five-point
/// difference `(−f(2h) + 8f(h) − 8f(−h) + f(−2h)) / 12h` with h = 0.01 day has
/// truncation h⁴/30 · |f⁽⁵⁾| (≤ 10⁻¹⁸ au/day for the Earth, ≤ 10⁻¹⁵ for the
/// Moon, whose angular rate is 0.23 rad/day at 0.0027 au) and rounding
/// 18/12h times the noise of one position. For the Earth that noise is the
/// rounding of the phase `b + c·T` of the unit-amplitude term (½ ulp of
/// ≈ 620 rad at |T| = 100 yr, 5.7 × 10⁻¹⁴ au), so 10⁻¹¹ au/day; the Moon's
/// arguments are reduced modulo 360° first, so its noise is that of its
/// 0.0027 au position.
/// eraMoon98 takes the rate of Meeus's argument A₁ = 119.75° + 131.849°·T as
/// `ERFA_DD2R * al1` (0.003958, the amplitude of its additive term) instead of
/// 131.849°/century, and the crate reproduces that. The three additive terms
/// in A₁ (al1 = 0.003958°, ab3 = ab4 = 0.000175°) then carry a velocity off
/// by at most r · DD2R·(al1 + ab3 + ab4) · DD2R·(131.849 − 0.003958) / 36525
/// with r ≤ 0.00272 au: 1.29 × 10⁻¹¹ au/day (2.2 mm/s); the five-point
/// difference adds less than 10⁻¹⁵.
const MOON_A1_RATE: f64 = 1.3e-11;

#[test]
fn velocities_are_the_derivatives_of_the_positions() {
    let h = 0.01;
    let five_point = |f: &dyn Fn(f64) -> [f64; 3], day: f64, c: usize| {
        (-f(day + 2.0 * h)[c] + 8.0 * f(day + h)[c] - 8.0 * f(day - h)[c] + f(day - 2.0 * h)[c])
            / (12.0 * h)
    };
    let helio = |day: f64| epv00(jd(2_451_545.0, day)).heliocentric[0];
    let bary = |day: f64| epv00(jd(2_451_545.0, day)).barycentric[0];
    // moon98 differentiates the Meeus series in the ecliptic of date and
    // neglects the spin of that precessing frame (ERFA's note 7, ≤ 3 mm/s), so
    // its velocity is the derivative of the position in the ecliptic of date,
    // rotated with the matrix of the epoch
    let moon = |day: f64| {
        let d = jd(2_451_545.0, day);
        mtxv(&ecliptic_to_gcrs(d), moon98(d).geocentric[0])
    };
    let mut compared = 0;
    for k in 0..40 {
        let day = -36_000.0 + 1_801.3 * f64::from(k);
        let d = jd(2_451_545.0, day);
        let (e, m) = (epv00(d), moon98(d).geocentric);
        for c in 0..3 {
            let dh = five_point(&helio, day, c) - e.heliocentric[1][c];
            let db = five_point(&bary, day, c) - e.barycentric[1][c];
            let dm = five_point(&moon, day, c) - mtxv(&ecliptic_to_gcrs(d), m[1])[c];
            assert!(dh.abs() <= 1e-11, "earth {d:?} {c}: {dh:e}");
            assert!(db.abs() <= 1e-11, "bary {d:?} {c}: {db:e}");
            assert!(dm.abs() <= MOON_A1_RATE, "moon {d:?} {c}: {dm:e}");
            compared += 3;
        }
    }
    assert_eq!(compared, 360);
}

/// `in_range` is ERFA's status: |T| ≤ 100 Julian years from J2000.0.
#[test]
fn the_range_flag_is_erfas_status() {
    assert!(epv00(jd(2_451_545.0, 36_525.0)).in_range);
    assert!(epv00(jd(2_451_545.0, -36_525.0)).in_range);
    assert!(!epv00(jd(2_451_545.0, 36_525.1)).in_range);
    assert!(!epv00(jd(2_451_545.0, -36_525.1)).in_range);
}

/// The Moon's `in_range` is the span of ERFA's quoted accuracy for `eraMoon98`
/// (its note 3: compared with ELP/MPP02 over 1950–2100, T from −0.5 to +1.0
/// Julian centuries from J2000.0); ERFA's routine has no status, so the flag is
/// what tells a caller the date is outside it
#[test]
fn the_moon_flags_dates_outside_its_quoted_span() {
    // 36 525 days per Julian century: 1950.0 is −18 262.5 d, 2100.0 is +36 525 d
    assert!(moon98(jd(2_451_545.0, 0.0)).in_range);
    assert!(moon98(jd(2_451_545.0, -18_262.5)).in_range);
    assert!(moon98(jd(2_451_545.0, 27_393.75)).in_range); // 2075
    assert!(moon98(jd(2_451_545.0, 36_525.0)).in_range);
    assert!(!moon98(jd(2_451_545.0, -18_262.6)).in_range);
    assert!(!moon98(jd(2_451_545.0, 36_525.1)).in_range);
    assert!(!moon98(jd(2_451_545.0, 73_050.0)).in_range);
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(!moon98(jd(2_451_545.0, bad)).in_range);
    }
}

/// A non-finite date gives non-finite vectors (no panic, no finite garbage)
/// and the Earth flags it out of range.
#[test]
fn a_non_finite_date_gives_non_finite_vectors() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let d = jd(2_451_545.0, bad);
        let e = epv00(d);
        assert!(!e.in_range);
        assert!(e.heliocentric.iter().flatten().all(|v| !v.is_finite()));
        assert!(moon98(d)
            .geocentric
            .iter()
            .flatten()
            .all(|v| !v.is_finite()));
    }
}
