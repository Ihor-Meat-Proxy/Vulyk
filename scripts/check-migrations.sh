#!/usr/bin/env bash
# Fail if existing migration files were modified, or if version prefixes collide.
# Also hard-fail when a migration that exists on a release tag is modified/deleted.
# Usage:
#   ./scripts/check-migrations.sh
#   ./scripts/check-migrations.sh origin/main
# Env:
#   VULYK_RELEASE_TAG  Preferred release tag (default: v0.5.0). Falls back to
#                      latest v* tag when the preferred tag is missing.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

BASE_REF="${1:-}"
MIGRATIONS_DIR="backend/migrations"

die() { printf 'error: %s\n' "$*" >&2; exit 1; }
log() { printf '==> %s\n' "$*"; }

if [[ ! -d "${MIGRATIONS_DIR}" ]]; then
  die "${MIGRATIONS_DIR} missing"
fi

# ---------------------------------------------------------------------------
# Unique version prefixes (SQLx: digits before first underscore)
# ---------------------------------------------------------------------------
log "Checking migration version uniqueness in ${MIGRATIONS_DIR}"
dupes="$(
  # Portable listing (no GNU find -printf) for macOS bash 3.2 hosts
  for f in "${MIGRATIONS_DIR}"/*.sql; do
    [[ -f "${f}" ]] || continue
    base="$(basename "${f}")"
    # Skip sibling down files for uniqueness of the up version prefix
    case "${base}" in
      *.down.sql) continue ;;
    esac
    printf '%s\n' "${base}"
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

# ---------------------------------------------------------------------------
# Immutability helper: refuse edits/deletes of files that exist on REF
# ---------------------------------------------------------------------------
check_immutable_vs() {
  local ref="$1"
  local label="$2"
  local changed modified_existing path

  log "Checking migrations are immutable vs ${label} (${ref})"
  if ! git rev-parse --verify "${ref}" >/dev/null 2>&1; then
    echo "warning: ref ${ref} not found; skipping ${label} immutability check"
    return 0
  fi

  changed="$(git diff --name-only "${ref}"...HEAD -- "${MIGRATIONS_DIR}" 2>/dev/null || true)"
  if [[ -z "${changed}" ]]; then
    # Also compare working tree for local runs
    changed="$(git diff --name-only "${ref}" -- "${MIGRATIONS_DIR}" 2>/dev/null || true)"
  fi

  modified_existing=""
  while IFS= read -r path; do
    [[ -z "${path}" ]] && continue
    # Added files are OK; modified/deleted existing files are not
    if git cat-file -e "${ref}:${path}" 2>/dev/null; then
      modified_existing="${modified_existing}${path}"$'\n'
    fi
  done <<< "${changed}"

  if [[ -n "${modified_existing}" ]]; then
    echo "error: existing migration file(s) were modified or deleted (${label}):" >&2
    printf '%s' "${modified_existing}" >&2
    echo "Add a new timestamped migration instead. See docs/dev/migrations.md" >&2
    exit 1
  fi
  echo "OK: no edits to migrations that exist on ${label}"
}

# ---------------------------------------------------------------------------
# Base-ref immutability (main / PR base)
# ---------------------------------------------------------------------------
if [[ -z "${BASE_REF}" ]]; then
  if git rev-parse --verify origin/main >/dev/null 2>&1; then
    BASE_REF="origin/main"
  elif git rev-parse --verify main >/dev/null 2>&1; then
    BASE_REF="main"
  else
    echo "==> No base ref; skipping base immutability check"
  fi
fi

if [[ -n "${BASE_REF}" ]]; then
  check_immutable_vs "${BASE_REF}" "base ${BASE_REF}"
fi

# ---------------------------------------------------------------------------
# Release-tag immutability (hard-fail for already-released migration files)
# ---------------------------------------------------------------------------
resolve_release_tag() {
  local prefer="${VULYK_RELEASE_TAG:-v0.5.0}"
  local tag

  # Ensure tags are visible (shallow clones / sparse fetch)
  if ! git rev-parse --verify "refs/tags/${prefer}" >/dev/null 2>&1; then
    git fetch --tags --force origin 2>/dev/null || git fetch --tags --force 2>/dev/null || true
  fi

  if git rev-parse --verify "refs/tags/${prefer}" >/dev/null 2>&1; then
    printf '%s\n' "${prefer}"
    return 0
  fi

  tag="$(git tag -l 'v*' 2>/dev/null | sort -V | tail -1 || true)"
  if [[ -n "${tag}" ]]; then
    printf '%s\n' "${tag}"
    return 0
  fi
  return 1
}

RELEASE_TAG=""
if RELEASE_TAG="$(resolve_release_tag)"; then
  check_immutable_vs "${RELEASE_TAG}" "release tag ${RELEASE_TAG}"
else
  echo "warning: no v* release tag found; skipping release-tag immutability check"
fi
