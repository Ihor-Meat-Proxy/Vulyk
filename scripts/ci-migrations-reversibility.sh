#!/usr/bin/env bash
# When sibling *.down.sql files exist, run up -> down -> up and verify clean.
# Production is forward-only (CONTRIBUTING forbids downs); this path exercises
# optional/test downs for future use. Exit 0 with a skip message when none exist.
#
# Requires: DATABASE_URL, rust toolchain, psql, postgres reachable.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

MIGRATIONS_DIR="backend/migrations"

die() { printf 'error: %s\n' "$*" >&2; exit 1; }
log() { printf '==> %s\n' "$*"; }
need_cmd() { command -v "$1" >/dev/null 2>&1 || die "missing $1"; }

# Collect down siblings: VERSION_description.down.sql paired with .sql
DOWNS=()
for down in "${MIGRATIONS_DIR}"/*.down.sql; do
  [[ -f "${down}" ]] || continue
  up="${down%.down.sql}.sql"
  if [[ ! -f "${up}" ]]; then
    die "down without matching up: ${down}"
  fi
  DOWNS+=("${down}")
done

if [[ ${#DOWNS[@]} -eq 0 ]]; then
  log "No ${MIGRATIONS_DIR}/*.down.sql found; skipping reversibility (forward-only)"
  echo "OK: reversibility skipped"
  exit 0
fi

[[ -n "${DATABASE_URL:-}" ]] || die "DATABASE_URL is required"
need_cmd cargo
need_cmd psql

log "Building vulyk-api for reversibility checks..."
cargo build -p vulyk-api
API_BIN="${ROOT}/target/debug/vulyk-api"
[[ -x "${API_BIN}" ]] || die "missing ${API_BIN}"

migration_version() {
  local base
  base="$(basename "$1")"
  # Strip .down.sql or .sql, then take leading digits
  base="${base%.down.sql}"
  base="${base%.sql}"
  printf '%s\n' "${base}" | sed -n 's/^\([0-9][0-9]*\)_.*/\1/p'
}

reset_schema() {
  psql "${DATABASE_URL}" -v ON_ERROR_STOP=1 -c \
    "DROP SCHEMA IF EXISTS public CASCADE; CREATE SCHEMA public;" >/dev/null
}

assert_clean_status() {
  local status pending
  status="$("${API_BIN}" migrate status)"
  pending="$(printf '%s\n' "${status}" | grep -E '^pending:' | head -1 || true)"
  [[ "${pending}" == "pending: 0" ]] || die "status not clean after up/down/up: ${pending}"
}

# Sort downs by version ascending so we test each pair after earlier schema exists.
sorted_downs=()
while IFS= read -r line; do
  [[ -z "${line}" ]] && continue
  sorted_downs+=("${line}")
done < <(
  for d in "${DOWNS[@]}"; do
    ver="$(migration_version "${d}")"
    [[ -n "${ver}" ]] || die "cannot parse version from ${d}"
    printf '%s\t%s\n' "${ver}" "${d}"
  done | sort -n -k1,1 | cut -f2-
)

log "Reversibility: testing ${#sorted_downs[@]} down migration(s)"

for down in "${sorted_downs[@]}"; do
  up="${down%.down.sql}.sql"
  ver="$(migration_version "${down}")"
  log "up/down/up for version ${ver} ($(basename "${up}"))"

  reset_schema
  # Apply all migrations (including this up)
  "${API_BIN}" migrate run
  assert_clean_status

  # Apply down SQL and remove the version row so migrate can re-apply
  psql "${DATABASE_URL}" -v ON_ERROR_STOP=1 -f "${down}" >/dev/null
  psql "${DATABASE_URL}" -v ON_ERROR_STOP=1 -c \
    "DELETE FROM _sqlx_migrations WHERE version = ${ver};" >/dev/null

  # Re-apply up via migrator
  "${API_BIN}" migrate run
  assert_clean_status
  echo "OK: ${ver} up/down/up clean"
done

log "Reversibility checks passed"
