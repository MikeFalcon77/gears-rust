#!/usr/bin/env python3
"""Unit tests for junit_enrich.py (source locations for nextest/pytest JUnit).

    python3 -m unittest discover -s tools/scripts/tests
"""

from __future__ import annotations

import sys
import tempfile
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

import junit_enrich  # noqa: E402

# Shape of a real nextest 0.9.148 report (trimmed).
REPORT = """<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="nextest-run" tests="3" failures="2">
  <testsuite name="crate::probe" tests="3" failures="2">
    <testcase name="passes" classname="crate::probe" time="0.002"/>
    <testcase name="fails_assert_eq" classname="crate::probe" time="0.003">
      <failure message="thread &apos;fails_assert_eq&apos; (1) panicked at libs/x/tests/probe.rs:7:5" type="test failure">thread &apos;fails_assert_eq&apos; (1) panicked at libs/x/tests/probe.rs:7:5:
assertion `left == right` failed: math is broken
  left: 2
 right: 3
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace</failure>
    </testcase>
    <testcase name="times_out" classname="crate::probe" time="60.0">
      <failure message="test timed out" type="timeout">test timed out after 60s</failure>
    </testcase>
  </testsuite>
</testsuites>
"""


class EnrichTest(unittest.TestCase):
    def setUp(self) -> None:
        tmp = tempfile.NamedTemporaryFile("w", suffix=".xml", delete=False, encoding="utf-8")
        tmp.write(REPORT)
        tmp.close()
        self.path = tmp.name
        self.addCleanup(Path(self.path).unlink)

    def cases(self) -> dict[str, ET.Element]:
        return {c.get("name"): c for c in ET.parse(self.path).iter("testcase")}

    def test_panic_gets_location_and_assertion_text(self) -> None:
        self.assertEqual(junit_enrich.enrich(self.path), 1)
        case = self.cases()["fails_assert_eq"]
        self.assertEqual(case.get("file"), "libs/x/tests/probe.rs")
        self.assertEqual(case.get("line"), "7")
        message = case.find("failure").get("message")
        self.assertTrue(message.startswith("libs/x/tests/probe.rs:7: assertion `left == right` failed: math is broken"))
        self.assertIn("right: 3", message)
        self.assertNotIn("RUST_BACKTRACE", message)

    def test_unrecognised_failures_and_passes_are_untouched(self) -> None:
        junit_enrich.enrich(self.path)
        cases = self.cases()
        self.assertIsNone(cases["times_out"].get("file"))
        self.assertEqual(cases["times_out"].find("failure").get("message"), "test timed out")
        self.assertIsNone(cases["passes"].get("file"))

    def test_idempotent(self) -> None:
        junit_enrich.enrich(self.path)
        first = Path(self.path).read_text(encoding="utf-8")
        junit_enrich.enrich(self.path)
        self.assertEqual(Path(self.path).read_text(encoding="utf-8"), first)


# Shape of a pytest --tb=short report, run from the repository root.
PYTEST_REPORT = """<?xml version="1.0" encoding="utf-8"?>
<testsuites><testsuite name="e2e-probe" tests="2" failures="2">
  <testcase classname="suites.x.test_probe" name="test_direct" time="0.1">
    <failure message="AssertionError: health endpoint not ready&#10;assert 503 == 200">testing/e2e/suites/x/test_probe.py:9: in test_direct
    assert status == 200, "health endpoint not ready"
E   AssertionError: health endpoint not ready</failure>
  </testcase>
  <testcase classname="suites.x.test_probe" name="test_via_helper" time="0.1">
    <failure message="AssertionError: bad status 500">testing/e2e/suites/x/test_probe.py:17: in test_via_helper
    check(500)
testing/e2e/helpers/http.py:2: in check
    assert v == 200, f"bad status {v}"
/usr/lib/python3/site-packages/requests/api.py:59: in request
E   AssertionError: bad status 500</failure>
  </testcase>
</testsuite></testsuites>
"""


class EnrichPytestTest(unittest.TestCase):
    def setUp(self) -> None:
        tmp = tempfile.NamedTemporaryFile("w", suffix=".xml", delete=False, encoding="utf-8")
        tmp.write(PYTEST_REPORT)
        tmp.close()
        self.path = tmp.name
        self.addCleanup(Path(self.path).unlink)

    def test_location_is_the_test_file_line_and_message_is_kept(self) -> None:
        self.assertEqual(junit_enrich.enrich(self.path), 2)
        cases = {c.get("name"): c for c in ET.parse(self.path).iter("testcase")}
        direct, helper = cases["test_direct"], cases["test_via_helper"]
        self.assertEqual((direct.get("file"), direct.get("line")), ("testing/e2e/suites/x/test_probe.py", "9"))
        # The call site in the test, not the helper or a third-party frame.
        self.assertEqual((helper.get("file"), helper.get("line")), ("testing/e2e/suites/x/test_probe.py", "17"))
        self.assertEqual(helper.find("failure").get("message"), "AssertionError: bad status 500")


if __name__ == "__main__":
    unittest.main()
