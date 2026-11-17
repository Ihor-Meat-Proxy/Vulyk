"""Demo pytest cases mapped to Vulyk keys via marks / properties."""

from __future__ import annotations

import pytest


@pytest.mark.vulyk("KOC-12")
def test_login_valid_password():
    assert True


@pytest.mark.vulyk("KOC-12")
@pytest.mark.skip(reason="not ready")
def test_login_skipped_still_mapped():
    """Marker skips never run the body; pytest_vulyk still emits case_key."""
    raise AssertionError("unreachable")


def test_health_runtime_skip(record_property):
    # Runtime skip: record the property *before* pytest.skip.
    record_property("case_key", "KOC-13")
    pytest.skip("environment not ready")
