#!/usr/bin/env bash
# Dev Container first-boot: toolchain sanity + hooks. Does not start the app.
# Part of #294 — keep the contributor toolchain isolated from the host.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

echo "==> Vulyk Dev Container post-create"

# Match CI: rustfmt + clippy on stable (rust-toolchain.toml).
if command -v rustup >/dev/null 2>&1; then
  rustup component add rustfmt clippy >/dev/null
fi

# Prefer repo hooks (check.sh --quick / --full).
if [[ -d scripts/githooks ]]; then
  git config core.hooksPath scripts/githooks || true
fi

chmod +x scripts/install.sh scripts/npm-ci-secure.sh scripts/npm-security-fast-path.sh \
  scripts/check-npm-security-fast-path.sh 2>/dev/null || true

if [[ -x scripts/install.sh ]]; then
  ./scripts/install.sh check || {
    echo "warning: install.sh check reported issues (non-fatal for container create)" >&2
  }
fi

cat <<'EOF'

Dev Container ready. Typical next steps:

  ./scripts/install.sh dev
  export DATABASE_URL=postgres://vulyk:vulyk@localhost:5432/vulyk
  cargo run -p vulyk-api
  ./scripts/npm-ci-secure.sh frontend && (cd frontend && npm run dev)

Checks: ./scripts/check.sh
Policy: docs/dev/supply-chain.md
EOF
