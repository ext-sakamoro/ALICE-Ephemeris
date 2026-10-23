//! The Laws' semantics identifiers (src/law.rs) and the output pins.
//!
//! Each Law's identifier is the SHA-256 of its name, its data (tables and
//! constants as bit patterns), the alice-det-math `SEMANTICS_ID` where it
//! evaluates `f64` transcendentals, and the digest of its outputs over a
//! **frozen reference grid**. The rule: the identifier moves if and only if
//! the Law's inputs or its outputs on the reference grid move.
//!
//! - Reference grids (`reference_*_inputs`) are part of the identifier. Their
//!   inputs are pinned by their own SHA-256 and count
//!   (`reference_grids_are_frozen`), so editing a grid is a visible failure;
//!   a grid is changed only together with a change in meaning.
//! - Test grids (`test_*_inputs`) are ordinary regression pins
//!   (`test_grid_digests`), outside the identifier, and may grow freely.
//!
//! All digests are the same bits on every target (this file runs on each OS
//! of the CI matrix and on wasm32). Re-recording after an intentional change
//! of meaning, in this order: the `REFERENCE_*` digests from the failure
//! message, then the `semantics_id`s in src/law.rs from the next one.

use alice_ephemeris::law::{self, Law};
use alice_ephemeris::leap::{self, SEGMENTS};
use alice_ephemeris::tdb::{
    fb1990_coefficients, FB1990_BLOCKS, FB1990_JPL_ADJUSTMENT, FB1990_STATED_RANGE, SPICE_EB,
    SPICE_K, SPICE_M,
};
use alice_ephemeris::{
    civil_from_mjd, ExpiryPolicy, Instant, Jd2, Tai, TdbModel, TimeError, Tt, UtcDateTime,
    ATTO_PER_SEC, TT_MINUS_TAI,
};
use sha2::{Digest, Sha256};

/// SHA-256 and count of the frozen reference inputs.
const REFERENCE_UTC_INPUTS: (&str, usize) = (
    "b4f46c78fd19926c27bbaca53ef66b858dffb85bd9ed15e8304d70f34eb2a3f0",
    8340,
);
const REFERENCE_TT_INPUTS: (&str, usize) = (
    "62e2f19d71365f18bf23d04451361bdcda33ba4165837295fc5e62bcf5e6bcdb",
    9267,
);

/// Output digests over the reference grids (folded into the identifiers).
const REFERENCE_UTC_TAI: &str = "74d04ebde5696215f6a07d91c342fbfa986d58836e3e0e77b337f70e190cb569";
const REFERENCE_TT_TAI: &str = "0d5b624d4b53142542751355d88d8bb5817e4298dca5ee27ac9d26a8891ecf5f";
const REFERENCE_TDB_SPICE_ET: &str =
    "115f0b2aaf63ef8969a44dd0fb57840ca8be42adf07fee98837132efc782659f";
const REFERENCE_TDB_FB1990: &str =
    "906515b2d290c2006ddddad3a574770e1d314334f2f3fa6ac1dff5e45e47f5a6";
const REFERENCE_JULIAN_DATE: &str =
    "e5c5cd9f10a2f822f294d41c5306e743a3136a4f4f4c1135a4c4fef02e2b9314";

/// Output digests over the test grids (regression pins, not in the identifiers).
const TEST_UTC_TAI: &str = "8d80ebb33f6d292814013f553e652e423eef5a44c846d08dc8f8d127a18d2ba9";
const TEST_TT_TAI: &str = "77918963f016300a054787c853bd3437878ae0b300f2ac3d5c9cd193af85bc04";
const TEST_TDB_SPICE_ET: &str = "21aeef5e1b5b8563606c49e3d1680105ce57139608721ae4a4e35b08a88ea6f6";
const TEST_TDB_FB1990: &str = "ec1e6e98dc425c22948b0d2c0ca28d5fcc5dd49b00153a13a2b000a2c0aca96d";
const TEST_JULIAN_DATE: &str = "bb7e7861ff821656bdd6a52fea9ddf067152f3e40b14348a6fe5044936bd5666";

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn digest(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

// ---- grids -----------------------------------------------------------------

/// 1960-01-01 .. 2027-06-27 every 3 days at varying times, then 23:59:60 with
/// three fractions on the last day of every row (where that label exists).
fn reference_utc_inputs() -> Vec<UtcDateTime> {
    let mut v = Vec::new();
    let mut mjd = leap::FIRST_MJD;
    let mut k = 0u64;
    while mjd < leap::EXPIRES_MJD {
        let (y, m, d) = civil_from_mjd(mjd).unwrap();
        v.extend([UtcDateTime::new(
            y,
            m,
            d,
            (k % 24) as u8,
            (k * 7 % 60) as u8,
            (k * 13 % 60) as u8,
            k * 1_234_567_890_123_457 % ATTO_PER_SEC,
        )
        .unwrap()]);
        mjd += 3;
        k += 1;
    }
    for s in SEGMENTS.iter().skip(1) {
        let (y, m, d) = civil_from_mjd(s.start_mjd - 1).unwrap();
        for atto in [0, 50_000_000_000_000_000, 999_999_999_999_999_999] {
            v.extend([UtcDateTime::new(y, m, d, 23, 59, 60, atto).unwrap()]);
        }
    }
    v
}

/// 1600 .. 2200 TT, about 23 days apart, varying fraction.
fn reference_tt_inputs() -> Vec<Instant<Tt>> {
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

/// Every day of 1960–1975 and 2015–2018 at its last second and at noon.
fn test_utc_inputs() -> Vec<UtcDateTime> {
    let mut v = Vec::new();
    let days = (leap::FIRST_MJD..42_413).chain(57_023..58_484);
    for mjd in days {
        let (y, m, d) = civil_from_mjd(mjd).unwrap();
        for (h, mi, s, atto) in [(12, 0, 0, 0), (23, 59, 59, 999_000_000_000_000_000)] {
            v.extend([UtcDateTime::new(y, m, d, h, mi, s, atto).unwrap()]);
        }
    }
    v
}

/// 1600 .. 2200 TT, about 6 days apart, and instants near 0 and near the ends
/// of the f64-exact range.
fn test_tt_inputs() -> Vec<Instant<Tt>> {
    let mut v = Vec::new();
    let mut s = -12_622_780_800i64;
    let mut k = 0u64;
    while s < 6_311_390_400 {
        v.extend([Instant::<Tt>::new(s, k * 31_415_926_535_897 % ATTO_PER_SEC).unwrap()]);
        s += 530_000 + (k as i64 * 104_729) % 86_400;
        k += 1;
    }
    for s in [-1i64, 0, 1, (1 << 53) - 1, -(1 << 53) + 1] {
        for atto in [0, 1, ATTO_PER_SEC - 1] {
            v.extend([Instant::<Tt>::new(s, atto).unwrap()]);
        }
    }
    v
}

fn utc_input_bytes(v: &[UtcDateTime]) -> Vec<u8> {
    let mut out = Vec::new();
    for u in v {
        out.extend(u.year.to_le_bytes());
        out.extend([u.month, u.day, u.hour, u.minute, u.second]);
        out.extend(u.atto.to_le_bytes());
    }
    out
}

fn tt_input_bytes(v: &[Instant<Tt>]) -> Vec<u8> {
    v.iter()
        .flat_map(|t| t.total_attoseconds().to_le_bytes())
        .collect()
}

// ---- outputs ---------------------------------------------------------------

/// A stable code for each error (the Debug text is not part of a Law).
fn error_code(e: TimeError) -> u8 {
    match e {
        TimeError::OutOfRange => 1,
        TimeError::InvalidDate => 2,
        TimeError::InvalidTime => 3,
        TimeError::NotALeapSecond => 4,
        TimeError::BeforeUtc => 5,
        TimeError::LeapTableExpired { .. } => 6,
        _ => 255,
    }
}

fn utc_tai_outputs(inputs: &[UtcDateTime]) -> Vec<u8> {
    let mut out = Vec::new();
    for dt in inputs {
        match dt.to_tai(ExpiryPolicy::Strict) {
            Ok(c) => {
                out.extend([0]);
                out.extend(c.value.total_attoseconds().to_le_bytes());
                let back = UtcDateTime::from_tai(c.value, ExpiryPolicy::Strict).unwrap();
                out.extend(utc_input_bytes(&[back.value]));
            }
            Err(e) => out.extend([error_code(e)]),
        }
    }
    out
}

fn tt_tai_outputs(inputs: &[Instant<Tt>]) -> Vec<u8> {
    let mut out = Vec::new();
    for &tt in inputs {
        let tai: Instant<Tai> = tt.to_tai().unwrap();
        out.extend(tai.total_attoseconds().to_le_bytes());
        out.extend(tai.to_tt().unwrap().total_attoseconds().to_le_bytes());
    }
    out
}

fn tdb_outputs(model: TdbModel, inputs: &[Instant<Tt>]) -> Vec<u8> {
    let mut out = Vec::new();
    for &tt in inputs {
        out.extend(model.tdb_minus_tt(tt).to_bits().to_le_bytes());
        // far from J2000 Fb1990's shift reaches 1 s and the conversion is
        // OutOfRange: recorded as its code
        match tt.to_tdb(model) {
            Ok(tdb) => {
                out.extend(tdb.total_attoseconds().to_le_bytes());
                out.extend(tdb.to_tt(model).unwrap().total_attoseconds().to_le_bytes());
            }
            Err(e) => out.extend([error_code(e)]),
        }
    }
    out
}

fn julian_date_outputs(inputs: &[Instant<Tt>]) -> Vec<u8> {
    let mut out = Vec::new();
    for &tt in inputs {
        let jd = tt.to_jd2();
        out.extend(jd.jd1.to_bits().to_le_bytes());
        out.extend(jd.jd2.to_bits().to_le_bytes());
        // the canonical split, a shifted one and one with a negative fraction
        for (a, b) in [(0.0, 0.0), (-0.5, 0.5), (1.0, -1.0)] {
            let split = Jd2 {
                jd1: jd.jd1 + a,
                jd2: jd.jd2 + b,
            };
            out.extend(
                Instant::<Tt>::from_jd2(split)
                    .unwrap()
                    .total_attoseconds()
                    .to_le_bytes(),
            );
        }
        let m = tt.to_mjd();
        out.extend(m.day.to_le_bytes());
        out.extend(m.second_of_day.to_le_bytes());
    }
    out
}

type Case = (&'static str, Vec<u8>, &'static str);

fn check(kind: &str, cases: &[Case]) {
    let mut bad = Vec::new();
    for (name, bytes, want) in cases {
        assert!(!bytes.is_empty(), "{kind} {name}: no output");
        let got = digest(bytes);
        if got != *want {
            bad.extend([format!("const {kind}_{name}: &str = \"{got}\";")]);
        }
    }
    assert!(
        bad.is_empty(),
        "{kind} digests changed:\n{}",
        bad.join("\n")
    );
}

// ---- tests -----------------------------------------------------------------

#[test]
fn reference_grids_are_frozen() {
    let utc = reference_utc_inputs();
    let tt = reference_tt_inputs();
    let got = [
        (
            "UTC_INPUTS",
            digest(&utc_input_bytes(&utc)),
            utc.len(),
            REFERENCE_UTC_INPUTS,
        ),
        (
            "TT_INPUTS",
            digest(&tt_input_bytes(&tt)),
            tt.len(),
            REFERENCE_TT_INPUTS,
        ),
    ];
    let mut bad = Vec::new();
    for (name, h, n, want) in got {
        assert!(n > 1000, "{name}: {n}");
        if (h.as_str(), n) != want {
            bad.extend([format!(
                "const REFERENCE_{name}: (&str, usize) = (\"{h}\", {n});"
            )]);
        }
    }
    assert!(
        bad.is_empty(),
        "a reference grid changed (it is part of the Law identifiers):\n{}",
        bad.join("\n")
    );
}

#[test]
fn reference_digests() {
    let utc = reference_utc_inputs();
    let tt = reference_tt_inputs();
    check(
        "REFERENCE",
        &[
            ("UTC_TAI", utc_tai_outputs(&utc), REFERENCE_UTC_TAI),
            ("TT_TAI", tt_tai_outputs(&tt), REFERENCE_TT_TAI),
            (
                "TDB_SPICE_ET",
                tdb_outputs(TdbModel::SpiceEt, &tt),
                REFERENCE_TDB_SPICE_ET,
            ),
            (
                "TDB_FB1990",
                tdb_outputs(TdbModel::Fb1990, &tt),
                REFERENCE_TDB_FB1990,
            ),
            (
                "JULIAN_DATE",
                julian_date_outputs(&tt),
                REFERENCE_JULIAN_DATE,
            ),
        ],
    );
}

#[test]
fn test_grid_digests() {
    let utc = test_utc_inputs();
    let tt = test_tt_inputs();
    assert!(utc.len() > 10_000 && tt.len() > 30_000);
    check(
        "TEST",
        &[
            ("UTC_TAI", utc_tai_outputs(&utc), TEST_UTC_TAI),
            ("TT_TAI", tt_tai_outputs(&tt), TEST_TT_TAI),
            (
                "TDB_SPICE_ET",
                tdb_outputs(TdbModel::SpiceEt, &tt),
                TEST_TDB_SPICE_ET,
            ),
            (
                "TDB_FB1990",
                tdb_outputs(TdbModel::Fb1990, &tt),
                TEST_TDB_FB1990,
            ),
            ("JULIAN_DATE", julian_date_outputs(&tt), TEST_JULIAN_DATE),
        ],
    );
}

/// SHA-256 over a fixed prefix, the Law's name, its data, the reference grid's
/// input pin and the reference output digest.
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

    let tt = TT_MINUS_TAI.total_attoseconds().to_le_bytes().to_vec();

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
        (law::UTC_TAI, utc, REFERENCE_UTC_INPUTS, REFERENCE_UTC_TAI),
        (law::TT_TAI, tt, REFERENCE_TT_INPUTS, REFERENCE_TT_TAI),
        (
            law::TDB_SPICE_ET,
            spice,
            REFERENCE_TT_INPUTS,
            REFERENCE_TDB_SPICE_ET,
        ),
        (
            law::TDB_FB1990,
            fb,
            REFERENCE_TT_INPUTS,
            REFERENCE_TDB_FB1990,
        ),
        (
            law::JULIAN_DATE,
            jd,
            REFERENCE_TT_INPUTS,
            REFERENCE_JULIAN_DATE,
        ),
    ];
    let mut bad = Vec::new();
    for (l, data, grid, reference) in &cases {
        let id = law_id(l, data, *grid, reference);
        if id != l.semantics_id {
            let bytes: Vec<String> = id.iter().map(|b| format!("0x{b:02x}")).collect();
            bad.extend([format!("{}: [{}]", l.name, bytes.join(", "))]);
        }
    }
    assert!(bad.is_empty(), "semantics ids changed:\n{}", bad.join("\n"));
    let mut ids: Vec<[u8; 32]> = law::ALL.iter().map(|l| l.semantics_id).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), law::ALL.len());
    assert_eq!(TdbModel::SpiceEt.law(), law::TDB_SPICE_ET);
    assert_eq!(TdbModel::Fb1990.law(), law::TDB_FB1990);
}
