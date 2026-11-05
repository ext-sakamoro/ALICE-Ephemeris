#!/usr/bin/env python3
"""A module served to the public must not carry the path it was built at.

wasm_path_scan.py <module>...: every module's bytes are searched for what a build path looks like
(a home directory, a temporary directory, a cargo home). Any hit fails; so does scanning nothing.
"""
import re
import sys
from pathlib import Path

PATTERNS = re.compile(rb"/Users/|/home/|/private/|/var/folders/|\.cargo/|\.rustup/|[A-Za-z]:\\+Users\\+")


def scan(paths):
    """(modules scanned, findings)"""
    findings = []
    n = 0
    for p in paths:
        data = Path(p).read_bytes()
        if not data:
            findings.append(f"{p}: empty")
        n += 1
        for m in PATTERNS.finditer(data):
            around = data[m.start() : m.start() + 60].split(b"\0")[0].decode("latin-1")
            findings.append(f"{p}: {around!r}")
    return n, findings


def main(argv):
    n, findings = scan(argv)
    for f in findings[:20]:
        print(f)
    print(f"path scan: {n} module(s), {len(findings)} finding(s)")
    return 0 if n > 0 and not findings else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
