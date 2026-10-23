//! TT ↔ TDB against ERFA eraDtdb (tests/golden/tdb_fb1990.csv) and CSPICE
//! unitim with naif0012.tls (tests/golden/tdb_spice.csv).

mod common;
use alice_det_math::max64;
use alice_ephemeris::tdb::{fb1990_coefficients, FB1990_BLOCKS, FB1990_JPL_ADJUSTMENT};
use alice_ephemeris::{Instant, Tdb, TdbModel, Tt};
use common::{f64_, i64_, rows, ulps};

/// The rounding-error bound of the f64 evaluation of the Fairhead &
/// Bretagnon series at `t` Julian millennia, derived from the arithmetic
/// (u = 2⁻⁵³, the unit roundoff):
///
/// - t = s / 86400 / 365250 from the f64 seconds (within 1 ulp, 2u) and two
///   divisions: relative error ≤ 4u
/// - each argument f·t + p: |f·t|·4u from t, u·|f·t| for the product and
///   u·|f·t + p| for the sum
/// - sin64 is within 2 ulp (alice-det-math's documented bound, Payne–Hanek
///   reduction for large arguments), ≤ 4u absolute; a·sin rounds once (u·|a|)
/// - a block of n terms summed in order adds ≤ n·u·Σ|a|
/// - the block of T^k is multiplied by t^k (k·4u relative from t) and Horner's
///   scheme adds ≤ 8u·Σ_k W_k·|t|^k (W_k = Σ|a| of the block, γ₈ for degree 4)
/// - the JPL adjustment terms the same way, and the final sum rounds once
///
/// This is an upper bound, not a fit: every factor is a worst case.
fn fb1990_rounding_bound(t: f64) -> f64 {
    const U: f64 = 1.0 / 9_007_199_254_740_992.0; // 2^-53
    let at = t.abs();
    let coef = fb1990_coefficients();
    let mut total = 0.0;
    let mut horner = 0.0;
    let mut size = 0.0;
    for k in 0..5 {
        let (lo, hi) = (FB1990_BLOCKS[k], FB1990_BLOCKS[k + 1]);
        let (mut e, mut w) = (0.0, 0.0);
        for &[a, f, p] in &coef[lo..hi] {
            let ft = (f * t).abs();
            let darg = ft * 5.0 * U + (ft + p.abs()) * U;
            e += a.abs() * (darg + 5.0 * U);
            w += a.abs();
        }
        e += (hi - lo) as f64 * U * w;
        let tk = alice_det_math::powf64(at, k as f64);
        total += tk * (e + k as f64 * 4.0 * U * w);
        horner += w * tk;
        size += w * tk;
    }
    total += 8.0 * U * horner;
    let mut ej = 0.0;
    for [a, f, p] in &FB1990_JPL_ADJUSTMENT[..4] {
        let ft = (f * t).abs();
        ej += a.abs() * (ft * 5.0 * U + (ft + p.abs()) * U + 5.0 * U);
        size += a.abs();
    }
    let c = FB1990_JPL_ADJUSTMENT[4][0].abs() * at * at;
    ej += c * (8.0 * U + 2.0 * U) + 5.0 * U * c;
    size += c;
    total + ej + U * size
}

/// The f64 series against the same series evaluated exactly (mpmath, 60
/// digits, exact t), within the derived bound; and against ERFA eraDtdb,
/// whose own libm / fused multiply-add rounding is on its side of the
/// comparison: within twice the bound (ERFA's arithmetic has the same
/// structure, so its own error obeys the same bound).
fn check_fb1990_table(name: &str, sec_col: usize) -> (usize, f64, f64) {
    let mut n = 0;
    let (mut worst_exact, mut worst_erfa) = (0.0f64, 0.0f64);
    for r in rows(name) {
        let sec = i64_(&r[sec_col]);
        let tt = Instant::<Tt>::new(sec, 0).unwrap();
        let got = TdbModel::Fb1990.tdb_minus_tt(tt);
        let (erfa, exact) = (f64_(&r[sec_col + 1]), f64_(&r[sec_col + 2]));
        let t = sec as f64 / 86_400.0 / 365_250.0;
        // the exact value is read back as the nearest f64: + 1 ulp of it
        let bound = fb1990_rounding_bound(t) + exact.abs() * f64::EPSILON;
        let d_exact = (got - exact).abs();
        let d_erfa = (got - erfa).abs();
        assert!(
            d_exact <= bound,
            "{r:?}: |ours - exact| {d_exact:e} > {bound:e}"
        );
        assert!(
            d_erfa <= 2.0 * bound,
            "{r:?}: |ours - ERFA| {d_erfa:e} > {:e}",
            2.0 * bound
        );
        worst_exact = max64(worst_exact, d_exact / bound);
        worst_erfa = max64(worst_erfa, d_erfa / bound);
        n += 1;
    }
    (n, worst_exact, worst_erfa)
}

#[test]
fn fb1990_matches_the_exact_series_and_erfa_dtdb() {
    let (n, we, wr) = check_fb1990_table("tdb_fb1990.csv", 0);
    println!(
        "Fb1990 1600-2200: {n} rows, worst |ours-exact|/bound {we:.3}, |ours-ERFA|/bound {wr:.3}"
    );
    assert!(n > 1000);
}

#[test]
fn spice_et_matches_cspice_unitim() {
    let mut n = 0;
    for r in rows("tdb_spice.csv") {
        let sec = i64_(&r[0]);
        let tt = Instant::<Tt>::new(sec, 0).unwrap();
        // forward: SPICE's f64 result is tt + Δ rounded once; ours is exact
        // tt + Δ (Δ to the attosecond) read back as f64 (within 1 ulp): ≤ 2 ulp apart
        let got = tt.to_tdb(TdbModel::SpiceEt).unwrap().to_seconds_f64();
        let want = f64_(&r[1]);
        assert!(
            ulps(got, want) <= 2 || (got - want).abs() < 1e-18,
            "{r:?}: {got} vs {want}"
        );
        // inverse: SPICE solves TDB → TDT iteratively; same comparison
        let tdb = Instant::<Tdb>::new(sec, 0).unwrap();
        let got = tdb.to_tt(TdbModel::SpiceEt).unwrap().to_seconds_f64();
        let want = f64_(&r[2]);
        assert!(
            ulps(got, want) <= 2 || (got - want).abs() < 1e-18,
            "{r:?}: {got} vs {want}"
        );
        n += 1;
    }
    assert!(n > 400);
}

#[test]
fn spice_and_fb1990_differ_by_tens_of_microseconds() {
    // the measured size of the SpiceEt approximation, 1950–2050 (the range
    // where ERFA states ±3 ns for Fb1990); documented on TdbModel::SpiceEt
    let mut worst = 0.0f64;
    let mut n = 0;
    let mut s = -1_577_880_000i64;
    while s < 1_577_880_000 {
        let tt = Instant::<Tt>::new(s, 0).unwrap();
        let diff = TdbModel::SpiceEt.tdb_minus_tt(tt) - TdbModel::Fb1990.tdb_minus_tt(tt);
        worst = max64(worst, diff.abs());
        s += 86_400 * 3 + 7_777;
        n += 1;
    }
    println!("SpiceEt − Fb1990, 1950–2050: {n} points, largest {worst:e} s");
    assert!(n > 10_000);
    assert!(worst > 1e-6 && worst < 40e-6, "{worst:e}");
}

#[test]
fn tdb_round_trips_within_the_documented_bound() {
    // TT → TDB → TT is exact unless TT is near an f64 rounding boundary, and
    // then within 2 |dΔ/dt| ulp(TT) (src/tdb.rs); ±630 years around J2000
    let mut n = 0;
    let mut exact = 0;
    for model in [TdbModel::SpiceEt, TdbModel::Fb1990] {
        let mut s = -20_000_000_000i64;
        while s < 20_000_000_000 {
            let tt = Instant::<Tt>::new(s, 123_456_789_012_345_678).unwrap();
            let back = tt.to_tdb(model).unwrap().to_tt(model).unwrap();
            let d = (back.total_attoseconds() - tt.total_attoseconds()).abs();
            let x = tt.to_seconds_f64().abs();
            let ulp = f64::from_bits(x.to_bits() + 1) - x;
            let bound = 2.0 * 3.4e-10 * ulp * 1e18 + 1.0;
            assert!(d as f64 <= bound, "{model:?} {s}: {d} as > {bound}");
            exact += usize::from(d <= 1);
            s += 99_999_989;
            n += 1;
        }
    }
    assert!(n > 700);
    println!("round trips: {exact} of {n} within 1 as");
}

#[test]
fn fb1990_far_from_j2000_reaches_one_second_where_erfa_does() {
    // ±200,000 years, far outside 1950–2050 where the series means anything:
    // the f64 value stays within the derived bound of the exact series (which
    // grows as t⁴), and the conversion is OutOfRange exactly where ERFA's
    // |TDB − TT| ≥ 1 s (first near −131,600 and +147,200 years)
    let (n, we, wr) = check_fb1990_table("tdb_fb1990_far.csv", 1);
    println!("Fb1990 far: {n} rows, worst |ours-exact|/bound {we:.3}, |ours-ERFA|/bound {wr:.3}");
    let (mut ok, mut out) = (0, 0);
    for r in rows("tdb_fb1990_far.csv") {
        let tt = Instant::<Tt>::new(i64_(&r[1]), 0).unwrap();
        let erfa = f64_(&r[2]);
        match tt.to_tdb(TdbModel::Fb1990) {
            Ok(_) => {
                assert!(erfa.abs() < 1.0, "{r:?}");
                ok += 1;
            }
            Err(e) => {
                assert_eq!(e, alice_ephemeris::TimeError::OutOfRange);
                assert!(erfa.abs() >= 1.0, "{r:?}");
                out += 1;
            }
        }
    }
    assert!(ok > 50 && out > 20, "{ok} / {out}");
}
