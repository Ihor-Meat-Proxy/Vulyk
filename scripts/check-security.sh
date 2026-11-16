#!/usr/bin/env bash
# Local security checks mirroring .github/workflows/security.yml (subset).
# Usage:
#   ./scripts/check-security.sh           # audit + deny + npm audit
#   ./scripts/check-security.sh --quick   # cargo audit + deny only
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
need_cmd cargo-audit "cargo install cargo-audit --locked"
need_cmd cargo-deny "cargo install cargo-deny --locked"

echo "==> cargo audit"
cargo audit

echo "==> cargo deny check"
cargo deny check

if [[ "${MODE}" != "quick" ]]; then
  need_cmd npm "https://nodejs.org"
  echo "==> frontend npm audit (high+)"
  (
    cd frontend
    if [[ ! -d node_modules ]]; then npm ci; fi
    # Fast CI gate uses --omit=dev; full tree (incl. dev) remains useful locally.
    npm audit --omit=dev --audit-level=high
  )
  echo "==> e2e npm audit (prod, high+)"
  (
    cd e2e
    if [[ ! -d node_modules ]]; then npm ci; fi
    npm audit --omit=dev --audit-level=high
  )
fi

echo "OK: ./scripts/check-security.sh (${MODE}) passed"
