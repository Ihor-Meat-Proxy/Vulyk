"""Load the local Vulyk case-key plugin.

Prefer this root ``conftest.py`` (or ``pytest -p pytest_vulyk`` / ``addopts``).
Do **not** put ``pytest_plugins`` in ``pytest.ini`` — pytest rejects that and
the plugin will never load, so marker-skipped tests lose their ``case_key``.
"""

pytest_plugins = ["pytest_vulyk"]
