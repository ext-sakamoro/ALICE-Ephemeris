#!/usr/bin/env python3
"""Tests of scripts/mutants_equivalent_check.py on hand-made cargo-mutants
output directories (no cargo run). Each case states the exit code it needs."""

import json
import pathlib
import shutil
import subprocess
import sys
import tempfile

HERE = pathlib.Path(__file__).resolve().parent
CHECK = HERE / "mutants_equivalent_check.py"

SURVIVOR = "src/jd.rs:111:16: replace < with <= in floor_i64"
LISTED = "src/jd.rs: replace < with <= in floor_i64 | same result on both paths"


def run_dir(root: pathlib.Path, name: str, tested: list, missed: list, timeouts=()) -> pathlib.Path:
    out = root / name / "mutants.out"
    out.mkdir(parents=True)
    outcomes = [{"scenario": "Baseline"}] + [{"scenario": {"Mutant": {"file": f}}} for f in tested]
    (out / "outcomes.json").write_text(json.dumps({"outcomes": outcomes}), encoding="utf-8")
    (out / "missed.txt").write_text("".join(m + "\n" for m in missed), encoding="utf-8")
    (out / "timeout.txt").write_text("".join(t + "\n" for t in timeouts), encoding="utf-8")
    return out.parent


def check(listing: str, *dirs: pathlib.Path) -> int:
    with tempfile.TemporaryDirectory() as tmp:
        scripts = pathlib.Path(tmp)
        shutil.copy(CHECK, scripts / CHECK.name)
        (scripts / "mutants-equivalent.txt").write_text(listing, encoding="utf-8")
        r = subprocess.run([sys.executable, str(scripts / CHECK.name), *map(str, dirs)],
                           capture_output=True, text=True)
        return r.returncode


def main() -> int:
    failures = 0
    with tempfile.TemporaryDirectory() as tmp:
        root = pathlib.Path(tmp)
        jd = run_dir(root, "jd", ["src/jd.rs"] * 5, [SURVIVOR])
        clean = run_dir(root, "clean", ["src/jd.rs"] * 5, [])
        utc = run_dir(root, "utc", ["src/utc.rs"] * 5, [], ["src/utc.rs:1:1: replace x with y in f"])
        empty = run_dir(root, "empty", [], [])
        cases = [
            ("listed survivor passes", "# c\n" + LISTED + "\n", [jd], 0),
            ("unlisted survivor is a gap", "none\n", [jd], 1),
            ("listed entry no longer surviving is stale", LISTED + "\n", [clean], 1),
            ("entry for an untested file is not stale", LISTED + "\n", [utc], 0),
            ("shards are unioned", LISTED + "\n", [utc, jd], 0),
            ("no survivor and `none` passes", "none\n", [clean], 0),
            ("comments only fails", "# only a comment\n", [clean], 1),
            ("`none` with entries fails", "none\n" + LISTED + "\n", [jd], 1),
            ("entry without a reason fails", "src/jd.rs: replace < with <= in floor_i64\n", [jd], 1),
            ("nothing examined fails", "none\n", [empty], 1),
            ("no directory given fails", "none\n", [], 2),
            ("list only: a listed entry passes", LISTED + "\n", ["--list-only"], 0),
            ("list only: `none` passes", "none\n", ["--list-only"], 0),
            ("list only: comments only fails", "# c\n", ["--list-only"], 1),
            ("list only: entry without a reason fails", "src/jd.rs: x\n", ["--list-only"], 1),
        ]
        for name, listing, dirs, want in cases:
            got = check(listing, *dirs)
            ok = got == want
            failures += 0 if ok else 1
            print(f"{'ok  ' if ok else 'FAIL'} {name}: exit {got} (want {want})")
    print(f"{len(cases) - failures} of {len(cases)} cases")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
