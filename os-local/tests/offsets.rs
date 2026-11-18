//! The offset at an instant, read from the OS, against the IANA time zone database.
//!
//! oracle: Python's `zoneinfo` (IANA tzdata) offsets for these instants, written here as literals;
//! the code under test is not called to make them. On unix the zone is chosen with `TZ` (one test
//! function, so no other test changes `TZ` meanwhile). On Windows the local zone cannot be chosen without
//! changing the system, so the same conversion is run on a named zone ("Eastern Standard Time",
//! with daylight saving) against the same values, and the local zone is compared with the OS's
//! .NET reading of the same instants.

use alice_ephemeris_os_local::utc_offset_at_unix;

#[cfg(unix)]
const CASES: &[(&str, i64, i32)] = &[
    ("UTC", 1_768_478_400, 0),
    ("UTC", -14_182_980, 0),
    ("Asia/Tokyo", 1_768_478_400, 32_400),
    ("Asia/Tokyo", 1_784_116_800, 32_400),
    ("Asia/Kolkata", 1_768_478_400, 19_800),
    // New York: standard time in January, daylight time in July
    ("America/New_York", 1_768_478_400, -18_000),
    ("America/New_York", 1_784_116_800, -14_400),
    // 2026-03-08T07:00:00Z: the second before is still standard time, the instant itself daylight
    ("America/New_York", 1_772_953_199, -18_000),
    ("America/New_York", 1_772_953_200, -14_400),
    // 2026-11-01T06:00:00Z: back to standard time
    ("America/New_York", 1_793_512_799, -14_400),
    ("America/New_York", 1_793_512_800, -18_000),
    ("Europe/London", 1_768_478_400, 0),
    ("Europe/London", 1_784_116_800, 3_600),
    // 1969-07-20T20:17:00Z: before 1970, during British Standard Time (UTC+1 all year)
    ("Europe/London", -14_182_980, 3_600),
];

#[cfg(unix)]
#[test]
fn the_offset_is_the_zone_database_one_at_each_instant() {
    let before = std::env::var_os("TZ");
    for &(zone, secs, want) in CASES {
        std::env::set_var("TZ", zone);
        assert_eq!(utc_offset_at_unix(secs), Some(want), "{zone} at {secs}");
    }
    match before {
        Some(v) => std::env::set_var("TZ", v),
        None => std::env::remove_var("TZ"),
    }
}

/// Windows, a zone with daylight saving chosen by name (the machine's own zone is not changed):
/// the same instants and IANA values as New York above (Windows' "Eastern Standard Time").
#[cfg(windows)]
#[test]
fn a_windows_zone_gives_the_zone_database_offset_at_each_instant() {
    use alice_ephemeris_os_local::utc_offset_at_unix_in_windows_zone as at;
    for (secs, want) in [
        (1_768_478_400, -18_000),
        (1_784_116_800, -14_400),
        (1_772_953_199, -18_000),
        (1_772_953_200, -14_400),
        (1_793_512_799, -14_400),
        (1_793_512_800, -18_000),
    ] {
        assert_eq!(at("Eastern Standard Time", secs), Some(want), "{secs}");
    }
    assert_eq!(at("Tokyo Standard Time", 1_784_116_800), Some(32_400));
    assert_eq!(at("UTC", 1_784_116_800), Some(0));
    assert_eq!(at("No Such Zone", 1_784_116_800), None);
}

/// Windows, the machine's own zone: against the OS's .NET reading of the same instants
/// (`TimeZoneInfo.Local.GetUtcOffset`), a separate implementation of the zone rules.
#[cfg(windows)]
#[test]
fn the_local_zone_agrees_with_dotnet() {
    let instants = [
        0_i64,
        1_768_478_400,
        1_772_953_199,
        1_772_953_200,
        1_784_116_800,
        1_793_512_800,
    ];
    let list: Vec<String> = instants.iter().map(i64::to_string).collect();
    let script = format!(
        "foreach ($s in @({})) {{ [int][TimeZoneInfo]::Local.GetUtcOffset([DateTimeOffset]::FromUnixTimeSeconds($s)).TotalSeconds }}",
        list.join(",")
    );
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .expect("powershell");
    let dotnet: Vec<i32> = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|l| l.trim().parse().unwrap())
        .collect();
    assert_eq!(dotnet.len(), instants.len(), "one .NET offset per instant");
    for (secs, want) in instants.iter().zip(dotnet) {
        assert_eq!(utc_offset_at_unix(*secs), Some(want), "{secs}");
    }
}

#[test]
fn an_instant_the_os_cannot_hold_has_no_offset() {
    // far past any OS's calendar (Windows from 1601; a 64-bit time_t holds it, but the year
    // overflows the `int` of `struct tm`)
    assert_eq!(utc_offset_at_unix(i64::MAX), None);
    assert_eq!(utc_offset_at_unix(i64::MIN), None);
}
