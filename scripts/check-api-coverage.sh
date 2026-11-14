#!/usr/bin/env bash
# OpenAPI inventory, Axum parity, and per-operation API coverage gate.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT_DIR="${API_COVERAGE_OUTPUT_DIR:-${ROOT}/target/api-coverage}"

cd "${ROOT}"
mkdir -p "${OUTPUT_DIR}"

echo "==> OpenAPI/Axum source parity"
cargo test -p vulyk-api api_coverage::tests --lib

echo "==> API operation runtime coverage"
API_COVERAGE_OUTPUT_DIR="${OUTPUT_DIR}" \
  cargo test -p vulyk-api --test api_coverage -- --nocapture

test -s "${OUTPUT_DIR}/api-coverage-report.json"
test -s "${OUTPUT_DIR}/api-coverage-summary.md"
python3 -c '
import json
import os
import pathlib
import sys

report = json.loads(pathlib.Path(sys.argv[1]).read_text())
summary = report["summary"]
total = summary["total_operations"]
if not total:
    raise SystemExit("API coverage report contains no operations")
if summary["route_covered"] != total:
    raise SystemExit("API route coverage is incomplete")
if summary["auth_covered_or_na"] != total:
    raise SystemExit("API auth coverage is incomplete")
if summary["success_contract_covered"] != total:
    raise SystemExit("one or more operations have no documented 2xx/3xx response")
if summary["coverage_percent"] != 100.0:
    raise SystemExit(f"API coverage is {summary['"'"'coverage_percent'"'"']}%, expected 100%")
if os.environ.get("CI") and not summary["database_sweeps_run"]:
    raise SystemExit("CI must run the database-backed API coverage sweeps")
print(f"OK: {total} OpenAPI operations covered (100%)")
' "${OUTPUT_DIR}/api-coverage-report.json"

echo "Reports:"
echo "  ${OUTPUT_DIR}/api-coverage-report.json"
echo "  ${OUTPUT_DIR}/api-coverage-summary.md"
