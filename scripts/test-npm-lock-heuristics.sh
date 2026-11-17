#!/usr/bin/env bash
# Unit-ish checks for check-npm-lock-heuristics.py (ASCII-only).
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

die() { printf 'error: %s\n' "$*" >&2; exit 1; }

BASE_REF=HEAD HEAD_REF=HEAD python3 scripts/check-npm-lock-heuristics.py \
  | grep -q 'OK: npm lock heuristics clean' \
  || die "expected clean heuristics on identical refs"

# Typosquat detector smoke via python import
python3 - <<'PY'
import importlib.util
from pathlib import Path
spec = importlib.util.spec_from_file_location(
    "heur", Path("scripts/check-npm-lock-heuristics.py")
)
mod = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mod)
hits = mod.typosquat_hits("lodahs")  # near lodash
assert "lodash" in hits, hits
assert mod.typosquat_hits("lodash") == []
assert mod.typosquat_hits("react") == []
print("OK: typosquat helpers")
PY

echo "OK: npm lock heuristics tests"
