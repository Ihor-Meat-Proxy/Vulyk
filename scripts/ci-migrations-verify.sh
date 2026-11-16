#!/usr/bin/env bash
# Apply all migrations to an empty Postgres and smoke-check schema vs code.
# Practical "sqlx offline" path: when backend/.sqlx or query! macros exist,
# run `cargo sqlx prepare --check`; otherwise migrate + DB-backed tests.
#
# Requires: DATABASE_URL, rust toolchain, postgres reachable.
# Usage:
#   DATABASE_URL=postgres://... ./scripts/ci-migrations-verify.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

die() { printf 'error: %s\n' "$*" >&2; exit 1; }
log() { printf '==> %s\n' "$*"; }

[[ -n "${DATABASE_URL:-}" ]] || die "DATABASE_URL is required"
need_cmd() { command -v "$1" >/dev/null 2>&1 || die "missing $1"; }
need_cmd cargo
need_cmd git

reset_schema() {
  need_cmd psql
  log "Resetting public schema for fresh migrate..."
  psql "${DATABASE_URL}" -v ON_ERROR_STOP=1 -c \
    "DROP SCHEMA IF EXISTS public CASCADE; CREATE SCHEMA public;" >/dev/null
}

has_sqlx_offline_data() {
  if [[ -d "${ROOT}/backend/.sqlx" ]]; then
    return 0
  fi
  # Detect compile-time query macros in backend sources
  if grep -R --include='*.rs' -E 'sqlx::query(_as|_scalar)?!' backend/src >/dev/null 2>&1; then
    return 0
  fi
  return 1
}

ensure_sqlx_cli() {
  if command -v sqlx >/dev/null 2>&1; then
    return 0
  fi
  log "Installing sqlx-cli (locked) for prepare --check..."
  cargo install sqlx-cli --no-default-features --features postgres,rustls --locked
}

log "Building vulyk-api..."
cargo build -p vulyk-api

API_BIN="${ROOT}/target/debug/vulyk-api"
[[ -x "${API_BIN}" ]] || die "missing ${API_BIN}"

if command -v psql >/dev/null 2>&1; then
  reset_schema
else
  log "psql not found; assuming DATABASE_URL points at an empty database"
fi

log "Running migrations on empty database..."
"${API_BIN}" migrate run

log "Asserting migrate status is clean (no pending)..."
STATUS_OUT="$("${API_BIN}" migrate status)"
printf '%s\n' "${STATUS_OUT}"
PENDING_LINE="$(printf '%s\n' "${STATUS_OUT}" | grep -E '^pending:' | head -1 || true)"
[[ "${PENDING_LINE}" == "pending: 0" ]] || die "expected pending: 0, got: ${PENDING_LINE:-none}"
echo "${STATUS_OUT}" | grep -Fq '  (none)' || die "expected '(none)' under pending"

if command -v psql >/dev/null 2>&1; then
  log "Schema smoke: core tables present..."
  for table in projects test_cases _sqlx_migrations; do
    exists="$(psql "${DATABASE_URL}" -Atc \
      "SELECT to_regclass('public.${table}') IS NOT NULL")"
    [[ "${exists}" == "t" ]] || die "missing table after migrate: ${table}"
  done
  APPLIED="$(psql "${DATABASE_URL}" -Atc "SELECT COUNT(*) FROM _sqlx_migrations")"
  [[ "${APPLIED}" -gt 0 ]] || die "_sqlx_migrations empty after migrate"
  echo "OK: ${APPLIED} applied migration row(s); core tables exist"
fi

if has_sqlx_offline_data; then
  log "Offline query data / query! macros detected; running cargo sqlx prepare --check"
  ensure_sqlx_cli
  (
    cd backend
    cargo sqlx prepare --check -- --lib
  )
else
  log "No backend/.sqlx / query! macros; skipping sqlx prepare --check"
  log "Running DB-backed migrate_cli test instead..."
  cargo test -p vulyk-api --test migrate_cli -- --nocapture
fi

log "Migrations verify passed"
