//! The native build of `sun_alt_az` over `sky/expected.txt` gives the digest written there (the
//! wasm runner checks the same file, so the two builds give the same bits), and the export
//! refuses what is not a time or a place.
//!
//! Author: Moroya Sakamoto

use sha2::{Digest, Sha256};

fn call(ms: f64, lat: f64, lon: f64) -> [f64; 2] {
    let p = alice_ephemeris_sky::sun_alt_az(ms, lat, lon);
    // the export returns its two-value buffer
    unsafe { [p.read(), p.add(1).read()] }
}

#[test]
fn the_outputs_hash_to_the_pinned_digest() {
    let text = include_str!("../expected.txt");
    let mut h = Sha256::new();
    let mut n = 0;
    let mut pinned = None;
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty() && !l.starts_with("moon-sha256 "))
    {
        if let Some(d) = line.strip_prefix("sha256 ") {
            pinned = Some(d.to_owned());
            continue;
        }
        let v: Vec<f64> = line
            .split_whitespace()
            .map(|x| x.parse().expect("a number"))
            .collect();
        let [alt, az] = call(v[0], v[1], v[2]);
        assert!(alt.is_finite() && az.is_finite(), "{line}");
        h.update(alt.to_le_bytes());
        h.update(az.to_le_bytes());
        n += 1;
    }
    assert_eq!(n, 24, "inputs");
    let got: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(pinned.as_deref(), Some(got.as_str()), "sha256 {got}");
}

#[test]
fn what_is_not_a_time_or_a_place_is_nan() {
    let t = 1_774_017_960_000.0; // 2026-03-20 14:46 UTC
    for [ms, lat, lon] in [
        [t + 0.5, 0.0, 0.0],
        [-2_208_988_800_000.0, 0.0, 0.0],
        [f64::NAN, 0.0, 0.0],
        [t, f64::NAN, 0.0],
        [t, 0.0, f64::INFINITY],
        [t, 90.5, 0.0],
        [t, -91.0, 0.0],
        [t, 0.0, 180.5],
        [t, 0.0, -181.0],
    ] {
        let [alt, az] = call(ms, lat, lon);
        assert!(alt.is_nan() && az.is_nan(), "{ms} {lat} {lon}");
    }
    let [alt, _] = call(t, 0.0, -(15.0 * (14.0 + 46.0 / 60.0) - 180.0 - 7.5 / 4.0));
    assert!(alt.to_degrees() > 89.0, "{}", alt.to_degrees());
}

fn moon(ms: f64, lat: f64, lon: f64) -> [f64; 3] {
    let p = alice_ephemeris_sky::moon_alt_az_lit(ms, lat, lon);
    unsafe { [p.read(), p.add(1).read(), p.add(2).read()] }
}

#[test]
fn the_moon_outputs_hash_to_the_pinned_digest() {
    let text = include_str!("../expected.txt");
    let mut h = Sha256::new();
    let mut n = 0;
    let mut pinned = None;
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty() && !l.starts_with("sha256 "))
    {
        if let Some(d) = line.strip_prefix("moon-sha256 ") {
            pinned = Some(d.to_owned());
            continue;
        }
        let v: Vec<f64> = line
            .split_whitespace()
            .map(|x| x.parse().expect("a number"))
            .collect();
        for x in moon(v[0], v[1], v[2]) {
            assert!(x.is_finite(), "{line}");
            h.update(x.to_le_bytes());
        }
        n += 1;
    }
    assert_eq!(n, 24, "inputs");
    let got: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(pinned.as_deref(), Some(got.as_str()));
}

#[test]
fn the_moon_export_agrees_with_the_library_and_refuses_what_the_sun_refuses() {
    use alice_ephemeris::horizon::{sky_at_unix_ms, Site};
    let deg = core::f64::consts::PI / 180.0;
    let ms = 1_793_000_000_000i64;
    let sky = sky_at_unix_ms(
        ms,
        &Site {
            latitude: 36.0 * deg,
            longitude: 140.0 * deg,
            height: 0.0,
        },
    )
    .unwrap();
    let got = moon(ms as f64, 36.0, 140.0);
    assert_eq!(got[0].to_bits(), sky.moon.altitude.to_bits());
    assert_eq!(got[1].to_bits(), sky.moon.azimuth.to_bits());
    assert_eq!(
        got[2].to_bits(),
        sky.moon_phase.illuminated_fraction.to_bits()
    );
    assert!((0.0..=1.0).contains(&got[2]));
    for bad in [
        moon(ms as f64 + 0.5, 0.0, 0.0),
        moon(f64::NAN, 0.0, 0.0),
        moon(0.0, 91.0, 0.0),
        moon(0.0, 0.0, 181.0),
    ] {
        assert!(bad.iter().all(|x| x.is_nan()), "{bad:?}");
    }
}
