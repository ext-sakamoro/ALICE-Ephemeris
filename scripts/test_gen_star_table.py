#!/usr/bin/env python3
"""The star table generator on written records: not real stars, only the columns.

Author: Moroya Sakamoto
"""

import importlib.util
import math
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location("g", Path(__file__).with_name("gen_star_table.py"))
g = importlib.util.module_from_spec(spec)
spec.loader.exec_module(g)


def record(rah="06", ram="30", ras="00.0", sign="+", ded="45", dem="30", des="00", mag=" 1.00", pmra="+0.100", pmde="-0.200"):
    line = [" "] * 197
    def put(cols, text):
        line[cols[0] - 1 : cols[1]] = list(text.rjust(cols[1] - cols[0] + 1))
    put(g.RAH, rah); put(g.RAM, ram); put(g.RAS, ras); put(g.DESIGN, sign); put(g.DED, ded); put(g.DEM, dem); put(g.DES, des)
    put(g.VMAG, mag); put(g.PM_RA, pmra); put(g.PM_DEC, pmde)
    return "".join(line)


class Read(unittest.TestCase):
    def test_the_columns_read_to_radians(self):
        ra, dec, pa, pd, m = g.read(record())
        self.assertAlmostEqual(ra, 6.5 * math.pi / 12)
        self.assertAlmostEqual(dec, 45.5 * math.pi / 180)
        self.assertAlmostEqual(pa, 0.1 * math.pi / 180 / 3600)
        self.assertAlmostEqual(pd, -0.2 * math.pi / 180 / 3600)
        self.assertEqual(m, 1.0)

    def test_south_is_negative(self):
        self.assertLess(g.read(record(sign="-"))[1], 0)

    def test_no_position_or_no_magnitude_is_left_out(self):
        self.assertIsNone(g.read(record(rah="  ")))
        self.assertIsNone(g.read(record(mag="     ")))

    def test_what_does_not_read_stops(self):
        with self.assertRaises(ValueError):
            g.read(record(ram="xx"))
        with self.assertRaises(KeyError):
            g.read(record(sign="?"))


class Brightest(unittest.TestCase):
    def test_the_brightest_come_first_and_the_rest_are_left(self):
        lines = [record(mag=m) for m in [" 3.00", "-1.00", " 2.00", " 5.00"]]
        self.assertEqual([s[4] for s in g.brightest(lines, 2)], [-1.0, 2.0])

    def test_too_few_or_none_stops(self):
        with self.assertRaises(ValueError):
            g.brightest([record()], 2)
        with self.assertRaises(ValueError):
            g.brightest([], 0)

    def test_the_same_records_give_the_same_table_in_any_order(self):
        lines = [record(ram=f"{k:02}", mag=" 1.00") for k in range(5)]
        self.assertEqual(g.brightest(lines, 3), g.brightest(list(reversed(lines)), 3))


class Table(unittest.TestCase):
    def test_the_table_names_its_source_and_its_length(self):
        text = g.table(g.brightest([record()], 1), "a written record")
        self.assertIn("// Source: a written record", text)
        self.assertIn("BRIGHTEST: [Star; 1]", text)

    def test_no_source_is_refused(self):
        self.assertEqual(g.main(["x", "1", " ", "y"]), 2)


if __name__ == "__main__":
    unittest.main()
