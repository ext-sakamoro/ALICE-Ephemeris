#!/usr/bin/env python3
"""Write sky/stars_bright.bin: the stars of sky/stars.bin no fainter than a magnitude, in its own layout (each column
as long as the stars it holds), for a page to draw before the whole table comes (the sky module's
stars_table_written_brightest)

    python3 scripts/gen_star_bright.py sky/stars.bin <count> <max mag> sky/stars_bright.bin

Author: Moroya Sakamoto
"""

import importlib.util
import sys
from pathlib import Path

spec = importlib.util.spec_from_file_location("g", Path(__file__).with_name("gen_star_table.py"))
g = importlib.util.module_from_spec(spec)
spec.loader.exec_module(g)


def main(argv):
    if len(argv) != 4:
        print(__doc__, file=sys.stderr)
        return 2
    src, count, max_mag, out = argv[0], int(argv[1]), float(argv[2]), argv[3]
    data = Path(src).read_bytes()
    n = g.bright_count(data, count, max_mag)
    Path(out).write_bytes(g.brightest_of(data, count, n))
    print(f"wrote the {n} stars of magnitude {max_mag} or brighter to {out}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
