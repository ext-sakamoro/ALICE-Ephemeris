//! Oracle: Earth rotation angle, GMST (1982), polar motion and TEME → ITRS.
//!
//! - `tests/golden/frames.csv` (scripts/gen_frames_golden.py):
//!   `era00`, `gmst82` and `sp00` of two-part Julian dates. The bit reference
//!   is ERFA 2.0.1's expressions evaluated one IEEE operation at a time in
//!   Python; pyerfa 2.0.1.5 itself is compared within the rounding of the
//!   unreduced angle, because its arm64 build fuses a multiply and an add.
//!   `pom00` matrices and TEME → ITRS positions (`pom00 · Rz(gmst82) · r`),
//!   which go through sine and cosine (alice-det-math here, the platform libm
//!   in ERFA), match within a few ulp.
//! - The Earth rotation angle of exact instants against its definition
//!   evaluated at 60 digits (mpmath), within the rounding of the two-part
//!   Julian date (≤ 1 ulp of the day fraction, 2π·1.0027 × 1.1 × 10⁻¹⁶ rad)
//!   plus that of the formula.
//! - The worked example of Vallado, Crawford, Hujsak & Kelso, "Revisiting
//!   Spacetrack Report #3" (AIAA 2006-6753), TEME → ITRF of a position and a
//!   velocity on 2004-04-06, to the printed digits.
//!
//! Author: Moroya Sakamoto

mod common;

use alice_ephemeris::frames::{self, Mat3, TemeToItrs};
use alice_ephemeris::{Instant, Jd2, Tt, Ut1};
use common::{f64_, i64_, rows, u64_};

fn jd(a: &str, b: &str) -> Jd2 {
    Jd2 {
        jd1: f64_(a),
        jd2: f64_(b),
    }
}

#[test]
fn era_gmst_and_s_prime_match_the_erfa_expressions_bit_for_bit() {
    let mut compared = 0;
    for r in rows("frames.csv").iter().filter(|r| r[0] == "jd") {
        let d = jd(&r[1], &r[2]);
        let ctx = format!("{d:?}");
        let (era, gmst, sp) = (frames::era00(d), frames::gmst82(d), frames::sp00(d));
        assert_eq!(era.to_bits(), f64_(&r[3]).to_bits(), "era {ctx}");
        assert_eq!(gmst.to_bits(), f64_(&r[4]).to_bits(), "gmst {ctx}");
        assert_eq!(sp.to_bits(), f64_(&r[5]).to_bits(), "sp {ctx}");
        // pyerfa: a fused multiply-add changes the unreduced angle by ≤ 1 ulp
        // per operation; bound 4 ulp of its largest magnitude
        let t = (d.jd1 - 2_451_545.0 + d.jd2).abs();
        let era_mag = core::f64::consts::TAU * (2.0 + 0.002_74 * t);
        let gmst_mag = 7.272_205_216_643_04e-5 * (1.3e5 + 236.56 * t);
        let near = |a: f64, b: f64, mag: f64| {
            let e = (a - b).abs();
            alice_det_math::min64(e, core::f64::consts::TAU - e) <= 4.0 * f64::EPSILON * mag
        };
        assert!(near(era, f64_(&r[6]), era_mag), "pyerfa era {ctx}");
        assert!(near(gmst, f64_(&r[7]), gmst_mag), "pyerfa gmst {ctx}");
        assert!(
            (sp - f64_(&r[8])).abs() <= 4.0 * f64::EPSILON * sp.abs(),
            "pyerfa sp {ctx}"
        );
        compared += 1;
    }
    assert_eq!(compared, 935);
}

#[test]
fn the_earth_rotation_angle_of_an_instant_matches_its_definition() {
    let mut compared = 0;
    let mut worst: f64 = 0.0;
    for r in rows("frames.csv").iter().filter(|r| r[0] == "era_exact") {
        let ut1 = Instant::<Ut1>::new(i64_(&r[1]), u64_(&r[2])).unwrap();
        let got = ut1.earth_rotation_angle();
        let want = f64_(&r[3]);
        // the angle wraps at 2π: compare on the circle
        let mut d = (got - want).abs();
        if d > core::f64::consts::PI {
            d = core::f64::consts::TAU - d;
        }
        worst = alice_det_math::max64(worst, d);
        // ERFA's form x = f + 0.779 + 0.00274·t (cycles, |x| ≤ 2 + 0.00274|t|)
        // rounds the product, two sums and the product by 2π, each ≤ u|·|;
        // the day fraction of the two-part date is within 1 ulp (u)
        let t = (ut1.seconds() as f64 / 86_400.0).abs();
        let x = 2.0 + 0.002_74 * t;
        let u = f64::EPSILON / 2.0;
        let bound = core::f64::consts::TAU * u * (0.002_74 * t + 4.0 * x + 2.0);
        assert!(d <= bound, "{ut1:?}: {d:e} rad > {bound:e}");
        compared += 1;
    }
    assert_eq!(compared, 824);
    assert!(worst > 0.0, "the comparison saw no rounding at all");
}

fn mat(r: &[String], from: usize) -> Mat3 {
    let v: Vec<f64> = r[from..from + 9].iter().map(|s| f64_(s)).collect();
    [[v[0], v[1], v[2]], [v[3], v[4], v[5]], [v[6], v[7], v[8]]]
}

#[test]
fn the_polar_motion_matrix_matches_erfa() {
    let mut compared = 0;
    for r in rows("frames.csv").iter().filter(|r| r[0] == "pom") {
        let got = frames::pom00(f64_(&r[1]), f64_(&r[2]), f64_(&r[3]));
        let want = mat(r, 4);
        for i in 0..3 {
            for j in 0..3 {
                assert!((got[i][j] - want[i][j]).abs() <= 4.5e-16, "{i}{j}: {got:?}");
                compared += 1;
            }
        }
    }
    assert_eq!(compared, 27);
}

#[test]
fn teme_positions_rotate_to_itrs_as_erfa_composes_them() {
    let mut compared = 0;
    for r in rows("frames.csv").iter().filter(|r| r[0] == "teme") {
        let rot = TemeToItrs::new(jd(&r[1], &r[2]), jd(&r[1], &r[3]), f64_(&r[4]), f64_(&r[5]));
        let p = [f64_(&r[6]), f64_(&r[7]), f64_(&r[8])];
        let got = rot.position(p);
        for c in 0..3 {
            // |r| ≤ 1e4 km, a few ulp of the matrix: 1e-11 km
            assert!((got[c] - f64_(&r[9 + c])).abs() <= 1e-11, "{c}: {got:?}");
            compared += 1;
        }
        // the inverse returns the TEME vector
        let back = rot.position_to_teme(got);
        for c in 0..3 {
            assert!((back[c] - p[c]).abs() <= 1e-11, "{c}: {back:?}");
        }
    }
    assert_eq!(compared, 120);
}

/// AIAA 2006-6753 §"TEME", 2004-04-06 07:51:28.386009 UTC, ΔUT1 = −0.4399619 s,
/// ΔAT = 32 s, xp = −0.140682″, yp = 0.333309″, LOD = 1.5563 ms.
///
/// The published state is printed to 10⁻⁷ km and 10⁻⁹ km/s, but its GMST
/// comes from a one-part Julian date of UT1 (ulp 2⁻³¹ day ≈ 40 µs at
/// JD 2.45 × 10⁶, rounding ≤ 20 µs = 1.5 × 10⁻⁹ rad) and a sidereal-seconds polynomial in
/// `(876600 h + B)·t` (≈ 1.3 × 10⁸ s at t = 0.04, ulp 1.5 × 10⁻⁸ s, another
/// 1.1 × 10⁻¹² rad): ≤ 2 × 10⁻⁹ rad × |r_xy| ≈ 8 000 km = 1.6 × 10⁻⁵ km, and
/// × |v| ≈ 7.4 km/s = 1.5 × 10⁻⁸ km/s. The bounds are those.
#[test]
fn the_vallado_2006_example_rotates_to_the_published_itrf_state() {
    // seconds of the UTC label since 2000-01-01 12:00
    let days = 1557; // 2000-01-01 .. 2004-04-06
    let label = days * 86_400 - 43_200 + 7 * 3600 + 51 * 60 + 28;
    // + 0.386009 s, then UT1 = UTC − 0.4399619 s, TT = UTC + 32 + 32.184 s
    let ut1 = Instant::<Ut1>::new(label - 1, 946_047_100_000_000_000).unwrap();
    let tt = Instant::<Tt>::new(label + 64, 570_009_000_000_000_000).unwrap();
    let as2r = core::f64::consts::PI / 648_000.0;
    let rot = TemeToItrs::new(
        ut1.to_jd2(),
        tt.to_jd2(),
        -0.140_682 * as2r,
        0.333_309 * as2r,
    );
    let (r, v) = rot.state(
        [5_094.180_162_10, 6_127.644_659_50, 6_380.344_532_70],
        [-4.746_131_487, 0.785_818_041, 5.531_931_288],
        0.001_556_3,
    );
    let r_want = [-1_033.479_383_0, 7_901.295_275_4, 6_380.356_595_8];
    let v_want = [-3.225_636_520, -2.872_451_450, 5.531_924_446];
    for c in 0..3 {
        assert!((r[c] - r_want[c]).abs() <= 1.6e-5, "r{c}: {r:?}");
        assert!((v[c] - v_want[c]).abs() <= 1.5e-8, "v{c}: {v:?}");
    }
    // the velocity rotates back
    let (r2, v2) = rot.state_to_teme(r, v, 0.001_556_3);
    assert!((r2[0] - 5_094.180_162_10).abs() < 1e-9 && (v2[2] - 5.531_931_288).abs() < 1e-12);
}

#[test]
fn degenerate_input_gives_nan_not_a_panic() {
    let nan = Jd2 {
        jd1: f64::NAN,
        jd2: 0.0,
    };
    assert!(frames::era00(nan).is_nan());
    assert!(frames::gmst82(nan).is_nan());
    let inf = Jd2 {
        jd1: f64::INFINITY,
        jd2: 0.0,
    };
    assert!(frames::era00(inf).is_nan());
    let rot = TemeToItrs::new(nan, nan, 0.0, 0.0);
    assert!(rot.position([1.0, 0.0, 0.0])[0].is_nan());
    // extreme instants still give an angle in [0, 2π)
    for s in [i64::MIN, i64::MAX, 0] {
        let a = Instant::<Ut1>::new(s, 0).unwrap().earth_rotation_angle();
        assert!((0.0..core::f64::consts::TAU).contains(&a), "{s}: {a}");
    }
}

/// The excess length of day slows the rotation: the ITRS velocity changes by
/// `(ω·LOD/86400 s) ẑ × r` (closed form), here with no polar motion.
#[test]
fn the_length_of_day_slows_the_earth_rotation_term() {
    let d = Jd2 {
        jd1: 2_451_545.0,
        jd2: 0.3,
    };
    let rot = TemeToItrs::new(d, d, 0.0, 0.0);
    let r = [7000.0, -1200.0, 300.0];
    let v = [1.0, 7.0, 0.5];
    let lod = 0.002;
    let (rp, v0) = rot.state(r, v, 0.0);
    let (_, v1) = rot.state(r, v, lod);
    let dw = frames::EARTH_ROTATION_RATE * lod / 86_400.0;
    // ẑ × r = (−y, x, 0), up to the s′ rotation (|s′| < 1e-11 rad)
    let want = [-dw * rp[1], dw * rp[0], 0.0];
    for c in 0..3 {
        assert!(
            ((v1[c] - v0[c]) - want[c]).abs() <= 1e-15,
            "{c}: {:e}",
            v1[c] - v0[c]
        );
    }
    assert!((v1[0] - v0[0]).abs() > 1e-9, "the change is visible");
}
