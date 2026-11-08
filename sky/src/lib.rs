//! The Sun, the Moon and the stars in a web page's sky: [`sun_alt_az`], [`moon_alt_az_lit`] and
//! [`stars_alt_az_mag`] (over the star table the page hands it: `sky/stars.bin`, fetched once it is
//! dark so it is not part of the first load), for a page's clock and the visitor's place, through [`alice_ephemeris::horizon::sky_at_unix_ms`]. Nothing else
//! is in the module, so a page loads only this (the Law identifier checks are in `browser/`).
//!
//! `scripts/run_sky_wasm.sh` builds it for wasm32-unknown-unknown and checks under node that the
//! module exports only these, [`stars_len`] and `memory`, that it gives the bits the native build gives
//! over `sky/expected.txt` (written by this crate's test), and that the Sun is overhead at the
//! subsolar point of the 2026 March equinox.
//!
//! Author: Moroya Sakamoto

#![allow(clippy::missing_safety_doc)]

use alice_ephemeris::horizon::{sky_at_unix_ms, Site};

/// A whole number of milliseconds an `i64` holds, and a place on the Earth. `2^63` is whole and would
/// pass the round trip through the cast, which saturates: it is out of range
fn valid_time_and_place(unix_ms: f64, latitude_deg: f64, longitude_deg: f64) -> bool {
    const LIMIT: f64 = 9_223_372_036_854_775_808.0;
    unix_ms.is_finite()
        && (-LIMIT..LIMIT).contains(&unix_ms)
        && unix_ms == (unix_ms as i64) as f64
        && latitude_deg.abs() <= 90.0
        && longitude_deg.abs() <= 180.0
}

/// Where [`sun_alt_az`] writes the Sun's altitude and azimuth (radians), two `f64`.
static mut SUN_ALT_AZ: [f64; 2] = [0.0; 2];

/// The Sun's topocentric altitude and azimuth (radians, azimuth from north through east) for a
/// page: `unix_ms` is the browser's clock (`Date.now()`), the site in degrees. Writes the two
/// values at the returned pointer (read as a `Float64Array` of length 2), both NaN when the
/// time is not an integral number of milliseconds after UTC began (1960) or the site is not on
/// the earth (not finite, |latitude| > 90, |longitude| > 180). Through [`sky_at_unix_ms`] (UT1 taken as UTC, pole coordinates zero).
#[no_mangle]
pub extern "C" fn sun_alt_az(unix_ms: f64, latitude_deg: f64, longitude_deg: f64) -> *const f64 {
    const DEG: f64 = core::f64::consts::PI / 180.0;
    let valid = valid_time_and_place(unix_ms, latitude_deg, longitude_deg);
    let site = Site {
        latitude: latitude_deg * DEG,
        longitude: longitude_deg * DEG,
        height: 0.0,
    };
    let out = match valid.then(|| sky_at_unix_ms(unix_ms as i64, &site)) {
        Some(Ok(sky)) => [sky.sun.altitude, sky.sun.azimuth],
        _ => [f64::NAN; 2],
    };
    let at = core::ptr::addr_of_mut!(SUN_ALT_AZ);
    // a wasm32 page runs this on one thread; nothing else holds the buffer
    unsafe { at.write(out) };
    at.cast::<f64>().cast_const()
}

/// Where [`moon_alt_az_lit`] writes the Moon's altitude and azimuth (radians) and the illuminated
/// fraction of its disc, three `f64`.
static mut MOON_ALT_AZ_LIT: [f64; 3] = [0.0; 3];

/// The Moon's topocentric altitude and azimuth (radians, azimuth from north through east) and the
/// illuminated fraction of its disc: as [`sun_alt_az`], for the same clock and place, three values
/// at the returned pointer, all NaN when [`sun_alt_az`] would give NaN or the date is outside the
/// Moon series' range.
#[no_mangle]
pub extern "C" fn moon_alt_az_lit(
    unix_ms: f64,
    latitude_deg: f64,
    longitude_deg: f64,
) -> *const f64 {
    const DEG: f64 = core::f64::consts::PI / 180.0;
    let valid = valid_time_and_place(unix_ms, latitude_deg, longitude_deg);
    let site = Site {
        latitude: latitude_deg * DEG,
        longitude: longitude_deg * DEG,
        height: 0.0,
    };
    // the Moon's series has a range of dates; outside it there is no Moon to give
    let in_range = valid
        && alice_ephemeris::horizon::tt_ut1_at_unix_ms(unix_ms as i64)
            .is_ok_and(|(tt, _)| alice_ephemeris::sunmoon::moon98(tt).in_range);
    let out = match in_range.then(|| sky_at_unix_ms(unix_ms as i64, &site)) {
        Some(Ok(sky)) => [
            sky.moon.altitude,
            sky.moon.azimuth,
            sky.moon_phase.illuminated_fraction,
        ],
        _ => [f64::NAN; 3],
    };
    let at = core::ptr::addr_of_mut!(MOON_ALT_AZ_LIT);
    // a wasm32 page runs this on one thread; nothing else holds the buffer
    unsafe { at.write(out) };
    at.cast::<f64>().cast_const()
}

/// How many stars [`stars_alt_az_mag`] writes.
#[no_mangle]
pub extern "C" fn stars_len() -> u32 {
    STARS as u32
}

const STARS: usize = alice_ephemeris::star_table::STAR_COUNT;
/// `sky/stars.bin`: the catalogue's decimals as whole numbers, a column at a time, little endian
/// (`scripts/gen_star_table.py`): ra and dec as `i32` in 1e-4 degree, pm_ra and pm_dec as `i16` in 1e-3 arcsec a
/// year, mag as `i16` in hundredths, each column [`stars_len`] long. 14 bytes a star.
const STAR_BYTES: usize = 14;
/// Where the page writes `sky/stars.bin`.
static mut STAR_TABLE: [u8; STARS * STAR_BYTES] = [0; STARS * STAR_BYTES];
/// Whether [`stars_table_written`] has been told a whole table is there.
static mut STAR_TABLE_READY: bool = false;

/// Where to write `sky/stars.bin`, [`stars_len`] times 14 bytes, before [`stars_table_written`].
#[no_mangle]
pub extern "C" fn stars_table() -> *mut u8 {
    core::ptr::addr_of_mut!(STAR_TABLE).cast::<u8>()
}

/// Say how many bytes were written at [`stars_table`]. 1 if that is a whole table, 0 if not, and
/// then [`stars_alt_az_mag`] gives NaN.
#[no_mangle]
pub extern "C" fn stars_table_written(bytes: u32) -> u32 {
    let whole = bytes as usize == STARS * STAR_BYTES;
    // a wasm32 page runs this on one thread; nothing else holds the flag
    unsafe { core::ptr::addr_of_mut!(STAR_TABLE_READY).write(whole) };
    u32::from(whole)
}

/// A degree and an arcsecond in radians, as the generator takes them (`math.pi / 180`, `math.pi / 180 / 3600`)
const DEG: f64 = core::f64::consts::PI / 180.0;
const ARCSEC: f64 = core::f64::consts::PI / 180.0 / 3600.0;

fn star_in_table(table: &[u8; STARS * STAR_BYTES], i: usize) -> alice_ephemeris::stars::Star {
    // column starts: ra, dec (4 bytes a star), then pm_ra, pm_dec, mag (2 bytes a star)
    let i32_at = |col: usize| {
        let at = col * 4 * STARS + i * 4;
        let mut b = [0u8; 4];
        b.copy_from_slice(&table[at..at + 4]);
        f64::from(i32::from_le_bytes(b))
    };
    let i16_at = |col: usize| {
        let at = 8 * STARS + col * 2 * STARS + i * 2;
        f64::from(i16::from_le_bytes([table[at], table[at + 1]]))
    };
    // each a whole number of the catalogue's last decimal: divided by its scale (exact operands, one rounding) it is
    // the decimal the catalogue gives, then taken to radians as the table's own values were
    alice_ephemeris::stars::Star {
        ra: (i32_at(0) / 1e4) * DEG,
        dec: (i32_at(1) / 1e4) * DEG,
        pm_ra: (i16_at(0) / 1e3) * ARCSEC,
        pm_dec: (i16_at(1) / 1e3) * ARCSEC,
        mag: (i16_at(2) / 100.0) as f32,
    }
}

/// Where [`stars_alt_az_mag`] writes.
static mut STARS_ALT_AZ_MAG: [f64; STARS * 3] = [0.0; STARS * 3];

/// Each star's altitude and azimuth (radians, azimuth from north through east) and magnitude, three
/// `f64` per star, brightest first, at the returned pointer: as [`sun_alt_az`] for the clock and the
/// place (UT1 taken as UTC, no pole), all NaN when it would give NaN. Below the horizon its altitude
/// is negative: which to draw is the page's. All NaN until a whole table has been written.
#[no_mangle]
pub extern "C" fn stars_alt_az_mag(
    unix_ms: f64,
    latitude_deg: f64,
    longitude_deg: f64,
) -> *const f64 {
    const DEG: f64 = core::f64::consts::PI / 180.0;
    // a wasm32 page runs this on one thread; nothing else holds the flag
    let ready = unsafe { core::ptr::addr_of!(STAR_TABLE_READY).read() };
    let valid = ready && valid_time_and_place(unix_ms, latitude_deg, longitude_deg);
    let site = Site {
        latitude: latitude_deg * DEG,
        longitude: longitude_deg * DEG,
        height: 0.0,
    };
    let mut out = [f64::NAN; STARS * 3];
    if valid {
        if let Ok((tt, ut1)) = alice_ephemeris::horizon::tt_ut1_at_unix_ms(unix_ms as i64) {
            // a wasm32 page runs this on one thread; the table is written before this is called
            let table = unsafe { &*core::ptr::addr_of!(STAR_TABLE) };
            let stars = [(); STARS].map({
                let mut i = 0;
                move |()| {
                    i += 1;
                    star_in_table(table, i - 1)
                }
            });
            for (slot, (h, s)) in out.chunks_exact_mut(3).zip(
                alice_ephemeris::stars::stars_in_sky(&stars, tt, ut1, 0.0, 0.0, &site).zip(&stars),
            ) {
                slot[0] = h.altitude;
                slot[1] = h.azimuth;
                slot[2] = f64::from(s.mag);
            }
        }
    }
    let at = core::ptr::addr_of_mut!(STARS_ALT_AZ_MAG);
    // a wasm32 page runs this on one thread; nothing else holds the buffer
    unsafe { at.write(out) };
    at.cast::<f64>().cast_const()
}
