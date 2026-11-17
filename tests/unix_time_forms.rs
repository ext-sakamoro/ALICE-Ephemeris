//! Text forms, unix counts and arithmetic of [`Timestamp`], pinned for fixed instants
//!
//! The expected values are those of a widely used Rust date-time library (its RFC 3339 writer with
//! `Z` and the shortest SI fraction, its `Display`, its unix second and millisecond counts), taken
//! from it and written here as literals; the code under test is not called to make them

use alice_ephemeris::time::Timestamp;

mod t {
    pub use alice_ephemeris::time::Timestamp as Time;

    pub fn at(secs: i64, nanos: u32) -> Time {
        Time::from_unix(secs, nanos).unwrap()
    }
    pub fn from_ms(ms: i64) -> Option<Time> {
        Time::from_unix_millis(ms)
    }
    pub fn ms(t: Time) -> i64 {
        t.unix_millis()
    }
    pub fn secs(t: Time) -> i64 {
        t.unix_seconds()
    }
    pub fn display(t: Time) -> String {
        t.to_string()
    }
    pub fn rfc3339(t: Time) -> String {
        t.rfc3339().to_string()
    }
    pub fn compact(t: Time) -> String {
        t.compact().to_string()
    }
    pub fn plus_days(t: Time, days: i64) -> Time {
        t.plus_days(days)
    }
    pub fn whole_secs(later: Time, earlier: Time) -> i64 {
        later.whole_seconds_since(earlier)
    }
    pub fn whole_days(later: Time, earlier: Time) -> i64 {
        later.whole_days_since(earlier)
    }
}

use t::{at, Time};

/// The `Z` form (the serde form)
fn serde_str(time: Time) -> String {
    time.rfc3339_z().to_string()
}

fn decode(s: &str) -> Option<Time> {
    Timestamp::parse_rfc3339(s)
}

const T: i64 = 1_700_000_000; // 2023-11-14T22:13:20Z

/// (seconds, nanoseconds, serde form)
const SERDE_FORMS: &[(i64, u32, &str)] = &[
    (T, 0, "2023-11-14T22:13:20Z"),
    (T, 123_000_000, "2023-11-14T22:13:20.123Z"),
    (T, 123_456_000, "2023-11-14T22:13:20.123456Z"),
    (T, 123_456_789, "2023-11-14T22:13:20.123456789Z"),
    (T, 100, "2023-11-14T22:13:20.000000100Z"),
    (T, 1_000, "2023-11-14T22:13:20.000001Z"),
    (T, 1_000_000, "2023-11-14T22:13:20.001Z"),
    (0, 0, "1970-01-01T00:00:00Z"),
    (-1, 999_999_999, "1969-12-31T23:59:59.999999999Z"),
    (-2, 499_500_000, "1969-12-31T23:59:58.499500Z"),
    (1_709_208_000, 0, "2024-02-29T12:00:00Z"),
    (-62_167_219_200, 0, "0000-01-01T00:00:00Z"),
    (-62_167_219_201, 0, "-0001-12-31T23:59:59Z"),
    (253_402_300_799, 0, "9999-12-31T23:59:59Z"),
    (253_402_300_800, 0, "+10000-01-01T00:00:00Z"),
];

#[test]
fn rfc3339_z_fraction_digits() {
    for &(s, n, want) in SERDE_FORMS {
        assert_eq!(serde_str(at(s, n)), want, "({s}, {n})");
    }
}

#[test]
fn rfc3339_z_reads_back() {
    for &(s, n, form) in SERDE_FORMS {
        assert_eq!(decode(form), Some(at(s, n)), "{form}");
    }
}

#[test]
fn rfc3339_offsets_read_as_utc() {
    let want = Some(at(T, 0));
    for form in [
        "2023-11-14T22:13:20+00:00",
        "2023-11-14T22:13:20-00:00",
        "2023-11-15T07:13:20+09:00",
        "2023-11-14T17:13:20-05:00",
        "2023-11-14t22:13:20z",
        "2023-11-14 22:13:20Z",
    ] {
        assert_eq!(decode(form), want, "{form}");
    }
    assert_eq!(
        decode("2023-11-14T22:13:20.5+00:00"),
        Some(at(T, 500_000_000))
    );
    assert_eq!(
        decode("2023-11-14T22:13:20.1234567891Z"),
        Some(at(T, 123_456_789))
    );
}

#[test]
fn rfc3339_malformed_refused() {
    for form in [
        "",
        "garbage",
        "2023-11-14",
        "2023-11-14T22:13:20",
        "2023-13-14T22:13:20Z",
        "2023-02-29T22:13:20Z",
        "2023-11-14T24:13:20Z",
        "2023-11-14T22:60:20Z",
        "2023-11-14T22:13:20.Z",
        "2023-11-14T22:13:20+24:00",
        "2023-11-14T22:13:20Zjunk",
    ] {
        assert_eq!(decode(form), None, "{form:?}");
    }
}

#[test]
fn unix_ms_and_seconds_floor() {
    assert_eq!(t::ms(at(T, 123_999_999)), 1_700_000_000_123);
    assert_eq!(t::ms(at(-1, 999_999_999)), -1);
    assert_eq!(t::ms(at(-2, 499_500_000)), -1501);
    assert_eq!(t::secs(at(-1, 999_999_999)), -1);
    assert_eq!(t::secs(at(T, 999_999_999)), T);
}

#[test]
fn unix_ms_range_edges() {
    assert_eq!(t::from_ms(-1501), Some(at(-2, 499_000_000)));
    assert_eq!(t::from_ms(1_700_000_000_123), Some(at(T, 123_000_000)));
    let max = 8_210_266_876_799_999; // 262142-12-31T23:59:59.999Z
    let min = -8_334_601_228_800_000; // -262143-01-01T00:00:00Z
    assert_eq!(
        t::from_ms(max).map(serde_str),
        Some("+262142-12-31T23:59:59.999Z".to_string())
    );
    assert_eq!(
        t::from_ms(min).map(serde_str),
        Some("-262143-01-01T00:00:00Z".to_string())
    );
    assert_eq!(t::from_ms(max + 1), None);
    assert_eq!(t::from_ms(min - 1), None);
    assert_eq!(t::from_ms(i64::MAX), None);
    assert_eq!(t::from_ms(i64::MIN), None);
}

#[test]
fn display_rfc3339_offset_compact() {
    assert_eq!(t::display(at(T, 0)), "2023-11-14 22:13:20 UTC");
    assert_eq!(
        t::display(at(T, 123_000_000)),
        "2023-11-14 22:13:20.123 UTC"
    );
    assert_eq!(
        t::display(at(T, 123_456_789)),
        "2023-11-14 22:13:20.123456789 UTC"
    );
    assert_eq!(
        t::display(at(-62_167_219_201, 0)),
        "-0001-12-31 23:59:59 UTC"
    );
    assert_eq!(t::rfc3339(at(T, 0)), "2023-11-14T22:13:20+00:00");
    assert_eq!(
        t::rfc3339(at(T, 845_000_000)),
        "2023-11-14T22:13:20.845+00:00"
    );
    assert_eq!(
        t::rfc3339(at(-1, 999_000_000)),
        "1969-12-31T23:59:59.999+00:00"
    );
    assert_eq!(t::compact(at(T, 999_999_999)), "20231114T221320Z");
    assert_eq!(t::compact(at(0, 0)), "19700101T000000Z");
}

#[test]
fn plus_days_keeps_time_of_day() {
    assert_eq!(
        serde_str(t::plus_days(at(1_709_208_000, 123_456_789), 365)),
        "2025-02-28T12:00:00.123456789Z"
    );
    assert_eq!(
        serde_str(t::plus_days(at(T, 1), -2)),
        "2023-11-12T22:13:20.000000001Z"
    );
}

#[test]
fn whole_seconds_days_toward_zero() {
    let base = at(T, 0);
    assert_eq!(t::whole_secs(at(T, 500_000_000), base), 0);
    assert_eq!(t::whole_secs(base, at(T, 500_000_000)), 0);
    assert_eq!(t::whole_secs(base, at(T + 1, 500_000_000)), -1);
    assert_eq!(t::whole_secs(at(T + 1, 500_000_000), base), 1);
    assert_eq!(t::whole_days(at(T + 86_400 * 3 - 1, 0), base), 2);
    assert_eq!(t::whole_days(base, at(T + 86_400 * 3 - 1, 0)), -2);
    assert!(at(T, 1) > at(T, 0));
    assert!(at(-1, 999_999_999) < at(0, 0));
}

/// The serde form is the `Z` text, written and read as a string
#[cfg(feature = "serde")]
#[test]
fn serde_is_the_rfc3339_z_string() {
    for &(s, n, form) in SERDE_FORMS {
        let json = serde_json::to_string(&at(s, n)).unwrap();
        assert_eq!(json, format!("\"{form}\""));
        assert_eq!(serde_json::from_str::<Time>(&json).unwrap(), at(s, n));
    }
    assert!(serde_json::from_str::<Time>("\"2023-02-29T22:13:20Z\"").is_err());
    assert!(serde_json::from_str::<Time>("1700000000").is_err());
}
