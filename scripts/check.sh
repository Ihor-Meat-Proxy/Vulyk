#!/usr/bin/env bash
# Local checks that mirror the required CI jobs (fast path by default).
# Usage:
#   ./scripts/check.sh           # fmt, clippy, seed keys, migrations, frontend lint/tsc/test-ids, e2e tsc
#   ./scripts/check.sh --quick   # skip clippy and frontend vitest (pre-commit)
#   ./scripts/check.sh --full    # also cargo test + frontend vitest (pre-push / CI parity)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

MODE="default"
for arg in "$@"; do
  case "$arg" in
    --quick) MODE="quick" ;;
    --full) MODE="full" ;;
    -h|--help)
      sed -n '2,8p' "$0"
      exit 0
      ;;
    *)
      echo "unknown arg: $arg (use --quick or --full)" >&2
      exit 2
      ;;
  esac
done

echo "==> seed keys"
chmod +x scripts/check-seed-keys.sh
./scripts/check-seed-keys.sh

echo "==> migrations"
chmod +x scripts/check-migrations.sh
./scripts/check-migrations.sh

if [[ -x scripts/check-version.sh ]]; then
  echo "==> version alignment"
  ./scripts/check-version.sh
fi

echo "==> cargo fmt"
cargo fmt --all -- --check

if [[ "${MODE}" != "quick" ]]; then
  echo "==> cargo clippy"
  cargo clippy --workspace --all-targets -- -D warnings
else
  echo "==> cargo clippy (skipped in --quick)"
fi

if [[ "${MODE}" == "full" ]]; then
  echo "==> cargo test"
  cargo test --workspace
fi

echo "==> frontend lint / typecheck / test-ids"
(
  cd frontend
  if [[ ! -d node_modules ]]; then npm ci; fi
  npm run lint
  npm run typecheck
  npm run check:test-ids
  if [[ "${MODE}" == "full" ]]; then
    npm run test
  fi
)

echo "==> e2e typecheck + Vulyk case-key annotations"
(
  cd e2e
  if [[ ! -d node_modules ]]; then npm ci; fi
  npm run typecheck
  npm run check:vulyk-case-links
)

echo "OK: ./scripts/check.sh (${MODE}) passed"
