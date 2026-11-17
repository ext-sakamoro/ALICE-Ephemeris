//! The bright stars' table (`sky/stars_bright.bin`, taken by `stars_table_written_brightest`) gives, for each of its
//! stars, the very bits the whole table gives for that star, at every input of `sky/expected.txt`, and NaN for the
//! stars it does not hold; a count of none or of all is refused, and the bright table is never taken as the whole
//! one. The whole table's outputs are the same before and after (its digest in `sky/expected.txt`).
//!
//! The sky module keeps its table in one static, so this file is a test binary of its own, one test.
//!
//! Author: Moroya Sakamoto

use sha2::{Digest, Sha256};

fn inputs() -> Vec<[f64; 3]> {
    include_str!("../expected.txt")
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty() && !l.contains("sha256"))
        .map(|l| {
            let v: Vec<f64> = l
                .split_whitespace()
                .map(|x| x.parse().expect("a number"))
                .collect();
            [v[0], v[1], v[2]]
        })
        .collect()
}

fn write(table: &[u8]) {
    unsafe {
        core::ptr::copy_nonoverlapping(
            table.as_ptr(),
            alice_ephemeris_sky::stars_table(),
            table.len(),
        );
    }
}

fn read(n: usize, at: [f64; 3]) -> Vec<f64> {
    let p = alice_ephemeris_sky::stars_alt_az_mag(at[0], at[1], at[2]);
    (0..n * 3).map(|i| unsafe { p.add(i).read() }).collect()
}

#[test]
fn the_bright_table_gives_the_whole_tables_bits_for_its_stars() {
    let n = alice_ephemeris_sky::stars_len() as usize;
    let full = include_bytes!("../stars.bin");
    let bright = include_bytes!("../stars_bright.bin");
    assert_eq!(bright.len() % 14, 0);
    let k = bright.len() / 14;
    assert!(k > 0 && k < n, "{k} bright stars of {n}");
    let at = inputs();
    assert_eq!(at.len(), 24);

    // the whole table's outputs
    write(full);
    assert_eq!(
        alice_ephemeris_sky::stars_table_written(full.len() as u32),
        1
    );
    let whole: Vec<Vec<f64>> = at.iter().map(|a| read(n, *a)).collect();

    // the bright table, never taken as the whole one; written over zeros, so nothing left of the whole table can stand
    // in for it
    write(&vec![0u8; full.len()]);
    write(bright);
    assert_eq!(
        alice_ephemeris_sky::stars_table_written(bright.len() as u32),
        0
    );
    assert!(read(n, at[0]).iter().all(|x| x.is_nan()));
    // a count of none or of all is refused, and leaves no table
    for bad in [0, n as u32, n as u32 + 1, u32::MAX] {
        assert_eq!(
            alice_ephemeris_sky::stars_table_written_brightest(bad),
            0,
            "{bad}"
        );
        assert!(read(n, at[0]).iter().all(|x| x.is_nan()), "{bad}");
    }
    assert_eq!(
        alice_ephemeris_sky::stars_table_written_brightest(k as u32),
        1
    );
    let mut compared = 0;
    for (a, w) in at.iter().zip(&whole) {
        let got = read(n, *a);
        for i in 0..k * 3 {
            assert_eq!(got[i].to_bits(), w[i].to_bits(), "{a:?} value {i}");
            compared += 1;
        }
        assert!(got[k * 3..].iter().all(|x| x.is_nan()), "{a:?}");
    }
    assert_eq!(compared, 24 * k * 3);

    // and the whole table again gives the digest pinned for it
    write(full);
    assert_eq!(
        alice_ephemeris_sky::stars_table_written(full.len() as u32),
        1
    );
    let pinned = include_str!("../expected.txt")
        .lines()
        .find_map(|l| l.strip_prefix("stars-sha256 "))
        .expect("the stars' digest");
    let mut h = Sha256::new();
    for a in &at {
        for x in read(n, *a) {
            h.update(x.to_le_bytes());
        }
    }
    let got: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(got, pinned);
}
