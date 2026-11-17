"""Pytest plugin: attach Vulyk case keys to JUnit even for marker-skipped tests.

Copy of ``ci/pytest_vulyk.py`` so this example runs standalone. Prefer importing
the repo-root plugin when the automation repo vendors the Vulyk tree.
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
        existing = {str(n).lower() for n, _ in getattr(item, "user_properties", [])}
        if "case_key" in existing or "vulyk_key" in existing:
            continue
        item.user_properties.append(("case_key", key))
