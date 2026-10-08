"""FORK-ONLY probe for the JUnit -> GitHub Checks pipeline. Never merge upstream."""


def _expect_ok(status: int) -> None:
    assert status == 200, f"unexpected status {status}"


def test_probe_e2e_passes():
    assert True


def test_probe_e2e_fails_in_helper():
    _expect_ok(503)
