#!/usr/bin/env python3
"""The star table generator on written rows: not real stars, only the format.

Author: Moroya Sakamoto
"""

import importlib.util
import math
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location("g", Path(__file__).with_name("gen_star_table.py"))
g = importlib.util.module_from_spec(spec)
spec.loader.exec_module(g)


def row(hr, ra="97.5", dec="45.5", pmra="0.1", pmdec="-0.2", vmag="1.0"):
    return {"hr": str(hr), "ra": ra, "dec": dec, "pmra": pmra, "pmdec": pmdec, "vmag": vmag}


def not_stars():
    return [row(hr, vmag="") for hr in sorted(g.NOT_STARS)]


def tdat(rows_, total=None):
    fields = ["hr", "ra", "dec", "pmra", "pmdec", "vmag"]
    head = f"<HEADER>\\n#    TOTAL ROWS: {len(rows_) if total is None else total}\\nline[1] = {' '.join(fields)}\\n<DATA>\\n"
    return head.replace("\\n", "\n") + "\n".join("|".join(r[f] for f in fields) + "|" for r in rows_)


class Rows(unittest.TestCase):
    def test_a_table_reads_and_its_count_is_checked(self):
        self.assertEqual(len(g.rows(tdat([row(1), row(2)]))), 2)
        with self.assertRaises(ValueError):
            g.rows(tdat([row(1)], total=2))

    def test_what_is_not_a_table_stops(self):
        with self.assertRaises(ValueError):
            g.rows("nothing here")


class Star(unittest.TestCase):
    def test_a_row_reads_to_radians(self):
        ra, dec, pa, pd, m = g.star(row(1))
        self.assertAlmostEqual(ra, 97.5 * math.pi / 180)
        self.assertAlmostEqual(dec, 45.5 * math.pi / 180)
        self.assertAlmostEqual(pa, 0.1 * math.pi / 180 / 3600)
        self.assertAlmostEqual(pd, -0.2 * math.pi / 180 / 3600)
        self.assertEqual(m, 1.0)

    def test_no_proper_motion_is_none_and_no_magnitude_is_no_star(self):
        self.assertEqual(g.star(row(1, pmra="", pmdec=""))[2:4], (0.0, 0.0))
        self.assertIsNone(g.star(row(1, vmag=" ")))

    def test_what_does_not_read_stops(self):
        with self.assertRaises(ValueError):
            g.star(row(1, ra="x"))


class Brightest(unittest.TestCase):
    def test_the_brightest_come_first(self):
        t = [row(i + 1, vmag=m) for i, m in enumerate(["3.0", "-1.0", "2.0", "5.0"])] + not_stars()
        self.assertEqual([s[4] for s in g.brightest(t, 2)], [-1.0, 2.0])

    def test_the_fourteen_that_are_not_stars_are_left_out(self):
        t = [row(1)] + not_stars()
        self.assertEqual(len(g.brightest(t, 1)), 1)

    def test_a_star_with_no_magnitude_or_a_missing_one_of_the_fourteen_stops(self):
        with self.assertRaises(ValueError):
            g.brightest([row(1, vmag="")] + not_stars(), 0 + 1)
        with self.assertRaises(ValueError):
            g.brightest([row(1)] + not_stars()[1:], 1)
        # one of the fourteen with a magnitude would be drawn as a star
        t = [row(1)] + not_stars()
        t[1]["vmag"] = "5.0"
        with self.assertRaises(ValueError):
            g.brightest(t, 1)

    def test_too_few_stops(self):
        with self.assertRaises(ValueError):
            g.brightest([row(1)] + not_stars(), 2)

    def test_the_same_rows_give_the_same_result_in_any_order(self):
        t = [row(i + 1, ra=str(10 * i), vmag="1.0") for i in range(5)] + not_stars()
        self.assertEqual(g.brightest(t, 3), g.brightest(list(reversed(t)), 3))


class Table(unittest.TestCase):
    def test_the_table_names_source_identifier_licence_date_and_citation(self):
        text = g.table(g.brightest([row(1)] + not_stars(), 1), "2026-11-06")
        for needed in ["heasarc.gsfc.nasa.gov/W3Browse/star-catalog/bsc5p.html", "ivo://nasa.heasarc/bsc5p", "usa.gov/government-works", "retrieved 2026-11-06", "Hoffleit", "HEASARC", "STAR_COUNT: usize = 1", "does not imply endorsement"]:
            self.assertIn(needed, text)

    def test_no_date_is_refused(self):
        self.assertEqual(g.main(["x", "1", "soon", "y", "z"]), 2)

    def test_the_binary_is_the_catalogue_decimals_in_columns_14_bytes_a_star(self):
        # two stars as a row of the catalogue reads (degrees, arcsec a year, magnitude)
        rows_ = [g.star(row(1, ra="101.2872", dec="-16.7161", pmra="-0.553", pmdec="-1.205", vmag="-1.46")),
                 g.star(row(2, ra="0.5", dec="45.5", pmra="0.1", pmdec="0", vmag="6.0"))]
        b = g.binary(rows_)
        self.assertEqual(len(b), 2 * 14)
        # ra column first: 1012872 and 5000 in 1e-4 degree, little endian
        self.assertEqual(b[:8], (1012872).to_bytes(4, "little") + (5000).to_bytes(4, "little"))
        # the magnitudes last, in hundredths
        self.assertEqual(b[-4:], (-146).to_bytes(2, "little", signed=True) + (600).to_bytes(2, "little", signed=True))
        # read back, each value is the same float as the row gave, bit for bit
        back = g.read_binary(b, 2)
        for got, want in zip(back, rows_):
            self.assertEqual([x.hex() for x in got[:4]], [x.hex() for x in want[:4]])
            self.assertEqual(got[4], g.f32(want[4]))

    def test_a_value_finer_than_the_catalogue_decimals_stops(self):
        with self.assertRaises(ValueError):
            g.binary([(1.0, 0.0, 0.0, 0.0, 1.0)])
        with self.assertRaises(ValueError):
            g.binary([(0.0, 0.0, 0.0, 0.0, 1.005)])

    def test_a_file_of_the_wrong_length_stops(self):
        with self.assertRaises(ValueError):
            g.read_binary(b"\0" * 27, 2)

    def test_the_brightest_of_a_file_are_what_binary_writes_for_their_rows(self):
        # five stars, brightest first: the first n of each column are the file binary writes for the first n rows
        rows_ = [g.star(row(k, ra=f"{10 * k}.5", dec=f"-{k}.25", pmra=f"0.{k}", pmdec="-0.2", vmag=f"{k - 1}.5"))
                 for k in range(1, 6)]
        full = g.binary(rows_)
        compared = 0
        for n in range(1, 5):
            self.assertEqual(g.brightest_of(full, 5, n), g.binary(rows_[:n]), n)
            compared += 1
        self.assertEqual(compared, 4)
        # n of none, of all and of more, and a file of the wrong length, are refused
        for n in (0, 5, 6):
            with self.assertRaises(ValueError):
                g.brightest_of(full, 5, n)
        with self.assertRaises(ValueError):
            g.brightest_of(full[:-1], 5, 2)
        # how many are no fainter than a magnitude: 0.5, 1.5, 2.5 are at most 2.5; a file not brightest first stops
        self.assertEqual(g.bright_count(full, 5, 2.5), 3)
        with self.assertRaises(ValueError):
            g.bright_count(g.binary(rows_[::-1]), 5, 2.5)

    def test_the_committed_bright_table_is_the_brightest_of_the_committed_table(self):
        sky = Path(__file__).resolve().parent.parent / "sky"
        full, bright = (sky / "stars.bin").read_bytes(), (sky / "stars_bright.bin").read_bytes()
        count = len(full) // 14
        n = g.bright_count(full, count, 3.5)
        self.assertEqual(len(bright), n * 14)
        self.assertEqual(bright, g.brightest_of(full, count, n))
        # the next star is fainter than the cut
        mags = [m / 100 for m in __import__("struct").unpack_from(f"<{count}h", full, 12 * count)]
        self.assertLessEqual(mags[n - 1], 3.5)
        self.assertGreater(mags[n], 3.5)


if __name__ == "__main__":
    unittest.main()
