#!/usr/bin/env python3
"""Tests for scripts/machete_metadata_check.py: no manifest may tell cargo-machete to ignore
a dependency (a dependency it cannot see the use of has to be made visible, not listed)."""
from __future__ import annotations

import contextlib
import io
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import machete_metadata_check  # noqa: E402

CLEAN = '[package]\nname = "a"\n\n[dependencies]\nsha2 = "0.10"\n'


def run(files: dict[str, str]) -> tuple[int, str]:
    with tempfile.TemporaryDirectory() as d:
        for rel, text in files.items():
            p = Path(d, rel)
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(text, encoding="utf-8")
        out = io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(out):
            rc = machete_metadata_check.main([d])
        return rc, out.getvalue()


class MacheteMetadataCheck(unittest.TestCase):
    def test_clean_manifests_pass_and_are_counted(self) -> None:
        rc, out = run({"Cargo.toml": CLEAN, "browser/Cargo.toml": CLEAN})
        self.assertEqual(rc, 0, out)
        self.assertIn("2 manifest", out)

    def test_table_header_fails(self) -> None:
        bad = CLEAN + '\n[package.metadata.cargo-machete]\nignored = ["sha2"]\n'
        rc, out = run({"Cargo.toml": CLEAN, "browser/Cargo.toml": bad})
        self.assertEqual(rc, 1, out)
        self.assertIn("browser/Cargo.toml", out)

    def test_workspace_table_fails(self) -> None:
        rc, out = run({"Cargo.toml": CLEAN + '\n[workspace.metadata.cargo-machete]\nignored = ["x"]\n'})
        self.assertEqual(rc, 1, out)

    def test_inline_key_under_metadata_fails(self) -> None:
        rc, out = run({"Cargo.toml": CLEAN + '\n[package.metadata]\ncargo-machete = { ignored = ["sha2"] }\n'})
        self.assertEqual(rc, 1, out)

    def test_dotted_key_fails(self) -> None:
        rc, out = run({"Cargo.toml": CLEAN + '\n[package]\nmetadata.cargo-machete.ignored = ["sha2"]\n'})
        self.assertEqual(rc, 1, out)

    def test_a_comment_naming_it_passes(self) -> None:
        rc, out = run({"Cargo.toml": CLEAN + "# cargo-machete reads only src/\n"})
        self.assertEqual(rc, 0, out)

    def test_build_output_is_not_read(self) -> None:
        bad = CLEAN + '\n[package.metadata.cargo-machete]\nignored = ["sha2"]\n'
        rc, out = run({"Cargo.toml": CLEAN, "target/x/Cargo.toml": bad})
        self.assertEqual(rc, 0, out)

    def test_no_manifest_fails(self) -> None:
        rc, out = run({"README.md": "x\n"})
        self.assertEqual(rc, 1, out)
        self.assertIn("compared nothing", out)

    def test_this_repository_passes(self) -> None:
        out = io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(out):
            rc = machete_metadata_check.main([str(Path(__file__).resolve().parent.parent)])
        self.assertEqual(rc, 0, out.getvalue())


if __name__ == "__main__":
    unittest.main()
