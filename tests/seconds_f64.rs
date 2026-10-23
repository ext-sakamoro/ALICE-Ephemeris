//! `Instant::to_seconds_f64` against the nearest `f64` of the exact value
//! (tests/golden/seconds_f64.csv, exact rationals rounded once by the
//! generator): bit-exact for whole seconds, within 1 ulp otherwise, over the
//! whole `i64` range including |sec| > 2⁵³.

mod common;
use alice_ephemeris::{Instant, Tt};
use common::{f64_, i64_, rows, u64_, ulps};

#[test]
fn whole_seconds_are_exact_and_fractions_within_one_ulp() {
    let (mut whole, mut frac, mut big_negative_whole) = (0, 0, 0);
    for r in rows("seconds_f64.csv") {
        let (sec, atto, want) = (i64_(&r[0]), u64_(&r[1]), f64_(&r[2]));
        let got = Instant::<Tt>::new(sec, atto).unwrap().to_seconds_f64();
        if atto == 0 {
            assert_eq!(got.to_bits(), want.to_bits(), "{r:?}: {got:?}");
            whole += 1;
            big_negative_whole += usize::from(sec < -(1i64 << 53));
        } else {
            assert!(
                got.is_sign_negative() == want.is_sign_negative() || got == 0.0,
                "{r:?}"
            );
            assert!(
                ulps(got.abs(), want.abs()) <= 1,
                "{r:?}: {got:?} vs {want:?}"
            );
            frac += 1;
        }
    }
    assert!(
        whole > 800 && frac > 3000,
        "{whole} whole, {frac} fractional"
    );
    // the double-rounding case: |sec| > 2^53 with no fraction
    assert!(big_negative_whole > 300, "{big_negative_whole}");
}
