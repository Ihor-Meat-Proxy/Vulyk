"""Pytest plugin: attach Vulyk case keys to JUnit even for marker-skipped tests.

``@pytest.mark.skip`` never runs the test body, so ``record_property(...)``
inside the function never fires. Put the key on a mark instead:

    @pytest.mark.vulyk("KOC-12")
    @pytest.mark.skip(reason="not ready")
    def test_health():
        ...

Enable with ``pytest -p ci.pytest_vulyk`` (or add this module to
``pytest_plugins`` / ``PYTEST_ADDOPTS``). Prefer ``junit_family=xunit1`` when
using ``record_property`` at runtime.
"""

from __future__ import annotations

import pytest


def _key_from_item(item: pytest.Item) -> str | None:
    for name in ("vulyk", "case_key", "vulyk_key"):
        for mark in item.iter_markers(name=name):
            if mark.args:
                key = str(mark.args[0]).strip()
                if key:
                    return key
            key = str(mark.kwargs.get("key") or mark.kwargs.get("value") or "").strip()
            if key:
                return key
    return None


def pytest_collection_modifyitems(items: list[pytest.Item]) -> None:
    """Copy mark keys onto ``user_properties`` so JUnit XML includes them for skips."""
    for item in items:
        key = _key_from_item(item)
        if not key:
            continue
        # Avoid duplicates if a fixture already recorded the same property.
        existing = {str(n).lower() for n, _ in getattr(item, "user_properties", [])}
        if "case_key" in existing or "vulyk_key" in existing:
            continue
        item.user_properties.append(("case_key", key))
