# pytest example

Minimal pytest suite that stamps Vulyk case keys into JUnit XML (including
marker-skipped tests via [`pytest_vulyk.py`](pytest_vulyk.py)).

## Local

```bash
cd examples/pytest
python3 -m venv .venv
source .venv/bin/activate
pip install -e .
pytest
# report.xml written next to pyproject.toml
```

## Report into Vulyk

```bash
export VULYK_API_URL=https://vulyk.example.com
export VULYK_API_TOKEN=vulyk_ci_…
export VULYK_PROJECT_KEY=KOC

VULYK_RUN_ID="$(../../scripts/vulyk-upload.sh --open-run --print-run-id \
  --suite-name "pytest-local" --provider other)"
export VULYK_RUN_ID

../../scripts/vulyk-upload.sh --file report.xml --format junit
../../scripts/vulyk-upload.sh --close-run
```

Or upload pytest-json-report output with `--format pytest_json`.

## CI

Copy a provider file from [`ci/`](ci/). Guide:
[framework-examples.md](../../docs/guides/framework-examples.md#pytest).
