//! The TAI − UTC table against its source files: the copy of IANA
//! leap-seconds.list (its own SHA-1, expiry and every leap) and the 1960–1971
//! rows of ERFA dat.c (USNO tai-utc.dat), transcribed here from dat.c.

use alice_ephemeris::leap::{self, SEGMENTS};
use alice_ephemeris::{mjd_from_civil, tai_minus_utc, ExpiryPolicy};
use sha1::{Digest, Sha1};

const ATTO: i128 = 1_000_000_000_000_000_000;

fn iana() -> String {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/leap-seconds.list"
    ))
    .unwrap()
}

#[test]
fn iana_file_hash_is_the_published_one() {
    // the #h line: SHA-1 over the #$ and #@ values and each leap line's
    // fields, whitespace and comments removed
    let text = iana();
    let mut data = String::new();
    let mut published = None;
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("#$").or_else(|| line.strip_prefix("#@")) {
            data += v.split_whitespace().next().unwrap();
        } else if let Some(v) = line.strip_prefix("#h") {
            published = Some(v.split_whitespace().collect::<String>());
        } else if !line.starts_with('#') && !line.trim().is_empty() {
            data.extend(line.split('#').next().unwrap().split_whitespace());
        }
    }
    let got: String = Sha1::digest(data.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(Some(got.clone()), published);
    let words: String = leap::SOURCE_SHA1
        .iter()
        .map(|w| format!("{w:08x}"))
        .collect();
    assert_eq!(words, got);
}

#[test]
fn leap_rows_equal_the_iana_file() {
    let text = iana();
    let mut rows = Vec::new();
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("#@") {
            assert_eq!(v.trim().parse::<u64>().unwrap(), leap::SOURCE_EXPIRES_NTP);
        } else if let Some(v) = line.strip_prefix("#$") {
            assert_eq!(v.trim().parse::<u64>().unwrap(), leap::SOURCE_UPDATED_NTP);
        } else if !line.starts_with('#') && !line.trim().is_empty() {
            let mut f = line.split_whitespace();
            let ntp: i64 = f.next().unwrap().parse().unwrap();
            let dtai: i64 = f.next().unwrap().parse().unwrap();
            assert_eq!(ntp % 86_400, 0);
            // NTP epoch 1900-01-01 = MJD 15020
            rows.extend([(ntp / 86_400 + 15_020, dtai * 10_000_000)]);
        }
    }
    let ours: Vec<(i64, i64)> = SEGMENTS
        .iter()
        .filter(|s| s.start_mjd >= leap::LEAP_ERA_MJD)
        .map(|s| {
            assert_eq!((s.rate_e7, s.ref_mjd), (0, 0));
            (s.start_mjd, s.delat_e7)
        })
        .collect();
    assert_eq!(rows.len(), 28);
    assert_eq!(ours, rows);
    assert_eq!(
        leap::EXPIRES_MJD,
        (leap::SOURCE_EXPIRES_NTP / 86_400) as i64 + 15_020
    );
    assert_eq!(leap::EXPIRES_MJD, mjd_from_civil(2027, 6, 28).unwrap());
}

#[test]
fn pre_1972_rows_equal_erfa_dat_c() {
    // (year, month, delat s, ref MJD, rate s/day) as printed in ERFA v2.0.1 dat.c
    let dat_c: [(i64, u8, &str, i64, &str); 14] = [
        (1960, 1, "1.4178180", 37300, "0.0012960"),
        (1961, 1, "1.4228180", 37300, "0.0012960"),
        (1961, 8, "1.3728180", 37300, "0.0012960"),
        (1962, 1, "1.8458580", 37665, "0.0011232"),
        (1963, 11, "1.9458580", 37665, "0.0011232"),
        (1964, 1, "3.2401300", 38761, "0.0012960"),
        (1964, 4, "3.3401300", 38761, "0.0012960"),
        (1964, 9, "3.4401300", 38761, "0.0012960"),
        (1965, 1, "3.5401300", 38761, "0.0012960"),
        (1965, 3, "3.6401300", 38761, "0.0012960"),
        (1965, 7, "3.7401300", 38761, "0.0012960"),
        (1965, 9, "3.8401300", 38761, "0.0012960"),
        (1966, 1, "4.3131700", 39126, "0.0025920"),
        (1968, 2, "4.2131700", 39126, "0.0025920"),
    ];
    // "1.4178180" -> 14178180 (units of 1e-7 s; every literal has 7 decimals)
    let e7 = |s: &str| -> i64 {
        let (a, b) = s.split_once('.').unwrap();
        assert_eq!(b.len(), 7);
        a.parse::<i64>().unwrap() * 10_000_000 + b.parse::<i64>().unwrap()
    };
    for (seg, (y, m, delat, r, rate)) in SEGMENTS.iter().zip(dat_c) {
        assert_eq!(seg.start_mjd, mjd_from_civil(y, m, 1).unwrap(), "{y}-{m}");
        assert_eq!(seg.delat_e7, e7(delat), "{y}-{m}");
        assert_eq!((seg.ref_mjd, seg.rate_e7), (r, e7(rate)), "{y}-{m}");
    }
    assert_eq!(SEGMENTS[0].start_mjd, leap::FIRST_MJD);
    assert_eq!(SEGMENTS[14].start_mjd, leap::LEAP_ERA_MJD);
}

#[test]
fn whole_seconds_from_1972_are_exact() {
    // e.g. 2017-01-01 onwards: 37 s exactly, to the attosecond
    let mut n = 0;
    for s in SEGMENTS
        .iter()
        .filter(|s| s.start_mjd >= leap::LEAP_ERA_MJD)
    {
        for sod in [0i128, 43_200 * ATTO] {
            let v = tai_minus_utc(s.start_mjd, sod, ExpiryPolicy::Strict)
                .unwrap()
                .value;
            assert_eq!(v, i128::from(s.delat_e7) * 100_000_000_000);
            n += 1;
        }
    }
    assert_eq!(n, 56);
    // the end of 1971: 4.2131700 + (41317 − 39126) × 0.002592 = 9.892242 s
    let end_1971 = tai_minus_utc(41_316, 86_400 * ATTO, ExpiryPolicy::Strict)
        .unwrap()
        .value;
    assert_eq!(end_1971, 9_892_242_000_000_000_000);
}

#[test]
fn rows_never_shorten_a_day_below_86400_s_nor_lengthen_it_past_86401_s() {
    // the two facts UtcDateTime::from_tai relies on: no row has a negative
    // drift (inside a row every day lasts at least 86400 s of TAI), and no
    // step at midnight exceeds one second (a second of day ≥ 86400 is 23:59:60)
    for s in &SEGMENTS {
        assert!(s.rate_e7 >= 0, "{s:?}");
    }
    let mut steps = 0;
    for w in SEGMENTS.windows(2) {
        let d = w[1].start_mjd;
        let before = tai_minus_utc(d - 1, 86_400 * ATTO, ExpiryPolicy::Strict)
            .unwrap()
            .value;
        let after = tai_minus_utc(d, 0, ExpiryPolicy::Strict).unwrap().value;
        assert!(
            after - before <= ATTO,
            "step {} as at MJD {d}",
            after - before
        );
        steps += 1;
    }
    assert_eq!(steps, SEGMENTS.len() - 1);
}
