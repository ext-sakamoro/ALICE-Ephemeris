//! TT → TDB → TT returns within 2 |dΔ/dt| ulp(TT) (+1 as) whenever the conversion succeeds (it
//! returns OutOfRange once |TDB − TT| ≥ 1 s, and for Fb1990 outside its domain of
//! ±110,000 years, where the shift is NaN), and from_jd2 never panics on any pair of f64.
#![no_main]
use alice_ephemeris::tdb::FB1990_DOMAIN_MILLENNIA;
use alice_ephemeris::{Instant, Jd2, TdbModel, Tt};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|d: (i64, u64, bool, f64, f64)| {
    let (sec, atto, fb, a, b) = d;
    let model = if fb { TdbModel::Fb1990 } else { TdbModel::SpiceEt };
    if let Ok(tt) = Instant::<Tt>::new(sec, atto % 1_000_000_000_000_000_000) {
        let delta = model.tdb_minus_tt(tt);
        let t = tt.to_seconds_f64() / 86_400.0 / 365_250.0;
        let inside = !fb || t.abs() <= FB1990_DOMAIN_MILLENNIA;
        assert_eq!(delta.is_finite(), inside, "{tt:?}: {delta}");
        if !inside {
            assert!(tt.to_tdb(model).is_err(), "{tt:?}");
        }
        if let Ok(tdb) = tt.to_tdb(model) {
            assert!(delta.abs() < 1.0);
            if let Ok(back) = tdb.to_tt(model) {
                let d = back.total_attoseconds() - tt.total_attoseconds();
                let x = tt.to_seconds_f64().abs();
                let ulp = f64::from_bits(x.to_bits() + 1) - x;
                // |dΔ/dt| ≤ 3.4e-10 for SpiceEt; Fb1990's polynomial terms make
                // it larger away from J2000, bounded here while |Δ| < 1 s
                let bound = 2.0 * if fb { 1e-6 } else { 3.4e-10 } * ulp * 1e18 + 1.0;
                assert!((d.abs() as f64) <= bound, "{tt:?}: {d} as > {bound}");
            }
        }
        let _ = Instant::<Tt>::from_jd2(tt.to_jd2());
    }
    let _ = Instant::<Tt>::from_jd2(Jd2 { jd1: a, jd2: b });
});
