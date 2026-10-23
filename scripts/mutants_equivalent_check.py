#!/usr/bin/env python3
"""Compare cargo-mutants survivors with scripts/mutants-equivalent.txt.

usage: python3 scripts/mutants_equivalent_check.py <mutants.out dir> [<dir> ...]
       python3 scripts/mutants_equivalent_check.py --list-only

Each directory is one cargo-mutants run (a shard or a run limited with -f);
together they say which mutants were tested. Fails (exit 1) when
- a surviving mutant (missed.txt) is not listed: a gap in the tests;
- a listed entry is for a file these runs mutated but no longer survives:
  a stale entry, which would hide a future gap at the same site;
- the list has no entry (write the single line `none` when there is no
  equivalent survivor), or the runs examined no mutant at all.
Timeouts are reported as unverified and do not pass as caught.

--list-only checks the list alone (the rules above that need no run), for
CI, where no mutants run.
"""

import json
import pathlib
import re
import sys

LIST = pathlib.Path(__file__).resolve().parent / "mutants-equivalent.txt"
SITE = re.compile(r"^(?P<file>[^:]+):\d+:\d+: (?P<what>.*)$")


def key(line: str) -> str:
    m = SITE.match(line.strip())
    if not m:
        raise SystemExit(f"cannot read the mutant line: {line!r}")
    return f"{m['file']}: {m['what']}"


def main() -> int:
    list_only = sys.argv[1:] == ["--list-only"]
    dirs = [] if list_only else [pathlib.Path(a) for a in sys.argv[1:]]
    if not dirs and not list_only:
        print(__doc__)
        return 2
    entries = {}
    none = False
    for raw in LIST.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        if line == "none":
            none = True
            continue
        mutant, sep, reason = line.partition(" | ")
        if not sep or not reason.strip():
            raise SystemExit(f"{LIST.name}: an entry needs ' | <reason>': {line!r}")
        entries[mutant.strip()] = reason.strip()
    if not entries and not none:
        print(f"{LIST.name}: no entry (write `none` if there is no equivalent survivor)")
        return 1
    if entries and none:
        print(f"{LIST.name}: `none` together with entries")
        return 1
    if list_only:
        print(f"{LIST.name}: {len(entries)} entr{'y' if len(entries) == 1 else 'ies'}" if entries else f"{LIST.name}: none")
        return 0

    examined, mutated_files, missed, timeouts = 0, set(), set(), []
    for d in dirs:
        out = d / "mutants.out" if (d / "mutants.out").is_dir() else d
        # the mutants actually tested (a stopped run lists more in mutants.json)
        for o in json.loads((out / "outcomes.json").read_text(encoding="utf-8"))["outcomes"]:
            sc = o.get("scenario")
            if isinstance(sc, dict) and "Mutant" in sc:
                mutated_files.add(sc["Mutant"]["file"])
                examined += 1
        missed |= {key(l) for l in (out / "missed.txt").read_text(encoding="utf-8").splitlines() if l.strip()}
        timeouts += [l for l in (out / "timeout.txt").read_text(encoding="utf-8").splitlines() if l.strip()]
    if examined == 0:
        print("no mutant examined")
        return 1

    gaps = sorted(missed - entries.keys())
    stale = sorted(e for e in entries if e.split(":", 1)[0] in mutated_files and e not in missed)
    print(f"examined {examined} mutants in {len(dirs)} run(s); survivors {len(missed)}, "
          f"listed equivalent {len(entries)}, unverified (timeout) {len(timeouts)}")
    for t in timeouts:
        print(f"  unverified: {t}")
    for g in gaps:
        print(f"  GAP (survivor not listed): {g}")
    for s in stale:
        print(f"  STALE (listed, but no longer survives): {s}")
    return 1 if gaps or stale else 0


if __name__ == "__main__":
    sys.exit(main())
