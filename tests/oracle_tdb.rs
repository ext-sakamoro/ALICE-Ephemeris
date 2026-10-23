//! TT ↔ TDB against ERFA eraDtdb (tests/golden/tdb_fb1990.csv) and CSPICE
//! unitim with naif0012.tls (tests/golden/tdb_spice.csv).

mod common;
use alice_det_math::max64;
use alice_ephemeris::{Instant, Tdb, TdbModel, Tt};
use common::{f64_, i64_, rows, ulps};

/// eraDtdb uses the platform libm `sin` (and the C compiler may fuse
/// `f·t + p`); this crate uses alice-det-math. The measured largest
/// difference over the table is printed; the bound is 10⁻¹⁵ s (1 fs),
/// six orders below the model's stated ±3 ns.
const FB_TOL_S: f64 = 1e-15;

#[test]
fn fb1990_matches_erfa_dtdb() {
    let mut worst = 0.0f64;
    let mut n = 0;
    for r in rows("tdb_fb1990.csv") {
        let tt = Instant::<Tt>::new(i64_(&r[0]), 0).unwrap();
        let got = TdbModel::Fb1990.tdb_minus_tt(tt);
        let want = f64_(&r[1]);
        let d = (got - want).abs();
        assert!(d <= FB_TOL_S, "{r:?}: got {got:e}, ERFA {want:e}");
        worst = max64(worst, d);
        n += 1;
    }
    println!("Fb1990 vs eraDtdb: {n} rows, largest difference {worst:e} s");
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
