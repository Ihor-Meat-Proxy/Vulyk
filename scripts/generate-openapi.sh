#!/usr/bin/env bash
# Regenerate the committed OpenAPI document from ApiDoc.
# Usage: ./scripts/generate-openapi.sh [OUT_PATH]
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

OUT="${1:-docs/api/openapi.json}"

cargo run -p vulyk-api --example dump_openapi --quiet -- --out "${OUT}"
echo "OK: wrote ${OUT}"
