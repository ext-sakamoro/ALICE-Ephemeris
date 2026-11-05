#!/usr/bin/env python3
"""No Cargo.toml in the repository may tell cargo-machete to ignore a dependency.

cargo-machete reports a dependency it cannot see used; the answer is to make the use visible
(or remove the dependency), never to list it as ignored. This check fails on any
`cargo-machete` key or table under `package.metadata` / `workspace.metadata` (table header,
dotted key, or inline key), in every Cargo.toml below the root except build output, and fails
when it finds no manifest (compared nothing).

usage: scripts/machete_metadata_check.py [root]
"""
from __future__ import annotations

import os
import re
import sys
from pathlib import Path

SKIP_DIRS = {"target", ".git", "node_modules"}
HEADER = re.compile(r"^\s*\[\s*(package|workspace)\s*\.\s*metadata\s*\.\s*\"?cargo-machete\"?\s*[\].]")
METADATA = re.compile(r"^\s*\[\s*(package|workspace)\s*\.\s*metadata\s*\]")
TABLE = re.compile(r"^\s*\[")
KEY = re.compile(r"^\s*\"?cargo-machete\"?\s*[.=]")
DOTTED = re.compile(r"^\s*metadata\s*\.\s*\"?cargo-machete\"?\s*[.=]")


def findings(text: str) -> list[int]:
    out = []
    section = ""
    for n, line in enumerate(text.splitlines(), 1):
        code = line.split("#", 1)[0]
        if not code.strip():
            continue
        if HEADER.match(code):
            out.append(n)
            section = "machete"
            continue
        if TABLE.match(code):
            section = "metadata" if METADATA.match(code) else (
                "package" if re.match(r"^\s*\[\s*(package|workspace)\s*\]", code) else "other")
            continue
        if section == "metadata" and KEY.match(code):
            out.append(n)
        elif section == "package" and DOTTED.match(code):
            out.append(n)
    return out


def main(argv: list[str]) -> int:
    root = Path(argv[0] if argv else ".").resolve()
    manifests = []
    for d, dirs, files in os.walk(root):
        dirs[:] = sorted(x for x in dirs if x not in SKIP_DIRS)
        if "Cargo.toml" in files:
            manifests.append(Path(d, "Cargo.toml"))
    if not manifests:
        print("machete_metadata_check: compared nothing (no Cargo.toml)", file=sys.stderr)
        return 1
    bad = 0
    for m in manifests:
        for n in findings(m.read_text(encoding="utf-8")):
            print(f"{m.relative_to(root)}:{n}: cargo-machete metadata (make the use visible instead)",
                  file=sys.stderr)
            bad += 1
    print(f"machete_metadata_check: {len(manifests)} manifest(s), {bad} finding(s)")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
