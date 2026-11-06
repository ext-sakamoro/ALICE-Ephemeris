#!/usr/bin/env python3
"""Tests of wasm_path_scan.py: each kind of build path is found; a clean module passes; nothing
scanned, or an empty module, fails."""
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import wasm_path_scan as w


class Scan(unittest.TestCase):
    def run_on(self, data):
        with tempfile.TemporaryDirectory() as d:
            p = Path(d) / "m.wasm"
            p.write_bytes(data)
            return w.main([str(p)])

    def test_clean_passes(self):
        self.assertEqual(self.run_on(b"\0asm/src/lib.rs/cargo/registry/x.rs/rustc/abc/library"), 0)

    def test_each_path_kind_fails(self):
        for s in [b"/Users/", b"/home/", b"/private/tmp/a", b"/var/folders/a",
                  b"x/.cargo/git/a.rs", b"x/.rustup/a.rs", b"/rust/lib/rustlib/src/rust/library/x.rs", b"C:\\Users\\x\\a.rs", b"C:\\\\Users\\\\x"]:
            self.assertEqual(self.run_on(b"\0asm" + s), 1, s)

    def test_nothing_scanned_fails(self):
        self.assertEqual(w.main([]), 1)

    def test_empty_module_fails(self):
        self.assertEqual(self.run_on(b""), 1)


if __name__ == "__main__":
    unittest.main()
