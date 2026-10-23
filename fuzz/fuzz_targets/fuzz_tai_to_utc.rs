//! Any TAI instant either errors or has a UTC label that converts back to it
//! (within 1 as), and later instants never get earlier labels.
#![no_main]
use alice_ephemeris::{ExpiryPolicy, Instant, Tai, UtcDateTime};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|d: (i64, u64, u32)| {
    let (sec, atto, step) = d;
    let policy = ExpiryPolicy::AssumeNoNewLeapSeconds;
    let Ok(t) = Instant::<Tai>::new(sec % 4_000_000_000, atto % 1_000_000_000_000_000_000) else {
        return;
    };
    let Ok(u) = UtcDateTime::from_tai(t, policy) else { return };
    let back = u.value.to_tai(policy).expect("a label of an instant converts");
    assert!((back.value.total_attoseconds() - t.total_attoseconds()).abs() <= 1);
    let later = Instant::<Tai>::from_total_attoseconds(t.total_attoseconds() + i128::from(step) * 1_000_000_000)
        .unwrap();
    if let Ok(v) = UtcDateTime::from_tai(later, policy) {
        assert!(v.value >= u.value, "{u:?} then {v:?}");
    }
});
