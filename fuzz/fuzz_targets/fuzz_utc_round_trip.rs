//! Any civil UTC input either errors or maps to a TAI instant whose UTC label
//! is the same input (exact from 1972, within 1 as before).
#![no_main]
use alice_ephemeris::{ExpiryPolicy, UtcDateTime};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|d: (i16, u8, u8, u8, u8, u8, u64, bool)| {
    let (y, mo, day, h, mi, s, atto, assume) = d;
    let policy = if assume {
        ExpiryPolicy::AssumeNoNewLeapSeconds
    } else {
        ExpiryPolicy::Strict
    };
    let year = 1950 + i64::from(y.rem_euclid(120));
    let Ok(dt) = UtcDateTime::new(year, mo % 13, day % 32, h % 25, mi % 61, s % 62, atto) else {
        return;
    };
    let Ok(tai) = dt.to_tai(policy) else { return };
    let back = UtcDateTime::from_tai(tai.value, policy).expect("a converted instant maps back");
    let again = back.value.to_tai(policy).expect("its label converts");
    let d = again.value.total_attoseconds() - tai.value.total_attoseconds();
    assert!(d.abs() <= 1, "{dt:?} -> {:?} -> {:?}", tai.value, back.value);
    if year >= 1972 {
        assert_eq!(back.value, dt);
    }
});
