//! Semantics identifiers of the Sun and Moon Laws (the Earth's position and
//! velocity, ERFA `eraEpv00`; the Moon's, ERFA `eraMoon98`; the two in a
//! site's sky).
//!
//! Same rule as tests/law_ids.rs: an identifier is the SHA-256 of the Law's
//! name, its data, the alice-det-math `SEMANTICS_ID`, and the digest of its
//! outputs over a frozen reference grid whose inputs are pinned by their own
//! SHA-256 and count. The identifiers are in src/law.rs (`law::ALL`).
//!
//! Author: Moroya Sakamoto

use alice_ephemeris::horizon::{self, Site, WGS84_A, WGS84_F};
use alice_ephemeris::law::{self, Law, EARTH_EPV00, IAU2006_CIO, MOON_98, SUN_MOON_HORIZON};
use alice_ephemeris::sunmoon::{epv00, moon98};
use alice_ephemeris::Jd2;
use sha2::{Digest, Sha256};

/// ERFA v2.0.1 sources of the tables (scripts/gen_sunmoon_tables.py).
const ERFA_EPV00: &str = "9915ba38c9365f8b0738269b8c2ac1fdd5f8dee3 \
    4568b28026cd83800b4a4f66d44f8f3451d7306f1c3fcf7e4cd6bfeb206dde57";
const ERFA_MOON98: &str = "9915ba38c9365f8b0738269b8c2ac1fdd5f8dee3 \
    8633f78b8ec33132743596700fd3ecfbb2ebcb4e886894e377556de9655d1237";

const REFERENCE_JD_INPUTS: (&str, usize) = (
    "373b555ba15e913c9befff27ed54f8bcad2b7d970e535a1dfa93f31afb7c1842",
    795,
);
const REFERENCE_EARTH_EPV00: &str =
    "c631b28181bdd3874d24d0fb838a28ec148c697de17b5b6d07d16b174cfd3234";
const REFERENCE_SKY_INPUTS: (&str, usize) = (
    "1f9b539a71e69a6e37b93afd3e24b34d85670004d936cd4c67d2104a70742697",
    199,
);
const REFERENCE_SUN_MOON_HORIZON: &str =
    "961a3bb0587372d7f901110c42d1bc1f1d5d3d255dddce4d447b24e9cea5c362";
const REFERENCE_MOON_98: &str = "fd4dce2892c8cc69351256aaa3354ea433369ed74722f275051e2749a6700bd8";

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn digest(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// TDB (TT) two-part dates 1800..2200 every 211 days plus a varying fraction,
/// both orders of the parts on every 7th, and three dates out to 1000 and 3000.
fn reference_jd_inputs() -> Vec<Jd2> {
    let mut v = Vec::new();
    let mut day = -73_050.0;
    let mut k = 0u32;
    while day <= 73_050.0 {
        let frac = f64::from(k % 991) / 991.0;
        v.push(Jd2 {
            jd1: 2_451_545.0,
            jd2: day + frac,
        });
        if k % 7 == 0 {
            v.push(Jd2 {
                jd1: day + frac,
                jd2: 2_451_545.0,
            });
        }
        day += 211.0;
        k += 1;
    }
    for day in [-365_250.0, 0.5, 365_250.0] {
        v.push(Jd2 {
            jd1: 2_451_545.0,
            jd2: day,
        });
    }
    v
}

fn jd_input_bytes(v: &[Jd2]) -> Vec<u8> {
    v.iter()
        .flat_map(|d| [d.jd1.to_bits().to_le_bytes(), d.jd2.to_bits().to_le_bytes()].concat())
        .collect()
}

fn pv_bytes(pv: &[[f64; 3]; 2]) -> impl Iterator<Item = u8> + '_ {
    pv.iter().flatten().flat_map(|x| x.to_bits().to_le_bytes())
}

fn earth_outputs(v: &[Jd2]) -> Vec<u8> {
    let mut out = Vec::new();
    for &d in v {
        let e = epv00(d);
        out.extend(pv_bytes(&e.heliocentric));
        out.extend(pv_bytes(&e.barycentric));
        out.push(u8::from(e.in_range));
    }
    out
}

fn moon_outputs(v: &[Jd2]) -> Vec<u8> {
    v.iter()
        .flat_map(|&d| pv_bytes(&moon98(d).geocentric).collect::<Vec<u8>>())
        .collect()
}

/// Every 4th date of the JD grid at five sites (the poles, the equator, two
/// mid-latitudes, a height of 4 km), UT1 = TT − 69.184 s, pole coordinates of
/// the size of the real ones.
fn reference_sky_inputs() -> Vec<(Jd2, Jd2, f64, f64, Site)> {
    let sites = [
        (core::f64::consts::FRAC_PI_2, 0.0, 0.0),
        (-core::f64::consts::FRAC_PI_2, 1.0, 0.0),
        (0.0, -2.0, 0.0),
        (0.623, 2.438, 40.0),
        (-0.59, -1.2, 4_000.0),
    ];
    let mut v = Vec::new();
    for (k, d) in reference_jd_inputs().into_iter().step_by(4).enumerate() {
        let (lat, lon, h) = sites[k % sites.len()];
        let ut1 = Jd2 {
            jd1: d.jd1,
            jd2: d.jd2 - 69.184 / 86_400.0,
        };
        let xp = 1e-6 * ((k % 7) as f64 - 3.0) / 3.0;
        let yp = 2e-6 * ((k % 5) as f64 - 2.0) / 2.0;
        let site = Site {
            latitude: lat,
            longitude: lon,
            height: h,
        };
        v.push((d, ut1, xp, yp, site));
    }
    v
}

fn sky_input_bytes(v: &[(Jd2, Jd2, f64, f64, Site)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (tt, ut1, xp, yp, s) in v {
        for x in [
            tt.jd1,
            tt.jd2,
            ut1.jd1,
            ut1.jd2,
            *xp,
            *yp,
            s.latitude,
            s.longitude,
            s.height,
        ] {
            out.extend(x.to_bits().to_le_bytes());
        }
    }
    out
}

fn sky_outputs(v: &[(Jd2, Jd2, f64, f64, Site)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (tt, ut1, xp, yp, s) in v {
        let sky = horizon::sky(*tt, *ut1, *xp, *yp, s);
        for h in [sky.sun, sky.moon] {
            for x in [h.altitude, h.azimuth, h.distance] {
                out.extend(x.to_bits().to_le_bytes());
            }
        }
        let p = sky.moon_phase;
        out.extend(p.phase_angle.to_bits().to_le_bytes());
        out.extend(p.illuminated_fraction.to_bits().to_le_bytes());
        out.extend([u8::from(p.waxing)]);
        for x in s.itrs() {
            out.extend(x.to_bits().to_le_bytes());
        }
    }
    out
}

fn law_id(law: &Law, data: &[u8], grid: (&str, usize), reference: &str) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"alice-ephemeris law v1\0");
    h.update(law.name.as_bytes());
    h.update([0]);
    h.update((data.len() as u64).to_le_bytes());
    h.update(data);
    h.update(grid.0.as_bytes());
    h.update((grid.1 as u64).to_le_bytes());
    h.update(reference.as_bytes());
    h.finalize().into()
}

#[test]
fn reference_grid_is_frozen() {
    let jd = reference_jd_inputs();
    let got = (digest(&jd_input_bytes(&jd)), jd.len());
    assert!(got.1 > 600, "{}", got.1);
    let sky = reference_sky_inputs();
    let got_sky = (digest(&sky_input_bytes(&sky)), sky.len());
    assert!(got_sky.1 > 150, "{}", got_sky.1);
    assert_eq!(
        (got_sky.0.as_str(), got_sky.1),
        REFERENCE_SKY_INPUTS,
        "a reference grid changed:\nconst REFERENCE_SKY_INPUTS: (&str, usize) = (\"{}\", {});",
        got_sky.0,
        got_sky.1
    );
    assert_eq!(
        (got.0.as_str(), got.1),
        REFERENCE_JD_INPUTS,
        "a reference grid changed:\nconst REFERENCE_JD_INPUTS: (&str, usize) = (\"{}\", {});",
        got.0,
        got.1
    );
}

#[test]
fn reference_digests() {
    let jd = reference_jd_inputs();
    let cases = [
        ("EARTH_EPV00", earth_outputs(&jd), REFERENCE_EARTH_EPV00),
        ("MOON_98", moon_outputs(&jd), REFERENCE_MOON_98),
        (
            "SUN_MOON_HORIZON",
            sky_outputs(&reference_sky_inputs()),
            REFERENCE_SUN_MOON_HORIZON,
        ),
    ];
    let mut bad = Vec::new();
    for (name, bytes, want) in &cases {
        assert!(!bytes.is_empty(), "{name}: no output");
        let got = digest(bytes);
        if got != *want {
            bad.push(format!("const REFERENCE_{name}: &str = \"{got}\";"));
        }
    }
    assert!(
        bad.is_empty(),
        "reference digests changed:\n{}",
        bad.join("\n")
    );
}

#[test]
fn semantics_ids() {
    let mut earth = ERFA_EPV00.as_bytes().to_vec();
    earth.extend(alice_det_math::SEMANTICS_ID);
    let mut moon = ERFA_MOON98.as_bytes().to_vec();
    moon.extend(alice_det_math::SEMANTICS_ID);
    // the Laws it builds on, and the ellipsoid
    let mut sky = Vec::new();
    for l in [EARTH_EPV00, MOON_98, IAU2006_CIO] {
        sky.extend(l.semantics_id);
    }
    sky.extend(WGS84_A.to_bits().to_le_bytes());
    sky.extend(WGS84_F.to_bits().to_le_bytes());
    sky.extend(alice_det_math::SEMANTICS_ID);
    let cases = [
        (
            EARTH_EPV00,
            earth,
            REFERENCE_JD_INPUTS,
            REFERENCE_EARTH_EPV00,
        ),
        (MOON_98, moon, REFERENCE_JD_INPUTS, REFERENCE_MOON_98),
        (
            SUN_MOON_HORIZON,
            sky,
            REFERENCE_SKY_INPUTS,
            REFERENCE_SUN_MOON_HORIZON,
        ),
    ];
    let mut bad = Vec::new();
    for (l, data, grid, reference) in &cases {
        let id = law_id(l, data, *grid, reference);
        if id != l.semantics_id {
            let bytes: Vec<String> = id.iter().map(|b| format!("0x{b:02x}")).collect();
            bad.push(format!("{}: [{}]", l.name, bytes.join(", ")));
        }
    }
    assert!(bad.is_empty(), "semantics ids changed:\n{}", bad.join("\n"));
    for l in [EARTH_EPV00, MOON_98, SUN_MOON_HORIZON] {
        assert!(law::ALL.contains(&l), "{} is not in law::ALL", l.name);
    }
}
