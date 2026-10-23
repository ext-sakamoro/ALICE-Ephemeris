//! Reader for the reference tables in tests/golden (written by
//! scripts/gen_time_golden.py from ERFA, SPICE and exact rationals).

#![allow(dead_code)]

/// Data rows of `tests/golden/<name>` as string fields, header and `#` lines
/// skipped. Panics when the file has no data row.
pub fn rows(name: &str) -> Vec<Vec<String>> {
    let path = format!("{}/tests/golden/{name}", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut lines = text.lines().filter(|l| !l.starts_with('#'));
    let header = lines.next().unwrap_or_else(|| panic!("{path}: no header"));
    let width = header.split(',').count();
    let out: Vec<Vec<String>> = lines
        .map(|l| {
            let f: Vec<String> = l.split(',').map(str::to_owned).collect();
            assert_eq!(f.len(), width, "{path}: {l}");
            f
        })
        .collect();
    assert!(!out.is_empty(), "{path}: no data rows");
    out
}

pub fn i64_(s: &str) -> i64 {
    s.parse().unwrap_or_else(|e| panic!("{s}: {e}"))
}

pub fn u64_(s: &str) -> u64 {
    s.parse().unwrap_or_else(|e| panic!("{s}: {e}"))
}

pub fn u8_(s: &str) -> u8 {
    s.parse().unwrap_or_else(|e| panic!("{s}: {e}"))
}

pub fn f64_(s: &str) -> f64 {
    s.parse().unwrap_or_else(|e| panic!("{s}: {e}"))
}

/// Distance in units in the last place between two finite f64 of one sign.
pub fn ulps(a: f64, b: f64) -> u64 {
    let (x, y) = (a.to_bits() as i64, b.to_bits() as i64);
    x.abs_diff(y)
}
