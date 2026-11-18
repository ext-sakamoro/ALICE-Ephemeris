//! The UTC offset of the operating system's local time zone at an instant.
//!
//! [`utc_offset_at_unix`] answers for the instant asked, not for now: across a daylight saving
//! change the two sides get their own offsets. It reads the zone the OS gives the process each time
//! (on unix the `TZ` variable or the system zone, read again on every call; on Windows the
//! zone of the account, with the dynamic daylight saving rules of the instant's year).
//!
//! On unix each call reads the environment (`TZ`): do not change the environment from another
//! thread while asking (a data race in the C library, as with `std::env::set_var`).
//!
//! This crate holds the only `unsafe` of alice-ephemeris (the two OS calls); alice-ephemeris itself
//! forbids `unsafe` and reaches this through its `os-local` feature.

/// The offset in seconds east of UTC (Tokyo `32400`, New York in January `-18000`) of the local
/// time zone at `secs` seconds after 1970-01-01T00:00:00Z. `None` when the OS cannot answer: the
/// instant outside what the OS represents (on Windows the years 1601 to 30827), an OS without a
/// zone (wasm, other targets), or an offset of a day or more.
#[must_use]
pub fn utc_offset_at_unix(secs: i64) -> Option<i32> {
    os::offset(secs).filter(|o| o.unsigned_abs() < 86_400)
}

/// The offset in the Windows time zone with the registry key name `key` (`"Eastern Standard
/// Time"`) at `secs`, by the same conversion as [`utc_offset_at_unix`] uses for the local zone.
/// For tests on Windows: a zone with daylight saving can be checked without changing the
/// machine's zone. `None` for an unknown key, and on every other OS.
#[doc(hidden)]
#[must_use]
pub fn utc_offset_at_unix_in_windows_zone(key: &str, secs: i64) -> Option<i32> {
    #[cfg(windows)]
    {
        os::offset_in_named(key, secs).filter(|o| o.unsigned_abs() < 86_400)
    }
    #[cfg(not(windows))]
    {
        let _ = (key, secs);
        None
    }
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
))]
mod os {
    extern "C" {
        // POSIX; libc does not declare it on every unix
        fn tzset();
    }

    pub fn offset(secs: i64) -> Option<i32> {
        let t = libc::time_t::try_from(secs).ok()?;
        // SAFETY: `tm` is plain data, so all-zero is a valid value; `localtime_r` writes it and
        // returns null on failure; `tzset` has no arguments but reads the environment (`TZ`), so
        // a caller must not change the environment from another thread meanwhile (the same data
        // race that makes `std::env::set_var` unsafe in edition 2024)
        let tm = unsafe {
            tzset();
            let mut tm: libc::tm = core::mem::zeroed();
            if libc::localtime_r(&t, &mut tm).is_null() {
                return None;
            }
            tm
        };
        i32::try_from(tm.tm_gmtoff).ok()
    }
}

#[cfg(windows)]
mod os {
    use windows_sys::Win32::Foundation::SYSTEMTIME;
    use windows_sys::Win32::System::Time::{
        EnumDynamicTimeZoneInformation, GetDynamicTimeZoneInformation,
        SystemTimeToTzSpecificLocalTimeEx, DYNAMIC_TIME_ZONE_INFORMATION,
    };

    /// Days from 1970-01-01 to the proleptic Gregorian date (Howard Hinnant's days_from_civil).
    fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
        let y = if m <= 2 { y - 1 } else { y };
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    /// The date of a day count from 1970-01-01 (civil_from_days, the inverse of the above).
    fn civil_from_days(z: i64) -> (i64, i64, i64) {
        let z = z + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        (yoe + era * 400 + i64::from(m <= 2), m, d)
    }

    fn secs_of(st: &SYSTEMTIME) -> i64 {
        days_from_civil(
            i64::from(st.wYear),
            i64::from(st.wMonth),
            i64::from(st.wDay),
        ) * 86_400
            + i64::from(st.wHour) * 3600
            + i64::from(st.wMinute) * 60
            + i64::from(st.wSecond)
    }

    pub fn offset(secs: i64) -> Option<i32> {
        // SAFETY: plain data (all-zero is valid), written by the call; failure is u32::MAX
        let zone = unsafe {
            let mut zone: DYNAMIC_TIME_ZONE_INFORMATION = core::mem::zeroed();
            if GetDynamicTimeZoneInformation(&mut zone) == u32::MAX {
                return None;
            }
            zone
        };
        offset_in(&zone, secs)
    }

    pub fn offset_in_named(key: &str, secs: i64) -> Option<i32> {
        let want: Vec<u16> = key.encode_utf16().collect();
        for index in 0.. {
            // SAFETY: as above; a non-zero return is the end of the list or an error
            let zone = unsafe {
                let mut zone: DYNAMIC_TIME_ZONE_INFORMATION = core::mem::zeroed();
                if EnumDynamicTimeZoneInformation(index, &mut zone) != 0 {
                    return None;
                }
                zone
            };
            let name = &zone.TimeZoneKeyName;
            let len = name.iter().position(|&c| c == 0).unwrap_or(name.len());
            if name[..len] == want[..] {
                return offset_in(&zone, secs);
            }
        }
        None
    }

    fn offset_in(zone: &DYNAMIC_TIME_ZONE_INFORMATION, secs: i64) -> Option<i32> {
        let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
        let rem = secs.rem_euclid(86_400);
        let field = |v: i64| u16::try_from(v).ok();
        let utc = SYSTEMTIME {
            wYear: field(y).filter(|y| (1601..=30827).contains(y))?,
            wMonth: field(m)?,
            wDayOfWeek: 0,
            wDay: field(d)?,
            wHour: field(rem / 3600)?,
            wMinute: field(rem / 60 % 60)?,
            wSecond: field(rem % 60)?,
            wMilliseconds: 0,
        };
        // SAFETY: plain data (all-zero is valid); the call reads `zone` and `utc` and writes
        // `local`, a failure is a zero return
        let local = unsafe {
            let mut local: SYSTEMTIME = core::mem::zeroed();
            if SystemTimeToTzSpecificLocalTimeEx(zone, &utc, &mut local) == 0 {
                return None;
            }
            local
        };
        i32::try_from(secs_of(&local) - secs_of(&utc)).ok()
    }
}

#[cfg(not(any(
    windows,
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
)))]
mod os {
    pub fn offset(_secs: i64) -> Option<i32> {
        None
    }
}
