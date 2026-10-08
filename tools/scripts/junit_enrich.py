#!/usr/bin/env python3
"""
Make nextest's JUnit failures point at the Rust source that failed.

nextest records a failing test as

    <failure message="thread 'x' panicked at libs/foo/tests/bar.rs:7:5" ...>
    thread 'x' panicked at libs/foo/tests/bar.rs:7:5:
    assertion `left == right` failed: math is broken
      left: 2
     right: 3
    note: run with `RUST_BACKTRACE=1` ...
    </failure>

so the one line a report shows (`message`) carries the location and drops the
reason, and nothing machine-readable says which file or line it was. This
rewrites, in place, each failing <testcase> whose output has a panic location:

- `message` becomes the panic message -- the assertion text, `left`/`right`
  included -- with the location in front of it;
- `file` and `line` are set on the <testcase>, the attributes test reporters
  use for source annotations.

Anything it cannot recognise (a timeout, a crash without a panic) is left
exactly as nextest wrote it. Called by the Makefile's `nextest_run` after it
saves a report; a failure here must never change the test outcome, so the
caller treats a non-zero exit as a warning.

Usage: junit_enrich.py REPORT.xml [REPORT.xml ...]
"""

import re
import sys
import xml.etree.ElementTree as ET

# `thread 'name' (tid) panicked at path/to/file.rs:LINE:COL:` -- the thread id
# is printed by newer toolchains only.
PANIC_RE = re.compile(
    r"panicked at (?P<file>[^\s:][^:\n]*\.rs):(?P<line>\d+):\d+:\n(?P<msg>.*?)(?:\nnote: |\nstack backtrace:|\Z)",
    re.S,
)

# Keeps the one-line summaries in the reporter readable; the full text stays
# in the <failure> body.
MAX_MESSAGE = 1000


def enrich(path: str) -> int:
    tree = ET.parse(path)
    changed = 0
    for case in tree.iter("testcase"):
        failure = case.find("failure")
        if failure is None:
            failure = case.find("error")
        if failure is None or not failure.text:
            continue
        m = PANIC_RE.search(failure.text)
        if m is None:
            continue
        file = m.group("file").replace("\\", "/")
        line = m.group("line")
        msg = m.group("msg").strip() or failure.get("message", "")
        if len(msg) > MAX_MESSAGE:
            msg = msg[:MAX_MESSAGE] + "…"
        case.set("file", file)
        case.set("line", line)
        failure.set("message", f"{file}:{line}: {msg}")
        changed += 1
    if changed:
        tree.write(path, encoding="UTF-8", xml_declaration=True)
    return changed


def main(argv: list[str]) -> int:
    if not argv:
        print(__doc__.strip().splitlines()[-1], file=sys.stderr)
        return 2
    for path in argv:
        print(f"junit_enrich: {path}: {enrich(path)} failure(s) annotated")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
