//! Golden digests of every Law's outputs over fixed grids (the same bits on
//! every target: this file runs on each OS of the CI matrix and on wasm32),
//! and the Laws' semantics identifiers recomputed from their data and these
//! digests (src/law.rs).
//!
//! Re-recording after an intentional change, in this order: update the
//! digest constants below from the failure message first, then the
//! `semantics_id`s in src/law.rs from the second failure message.

use alice_ephemeris::law::{self, Law};
use alice_ephemeris::leap::{self, SEGMENTS};
use alice_ephemeris::tdb::{
    fb1990_coefficients, FB1990_BLOCKS, FB1990_JPL_ADJUSTMENT, FB1990_STATED_RANGE, SPICE_EB,
    SPICE_K, SPICE_M,
};
use alice_ephemeris::{
    ExpiryPolicy, Instant, Jd2, Tai, TdbModel, Tt, UtcDateTime, ATTO_PER_SEC, TT_MINUS_TAI,
};
use sha2::{Digest, Sha256};

const GOLDEN_UTC_TAI: &str = "aaa3c370d897c3fdb8682229f31b23cf2c1a43cd9e983756e3371c84ec99f444";
const GOLDEN_TT_TAI: &str = "0d5b624d4b53142542751355d88d8bb5817e4298dca5ee27ac9d26a8891ecf5f";
const GOLDEN_TDB_SPICE_ET: &str =
    "115f0b2aaf63ef8969a44dd0fb57840ca8be42adf07fee98837132efc782659f";
const GOLDEN_TDB_FB1990: &str = "906515b2d290c2006ddddad3a574770e1d314334f2f3fa6ac1dff5e45e47f5a6";
const GOLDEN_JULIAN_DATE: &str = "b966bd091c7906a97f3566c328a8887821a0e2bb79fbd8fefe1ac7bbe6df5786";

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn utc_tai_outputs() -> (Vec<u8>, usize) {
    let mut out = Vec::new();
    let mut n = 0;
    // 1960-01-01 .. 2027-06-27 every 3 days at varying times, plus every leap second
    let mut mjd = leap::FIRST_MJD;
    let mut k = 0u64;
    while mjd < leap::EXPIRES_MJD {
        let (y, m, d) = alice_ephemeris::civil_from_mjd(mjd).unwrap();
        let dt = UtcDateTime::new(
            y,
            m,
            d,
            (k % 24) as u8,
            (k * 7 % 60) as u8,
            (k * 13 % 60) as u8,
            k * 1_234_567_890_123_457 % ATTO_PER_SEC,
        )
        .unwrap();
        let tai = dt.to_tai(ExpiryPolicy::Strict).unwrap().value;
        out.extend(tai.total_attoseconds().to_le_bytes());
        let back = UtcDateTime::from_tai(tai, ExpiryPolicy::Strict)
            .unwrap()
            .value;
        out.extend(back.atto.to_le_bytes());
        mjd += 3;
        k += 1;
        n += 1;
    }
    for s in SEGMENTS.iter().skip(1) {
        let (y, m, d) = alice_ephemeris::civil_from_mjd(s.start_mjd - 1).unwrap();
        for atto in [0, 50_000_000_000_000_000, 999_999_999_999_999_999] {
            if let Ok(dt) = UtcDateTime::new(y, m, d, 23, 59, 60, atto) {
                match dt.to_tai(ExpiryPolicy::Strict) {
                    Ok(c) => out.extend(c.value.total_attoseconds().to_le_bytes()),
                    Err(e) => out.extend(format!("{e:?}").bytes()),
                }
                n += 1;
            }
        }
    }
    (out, n)
}

fn tt_grid() -> Vec<Instant<Tt>> {
    // 1600 .. 2200 TT, ~6 days apart, varying fraction
    let mut v = Vec::new();
    let mut s = -12_622_780_800i64;
    let mut k = 0u64;
    while s < 6_311_390_400 {
        v.extend([Instant::<Tt>::new(s, k * 98_765_432_109_877 % ATTO_PER_SEC).unwrap()]);
        s += 2_000_000 + (k as i64 * 7919) % 86_400;
        k += 1;
    }
    v
}

fn tdb_outputs(model: TdbModel) -> (Vec<u8>, usize) {
    let mut out = Vec::new();
    let grid = tt_grid();
    for &tt in &grid {
        out.extend(model.tdb_minus_tt(tt).to_bits().to_le_bytes());
        let tdb = tt.to_tdb(model).unwrap();
        out.extend(tdb.total_attoseconds().to_le_bytes());
        out.extend(tdb.to_tt(model).unwrap().total_attoseconds().to_le_bytes());
    }
    (out, grid.len())
}

fn tt_tai_outputs() -> (Vec<u8>, usize) {
    let mut out = Vec::new();
    let grid = tt_grid();
    for &tt in &grid {
        let tai: Instant<Tai> = tt.to_tai().unwrap();
        out.extend(tai.total_attoseconds().to_le_bytes());
        out.extend(tai.to_tt().unwrap().total_attoseconds().to_le_bytes());
    }
    (out, grid.len())
}

fn julian_date_outputs() -> (Vec<u8>, usize) {
    let mut out = Vec::new();
    let grid = tt_grid();
    for &tt in &grid {
        let jd = tt.to_jd2();
        out.extend(jd.jd1.to_bits().to_le_bytes());
        out.extend(jd.jd2.to_bits().to_le_bytes());
        out.extend(
            Instant::<Tt>::from_jd2(jd)
                .unwrap()
                .total_attoseconds()
                .to_le_bytes(),
        );
        // an arbitrary split of the same date
        let other = Jd2 {
            jd1: jd.jd1 - 0.5,
            jd2: jd.jd2 + 0.5,
        };
        out.extend(
            Instant::<Tt>::from_jd2(other)
                .unwrap()
                .total_attoseconds()
                .to_le_bytes(),
        );
        let m = tt.to_mjd();
        out.extend(m.day.to_le_bytes());
        out.extend(m.second_of_day.to_le_bytes());
    }
    (out, grid.len())
}

fn digest(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

#[test]
fn golden_digests() {
    type Case = (&'static str, (Vec<u8>, usize), &'static str);
    let cases: [Case; 5] = [
        ("UTC_TAI", utc_tai_outputs(), GOLDEN_UTC_TAI),
        ("TT_TAI", tt_tai_outputs(), GOLDEN_TT_TAI),
        (
            "TDB_SPICE_ET",
            tdb_outputs(TdbModel::SpiceEt),
            GOLDEN_TDB_SPICE_ET,
        ),
        (
            "TDB_FB1990",
            tdb_outputs(TdbModel::Fb1990),
            GOLDEN_TDB_FB1990,
        ),
        ("JULIAN_DATE", julian_date_outputs(), GOLDEN_JULIAN_DATE),
    ];
    let mut bad = Vec::new();
    for (name, (bytes, n), want) in &cases {
        assert!(*n > 1000, "{name}: only {n} grid points");
        let got = digest(bytes);
        if got != *want {
            bad.extend([format!(
                "const GOLDEN_{name}: &str = \"{got}\"; // {n} points"
            )]);
        }
    }
    assert!(
        bad.is_empty(),
        "golden digests changed:\n{}",
        bad.join("\n")
    );
}

/// The identifier of a Law: SHA-256 over a fixed prefix, its name, its data
/// and its golden digest.
fn law_id(law: &Law, data: &[u8], golden: &str) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"alice-ephemeris law v1\0");
    h.update(law.name.as_bytes());
    h.update([0]);
    h.update((data.len() as u64).to_le_bytes());
    h.update(data);
    h.update(golden.as_bytes());
    h.finalize().into()
}

fn f64s(v: &[f64]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_bits().to_le_bytes()).collect()
}

#[test]
fn semantics_ids() {
    let mut utc = Vec::new();
    for s in &SEGMENTS {
        for x in [s.start_mjd, s.delat_e7, s.ref_mjd, s.rate_e7] {
            utc.extend(x.to_le_bytes());
        }
    }
    for x in [leap::FIRST_MJD, leap::LEAP_ERA_MJD, leap::EXPIRES_MJD] {
        utc.extend(x.to_le_bytes());
    }
    utc.extend(leap::SOURCE_UPDATED_NTP.to_le_bytes());
    utc.extend(leap::SOURCE_EXPIRES_NTP.to_le_bytes());
    for w in leap::SOURCE_SHA1 {
        utc.extend(w.to_le_bytes());
    }

    let mut tt = Vec::new();
    tt.extend(TT_MINUS_TAI.total_attoseconds().to_le_bytes());

    let mut spice = f64s(&[SPICE_K, SPICE_EB, SPICE_M[0], SPICE_M[1]]);
    spice.extend(alice_det_math::SEMANTICS_ID);

    let mut fb: Vec<u8> = fb1990_coefficients().iter().flat_map(|t| f64s(t)).collect();
    for b in FB1990_BLOCKS {
        fb.extend((b as u64).to_le_bytes());
    }
    fb.extend(FB1990_JPL_ADJUSTMENT.iter().flat_map(|t| f64s(t)));
    fb.extend(FB1990_STATED_RANGE.0.to_le_bytes());
    fb.extend(FB1990_STATED_RANGE.1.to_le_bytes());
    fb.extend(alice_det_math::SEMANTICS_ID);

    let jd = alice_det_math::SEMANTICS_ID.to_vec();

    let cases = [
        (law::UTC_TAI, utc, GOLDEN_UTC_TAI),
        (law::TT_TAI, tt, GOLDEN_TT_TAI),
        (law::TDB_SPICE_ET, spice, GOLDEN_TDB_SPICE_ET),
        (law::TDB_FB1990, fb, GOLDEN_TDB_FB1990),
        (law::JULIAN_DATE, jd, GOLDEN_JULIAN_DATE),
    ];
    let mut bad = Vec::new();
    for (l, data, golden) in &cases {
        let id = law_id(l, data, golden);
        if id != l.semantics_id {
            let bytes: Vec<String> = id.iter().map(|b| format!("0x{b:02x}")).collect();
            bad.extend([format!("{}: [{}]", l.name, bytes.join(", "))]);
        }
    }
    assert!(bad.is_empty(), "semantics ids changed:\n{}", bad.join("\n"));
    // every Law is listed once, with a distinct identifier
    let mut ids: Vec<[u8; 32]> = law::ALL.iter().map(|l| l.semantics_id).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), law::ALL.len());
    assert_eq!(TdbModel::SpiceEt.law(), law::TDB_SPICE_ET);
    assert_eq!(TdbModel::Fb1990.law(), law::TDB_FB1990);
}
