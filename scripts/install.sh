#!/usr/bin/env bash
# Install / bootstrap Vulyk on Linux, macOS, or WSL2.
# Verified modes: compose | prod | dev | setup | check | helm-template | perf-smoke
#                 upgrade | backup | restore
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

BACKUP_DIR="${ROOT}/backups"
BACKUP_RETENTION="${VULYK_BACKUP_RETENTION:-10}"
UPGRADE_TO=""
NO_BACKUP=0
RESTORE_FILE=""

log() { printf '==> %s\n' "$*"; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }
warn() { printf 'warning: %s\n' "$*" >&2; }

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || die "missing required command: $1"
}

detect_os() {
  local uname_s
  uname_s="$(uname -s)"
  case "${uname_s}" in
    Linux*)
      if grep -qiE 'microsoft|wsl' /proc/version 2>/dev/null; then
        echo wsl2
      else
        echo linux
      fi
      ;;
    Darwin*) echo macos ;;
    MINGW*|MSYS*|CYGWIN*)
      die "native Windows is not supported; use WSL2 (Ubuntu) and re-run this script"
      ;;
    *) die "unsupported OS: ${uname_s}" ;;
  esac
}

random_secret() {
  if command -v openssl >/dev/null 2>&1; then
    openssl rand -hex 32
  else
    # Portable fallback
    head -c 32 /dev/urandom | od -An -tx1 | tr -d ' \n'
  fi
}

# ---------------------------------------------------------------------------
# First-run setup options (flags + env + interactive prompts)
# ---------------------------------------------------------------------------
ADMIN_EMAIL="${VULYK_ADMIN_EMAIL:-}"
ADMIN_PASSWORD="${VULYK_ADMIN_PASSWORD:-}"
DEFAULT_LOCALE="${VULYK_DEFAULT_LOCALE:-}"
DEFAULT_TIMEZONE="${VULYK_DEFAULT_TIMEZONE:-}"
REGISTRATION_MODE="${VULYK_REGISTRATION_MODE:-}"
INSTANCE_NAME="${VULYK_INSTANCE_NAME:-}"
SEED_DEMO="${VULYK_SEED_DEMO:-}"
BASE_URL="${VULYK_BASE_URL:-}"
ASSUME_YES=0

usage() {
  cat <<'EOF'
Usage: scripts/install.sh [MODE] [OPTIONS]

Modes:
  compose         Build and start the local/dev Docker Compose stack (default)
  prod            Production Compose (auth required, volumes, healthchecks)
  dev             Start Postgres only; print commands to run API + UI locally
  setup           Run first-run setup (docker compose run api ... or cargo run)
  check           Verify local toolchain (rustc, cargo, node, docker/helm optional)
  helm-template   Render the Helm chart (requires helm) as a dry-run validation
  perf-smoke      Seed ~100k cases and check list/search latency (needs running API)
  upgrade         Safe upgrade: backup, pull, rebuild, migrate, health-check
  backup          pg_dump to ./backups/vulyk-<version>-<timestamp>.sql.gz
  restore FILE    Restore a backup (confirmation required unless --yes)

First-run options (compose / prod / dev / setup):
  --admin-email=EMAIL
  --admin-password=PASSWORD
  --locale=en|uk|fr|es|de  Instance default UI language
  --timezone=TZ            e.g. UTC, Europe/Kyiv
  --registration-mode=MODE open | invite | disabled
  --instance-name=NAME
  --seed-demo=0|1
  --base-url=URL
  --yes, -y                Non-interactive (use flags/env; skip prompts)

Upgrade / backup options:
  --to=TAG|BRANCH          Checkout tag or branch before rebuilding (upgrade)
  --no-backup              Skip automatic pg_dump (upgrade only; not recommended)

Environment equivalents:
  VULYK_ADMIN_EMAIL, VULYK_ADMIN_PASSWORD, VULYK_DEFAULT_LOCALE,
  VULYK_DEFAULT_TIMEZONE, VULYK_REGISTRATION_MODE, VULYK_INSTANCE_NAME,
  VULYK_SEED_DEMO, VULYK_BASE_URL, VULYK_BACKUP_RETENTION

Written into .env (compose/dev) or .env.prod (prod). Defaults:
  compose/dev: seed demo=1, registration=open, admin@vulyk.local / admin
  prod:        seed demo=0, registration=disabled

After compose/prod/dev, if setup did not complete automatically:
  ./scripts/install.sh setup --yes
  # or: docker compose run --rm api vulyk-api setup --yes ...
  # or: cargo run -p vulyk-api -- setup ...

Upgrade (keeps the named Postgres volume; never runs docker compose down -v):
  ./scripts/install.sh upgrade --yes
  ./scripts/install.sh upgrade --yes --to=v0.2.0
  ./scripts/install.sh backup
  ./scripts/install.sh restore ./backups/vulyk-0.1.0-20260101T120000Z.sql.gz

WARNING: never run 'docker compose down -v' on a data you care about -
the -v flag deletes named volumes (Postgres data).

See docs/guides/first-run-setup.md and docs/guides/upgrade.md

Environment:
  Linux, macOS (bash 3.2+), and WSL2 are supported. Native Windows is not - use WSL2.
EOF
}

parse_args() {
  local arg
  MODE="compose"
  if [[ $# -gt 0 ]]; then
    case "$1" in
      -h|--help|help)
        usage
        exit 0
        ;;
      -*)
        # Flags without a mode -> default compose
        ;;
      *)
        MODE="$1"
        shift
        ;;
    esac
  fi
  while [[ $# -gt 0 ]]; do
    arg="$1"
    shift
    case "${arg}" in
      -h|--help|help)
        usage
        exit 0
        ;;
      -y|--yes)
        ASSUME_YES=1
        ;;
      --admin-email=*)
        ADMIN_EMAIL="${arg#*=}"
        ;;
      --admin-password=*)
        ADMIN_PASSWORD="${arg#*=}"
        ;;
      --locale=*)
        DEFAULT_LOCALE="${arg#*=}"
        ;;
      --timezone=*)
        DEFAULT_TIMEZONE="${arg#*=}"
        ;;
      --registration-mode=*)
        REGISTRATION_MODE="${arg#*=}"
        ;;
      --instance-name=*)
        INSTANCE_NAME="${arg#*=}"
        ;;
      --seed-demo=*)
        SEED_DEMO="${arg#*=}"
        ;;
      --base-url=*)
        BASE_URL="${arg#*=}"
        ;;
      --to=*)
        UPGRADE_TO="${arg#*=}"
        ;;
      --to)
        [[ $# -gt 0 ]] || die "--to requires a tag or branch"
        UPGRADE_TO="$1"
        shift
        ;;
      --no-backup)
        NO_BACKUP=1
        ;;
      --*=*)
        die "unknown option: $arg (try --help)"
        ;;
      -*)
        die "unknown option: $arg (try --help)"
        ;;
      *)
        if [[ "${MODE}" == "restore" && -z "${RESTORE_FILE}" ]]; then
          RESTORE_FILE="${arg}"
        else
          die "unexpected argument: $arg (try --help)"
        fi
        ;;
    esac
  done
}

prompt_default() {
  local label="$1" default="$2" reply
  if [[ -t 0 && "${ASSUME_YES}" -eq 0 ]]; then
    printf '%s [%s]: ' "${label}" "${default}" >&2
    read -r reply || true
    if [[ -n "${reply:-}" ]]; then
      printf '%s' "${reply}"
    else
      printf '%s' "${default}"
    fi
  else
    printf '%s' "${default}"
  fi
}

# Prompt for a password with confirmation. Empty input returns empty (caller may apply a default).
# When $2 (hint_default) is non-empty, show that Enter keeps the default (value not echoed).
prompt_secret_confirm() {
  local label="$1" hint_default="${2:-}" pass1 pass2
  if [[ ! -t 0 || "${ASSUME_YES}" -eq 1 ]]; then
    die "admin password required (set --admin-password= or VULYK_ADMIN_PASSWORD)"
  fi
  while true; do
    if [[ -n "${hint_default}" ]]; then
      printf '%s [Enter=default]: ' "${label}" >&2
    else
      printf '%s: ' "${label}" >&2
    fi
    if command -v stty >/dev/null 2>&1; then
      stty -echo 2>/dev/null || true
      read -r pass1 || true
      stty echo 2>/dev/null || true
      printf '\n' >&2
    else
      read -r pass1 || true
    fi
    if [[ -z "${pass1:-}" ]]; then
      printf '%s' ""
      return 0
    fi
    if command -v stty >/dev/null 2>&1; then
      printf 'Confirm %s: ' "${label}" >&2
      stty -echo 2>/dev/null || true
      read -r pass2 || true
      stty echo 2>/dev/null || true
      printf '\n' >&2
    else
      printf 'Confirm %s: ' "${label}" >&2
      read -r pass2 || true
    fi
    if [[ "${pass1}" != "${pass2:-}" ]]; then
      warn "passwords do not match"
      continue
    fi
    printf '%s' "${pass1}"
    return 0
  done
}

validate_locale() {
  case "$1" in
    en|uk|fr|es|de) return 0 ;;
    *) die "locale must be en, uk, fr, es, or de (got: $1)" ;;
  esac
}

validate_registration_mode() {
  case "$1" in
    open|invite|disabled) return 0 ;;
    *) die "registration-mode must be open, invite, or disabled (got: $1)" ;;
  esac
}

# Collect first-run values for a profile: compose | prod | bare
# Sets globals: ADMIN_EMAIL, ADMIN_PASSWORD, DEFAULT_LOCALE, DEFAULT_TIMEZONE,
# REGISTRATION_MODE, INSTANCE_NAME, SEED_DEMO, BASE_URL
collect_setup_options() {
  local profile="$1"
  local def_email def_password def_locale def_tz def_reg def_seed def_name

  case "${profile}" in
    compose|dev)
      def_email="admin@vulyk.local"
      def_password="admin"
      def_locale="en"
      def_tz="UTC"
      def_reg="open"
      def_seed="1"
      def_name="Vulyk"
      ;;
    prod)
      def_email="admin@vulyk.local"
      def_password=""
      def_locale="en"
      def_tz="UTC"
      def_reg="disabled"
      def_seed="0"
      def_name="Vulyk"
      ;;
    *)
      die "internal: unknown setup profile ${profile}"
      ;;
  esac

  if [[ "${ASSUME_YES}" -eq 0 && -t 0 ]]; then
    log "First-run setup (press Enter to accept defaults)..."
  fi

  ADMIN_EMAIL="$(prompt_default "Admin email" "${ADMIN_EMAIL:-${def_email}}")"
  if [[ -z "${ADMIN_PASSWORD}" ]]; then
    if [[ "${ASSUME_YES}" -eq 1 || ! -t 0 ]]; then
      ADMIN_PASSWORD="${def_password}"
      [[ -n "${ADMIN_PASSWORD}" ]] \
        || die "admin password required (set --admin-password= or VULYK_ADMIN_PASSWORD)"
    else
      # Interactive: confirm; empty Enter keeps profile default when present
      local prompted
      prompted="$(prompt_secret_confirm "Admin password" "${def_password}")"
      if [[ -z "${prompted}" ]]; then
        [[ -n "${def_password}" ]] || die "admin password is required"
        ADMIN_PASSWORD="${def_password}"
        log "Using default local admin password for ${profile}"
      else
        ADMIN_PASSWORD="${prompted}"
      fi
    fi
  fi
  DEFAULT_LOCALE="$(prompt_default "Locale (en/uk/fr/es/de)" "${DEFAULT_LOCALE:-${def_locale}}")"
  DEFAULT_TIMEZONE="$(prompt_default "Timezone" "${DEFAULT_TIMEZONE:-${def_tz}}")"
  REGISTRATION_MODE="$(prompt_default "Registration mode (open/invite/disabled)" "${REGISTRATION_MODE:-${def_reg}}")"
  INSTANCE_NAME="$(prompt_default "Instance name" "${INSTANCE_NAME:-${def_name}}")"
  SEED_DEMO="$(prompt_default "Seed demo users (0/1)" "${SEED_DEMO:-${def_seed}}")"
  if [[ -z "${BASE_URL}" ]]; then
    BASE_URL=""
  fi

  validate_locale "${DEFAULT_LOCALE}"
  validate_registration_mode "${REGISTRATION_MODE}"
  case "${SEED_DEMO}" in
    0|1|true|false|yes|no) ;;
    *) die "seed-demo must be 0 or 1 (got: ${SEED_DEMO})" ;;
  esac
  # Normalize seed demo to 0/1
  case "$(printf '%s' "${SEED_DEMO}" | tr '[:upper:]' '[:lower:]')" in
    1|true|yes) SEED_DEMO=1 ;;
    *) SEED_DEMO=0 ;;
  esac

  [[ -n "${ADMIN_EMAIL}" ]] || die "admin email is required"
  [[ -n "${ADMIN_PASSWORD}" ]] || die "admin password is required"
}

# Upsert KEY=VALUE in an env file (create file if missing).
env_upsert() {
  local file="$1" key="$2" value="$3"
  local tmp
  touch "${file}"
  tmp="$(mktemp)"
  if grep -qE "^${key}=" "${file}" 2>/dev/null; then
    # shellcheck disable=SC2016
    awk -v k="${key}" -v v="${value}" '
      BEGIN { done=0 }
      index($0, k "=") == 1 && !done { print k "=" v; done=1; next }
      { print }
      END { if (!done) print k "=" v }
    ' "${file}" >"${tmp}"
    mv "${tmp}" "${file}"
  else
    rm -f "${tmp}"
    printf '%s=%s\n' "${key}" "${value}" >>"${file}"
  fi
}

write_setup_env() {
  local env_file="$1"
  touch "${env_file}"
  env_upsert "${env_file}" "VULYK_ADMIN_EMAIL" "${ADMIN_EMAIL}"
  env_upsert "${env_file}" "VULYK_ADMIN_PASSWORD" "${ADMIN_PASSWORD}"
  env_upsert "${env_file}" "VULYK_DEFAULT_LOCALE" "${DEFAULT_LOCALE}"
  env_upsert "${env_file}" "VULYK_DEFAULT_TIMEZONE" "${DEFAULT_TIMEZONE}"
  env_upsert "${env_file}" "VULYK_REGISTRATION_MODE" "${REGISTRATION_MODE}"
  env_upsert "${env_file}" "VULYK_INSTANCE_NAME" "${INSTANCE_NAME}"
  env_upsert "${env_file}" "VULYK_SEED_DEMO" "${SEED_DEMO}"
  if [[ -n "${BASE_URL}" ]]; then
    env_upsert "${env_file}" "VULYK_BASE_URL" "${BASE_URL}"
  fi
  chmod 600 "${env_file}" 2>/dev/null || true
  log "Wrote first-run settings to ${env_file}"
}

ensure_prod_env() {
  local env_file="${1:-.env.prod}"
  if [[ -f "${env_file}" ]]; then
    log "Using existing ${env_file}"
  else
    log "Generating $env_file with random secrets..."
    cat >"${env_file}" <<EOF
POSTGRES_USER=vulyk
POSTGRES_PASSWORD=$(random_secret)
POSTGRES_DB=vulyk
VULYK_AUTH_REQUIRED=true
VULYK_JWT_SECRET=$(random_secret)
VULYK_SECRETS_KEY=$(random_secret)
VULYK_LLM_MOCK=0
API_PORT=8080
WEB_PORT=80
EOF
    chmod 600 "${env_file}" || true
    log "Wrote $env_file (mode 600)."
  fi
  write_setup_env "${env_file}"
}

ensure_compose_env() {
  local env_file="${1:-.env}"
  if [[ ! -f "${env_file}" ]]; then
    log "Creating ${env_file}..."
    cat >"${env_file}" <<EOF
# Generated by scripts/install.sh - local/dev Compose
VULYK_AUTH_REQUIRED=false
VULYK_JWT_SECRET=vulyk-compose-dev-secret
VULYK_LLM_MOCK=1
EOF
  fi
  write_setup_env "${env_file}"
}

print_setup_next_steps() {
  local profile="$1"
  cat <<EOF

First-run admin seeding uses env vars written above. If the API is already
running without an admin, complete setup with:

  ./scripts/install.sh setup --yes

Or manually:
  docker compose run --rm api vulyk-api setup --yes \\
    --admin-email=${ADMIN_EMAIL} --locale=${DEFAULT_LOCALE} \\
    --timezone=${DEFAULT_TIMEZONE} --registration-mode=${REGISTRATION_MODE}

  # without Docker:
  cargo run -p vulyk-api -- setup --yes \\
    --admin-email=${ADMIN_EMAIL} --locale=${DEFAULT_LOCALE} \\
    --timezone=${DEFAULT_TIMEZONE} --registration-mode=${REGISTRATION_MODE}

Web wizard + one-time setup token: see docs/guides/first-run-setup.md
EOF
  if [[ "${profile}" == "compose" || "${profile}" == "dev" ]]; then
    cat <<EOF

Default local credentials (when seeded):
  Admin: ${ADMIN_EMAIL} / (password from .env - default admin)
  Demo:  user@vulyk.local / user   (when VULYK_SEED_DEMO=1)
  Change the default admin password on first login (must_change_password).
EOF
  fi
}

resolve_compose() {
  if command -v docker >/dev/null 2>&1 && docker compose version >/dev/null 2>&1; then
    COMPOSE=(docker compose)
  elif command -v docker-compose >/dev/null 2>&1; then
    COMPOSE=(docker-compose)
  else
    COMPOSE=()
  fi
}

run_setup_cli() {
  local -a setup_args=(
    setup
    --admin-email="${ADMIN_EMAIL}"
    --admin-password="${ADMIN_PASSWORD}"
    --locale="${DEFAULT_LOCALE}"
    --timezone="${DEFAULT_TIMEZONE}"
    --registration-mode="${REGISTRATION_MODE}"
    --instance-name="${INSTANCE_NAME}"
  )
  if [[ -n "${BASE_URL}" ]]; then
    setup_args+=(--base-url="${BASE_URL}")
  fi
  if [[ "${ASSUME_YES}" -eq 1 ]]; then
    setup_args+=(--yes)
  fi

  resolve_compose
  if [[ ${#COMPOSE[@]} -gt 0 ]] && docker info >/dev/null 2>&1; then
    # Prefer compose stack when available
    if [[ -f docker-compose.yml ]]; then
      log "Running setup via docker compose..."
      # Ensure env is loaded by compose; also pass args explicitly
      if [[ -f .env ]]; then
        # shellcheck disable=SC1091
        set -a
        # shellcheck source=/dev/null
        source .env
        set +a
      fi
      "${COMPOSE[@]}" run --rm api vulyk-api "${setup_args[@]}"
      return 0
    fi
  fi

  need_cmd cargo
  log "Docker unavailable - running setup via cargo..."
  if [[ -f .env ]]; then
    # shellcheck disable=SC1091
    set -a
    # shellcheck source=/dev/null
    source .env
    set +a
  elif [[ -f .env.prod ]]; then
    # shellcheck disable=SC1091
    set -a
    # shellcheck source=/dev/null
    source .env.prod
    set +a
  fi
  export VULYK_ADMIN_EMAIL="${ADMIN_EMAIL}"
  export VULYK_ADMIN_PASSWORD="${ADMIN_PASSWORD}"
  export VULYK_DEFAULT_LOCALE="${DEFAULT_LOCALE}"
  export VULYK_DEFAULT_TIMEZONE="${DEFAULT_TIMEZONE}"
  export VULYK_REGISTRATION_MODE="${REGISTRATION_MODE}"
  export VULYK_INSTANCE_NAME="${INSTANCE_NAME}"
  [[ -n "${BASE_URL}" ]] && export VULYK_BASE_URL="${BASE_URL}"
  cargo run -p vulyk-api -- "${setup_args[@]}"
}

# ---------------------------------------------------------------------------
# Compose helpers (upgrade / backup / restore)
# ---------------------------------------------------------------------------
detect_compose_files() {
  # Sets COMPOSE_FILES (array of -f args) and COMPOSE_ENV_FILE
  COMPOSE_FILES=()
  COMPOSE_ENV_FILE=""
  if [[ -f .env.prod ]] && { [[ -f docker-compose.prod.yml ]]; }; then
    # Prefer prod when .env.prod exists and stack looks prod-oriented
    if docker compose -f docker-compose.prod.yml --env-file .env.prod ps -q db >/dev/null 2>&1 \
      && [[ -n "$(docker compose -f docker-compose.prod.yml --env-file .env.prod ps -q db 2>/dev/null || true)" ]]; then
      COMPOSE_FILES=(-f docker-compose.prod.yml)
      COMPOSE_ENV_FILE=".env.prod"
      return 0
    fi
  fi
  if [[ -f docker-compose.yml ]]; then
    COMPOSE_FILES=(-f docker-compose.yml)
    if [[ -f .env ]]; then
      COMPOSE_ENV_FILE=".env"
    fi
  fi
}

compose_cmd() {
  resolve_compose
  [[ ${#COMPOSE[@]} -gt 0 ]] || die "Docker Compose plugin or docker-compose required"
  detect_compose_files
  [[ ${#COMPOSE_FILES[@]} -gt 0 ]] || die "no docker-compose.yml found"
  if [[ -n "${COMPOSE_ENV_FILE}" ]]; then
    "${COMPOSE[@]}" "${COMPOSE_FILES[@]}" --env-file "${COMPOSE_ENV_FILE}" "$@"
  else
    "${COMPOSE[@]}" "${COMPOSE_FILES[@]}" "$@"
  fi
}

current_git_version() {
  if command -v git >/dev/null 2>&1 && git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    git describe --tags --always --dirty 2>/dev/null || git rev-parse --short HEAD 2>/dev/null || echo "unknown"
  else
    echo "unknown"
  fi
}

api_reported_version() {
  local url="${VULYK_API_URL:-http://127.0.0.1:8080}"
  if command -v curl >/dev/null 2>&1; then
    curl -fsS "${url}/health" 2>/dev/null | sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n1
  fi
}

disk_free_mb() {
  # Portable: prefer df -Pk (POSIX), fall back to df -m
  local avail
  avail="$(df -Pk "${ROOT}" 2>/dev/null | awk 'NR==2 {print int($4/1024)}')"
  if [[ -z "${avail}" || "${avail}" -eq 0 ]]; then
    avail="$(df -m "${ROOT}" 2>/dev/null | awk 'NR==2 {print $4}')"
  fi
  echo "${avail:-0}"
}

ensure_backup_dir() {
  mkdir -p "${BACKUP_DIR}"
}

prune_old_backups() {
  local keep="${BACKUP_RETENTION}"
  local count
  ensure_backup_dir
  count=0
  # Portable (bash 3.2 / macOS): glob + mtime sort via ls -t
  shopt -s nullglob 2>/dev/null || true
  local -a files=( "${BACKUP_DIR}"/vulyk-*.sql.gz )
  count="${#files[@]}"
  if [[ "${count}" -le "${keep}" ]]; then
    return 0
  fi
  log "Pruning backups (keep last ${keep})..."
  # shellcheck disable=SC2207
  files=( $(ls -t "${BACKUP_DIR}"/vulyk-*.sql.gz 2>/dev/null) )
  local idx
  idx="${keep}"
  while [[ "${idx}" -lt "${#files[@]}" ]]; do
    rm -f "${files[$idx]}"
    log "Removed old backup: ${files[$idx]}"
    idx=$((idx + 1))
  done
}

wait_for_db() {
  local _
  for _ in $(seq 1 60); do
    if compose_cmd exec -T db pg_isready -U "${POSTGRES_USER:-vulyk}" -d "${POSTGRES_DB:-vulyk}" >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done
  die "Postgres is not ready (pg_isready timed out)"
}

run_pg_dump_backup() {
  local version_label="$1"
  local ts out
  ensure_backup_dir
  ts="$(date -u +%Y%m%dT%H%M%SZ 2>/dev/null || date +%Y%m%dT%H%M%S)"
  version_label="$(printf '%s' "${version_label}" | tr -c 'A-Za-z0-9._-' '_')"
  out="${BACKUP_DIR}/vulyk-${version_label}-${ts}.sql.gz"
  log "Creating backup: ${out}"
  wait_for_db
  # Plain SQL gzip so restore works with psql (no custom-format tools required)
  if ! compose_cmd exec -T db \
    pg_dump -U "${POSTGRES_USER:-vulyk}" -d "${POSTGRES_DB:-vulyk}" --no-owner --no-acl \
    | gzip -c >"${out}"; then
    rm -f "${out}"
    die "pg_dump failed"
  fi
  [[ -s "${out}" ]] || die "backup file is empty: ${out}"
  prune_old_backups
  log "Backup OK: ${out}"
  printf '%s\n' "${out}"
}

confirm_or_yes() {
  local prompt="$1"
  if [[ "${ASSUME_YES}" -eq 1 ]]; then
    return 0
  fi
  if [[ ! -t 0 ]]; then
    die "refusing non-interactive restore without --yes"
  fi
  local reply
  printf '%s [y/N]: ' "${prompt}" >&2
  read -r reply || true
  case "$(printf '%s' "${reply}" | tr '[:upper:]' '[:lower:]')" in
    y|yes) return 0 ;;
    *) die "aborted" ;;
  esac
}

cmd_backup() {
  resolve_compose
  [[ ${#COMPOSE[@]} -gt 0 ]] || die "Docker Compose required for backup"
  need_cmd gzip
  local ver
  ver="$(api_reported_version)"
  [[ -n "${ver}" ]] || ver="$(current_git_version)"
  run_pg_dump_backup "${ver}"
}

cmd_restore() {
  local file="${RESTORE_FILE:-}"
  [[ -n "${file}" ]] || die "usage: ./scripts/install.sh restore <backup.sql.gz>"
  [[ -f "${file}" ]] || die "backup not found: ${file}"
  resolve_compose
  [[ ${#COMPOSE[@]} -gt 0 ]] || die "Docker Compose required for restore"
  need_cmd gzip
  confirm_or_yes "Restore ${file} into the running Postgres (DESTROYS current DB data)?"
  wait_for_db
  log "Stopping API/web so connections drop..."
  compose_cmd stop api web 2>/dev/null || true
  log "Restoring..."
  # Drop/recreate public schema then apply dump
  compose_cmd exec -T db \
    psql -U "${POSTGRES_USER:-vulyk}" -d "${POSTGRES_DB:-vulyk}" -v ON_ERROR_STOP=1 \
    -c "DROP SCHEMA public CASCADE; CREATE SCHEMA public; GRANT ALL ON SCHEMA public TO ${POSTGRES_USER:-vulyk}; GRANT ALL ON SCHEMA public TO public;"
  gzip -dc "${file}" | compose_cmd exec -T db \
    psql -U "${POSTGRES_USER:-vulyk}" -d "${POSTGRES_DB:-vulyk}" -v ON_ERROR_STOP=1
  log "Restore complete. Starting stack..."
  compose_cmd up -d
  log "Done. Verify: curl -fsS \${VULYK_API_URL:-http://127.0.0.1:8080}/health"
}

print_restore_hint() {
  local backup_file="${1:-}"
  cat <<EOF >&2

========================================================================
UPGRADE FAILED - Postgres data volume was NOT removed.
Never run: docker compose down -v   (the -v flag DELETES your database)

To restore from backup:
  ./scripts/install.sh restore ${backup_file:-./backups/vulyk-VERSION-TIMESTAMP.sql.gz} --yes

See docs/guides/upgrade.md
========================================================================
EOF
}

cmd_upgrade() {
  local backup_file="" ver free_mb pending_rc=0
  resolve_compose
  [[ ${#COMPOSE[@]} -gt 0 ]] || die "Docker Compose required for upgrade"
  need_cmd git
  need_cmd curl
  need_cmd gzip
  need_cmd docker

  log "Upgrade preflight..."
  docker info >/dev/null 2>&1 || die "Docker daemon not reachable"
  free_mb="$(disk_free_mb)"
  log "Free disk: ${free_mb} MB"
  if [[ "${free_mb}" -gt 0 && "${free_mb}" -lt 1024 ]]; then
    warn "Less than 1 GB free disk; upgrade may fail"
    if [[ "${ASSUME_YES}" -eq 0 ]]; then
      confirm_or_yes "Continue with low disk space?"
    fi
  fi

  ver="$(api_reported_version)"
  [[ -n "${ver}" ]] || ver="$(current_git_version)"
  log "Current version: ${ver}"
  log "Git HEAD: $(current_git_version)"

  # Ensure DB is up for status / backup (do not start new API yet)
  detect_compose_files
  log "Ensuring database is running..."
  compose_cmd up -d db
  wait_for_db

  # Load env for POSTGRES_* when using prod
  if [[ -n "${COMPOSE_ENV_FILE}" && -f "${COMPOSE_ENV_FILE}" ]]; then
    # shellcheck disable=SC1091
    set -a
    # shellcheck source=/dev/null
    source "${COMPOSE_ENV_FILE}"
    set +a
  fi

  log "Migration status (current image, if available)..."
  set +e
  compose_cmd run --rm --no-deps api vulyk-api migrate status
  pending_rc=$?
  set -e
  if [[ "${pending_rc}" -eq 2 ]]; then
    die "database schema is newer than the current API binary (refusing upgrade)"
  fi

  if [[ "${NO_BACKUP}" -eq 1 ]]; then
    warn "Skipping backup (--no-backup)"
  else
    backup_file="$(run_pg_dump_backup "${ver}")"
  fi

  if [[ "${ASSUME_YES}" -eq 0 ]]; then
    confirm_or_yes "Proceed with upgrade${UPGRADE_TO:+ to ${UPGRADE_TO}}?"
  fi

  if [[ -n "${UPGRADE_TO}" ]]; then
    log "Checking out ${UPGRADE_TO}..."
    git fetch --tags --prune origin 2>/dev/null || git fetch --tags --prune 2>/dev/null || true
    git checkout "${UPGRADE_TO}" || die "git checkout ${UPGRADE_TO} failed"
  else
    log "Pulling latest for current branch..."
    if git rev-parse --abbrev-ref HEAD >/dev/null 2>&1; then
      local branch
      branch="$(git rev-parse --abbrev-ref HEAD)"
      if [[ "${branch}" != "HEAD" ]]; then
        git pull --ff-only origin "${branch}" 2>/dev/null \
          || git pull --ff-only 2>/dev/null \
          || warn "git pull failed (continuing with local tree)"
      fi
    fi
  fi

  log "Stopping API/web (keeping database volume)..."
  # NEVER use down -v - volumes must be preserved
  compose_cmd stop api web 2>/dev/null || true

  log "Rebuilding images..."
  if ! compose_cmd build api web; then
    print_restore_hint "${backup_file}"
    die "image build failed"
  fi

  log "Running migrations (one-shot, before starting API)..."
  if ! compose_cmd run --rm --no-deps \
    -e VULYK_AUTO_MIGRATE=false \
    api vulyk-api migrate run; then
    print_restore_hint "${backup_file}"
    die "migrate run failed"
  fi

  log "Starting updated stack..."
  if ! compose_cmd up -d; then
    print_restore_hint "${backup_file}"
    die "compose up failed"
  fi

  log "Health check..."
  local health_url
  health_url="${VULYK_API_URL:-http://127.0.0.1:8080}/health"
  # Prod may use a different published port
  if [[ -n "${API_PORT:-}" ]]; then
    health_url="http://127.0.0.1:${API_PORT}/health"
  fi
  for _ in $(seq 1 90); do
    if curl -fsS "${health_url}" >/dev/null 2>&1; then
      local new_ver
      new_ver="$(api_reported_version)"
      log "Upgrade complete."
      log "Health: ${health_url} OK (version=${new_ver:-unknown})"
      if [[ -n "${backup_file}" ]]; then
        log "Backup kept at: ${backup_file}"
      fi
      cat <<EOF

Next:
  curl -fsS ${health_url}
  ./scripts/install.sh backup     # optional extra backup after upgrade

WARNING: do not run 'docker compose down -v' - that deletes Postgres data.
EOF
      return 0
    fi
    sleep 2
  done
  print_restore_hint "${backup_file}"
  die "API health check failed after upgrade"
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
OS="$(detect_os)"
log "Detected OS: ${OS}"

parse_args "$@"

case "${MODE}" in
  compose|prod|dev|setup|check|helm-template|perf-smoke|upgrade|backup|restore)
    ;;
  -h|--help|help)
    usage
    exit 0
    ;;
  *)
    die "unknown mode: ${MODE} (try --help)"
    ;;
esac

if [[ "${MODE}" == "backup" ]]; then
  cmd_backup
  exit 0
fi

if [[ "${MODE}" == "restore" ]]; then
  cmd_restore
  exit 0
fi

if [[ "${MODE}" == "upgrade" ]]; then
  cmd_upgrade
  exit 0
fi

if [[ "${MODE}" == "check" || "${MODE}" == "dev" || "${MODE}" == "perf-smoke" ]]; then
  need_cmd rustc
  need_cmd cargo
  need_cmd node
  need_cmd npm
  log "rustc $(rustc --version)"
  log "node $(node --version)"
fi

if [[ "${MODE}" == "check" ]]; then
  if command -v docker >/dev/null 2>&1; then
    log "docker $(docker --version)"
    if docker compose version >/dev/null 2>&1; then
      log "docker compose available"
    elif command -v docker-compose >/dev/null 2>&1; then
      log "docker-compose available"
    else
      warn "Docker Compose plugin/binary not found"
    fi
  else
    log "docker not found (optional for host-only dev)"
  fi
  if command -v helm >/dev/null 2>&1; then
    log "helm $(helm version --short 2>/dev/null || helm version)"
  else
    log "helm not found (optional; used by helm-template)"
  fi
  # Shell syntax self-check (works on bash 3.2+ / macOS)
  bash -n "$0"
  log "install.sh syntax OK"
  log "Toolchain OK for ${OS}"
  exit 0
fi

if [[ "${MODE}" == "helm-template" ]]; then
  need_cmd helm
  log "Rendering Helm chart..."
  helm template vulyk deploy/helm/vulyk \
    --set secrets.create=true \
    --set auth.required=true \
    --set setup.adminPassword=change-me-admin1 \
    >/tmp/vulyk-helm-render.yaml
  # Basic sanity: auth required + core resources present
  grep -q 'VULYK_AUTH_REQUIRED: "true"' /tmp/vulyk-helm-render.yaml \
    || die "Helm render missing VULYK_AUTH_REQUIRED=true"
  grep -q 'kind: Deployment' /tmp/vulyk-helm-render.yaml \
    || die "Helm render missing Deployments"
  grep -q 'kind: Job' /tmp/vulyk-helm-render.yaml \
    || die "Helm render missing Job"
  grep -q 'component: migrate' /tmp/vulyk-helm-render.yaml \
    || die "Helm render missing migrate Job"
  grep -q 'migrate", "run"' /tmp/vulyk-helm-render.yaml \
    || die "Helm render missing migrate run args"
  grep -q 'VULYK_AUTO_MIGRATE: "false"' /tmp/vulyk-helm-render.yaml \
    || die "Helm render missing VULYK_AUTO_MIGRATE=false"
  grep -q 'helm.sh/resource-policy: keep' /tmp/vulyk-helm-render.yaml \
    || die "Helm render missing PVC keep policy"
  grep -q 'VULYK_REGISTRATION_MODE' /tmp/vulyk-helm-render.yaml \
    || die "Helm render missing VULYK_REGISTRATION_MODE"
  log "Helm chart rendered OK -> /tmp/vulyk-helm-render.yaml"
  exit 0
fi

if [[ "${MODE}" == "perf-smoke" ]]; then
  exec "${ROOT}/scripts/perf-smoke.sh" "${@:2}"
fi

if [[ "${MODE}" == "setup" ]]; then
  # Load existing env for defaults if present
  if [[ -f .env ]]; then
    # shellcheck disable=SC1091
    set -a
    # shellcheck source=/dev/null
    source .env
    set +a
    ADMIN_EMAIL="${ADMIN_EMAIL:-${VULYK_ADMIN_EMAIL:-}}"
    ADMIN_PASSWORD="${ADMIN_PASSWORD:-${VULYK_ADMIN_PASSWORD:-}}"
    DEFAULT_LOCALE="${DEFAULT_LOCALE:-${VULYK_DEFAULT_LOCALE:-}}"
    DEFAULT_TIMEZONE="${DEFAULT_TIMEZONE:-${VULYK_DEFAULT_TIMEZONE:-}}"
    REGISTRATION_MODE="${REGISTRATION_MODE:-${VULYK_REGISTRATION_MODE:-}}"
    INSTANCE_NAME="${INSTANCE_NAME:-${VULYK_INSTANCE_NAME:-}}"
    SEED_DEMO="${SEED_DEMO:-${VULYK_SEED_DEMO:-}}"
    BASE_URL="${BASE_URL:-${VULYK_BASE_URL:-}}"
  fi
  collect_setup_options "compose"
  write_setup_env ".env"
  run_setup_cli
  exit 0
fi

if [[ "${MODE}" == "dev" ]]; then
  collect_setup_options "dev"
  ensure_compose_env ".env"
  resolve_compose
  if [[ ${#COMPOSE[@]} -gt 0 ]]; then
    log "Starting Postgres (dev overlay)..."
    "${COMPOSE[@]}" -f docker-compose.dev.yml up -d
  else
    warn "Docker Compose not found - skipping Postgres container; install Docker or start Postgres yourself"
  fi
  cat <<EOF

Postgres is up on localhost:5432 (user/pass/db: vulyk).
First-run settings written to .env.

In one terminal:
  set -a && source .env && set +a
  export DATABASE_URL=postgres://vulyk:vulyk@localhost:5432/vulyk
  cargo run -p vulyk-api
  # (env seeding creates admin when VULYK_ADMIN_PASSWORD is set)

  # Or run the setup CLI explicitly:
  cargo run -p vulyk-api -- setup --yes
  # Or: ./scripts/install.sh setup --yes

In another:
  cd frontend && npm install && npm run dev

API docs: http://localhost:8080/api/docs
UI:       http://localhost:5173
EOF
  print_setup_next_steps "dev"
  exit 0
fi

if [[ "${MODE}" == "prod" ]]; then
  collect_setup_options "prod"
  ensure_prod_env ".env.prod"
  resolve_compose
  [[ ${#COMPOSE[@]} -gt 0 ]] || die "Docker Compose plugin or docker-compose required"
  log "Building and starting Vulyk production Compose..."
  "${COMPOSE[@]}" -f docker-compose.prod.yml --env-file .env.prod up --build -d
  # shellcheck disable=SC1091
  set -a
  # shellcheck source=/dev/null
  source .env.prod
  set +a
  cat <<EOF

Vulyk production stack is starting (auth required).

  UI:        http://localhost:${WEB_PORT:-80}
  API:       http://localhost:${API_PORT:-8080}
  OpenAPI:   http://localhost:${API_PORT:-8080}/api/docs
  Health:    http://localhost:${API_PORT:-8080}/health

Admin seeding: VULYK_ADMIN_* from .env.prod (registration=${REGISTRATION_MODE}, seed_demo=${SEED_DEMO}).
If setup did not complete via env seed, run:
  ./scripts/install.sh setup --yes
  # or: docker compose -f docker-compose.prod.yml --env-file .env.prod run --rm api vulyk-api setup --yes

Logs:  docker compose -f docker-compose.prod.yml logs -f
Stop:  docker compose -f docker-compose.prod.yml down
       (NEVER use down -v - that deletes Postgres volumes)
Upgrade: ./scripts/install.sh upgrade --yes
Backups: ./scripts/install.sh backup  (see docs/guides/upgrade.md)
EOF
  print_setup_next_steps "prod"
  exit 0
fi

# Default: compose - write .env first, then require Docker
collect_setup_options "compose"
ensure_compose_env ".env"
resolve_compose
[[ ${#COMPOSE[@]} -gt 0 ]] || die "Docker Compose plugin or docker-compose required (first-run settings already in .env)"
log "Building and starting Vulyk (docker compose)..."
"${COMPOSE[@]}" --env-file .env up --build -d

cat <<EOF

Vulyk is starting (dev compose - auth optional).

  UI:        http://localhost:5173
  API:       http://localhost:8080
  OpenAPI:   http://localhost:8080/api/docs
  Health:    http://localhost:8080/health

Admin/demo seeding uses .env (VULYK_ADMIN_EMAIL=${ADMIN_EMAIL}, VULYK_SEED_DEMO=${SEED_DEMO}).
If setup did not complete automatically (or you need to re-run first-time configuration):
  ./scripts/install.sh setup --yes
  # or: docker compose run --rm api vulyk-api setup --yes

For production hardening: ./scripts/install.sh prod
Upgrade: ./scripts/install.sh upgrade --yes
Logs:  docker compose logs -f
Stop:  docker compose down
       (NEVER use down -v - that deletes the Postgres volume)
EOF
print_setup_next_steps "compose"
