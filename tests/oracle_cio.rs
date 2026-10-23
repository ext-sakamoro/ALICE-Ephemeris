//! Oracle: IAU 2006/2000A precession-nutation, the CIO locator s and the
//! celestial-to-terrestrial matrix (CIO based).
//!
//! - The test vectors of ERFA 2.0.1 (`t_erfa_c.c`: `t_nut00a`, `t_nut06a`,
//!   `t_pfw06`, `t_obl06`, `t_pnm06a`, `t_s06`, `t_c2i06a`, `t_c2t06a`), with
//!   the tolerances ERFA's own test uses.
//! - `tests/golden/cio.csv` (scripts/gen_cio_golden.py, pyerfa 2.0.1.5) over
//!   1800–2200: the same series evaluated with alice-det-math's sine and
//!   cosine here and the platform libm (and fused multiply-adds) in pyerfa.
//!   Each of the 1365 nutation terms then differs by a few ulp of its
//!   argument: below 10⁻¹² of the sum of the amplitudes (1.3 × 10⁻⁴ rad), so
//!   the nutation angles agree within 2 × 10⁻¹⁶ rad and the matrices within
//!   10⁻¹⁵, except where the Earth rotation angle enters (c2t), whose
//!   unreduced value may round differently under a fused multiply-add (4 ulp
//!   of 2π·(2 + 0.00274·|days|)).
//!
//! Measured on this table: |ΔΔψ| ≤ 6.2 × 10⁻¹⁷ rad, |ΔΔε| ≤ 2.8 × 10⁻¹⁷ rad.
//! A change below that (for example the IERS 2003 constant of l′ in place of
//! the MHB2000 one, 2 × 10⁻⁶″, about 7 × 10⁻¹⁷ rad in Δψ) does not show in the
//! nutation; the arguments themselves are therefore compared bit for bit with
//! ERFA's expressions evaluated in IEEE operations (`cio_args.csv`).
//!
//! Author: Moroya Sakamoto

// the ERFA test vectors are copied digit for digit from t_erfa_c.c
#![allow(clippy::excessive_precision)]

mod common;

use alice_ephemeris::frames::{self, Mat3};
use alice_ephemeris::Jd2;
use common::{f64_, rows};

fn jd(jd1: f64, jd2: f64) -> Jd2 {
    Jd2 { jd1, jd2 }
}

fn near(got: f64, want: f64, tol: f64, what: &str) {
    assert!(
        (got - want).abs() <= tol,
        "{what}: {got:e} vs {want:e} (diff {:e})",
        (got - want).abs()
    );
}

fn near_mat(got: &Mat3, want: [f64; 9], tol: f64, what: &str) {
    for i in 0..3 {
        for j in 0..3 {
            near(
                got[i][j],
                want[3 * i + j],
                tol,
                &format!("{what}[{i}][{j}]"),
            );
        }
    }
}

#[test]
fn the_erfa_test_vectors_are_reproduced() {
    let d = jd(2_400_000.5, 53_736.0);
    let (dp, de) = frames::nut00a(d);
    near(dp, -0.963_090_910_711_551_843_1e-5, 1e-13, "nut00a dpsi");
    near(de, 0.406_323_917_400_167_871_0e-4, 1e-13, "nut00a deps");
    let (dp, de) = frames::nut06a(d);
    near(dp, -0.963_091_202_582_030_879_7e-5, 1e-13, "nut06a dpsi");
    near(de, 0.406_323_849_688_724_979_8e-4, 1e-13, "nut06a deps");
    let f = frames::pfw06(jd(2_400_000.5, 50_123.999_9));
    near(f.gamb, -0.224_338_767_099_799_569_0e-5, 1e-16, "gamb");
    near(f.phib, 0.409_101_460_239_131_280_8, 1e-12, "phib");
    near(f.psib, -0.950_195_417_801_303_189_5e-3, 1e-14, "psib");
    near(f.epsa, 0.409_101_431_658_736_749_1, 1e-12, "epsa");
    near(
        frames::obl06(jd(2_400_000.5, 54_388.0)),
        0.409_074_922_938_725_820_4,
        1e-14,
        "obl06",
    );
    let pnm = frames::pnm06a(jd(2_400_000.5, 50_123.999_9));
    let tol = [
        1e-12, 1e-14, 1e-14, 1e-14, 1e-12, 1e-14, 1e-14, 1e-14, 1e-12,
    ];
    let want = [
        0.999_999_583_279_420_548_4,
        0.837_238_277_263_096_211_1e-3,
        0.363_968_477_114_062_309_9e-3,
        -0.837_253_374_474_368_360_5e-3,
        0.999_999_648_649_286_164_6,
        0.413_290_594_461_101_949_8e-4,
        -0.363_933_746_962_946_496_9e-3,
        -0.416_337_760_591_066_399_9e-4,
        0.999_999_932_909_426_005_7,
    ];
    for k in 0..9 {
        near(pnm[k / 3][k % 3], want[k], tol[k], &format!("pnm06a {k}"));
    }
    let s = frames::s06(
        d,
        0.579_130_848_670_601_100_0e-3,
        0.402_057_981_673_296_121_9e-4,
    );
    near(s, -0.122_003_221_307_646_311_7e-7, 1e-18, "s06");
    let c2i = frames::c2i06a(d);
    near_mat(
        &c2i,
        [
            0.999_999_832_303_715_937_9,
            0.558_112_132_958_761_378_7e-9,
            -0.579_130_848_774_052_974_9e-3,
            -0.238_425_316_945_230_658_1e-7,
            0.999_999_999_191_746_782_7,
            -0.402_057_939_289_568_255_8e-4,
            0.579_130_848_283_529_261_7e-3,
            0.402_058_009_945_402_031_0e-4,
            0.999_999_831_495_462_869_5,
        ],
        1e-12,
        "c2i06a",
    );
    let c2t = frames::c2t06a(d, d, 2.550_602_38e-7, 1.860_359_247e-6);
    near_mat(
        &c2t,
        [
            -0.181_033_212_830_589_728_2,
            0.983_476_980_693_859_229_6,
            0.655_555_096_299_843_650_5e-4,
            -0.983_476_813_413_621_489_7,
            -0.181_033_220_364_913_083_2,
            0.574_980_084_490_559_411_0e-3,
            0.577_347_402_474_854_587_8e-3,
            0.396_181_682_963_269_058_1e-4,
            0.999_999_832_550_174_778_5,
        ],
        1e-12,
        "c2t06a",
    );
}

#[test]
fn the_series_match_erfa_from_1800_to_2200() {
    let mut compared = 0;
    for r in rows("cio.csv") {
        let v: Vec<f64> = r.iter().map(|s| f64_(s)).collect();
        let d = jd(v[0], v[1]);
        let ctx = format!("{d:?}");
        let (dp, de) = frames::nut00a(d);
        near(dp, v[2], 2e-16, &format!("nut00a dpsi {ctx}"));
        near(de, v[3], 2e-16, &format!("nut00a deps {ctx}"));
        let (dp, de) = frames::nut06a(d);
        near(dp, v[4], 2e-16, &format!("nut06a dpsi {ctx}"));
        near(de, v[5], 2e-16, &format!("nut06a deps {ctx}"));
        let f = frames::pfw06(d);
        for (got, want, name) in [
            (f.gamb, v[6], "gamb"),
            (f.phib, v[7], "phib"),
            (f.psib, v[8], "psib"),
            (f.epsa, v[9], "epsa"),
        ] {
            near(
                got,
                want,
                4.0 * f64::EPSILON * alice_det_math::max64(want.abs(), 1e-3),
                &format!("{name} {ctx}"),
            );
        }
        let pnm = frames::pnm06a(d);
        near_mat(
            &pnm,
            v[11..20].try_into().unwrap(),
            1e-15,
            &format!("pnm06a {ctx}"),
        );
        let (x, y) = frames::bpn2xy(&pnm);
        near(frames::s06(d, x, y), v[10], 1e-18, &format!("s06 {ctx}"));
        near_mat(
            &frames::c2i06a(d),
            v[20..29].try_into().unwrap(),
            1e-15,
            &format!("c2i06a {ctx}"),
        );
        let ut1 = jd(v[0], v[29]);
        let days = (v[0] - 2_451_545.0 + v[29]).abs();
        let era_tol = 4.0 * f64::EPSILON * core::f64::consts::TAU * (2.0 + 0.002_74 * days) + 1e-15;
        let c2t = frames::c2t06a(d, ut1, v[30], v[31]);
        near_mat(
            &c2t,
            v[32..41].try_into().unwrap(),
            era_tol,
            &format!("c2t06a {ctx}"),
        );
        compared += 1;
    }
    assert_eq!(compared, 696);
}

#[test]
fn the_cio_matrix_is_a_rotation_and_carries_the_cip() {
    // a rotation: rows orthonormal; the CIP (X, Y) of pnm06a is the third row
    // of c2i06a (to the order of s·X)
    for days in [-50_000.0, -3_000.0, 0.0, 9_131.25, 60_000.0] {
        let d = jd(2_451_545.0, days);
        let c2i = frames::c2i06a(d);
        for i in 0..3 {
            for j in 0..3 {
                let dot: f64 = (0..3).map(|k| c2i[i][k] * c2i[j][k]).sum();
                let want = if i == j { 1.0 } else { 0.0 };
                assert!((dot - want).abs() <= 4e-16, "{days} {i}{j}: {dot:e}");
            }
        }
        let (x, y) = frames::bpn2xy(&frames::pnm06a(d));
        assert!(
            (c2i[2][0] - x).abs() <= 1e-15 && (c2i[2][1] - y).abs() <= 1e-15,
            "{days}"
        );
    }
}

#[test]
fn degenerate_input_gives_nan_not_a_panic() {
    let nan = jd(f64::NAN, 0.0);
    assert!(frames::nut00a(nan).0.is_nan());
    assert!(frames::c2i06a(nan)[0][0].is_nan());
    let inf = jd(f64::INFINITY, 0.0);
    assert!(frames::nut06a(inf).1.is_nan());
    // |X| or |Y| ≥ 1 has no CIO rotation: NaN, not a panic
    assert!(frames::c2ixys(1.0, 0.5, 0.0)[0][0].is_nan());
    // X = Y = 0 is the pole itself: the rotation is about z by −s only
    let m = frames::c2ixys(0.0, 0.0, 1e-8);
    assert!((m[0][1] + 1e-8).abs() < 1e-20 && m[2][2] == 1.0, "{m:?}");
}

/// The arguments of the series and the precession polynomials use no
/// transcendental, so they must equal ERFA's expressions evaluated one IEEE
/// operation at a time (`tests/golden/cio_args.csv`) bit for bit. This pins
/// every coefficient of them, including those whose effect on the nutation
/// is below the rounding of the series (the MHB2000 l′ constant differs from
/// the IERS 2003 one by 2 × 10⁻⁶″).
#[test]
fn the_arguments_and_precession_polynomials_match_erfa_bit_for_bit() {
    let mut compared = 0;
    for r in rows("cio_args.csv") {
        let v: Vec<f64> = r.iter().map(|s| f64_(s)).collect();
        let d = jd(v[0], v[1]);
        let (ls, pl) = frames::nut00a_arguments(d);
        let sa = frames::s06_arguments(d);
        let f = frames::pfw06(d);
        let got: Vec<f64> = ls
            .iter()
            .chain(&pl)
            .chain(&sa)
            .copied()
            .chain([f.gamb, f.phib, f.psib, f.epsa])
            .collect();
        assert_eq!(got.len(), 30);
        for (k, (g, w)) in got.iter().zip(&v[2..]).enumerate() {
            assert_eq!(g.to_bits(), w.to_bits(), "{d:?} column {k}: {g:e} vs {w:e}");
        }
        assert_eq!(frames::obl06(d).to_bits(), v[31].to_bits(), "obl06 {d:?}");
        compared += 1;
    }
    assert_eq!(compared, 696);
}
