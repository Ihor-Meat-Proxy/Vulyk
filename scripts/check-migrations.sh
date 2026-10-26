#!/usr/bin/env bash
# Fail if existing migration files were modified, or if version prefixes collide.
# Usage:
#   ./scripts/check-migrations.sh
#   ./scripts/check-migrations.sh origin/main
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

BASE_REF="${1:-}"
MIGRATIONS_DIR="backend/migrations"

if [[ ! -d "${MIGRATIONS_DIR}" ]]; then
  echo "error: ${MIGRATIONS_DIR} missing" >&2
  exit 1
fi

echo "==> Checking migration version uniqueness in ${MIGRATIONS_DIR}"
# Version = leading digits before first underscore (SQLx convention)
dupes="$(
  # Portable listing (no GNU find -printf) for macOS bash 3.2 hosts
  for f in "${MIGRATIONS_DIR}"/*.sql; do
    [[ -f "${f}" ]] || continue
    basename "${f}"
  done \
    | sed -n 's/^\([0-9][0-9]*\)_.*/\1/p' \
    | sort \
    | uniq -d
)"
if [[ -n "${dupes}" ]]; then
  echo "error: colliding migration version prefix(es):" >&2
  echo "${dupes}" >&2
  exit 1
fi
echo "OK: versions unique"

# When BASE_REF is set (CI), ensure files that existed on the base were not edited.
if [[ -z "${BASE_REF}" ]]; then
  if git rev-parse --verify origin/main >/dev/null 2>&1; then
    BASE_REF="origin/main"
  elif git rev-parse --verify main >/dev/null 2>&1; then
    BASE_REF="main"
  else
    echo "==> No base ref; skipping immutability check"
    exit 0
  fi
fi

echo "==> Checking migrations are immutable vs ${BASE_REF}"
if ! git rev-parse --verify "${BASE_REF}" >/dev/null 2>&1; then
  echo "warning: base ref ${BASE_REF} not found; skipping immutability check"
  exit 0
fi

# Files present on base that differ in the working tree / HEAD vs base
changed="$(git diff --name-only "${BASE_REF}"...HEAD -- "${MIGRATIONS_DIR}" || true)"
if [[ -z "${changed}" ]]; then
  # Also compare working tree for local runs
  changed="$(git diff --name-only "${BASE_REF}" -- "${MIGRATIONS_DIR}" || true)"
fi

modified_existing=""
while IFS= read -r path; do
  [[ -z "${path}" ]] && continue
  # Added files are OK; modified/deleted existing files are not
  if git cat-file -e "${BASE_REF}:${path}" 2>/dev/null; then
    # Exists on base - any change is forbidden
    modified_existing="${modified_existing}${path}"$'\n'
  fi
done <<< "${changed}"

if [[ -n "${modified_existing}" ]]; then
  echo "error: existing migration file(s) were modified or deleted (forward-only rule):" >&2
  printf '%s' "${modified_existing}" >&2
  echo "Add a new timestamped migration instead. See CONTRIBUTING.md" >&2
  exit 1
fi

echo "OK: no edits to migrations that exist on ${BASE_REF}"
