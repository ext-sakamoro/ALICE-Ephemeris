//! Semantics identifiers of the Earth orientation and frame Laws (EOP
//! interpolation, Earth rotation angle, TEME → ITRS, IAU 2006/2000A CIO).
//!
//! Same rule as tests/law_ids.rs: an identifier is the SHA-256 of the Law's
//! name, its data, the alice-det-math `SEMANTICS_ID` where it evaluates `f64`
//! transcendentals, and the digest of its outputs over a frozen reference grid
//! whose inputs are pinned by their own SHA-256 and count. The identifiers are
//! in src/law.rs (`law::ALL`).
//!
//! Author: Moroya Sakamoto

use alice_ephemeris::eop::{EopFlag, EopRow, EopTable};
use alice_ephemeris::frames::{self, TemeToItrs};
use alice_ephemeris::law::{self, Law, ERA_2000, IAU2006_CIO, TEME_ITRS, UT1_TAI};
use alice_ephemeris::{ExpiryPolicy, Instant, Jd2, Tai, TimeError, ATTO_PER_SEC};
use sha2::{Digest, Sha256};

/// ERFA v2.0.1 sources of the IAU 2000A and s series (scripts/gen_iau2006_tables.py).
const ERFA_SOURCES: &str = "9915ba38c9365f8b0738269b8c2ac1fdd5f8dee3 \
    a1538ec0277745db974ab76ca4b678a7602249bccecf9474e2b0268f9c2032ef \
    6ad19dd59c3150d9f065b3f76d254c1cf3adb0cb6bacd6422c1dc6b6f14122ea";

const REFERENCE_EOP_INPUTS: (&str, usize) = (
    "d5fba9e02e79ac58b526f8cfe0e8790fe6ca9a2cb8e00aa012041df96b4093da",
    325,
);
const REFERENCE_JD_INPUTS: (&str, usize) = (
    "a779da990612ba73f70dc3eb2ca19804d75f5ee0654927389b8f6a5895601a15",
    1014,
);
const REFERENCE_UT1_TAI: &str = "8c189f3400635b98ee3bf1f226f793b3ccb03db208deb73a55635332068833df";
const REFERENCE_ERA_2000: &str = "d6bda33064f34f2948c978564e6b84370e77d3453f967e0d52930c4a23a3a101";
const REFERENCE_TEME_ITRS: &str =
    "922d9553dc8200eff46deead1a075a2ead42e05dff5c49e51014c56788173dd1";
const REFERENCE_IAU2006_CIO: &str =
    "fe71c2f10c3823db3c22fa84413d8b7f44a2d6bae5ac81dc6bcebe93bff62975";

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn digest(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// A synthetic table of 40 days across the 2016-12-31 leap second: smooth
/// UT1 − UTC with the 1 s step, pole coordinates, dX dY on the first 30 rows,
/// the last 8 rows predicted.
fn reference_eop_rows() -> Vec<EopRow> {
    (0..40)
        .map(|i: i32| {
            let mjd = 57_734 + i64::from(i);
            let base = -4_000_000 + 13_000 * i - 97 * i * i;
            EopRow {
                mjd,
                ut1_minus_utc_e7: if mjd >= 57_754 {
                    base + 10_000_000
                } else {
                    base
                },
                ut1_flag: if i >= 32 {
                    EopFlag::Predicted
                } else {
                    EopFlag::Observed
                },
                xp_uas: 80_000 + 311 * i,
                yp_uas: 260_000 - 173 * i + 7 * i * i,
                polar_flag: if i >= 33 {
                    EopFlag::Predicted
                } else {
                    EopFlag::Observed
                },
                celestial_pole_offset_uas: (i < 30).then_some((100 - 3 * i, -50 + 2 * i)),
            }
        })
        .collect()
}

/// TAI instants from one day before the table to one day after it, every
/// 3 h 7 min plus a varying fraction, and the leap second itself.
fn reference_eop_queries() -> Vec<Instant<Tai>> {
    let first = (57_734 - 51_544) * 86_400 - 43_200 + 36;
    let mut v = Vec::new();
    let mut s = first - 86_400;
    let mut k = 0u64;
    while s < first + 41 * 86_400 {
        v.push(Instant::<Tai>::new(s, k * 1_234_567_890_123_457 % ATTO_PER_SEC).unwrap());
        s += 11_220;
        k += 1;
    }
    v.push(
        Instant::<Tai>::new(
            (57_754 - 51_544) * 86_400 - 43_200 + 36,
            500_000_000_000_000_000,
        )
        .unwrap(),
    );
    v
}

fn eop_input_bytes(rows: &[EopRow], queries: &[Instant<Tai>]) -> Vec<u8> {
    let mut out = Vec::new();
    for r in rows {
        out.extend(r.mjd.to_le_bytes());
        out.extend(r.ut1_minus_utc_e7.to_le_bytes());
        out.extend(r.xp_uas.to_le_bytes());
        out.extend(r.yp_uas.to_le_bytes());
        out.extend([
            u8::from(r.ut1_flag == EopFlag::Predicted),
            u8::from(r.polar_flag == EopFlag::Predicted),
        ]);
        match r.celestial_pole_offset_uas {
            Some((a, b)) => {
                out.push(1);
                out.extend(a.to_le_bytes());
                out.extend(b.to_le_bytes());
            }
            None => out.push(0),
        }
    }
    for q in queries {
        out.extend(q.total_attoseconds().to_le_bytes());
    }
    out
}

fn ut1_tai_outputs(rows: &[EopRow], queries: &[Instant<Tai>]) -> Vec<u8> {
    let table = EopTable::new(rows).unwrap();
    let mut out = Vec::new();
    for &q in queries {
        match table.at(q, ExpiryPolicy::Strict) {
            Ok(e) => {
                out.push(0);
                out.extend(e.ut1_minus_tai.total_attoseconds().to_le_bytes());
                out.extend(e.xp_arcsec.to_bits().to_le_bytes());
                out.extend(e.yp_arcsec.to_bits().to_le_bytes());
                match e.celestial_pole_offset_arcsec {
                    Some((a, b)) => {
                        out.push(1);
                        out.extend(a.to_bits().to_le_bytes());
                        out.extend(b.to_bits().to_le_bytes());
                    }
                    None => out.push(0),
                }
                out.extend([
                    u8::from(e.ut1_flag == EopFlag::Predicted),
                    u8::from(e.polar_flag == EopFlag::Predicted),
                ]);
                let (ut1, _) = q.to_ut1(&table, ExpiryPolicy::Strict).unwrap();
                out.extend(
                    ut1.to_tai(&table, ExpiryPolicy::Strict)
                        .map_or(-1, |t| t.total_attoseconds())
                        .to_le_bytes(),
                );
            }
            Err(TimeError::EopOutOfRange { .. }) => out.push(1),
            Err(_) => out.push(255),
        }
    }
    out
}

/// TT (and UT1) two-part dates 1800..2200 every 173 days plus a varying
/// fraction, both orders of the parts on every 5th.
fn reference_jd_inputs() -> Vec<Jd2> {
    let mut v = Vec::new();
    let mut day = -73_050.0;
    let mut k = 0u32;
    while day <= 73_050.0 {
        let frac = f64::from(k % 997) / 997.0;
        v.push(Jd2 {
            jd1: 2_451_545.0,
            jd2: day + frac,
        });
        if k % 5 == 0 {
            v.push(Jd2 {
                jd1: day + frac,
                jd2: 2_451_545.0,
            });
        }
        day += 173.0;
        k += 1;
    }
    v
}

fn jd_input_bytes(v: &[Jd2]) -> Vec<u8> {
    v.iter()
        .flat_map(|d| [d.jd1.to_bits().to_le_bytes(), d.jd2.to_bits().to_le_bytes()].concat())
        .collect()
}

fn mat_bytes(m: &frames::Mat3) -> Vec<u8> {
    m.iter()
        .flatten()
        .flat_map(|x| x.to_bits().to_le_bytes())
        .collect()
}

fn era_outputs(v: &[Jd2]) -> Vec<u8> {
    v.iter()
        .flat_map(|&d| frames::era00(d).to_bits().to_le_bytes())
        .collect()
}

fn teme_outputs(v: &[Jd2]) -> Vec<u8> {
    let mut out = Vec::new();
    for (k, &d) in v.iter().enumerate() {
        let xp = 1e-6 * (k % 7) as f64;
        let yp = -2e-6 + 1e-6 * (k % 5) as f64;
        let rot = TemeToItrs::new(d, d, xp, yp);
        out.extend(rot.gmst.to_bits().to_le_bytes());
        out.extend(mat_bytes(&rot.matrix));
        let (r, v) = rot.state([7000.0, -1200.0, 300.0], [1.0, 7.0, 0.5], 0.0015);
        for x in r.iter().chain(&v) {
            out.extend(x.to_bits().to_le_bytes());
        }
    }
    out
}

fn cio_outputs(v: &[Jd2]) -> Vec<u8> {
    let mut out = Vec::new();
    for (k, &d) in v.iter().enumerate() {
        let (dp, de) = frames::nut06a(d);
        out.extend(dp.to_bits().to_le_bytes());
        out.extend(de.to_bits().to_le_bytes());
        let c2i = frames::c2i06a(d);
        out.extend(mat_bytes(&c2i));
        let (x, y) = frames::bpn2xy(&frames::pnm06a(d));
        out.extend(frames::s06(d, x, y).to_bits().to_le_bytes());
        let xp = 1.5e-6 * ((k % 9) as f64 - 4.0) / 4.0;
        let yp = 2e-6 * ((k % 11) as f64 - 5.0) / 5.0;
        out.extend(mat_bytes(&frames::c2t06a(d, d, xp, yp)));
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
fn reference_grids_are_frozen() {
    let rows = reference_eop_rows();
    let q = reference_eop_queries();
    let jd = reference_jd_inputs();
    let got = [
        (
            "EOP_INPUTS",
            digest(&eop_input_bytes(&rows, &q)),
            q.len(),
            REFERENCE_EOP_INPUTS,
        ),
        (
            "JD_INPUTS",
            digest(&jd_input_bytes(&jd)),
            jd.len(),
            REFERENCE_JD_INPUTS,
        ),
    ];
    let mut bad = Vec::new();
    for (name, h, n, want) in got {
        assert!(n > 300, "{name}: {n}");
        if (h.as_str(), n) != want {
            bad.push(format!(
                "const REFERENCE_{name}: (&str, usize) = (\"{h}\", {n});"
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "a reference grid changed:\n{}",
        bad.join("\n")
    );
}

#[test]
fn reference_digests() {
    let rows = reference_eop_rows();
    let q = reference_eop_queries();
    let jd = reference_jd_inputs();
    let cases = [
        ("UT1_TAI", ut1_tai_outputs(&rows, &q), REFERENCE_UT1_TAI),
        ("ERA_2000", era_outputs(&jd), REFERENCE_ERA_2000),
        ("TEME_ITRS", teme_outputs(&jd), REFERENCE_TEME_ITRS),
        ("IAU2006_CIO", cio_outputs(&jd), REFERENCE_IAU2006_CIO),
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
    let mut ut1 = Vec::new();
    ut1.extend(4u64.to_le_bytes()); // rows in a Lagrange window
    ut1.extend(100_000_000_000u64.to_le_bytes()); // attoseconds per 1e-7 s
    let mut teme = frames::EARTH_ROTATION_RATE.to_bits().to_le_bytes().to_vec();
    teme.extend(alice_det_math::SEMANTICS_ID);
    let mut cio = ERFA_SOURCES.as_bytes().to_vec();
    cio.extend(alice_det_math::SEMANTICS_ID);
    let cases = [
        (UT1_TAI, ut1, REFERENCE_EOP_INPUTS, REFERENCE_UT1_TAI),
        (
            ERA_2000,
            Vec::new(),
            REFERENCE_JD_INPUTS,
            REFERENCE_ERA_2000,
        ),
        (TEME_ITRS, teme, REFERENCE_JD_INPUTS, REFERENCE_TEME_ITRS),
        (IAU2006_CIO, cio, REFERENCE_JD_INPUTS, REFERENCE_IAU2006_CIO),
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
    for l in [UT1_TAI, ERA_2000, TEME_ITRS, IAU2006_CIO] {
        assert!(law::ALL.contains(&l), "{} is not in law::ALL", l.name);
    }
}
