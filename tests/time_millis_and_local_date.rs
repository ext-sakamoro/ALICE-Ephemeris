//! The fixed-millisecond RFC 3339 form and the date at a UTC offset, for fixed instants.
//!
//! oracle: the dates and texts below are worked out by hand from the instants (1_700_000_000 s is
//! 2023-11-14T22:13:20Z) and match a widely used Rust date-time library's `Millis` RFC 3339 writer
//! and its date at a fixed offset; the code under test is not called to make them.

use alice_ephemeris::time::{CivilDate, Timestamp};

fn at(secs: i64, nanos: u32) -> Timestamp {
    Timestamp::from_unix(secs, nanos).unwrap()
}

#[test]
fn the_millisecond_form_always_has_three_digits_truncated() {
    for (t, want) in [
        (at(1_700_000_000, 0), "2023-11-14T22:13:20.000Z"),
        (at(1_700_000_000, 100_000_000), "2023-11-14T22:13:20.100Z"),
        (at(1_700_000_000, 123_000_000), "2023-11-14T22:13:20.123Z"),
        (at(1_700_000_000, 123_999_999), "2023-11-14T22:13:20.123Z"),
        (at(1_700_000_000, 999), "2023-11-14T22:13:20.000Z"),
        (at(-1, 999_999_999), "1969-12-31T23:59:59.999Z"),
        (Timestamp::UNIX_EPOCH, "1970-01-01T00:00:00.000Z"),
        (Timestamp::MAX, "+262142-12-31T23:59:59.999Z"),
        (Timestamp::MIN, "-262143-01-01T00:00:00.000Z"),
    ] {
        assert_eq!(t.rfc3339_millis_z().to_string(), want);
        // and it reads back as the instant truncated to the millisecond
        assert_eq!(
            Timestamp::parse_rfc3339(want).map(Timestamp::unix_millis),
            Some(t.unix_millis()),
            "{want}"
        );
    }
}

#[test]
fn the_date_at_an_offset_is_the_wall_clock_date() {
    let t = at(1_700_000_000, 0); // 2023-11-14T22:13:20Z
    for (offset, want) in [
        (0, "2023-11-14"),
        (32_400, "2023-11-15"),  // +09:00: 07:13 the next day
        (6_399, "2023-11-14"),   // 23:59:59
        (6_400, "2023-11-15"),   // 00:00:00
        (-18_000, "2023-11-14"), // -05:00
        (-80_000, "2023-11-14"), // 00:00:00
        (-80_001, "2023-11-13"), // 23:59:59 the day before
        (86_399, "2023-11-15"),
        (-86_399, "2023-11-13"),
    ] {
        let d = t
            .civil_date_at_offset(offset)
            .unwrap_or_else(|| panic!("{offset}"));
        assert_eq!(d.to_string(), want, "{offset}");
    }
    let d: CivilDate = at(-1, 0).civil_date_at_offset(0).unwrap();
    assert_eq!((d.year(), d.month(), d.day()), (1969, 12, 31));
    assert_eq!(
        at(0, 0).civil_date_at_offset(-1).unwrap().to_string(),
        "1969-12-31"
    );
    assert_eq!(
        at(0, 0).civil_date_at_offset(0).unwrap().to_string(),
        "1970-01-01"
    );
    // a leap day, and a year with its sign
    assert_eq!(
        at(1_709_164_800, 0)
            .civil_date_at_offset(0)
            .unwrap()
            .to_string(),
        "2024-02-29"
    );
    assert_eq!(
        Timestamp::MIN.civil_date_at_offset(0).unwrap().to_string(),
        "-262143-01-01"
    );
}

#[test]
fn an_offset_of_a_day_or_more_has_no_date() {
    let t = at(1_700_000_000, 0);
    for offset in [86_400, -86_400, i32::MAX, i32::MIN] {
        assert_eq!(t.civil_date_at_offset(offset), None, "{offset}");
    }
}

#[test]
fn a_zone_gives_each_instant_its_own_offset() {
    // a zone at -05:00 before 2026-03-08T07:00:00Z and -04:00 from it (New York's 2026 change)
    let zone = |t: Timestamp| {
        Some(if t.unix_seconds() < 1_772_953_200 {
            -18_000
        } else {
            -14_400
        })
    };
    // 2026-03-09T04:30:00Z: 23:30 on the 8th at -05:00 but 00:30 on the 9th at -04:00
    let after = at(1_773_030_600, 0);
    assert_eq!(
        after.civil_date_in(&zone).unwrap().to_string(),
        "2026-03-09"
    );
    // 2026-03-08T04:30:00Z, before the change: 23:30 on the 7th
    let before = at(1_772_944_200, 0);
    assert_eq!(
        before.civil_date_in(&zone).unwrap().to_string(),
        "2026-03-07"
    );
    // no offset, no date
    assert_eq!(after.civil_date_in(&|_: Timestamp| None), None);
}

#[cfg(all(feature = "os-local", unix))]
#[test]
fn the_os_zone_is_read_at_the_instant() {
    use alice_ephemeris::time::OsLocalZone;
    let before = std::env::var_os("TZ");
    std::env::set_var("TZ", "Asia/Tokyo");
    assert_eq!(
        at(1_700_000_000, 0)
            .civil_date_in(&OsLocalZone)
            .unwrap()
            .to_string(),
        "2023-11-15"
    );
    std::env::set_var("TZ", "America/New_York");
    assert_eq!(
        at(1_773_030_600, 0)
            .civil_date_in(&OsLocalZone)
            .unwrap()
            .to_string(),
        "2026-03-09"
    );
    assert_eq!(
        at(1_772_944_200, 0)
            .civil_date_in(&OsLocalZone)
            .unwrap()
            .to_string(),
        "2026-03-07"
    );
    match before {
        Some(v) => std::env::set_var("TZ", v),
        None => std::env::remove_var("TZ"),
    }
}

#[test]
fn checked_and_held_addition_stop_at_the_range() {
    let t = at(1_700_000_000, 5); // 2023-11-14T22:13:20.000000005Z
    assert_eq!(t.checked_plus_days(1), Some(at(1_700_086_400, 5)));
    assert_eq!(t.checked_plus_days(-1), Some(at(1_699_913_600, 5)));
    assert_eq!(t.checked_plus_seconds(-1_700_000_001), Some(at(-1, 5)));
    assert_eq!(t.plus_seconds(60), at(1_700_000_060, 5));
    // the last whole second in range: 262142-12-31T23:59:59 is 8_210_266_876_799 s
    let last = at(8_210_266_876_799, 5);
    assert_eq!(
        t.checked_plus_seconds(8_210_266_876_799 - 1_700_000_000),
        Some(last)
    );
    assert_eq!(
        t.checked_plus_seconds(8_210_266_876_800 - 1_700_000_000),
        None
    );
    assert_eq!(t.plus_seconds(i64::MAX), Timestamp::MAX);
    assert_eq!(t.plus_seconds(i64::MIN), Timestamp::MIN);
    assert_eq!(t.checked_plus_days(i64::MAX), None);
    assert_eq!(t.checked_plus_days(i64::MIN), None);
    // 96 million days is past the end (about 262 142 years ahead), 95 million is not
    assert_eq!(t.checked_plus_days(96_000_000), None);
    assert!(t.checked_plus_days(95_000_000).is_some());
}
