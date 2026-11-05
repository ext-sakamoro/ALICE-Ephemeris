//! The SHA-256 helpers of the Law identifier checks (tests/law_ids*.rs), shared by the native
//! test runs and this browser build: the test files import them as
//! `alice_ephemeris_browser::law_digest`, so both runs hash with the same code.
//!
//! Author: Moroya Sakamoto

use sha2::{Digest, Sha256};

/// Lower-case hex of `b`.
#[must_use]
pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// SHA-256 of `bytes`, as lower-case hex.
#[must_use]
pub fn digest(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// SHA-256 over a fixed prefix, the Law's name, its data, the reference grid's input pin and
/// the reference output digest (the rule every Law identifier in `law::ALL` follows).
#[must_use]
pub fn law_id(name: &str, data: &[u8], grid: (&str, usize), reference: &str) -> [u8; 32] {
    let mut h = Hasher::new();
    h.update(b"alice-ephemeris law v1\0");
    h.update(name.as_bytes());
    h.update([0]);
    h.update((data.len() as u64).to_le_bytes());
    h.update(data);
    h.update(grid.0.as_bytes());
    h.update((grid.1 as u64).to_le_bytes());
    h.update(reference.as_bytes());
    h.finalize()
}

/// An incremental SHA-256 that counts the bytes it consumed.
#[derive(Default)]
pub struct Hasher {
    sha: Sha256,
    len: u64,
}

impl Hasher {
    /// An empty hash.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Append `bytes`.
    pub fn update(&mut self, bytes: impl AsRef<[u8]>) {
        let bytes = bytes.as_ref();
        self.len += bytes.len() as u64;
        self.sha.update(bytes);
    }

    /// Number of bytes appended so far (what a check compared, as opposed to how many times
    /// its loop ran).
    #[must_use]
    pub fn len(&self) -> u64 {
        self.len
    }

    /// Nothing appended yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The digest of everything appended.
    #[must_use]
    pub fn finalize(self) -> [u8; 32] {
        self.sha.finalize().into()
    }
}
