#!/usr/bin/env bash
# Check relative / local markdown links (offline). Mirrors the CI Doc links job.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if ! command -v lychee >/dev/null 2>&1; then
  echo "lychee not found. Install one of:" >&2
  echo "  cargo install lychee --locked" >&2
  echo "  # or use the CI action lycheeverse/lychee-action" >&2
  exit 127
fi

ARGS=(
  --config lychee.toml
  --offline
  --no-progress
  --include-fragments
  README.md
  AGENTS.md
  CONTRIBUTING.md
  SECURITY.md
  "docs/**/*.md"
  e2e/README.md
)

echo "+ lychee ${ARGS[*]}"
exec lychee "${ARGS[@]}"
