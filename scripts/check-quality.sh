#!/usr/bin/env bash
# Local mirror of .github/workflows/quality.yml (subset; no Postgres required).
# Fast required machete/knip also run in .github/workflows/ci-fast-gates.yml.
# Usage:
#   ./scripts/check-quality.sh           # machete + frontend quality + rust lib cov
#   ./scripts/check-quality.sh --quick   # skip coverage
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

MODE="default"
for arg in "$@"; do
  case "$arg" in
    --quick) MODE="quick" ;;
    -h|--help)
      sed -n '2,6p' "$0"
      exit 0
      ;;
    *)
      echo "unknown arg: $arg (use --quick)" >&2
      exit 2
      ;;
  esac
done

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "error: missing '$1' (install: $2)" >&2
    exit 1
  fi
}

need_cmd cargo "https://rustup.rs"
need_cmd cargo-machete "cargo install cargo-machete --locked"

echo "==> cargo machete"
cargo machete

echo "==> frontend eslint / prettier / knip"
(
  if [[ ! -d frontend/node_modules ]]; then ./scripts/npm-ci-secure.sh frontend; fi
  cd frontend
  npm run lint
  npm run lint:eslint -- --max-warnings 50
  npm run format:check
  npm run knip
)

if [[ "${MODE}" != "quick" ]]; then
  need_cmd cargo-llvm-cov "cargo install cargo-llvm-cov --locked"
  rustup component add llvm-tools-preview >/dev/null
  echo "==> cargo llvm-cov --lib (floors in coverage-thresholds.toml)"
  cargo llvm-cov --workspace --lib --json --summary-only --output-path /tmp/vulyk-rust-cov.json
  python3 - <<'PY'
import json
from pathlib import Path
data = json.load(open("/tmp/vulyk-rust-cov.json"))["data"][0]["totals"]
lines = data["lines"]["percent"]
funcs = data["functions"]["percent"]
thr_lines, thr_funcs = 12.0, 14.0
for line in Path("coverage-thresholds.toml").read_text().splitlines():
    if line.strip().startswith("lines"):
        thr_lines = float(line.split("=")[1].strip())
    if line.strip().startswith("functions"):
        thr_funcs = float(line.split("=")[1].strip())
print(f"lines={lines:.2f}% (floor {thr_lines}), functions={funcs:.2f}% (floor {thr_funcs})")
if lines + 1e-6 < thr_lines or funcs + 1e-6 < thr_funcs:
    raise SystemExit("rust coverage below floor")
PY
  echo "==> frontend vitest coverage"
  (
    cd frontend
    npm run test:coverage
  )
fi

echo "OK: ./scripts/check-quality.sh (${MODE}) passed"
