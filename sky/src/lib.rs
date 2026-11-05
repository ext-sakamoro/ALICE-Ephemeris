//! The Sun in a web page's sky: one export, [`sun_alt_az`], for a page's clock and the visitor's
//! place, through [`alice_ephemeris::horizon::sky_at_unix_ms`]. Nothing else is in the module, so
//! a page loads only this (the Law identifier checks are in `browser/`).
//!
//! `scripts/run_sky_wasm.sh` builds it for wasm32-unknown-unknown and checks under node that the
//! module exports only `sun_alt_az` and `memory`, that it gives the bits the native build gives
//! over `sky/expected.txt` (written by this crate's test), and that the Sun is overhead at the
//! subsolar point of the 2026 March equinox.
//!
//! Author: Moroya Sakamoto

#![allow(clippy::missing_safety_doc)]

use alice_ephemeris::horizon::{sky_at_unix_ms, Site};

/// Where [`sun_alt_az`] writes the Sun's altitude and azimuth (radians), two `f64`.
static mut SUN_ALT_AZ: [f64; 2] = [0.0; 2];

/// The Sun's topocentric altitude and azimuth (radians, azimuth from north through east) for a
/// page: `unix_ms` is the browser's clock (`Date.now()`), the site in degrees. Writes the two
/// values at the returned pointer (read as a `Float64Array` of length 2), both NaN when the
/// time is not an integral number of milliseconds after UTC began (1960) or the site is not
/// finite. Through [`sky_at_unix_ms`] (UT1 taken as UTC, pole coordinates zero).
#[no_mangle]
pub extern "C" fn sun_alt_az(unix_ms: f64, latitude_deg: f64, longitude_deg: f64) -> *const f64 {
    const DEG: f64 = core::f64::consts::PI / 180.0;
    let valid_ms = unix_ms.is_finite()
        && unix_ms == (unix_ms as i64) as f64
        && latitude_deg.is_finite()
        && longitude_deg.is_finite();
    let site = Site {
        latitude: latitude_deg * DEG,
        longitude: longitude_deg * DEG,
        height: 0.0,
    };
    let out = match valid_ms.then(|| sky_at_unix_ms(unix_ms as i64, &site)) {
        Some(Ok(sky)) => [sky.sun.altitude, sky.sun.azimuth],
        _ => [f64::NAN; 2],
    };
    let at = core::ptr::addr_of_mut!(SUN_ALT_AZ);
    // a wasm32 page runs this on one thread; nothing else holds the buffer
    unsafe { at.write(out) };
    at.cast::<f64>().cast_const()
}
