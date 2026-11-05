#!/usr/bin/env python3
"""No Cargo.toml in the repository may tell cargo-machete to ignore a dependency.

cargo-machete reports a dependency it cannot see used; the answer is to make the use visible
(or remove the dependency), never to list it as ignored. Each manifest below the root (build
output skipped) is parsed as TOML, so every spelling of the same key is the same key (table
header, dotted key, inline table, quoted key): the check fails if `package.metadata` or
`workspace.metadata` holds a `cargo-machete` entry, if a manifest does not parse, and when it
finds no manifest (compared nothing).

usage: scripts/machete_metadata_check.py [root]
"""
from __future__ import annotations

import os
import sys
import tomllib
from pathlib import Path

SKIP_DIRS = {"target", ".git", "node_modules"}
KEY = "cargo-machete"


def findings(text: str) -> list[str]:
    """The tables in which the parsed manifest names cargo-machete (empty if none)."""
    doc = tomllib.loads(text)
    out = []
    for table in ("package", "workspace"):
        section = doc.get(table)
        if isinstance(section, dict):
            meta = section.get("metadata")
            if isinstance(meta, dict) and KEY in meta:
                out.append(f"{table}.metadata.{KEY}")
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
        try:
            found = findings(m.read_text(encoding="utf-8"))
        except tomllib.TOMLDecodeError as e:
            print(f"{m.relative_to(root)}: does not parse as TOML ({e})", file=sys.stderr)
            bad += 1
            continue
        for key in found:
            print(f"{m.relative_to(root)}: {key} (make the use visible instead)", file=sys.stderr)
            bad += 1
    print(f"machete_metadata_check: {len(manifests)} manifest(s), {bad} finding(s)")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
