#!/usr/bin/env bash
# Install / bootstrap Vulyk on Linux, macOS, or WSL2.
# Verified modes: compose | prod | dev | setup | check | helm-template | perf-smoke
#                 upgrade | backup | restore | rollback
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

BACKUP_DIR="${ROOT}/backups"
BACKUP_RETENTION="${VULYK_BACKUP_RETENTION:-10}"
# Version / rollback points (separate from ./backups and content backups from #30).
VERSION_BACKUP_DIR="${VULYK_VERSION_BACKUP_DIR:-${ROOT}/version-backups}"
ROLLBACK_RETENTION_DAYS="${VULYK_ROLLBACK_RETENTION_DAYS:-7}"
VERSION_BACKUP_COUNT="${VULYK_VERSION_BACKUP_COUNT:-3}"
# Minimum free disk (MB) before upgrade/backup. Override with VULYK_MIN_FREE_MB.
MIN_FREE_MB="${VULYK_MIN_FREE_MB:-2048}"
UPGRADE_TO=""
RELEASE_VERSION=""
NO_BACKUP=0
ALLOW_DOWNGRADE=0
RESTORE_FILE=""
ROLLBACK_CONFIRM="${VULYK_ROLLBACK_CONFIRM:-}"
# Override via env or --version (GHCR). Default owner derived from git remote when possible.
VULYK_GHCR_OWNER="${VULYK_GHCR_OWNER:-}"
VULYK_GHCR_API_IMAGE="${VULYK_GHCR_API_IMAGE:-}"
VULYK_GHCR_WEB_IMAGE="${VULYK_GHCR_WEB_IMAGE:-}"
# Snapshot of images before an upgrade (for auto-rollback).
PREV_API_IMAGE=""
PREV_WEB_IMAGE=""
VERSION_BACKUPS_PY="${ROOT}/scripts/version_backups.py"

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
  backup          Verified backup bundle under ./backups/vulyk-<version>-<timestamp>/
  restore PATH    Restore a backup bundle or legacy .sql.gz (confirmation unless --yes)
  rollback        Roll back to a post-upgrade rollback point (typed confirmation)

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

Upgrade / backup / release options:
  --version=X.Y.Z          Use published GHCR images for this semver (compose/prod/upgrade/rollback)
  --to=TAG|BRANCH          Checkout tag or branch and rebuild from source (upgrade)
  --no-backup              Skip automatic backup bundle (upgrade only; NOT recommended)
  --allow-downgrade        Permit target version < current (still refuses schema-newer DB;
                           prefer restore-from-backup instead)
  --confirm=ROLLBACK       Typed confirmation for rollback (or VULYK_ROLLBACK_CONFIRM)

Environment equivalents:
  VULYK_ADMIN_EMAIL, VULYK_ADMIN_PASSWORD, VULYK_DEFAULT_LOCALE,
  VULYK_DEFAULT_TIMEZONE, VULYK_REGISTRATION_MODE, VULYK_INSTANCE_NAME,
  VULYK_SEED_DEMO, VULYK_BASE_URL, VULYK_BACKUP_RETENTION,
  VULYK_VERSION_BACKUP_DIR, VULYK_ROLLBACK_RETENTION_DAYS, VULYK_VERSION_BACKUP_COUNT,
  VULYK_GHCR_OWNER, VULYK_GHCR_API_IMAGE, VULYK_GHCR_WEB_IMAGE

Written into .env (compose/dev) or .env.prod (prod). Defaults:
  compose/dev: seed demo=1, registration=open, admin@vulyk.local / admin
  prod:        seed demo=0, registration=disabled

After compose/prod/dev, if setup did not complete automatically:
  ./scripts/install.sh setup --yes
  # or: docker compose run --rm api vulyk-api setup --yes ...
  # or: cargo run -p vulyk-api -- setup ...

Install a published release (pulls GHCR; no local Rust/Node build):
  ./scripts/install.sh compose --yes --version 0.1.0
  ./scripts/install.sh prod --yes --version 0.1.0

Upgrade (keeps the named Postgres volume; never runs docker compose down -v):
  ./scripts/install.sh upgrade --yes
  ./scripts/install.sh upgrade --yes --version 0.1.0   # GHCR images
  ./scripts/install.sh upgrade --yes --to=v0.2.0       # git checkout + source build
  ./scripts/install.sh backup
  ./scripts/install.sh restore ./backups/vulyk-0.1.0-20260101T120000Z.sql.gz
  ./scripts/install.sh rollback --confirm=ROLLBACK
  ./scripts/install.sh rollback --version 0.1.0 --confirm=ROLLBACK

WARNING: never run 'docker compose down -v' on a data you care about -
the -v flag deletes named volumes (Postgres data).

See docs/guides/first-run-setup.md, docs/guides/upgrade.md, docs/guides/releasing.md

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
      --version=*)
        RELEASE_VERSION="${arg#*=}"
        ;;
      --version)
        [[ $# -gt 0 ]] || die "--version requires a semver (e.g. 0.1.0)"
        RELEASE_VERSION="$1"
        shift
        ;;
      --no-backup)
        NO_BACKUP=1
        ;;
      --allow-downgrade)
        ALLOW_DOWNGRADE=1
        ;;
      --confirm=*)
        ROLLBACK_CONFIRM="${arg#*=}"
        ;;
      --confirm)
        [[ $# -gt 0 ]] || die "--confirm requires a value (ROLLBACK)"
        ROLLBACK_CONFIRM="$1"
        shift
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

# Only add missing keys; never overwrite existing values (upgrade / re-run safe).
env_add_if_missing() {
  local file="$1" key="$2" value="$3"
  touch "${file}"
  if grep -qE "^${key}=" "${file}" 2>/dev/null; then
    return 0
  fi
  printf '%s=%s\n' "${key}" "${value}" >>"${file}"
  warn "Added missing config key ${key} to ${file} (default only; existing keys untouched)"
}

# Write first-run setup keys. When preserve_existing=1, never overwrite keys that
# are already set (upgrade / re-run safe). Secrets are never regenerated here.
write_setup_env() {
  local env_file="$1"
  local preserve_existing="${2:-0}"
  touch "${env_file}"
  if [[ "${preserve_existing}" -eq 1 ]]; then
    env_add_if_missing "${env_file}" "VULYK_ADMIN_EMAIL" "${ADMIN_EMAIL}"
    # Never fill in admin password on re-run - empty would be dangerous; skip if unset
    if [[ -n "${ADMIN_PASSWORD}" ]]; then
      env_add_if_missing "${env_file}" "VULYK_ADMIN_PASSWORD" "${ADMIN_PASSWORD}"
    fi
    env_add_if_missing "${env_file}" "VULYK_DEFAULT_LOCALE" "${DEFAULT_LOCALE}"
    env_add_if_missing "${env_file}" "VULYK_DEFAULT_TIMEZONE" "${DEFAULT_TIMEZONE}"
    env_add_if_missing "${env_file}" "VULYK_REGISTRATION_MODE" "${REGISTRATION_MODE}"
    env_add_if_missing "${env_file}" "VULYK_INSTANCE_NAME" "${INSTANCE_NAME}"
    env_add_if_missing "${env_file}" "VULYK_SEED_DEMO" "${SEED_DEMO}"
    if [[ -n "${BASE_URL}" ]]; then
      env_add_if_missing "${env_file}" "VULYK_BASE_URL" "${BASE_URL}"
    fi
    log "Preserved existing settings in ${env_file} (only missing keys added)"
  else
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
    log "Wrote first-run settings to ${env_file}"
  fi
  chmod 600 "${env_file}" 2>/dev/null || true
}

ensure_prod_env() {
  local env_file="${1:-.env.prod}"
  if [[ -f "${env_file}" ]]; then
    log "Using existing ${env_file} (will not regenerate secrets or overwrite admin settings)"
    write_setup_env "${env_file}" 1
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
    write_setup_env "${env_file}" 0
  fi
}

ensure_compose_env() {
  local env_file="${1:-.env}"
  if [[ -f "${env_file}" ]]; then
    log "Using existing ${env_file} (will not overwrite secrets or admin settings)"
    write_setup_env "${env_file}" 1
  else
    log "Creating ${env_file}..."
    cat >"${env_file}" <<EOF
# Generated by scripts/install.sh - local/dev Compose
VULYK_AUTH_REQUIRED=false
VULYK_JWT_SECRET=vulyk-compose-dev-secret
VULYK_SECRETS_KEY=vulyk-compose-dev-secrets
VULYK_LLM_MOCK=1
EOF
    write_setup_env "${env_file}" 0
  fi
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

# Normalize --version / tag input to bare semver (strip leading v).
normalize_release_version() {
  local raw="$1"
  raw="$(printf '%s' "${raw}" | tr -d '[:space:]')"
  [[ -n "${raw}" ]] || die "empty --version"
  raw="${raw#v}"
  [[ "${raw}" =~ ^[0-9]+\.[0-9]+\.[0-9]+([.-].*)?$ ]] \
    || die "invalid --version '${1}' (expected semver like 0.1.0)"
  printf '%s' "${raw}"
}

detect_ghcr_owner() {
  if [[ -n "${VULYK_GHCR_OWNER}" ]]; then
    printf '%s' "$(printf '%s' "${VULYK_GHCR_OWNER}" | tr '[:upper:]' '[:lower:]')"
    return 0
  fi
  local url owner
  if command -v git >/dev/null 2>&1 && git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    url="$(git remote get-url origin 2>/dev/null || true)"
    # git@github.com:Owner/Repo.git or https://github.com/Owner/Repo.git
    owner="$(printf '%s' "${url}" | sed -nE 's#.*(github\.com[:/])([^/]+)/.*#\2#p')"
    if [[ -n "${owner}" ]]; then
      printf '%s' "$(printf '%s' "${owner}" | tr '[:upper:]' '[:lower:]')"
      return 0
    fi
  fi
  printf '%s' "ihor-meat-proxy"
}

# Sets VULYK_API_IMAGE / VULYK_WEB_IMAGE for a published release (and exports them).
apply_release_images() {
  local version owner api_img web_img
  version="$(normalize_release_version "$1")"
  RELEASE_VERSION="${version}"
  owner="$(detect_ghcr_owner)"
  api_img="${VULYK_GHCR_API_IMAGE:-ghcr.io/${owner}/vulyk-api:${version}}"
  web_img="${VULYK_GHCR_WEB_IMAGE:-ghcr.io/${owner}/vulyk-web:${version}}"
  export VULYK_API_IMAGE="${api_img}"
  export VULYK_WEB_IMAGE="${web_img}"
  log "Using release images for v${version}:"
  log "  API: ${VULYK_API_IMAGE}"
  log "  web: ${VULYK_WEB_IMAGE}"
}

persist_release_images_env() {
  local env_file="$1"
  [[ -n "${VULYK_API_IMAGE:-}" ]] || return 0
  touch "${env_file}"
  env_upsert "${env_file}" "VULYK_API_IMAGE" "${VULYK_API_IMAGE}"
  env_upsert "${env_file}" "VULYK_WEB_IMAGE" "${VULYK_WEB_IMAGE}"
}

pull_release_images() {
  need_cmd docker
  [[ -n "${VULYK_API_IMAGE:-}" && -n "${VULYK_WEB_IMAGE:-}" ]] \
    || die "internal: release images not set"
  log "Pulling GHCR images (docker login ghcr.io if the packages are private)..."
  if ! docker pull "${VULYK_API_IMAGE}"; then
    die "failed to pull ${VULYK_API_IMAGE} (check version, network, or ghcr.io auth)"
  fi
  if ! docker pull "${VULYK_WEB_IMAGE}"; then
    die "failed to pull ${VULYK_WEB_IMAGE} (check version, network, or ghcr.io auth)"
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

# Prune oldest backup bundles (directories) and legacy flat *.sql.gz files.
# Never overwrites an existing bundle; timestamps make names unique.
prune_old_backups() {
  local keep="${BACKUP_RETENTION}"
  ensure_backup_dir
  shopt -s nullglob 2>/dev/null || true
  local -a bundles=()
  local -a legacy=()
  local path
  for path in "${BACKUP_DIR}"/vulyk-*; do
    [[ -e "${path}" ]] || continue
    if [[ -d "${path}" ]]; then
      bundles+=("${path}")
    elif [[ -f "${path}" && "${path}" == *.sql.gz ]]; then
      legacy+=("${path}")
    fi
  done
  # shellcheck disable=SC2207
  if [[ "${#bundles[@]}" -gt 0 ]]; then
    bundles=( $(ls -dt "${bundles[@]}" 2>/dev/null) )
  fi
  # shellcheck disable=SC2207
  if [[ "${#legacy[@]}" -gt 0 ]]; then
    legacy=( $(ls -t "${legacy[@]}" 2>/dev/null) )
  fi
  local idx
  if [[ "${#bundles[@]}" -gt "${keep}" ]]; then
    log "Pruning backup bundles (keep last ${keep})..."
    idx="${keep}"
    while [[ "${idx}" -lt "${#bundles[@]}" ]]; do
      rm -rf "${bundles[$idx]}"
      log "Removed old backup bundle: ${bundles[$idx]}"
      idx=$((idx + 1))
    done
  fi
  if [[ "${#legacy[@]}" -gt "${keep}" ]]; then
    log "Pruning legacy .sql.gz backups (keep last ${keep})..."
    idx="${keep}"
    while [[ "${idx}" -lt "${#legacy[@]}" ]]; do
      rm -f "${legacy[$idx]}"
      log "Removed old legacy backup: ${legacy[$idx]}"
      idx=$((idx + 1))
    done
  fi
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

# Compare dotted semver-ish strings: echo -1 / 0 / 1 for a<b / a==b / a>b.
semver_cmp() {
  local a="${1#v}" b="${2#v}"
  python3 - "$a" "$b" <<'PY'
import sys
def parts(s):
    out=[]
    for p in s.split("."):
        num=""
        rest=p
        while rest and rest[0].isdigit():
            num+=rest[0]; rest=rest[1:]
        out.append((int(num or 0), rest))
    return out
a,b=sys.argv[1],sys.argv[2]
pa,pb=parts(a),parts(b)
n=max(len(pa),len(pb))
pa+=[(0,"")]*(n-len(pa)); pb+=[(0,"")]*(n-len(pb))
for x,y in zip(pa,pb):
    if x<y: print(-1); raise SystemExit
    if x>y: print(1); raise SystemExit
print(0)
PY
}

# Integrity-check a gzip SQL dump by restoring into a temporary database, then drop it.
verify_sql_dump() {
  local dump="$1"
  local verify_db ts
  [[ -f "${dump}" ]] || die "dump not found: ${dump}"
  log "Verifying backup dump (gzip + restore into temp DB)..."
  gzip -t "${dump}" || die "gzip integrity check failed: ${dump}"
  ts="$(date -u +%Y%m%d%H%M%S 2>/dev/null || date +%Y%m%d%H%M%S)"
  verify_db="vulyk_verify_${ts}"
  compose_cmd exec -T db \
    psql -U "${POSTGRES_USER:-vulyk}" -d postgres -v ON_ERROR_STOP=1 \
    -c "DROP DATABASE IF EXISTS ${verify_db};" \
    -c "CREATE DATABASE ${verify_db} OWNER ${POSTGRES_USER:-vulyk};" \
    >/dev/null
  if ! gzip -dc "${dump}" | compose_cmd exec -T db \
    psql -U "${POSTGRES_USER:-vulyk}" -d "${verify_db}" -v ON_ERROR_STOP=1 -q >/dev/null; then
    compose_cmd exec -T db \
      psql -U "${POSTGRES_USER:-vulyk}" -d postgres -c "DROP DATABASE IF EXISTS ${verify_db};" \
      >/dev/null 2>&1 || true
    die "backup restore verification failed (dump may be corrupt): ${dump}"
  fi
  local mig_count
  mig_count="$(compose_cmd exec -T db \
    psql -U "${POSTGRES_USER:-vulyk}" -d "${verify_db}" -Atc \
    "SELECT COUNT(*) FROM _sqlx_migrations;" 2>/dev/null | tr -d '[:space:]')"
  compose_cmd exec -T db \
    psql -U "${POSTGRES_USER:-vulyk}" -d postgres -c "DROP DATABASE IF EXISTS ${verify_db};" \
    >/dev/null
  [[ -n "${mig_count}" && "${mig_count}" -gt 0 ]] \
    || die "verified dump has no _sqlx_migrations rows: ${dump}"
  log "Backup verified (migrations=${mig_count})"
}

# Resolve docker volume name for attachments (project-prefixed).
attachments_volume_name() {
  local proj
  proj="$(docker compose ls -q 2>/dev/null | head -n1 || true)"
  if [[ -z "${proj}" ]]; then
    proj="$(basename "${ROOT}" | tr '[:upper:]' '[:lower:]' | tr -cd 'a-z0-9_-')"
  fi
  # Prefer known compose volume names
  local cand
  for cand in "${proj}_vulyk_attachments" vulyk_attachments "${proj}_vulyk_pg_data"; do
    if docker volume inspect "${cand}" >/dev/null 2>&1; then
      if [[ "${cand}" == *attachments* ]]; then
        printf '%s' "${cand}"
        return 0
      fi
    fi
  done
  for cand in $(docker volume ls -q 2>/dev/null | grep -E 'vulyk_attachments$' || true); do
    printf '%s' "${cand}"
    return 0
  done
  return 1
}

# Full upgrade backup bundle: DB dump (verified) + env/config + image pins + attachments.
# Prints the bundle directory path on stdout (last line); logs go to stderr via log().
run_full_backup() {
  local version_label="$1"
  local ts bundle dump
  ensure_backup_dir
  ts="$(date -u +%Y%m%dT%H%M%SZ 2>/dev/null || date +%Y%m%dT%H%M%S)"
  version_label="$(printf '%s' "${version_label}" | tr -c 'A-Za-z0-9._-' '_')"
  bundle="${BACKUP_DIR}/vulyk-${version_label}-${ts}"
  if [[ -e "${bundle}" ]]; then
    die "backup path already exists (refusing overwrite): ${bundle}"
  fi
  mkdir -p "${bundle}/config"
  dump="${bundle}/db.sql.gz"
  log "Creating verified backup bundle: ${bundle}"
  wait_for_db
  if ! compose_cmd exec -T db \
    pg_dump -U "${POSTGRES_USER:-vulyk}" -d "${POSTGRES_DB:-vulyk}" --no-owner --no-acl \
    | gzip -c >"${dump}"; then
    rm -rf "${bundle}"
    die "pg_dump failed"
  fi
  [[ -s "${dump}" ]] || { rm -rf "${bundle}"; die "backup dump is empty"; }
  verify_sql_dump "${dump}"

  # Config / secrets - copy only; never regenerate
  local f
  for f in .env .env.prod; do
    if [[ -f "${ROOT}/${f}" ]]; then
      cp -p "${ROOT}/${f}" "${bundle}/config/${f}"
      chmod 600 "${bundle}/config/${f}" 2>/dev/null || true
      log "Copied ${f} into backup (includes VULYK_SECRETS_KEY / JWT when set)"
    fi
  done
  {
    echo "# Image pins at backup time"
    echo "VULYK_API_IMAGE=${VULYK_API_IMAGE:-}"
    echo "VULYK_WEB_IMAGE=${VULYK_WEB_IMAGE:-}"
    echo "BACKUP_VERSION_LABEL=${version_label}"
    echo "BACKUP_CREATED_UTC=${ts}"
  } >"${bundle}/config/image-state.env"
  chmod 600 "${bundle}/config/image-state.env" 2>/dev/null || true

  # Warn loudly if secrets key missing from active env
  if [[ -n "${COMPOSE_ENV_FILE:-}" && -f "${COMPOSE_ENV_FILE}" ]]; then
    if ! grep -qE '^VULYK_SECRETS_KEY=.' "${COMPOSE_ENV_FILE}"; then
      warn "VULYK_SECRETS_KEY is missing or empty in ${COMPOSE_ENV_FILE} - encrypted secrets may not decrypt after restore"
    fi
    if ! grep -qE '^VULYK_JWT_SECRET=.' "${COMPOSE_ENV_FILE}"; then
      warn "VULYK_JWT_SECRET is missing or empty in ${COMPOSE_ENV_FILE}"
    fi
  fi

  # Attachments volume (optional; content also lives in DB today)
  local vol
  if vol="$(attachments_volume_name)"; then
    log "Archiving attachments volume ${vol}..."
    if docker run --rm \
      -v "${vol}:/data:ro" \
      -v "${bundle}:/out" \
      alpine:3.20 \
      tar -czf /out/attachments.tar.gz -C /data . >/dev/null 2>&1; then
      log "Attachments archive OK"
    else
      warn "Could not archive attachments volume (continuing; DB dump still has BYTEA attachments)"
    fi
  else
    log "No attachments volume found (OK if using DB-stored attachments only)"
  fi

  {
    local sums_tmp
    sums_tmp="$(mktemp)"
    if command -v sha256sum >/dev/null 2>&1; then
      ( cd "${bundle}" && find . -type f -print0 2>/dev/null | xargs -0 sha256sum ) >"${sums_tmp}" 2>/dev/null || true
    elif command -v shasum >/dev/null 2>&1; then
      ( cd "${bundle}" && find . -type f -print0 2>/dev/null | xargs -0 shasum -a 256 ) >"${sums_tmp}" 2>/dev/null || true
    fi
    mv "${sums_tmp}" "${bundle}/SHA256SUMS"
  }
  cat >"${bundle}/MANIFEST.txt" <<EOF
Vulyk backup bundle
created_utc=${ts}
version_label=${version_label}
db=db.sql.gz (gzip SQL; verified via temp-DB restore)
config=config/ (.env / .env.prod / image-state.env - never regenerate on upgrade)
attachments=attachments.tar.gz (optional volume archive)
retention=${BACKUP_RETENTION}
EOF
  prune_old_backups
  log "Backup bundle OK: ${bundle}"
  printf '%s\n' "${bundle}"
}

# Backward-compatible wrapper name used by older docs.
run_pg_dump_backup() {
  run_full_backup "$1"
}

# Merge additive defaults into an existing env file during upgrade. Never touches
# secrets, admin password, or any key that is already set.
merge_env_defaults_for_upgrade() {
  local env_file="$1"
  [[ -f "${env_file}" ]] || return 0
  log "Merging additive config defaults into ${env_file} (existing values preserved)..."
  # Safe additive keys only - never JWT/SECRETS/POSTGRES_PASSWORD/ADMIN_PASSWORD
  env_add_if_missing "${env_file}" "VULYK_AUTO_MIGRATE" "false"
  env_add_if_missing "${env_file}" "VULYK_JWT_TTL_HOURS" "12"
  env_add_if_missing "${env_file}" "VULYK_SESSION_TTL_HOURS" "168"
  env_add_if_missing "${env_file}" "API_PORT" "8080"
  env_add_if_missing "${env_file}" "WEB_PORT" "80"
  local missing=0
  if ! grep -qE '^VULYK_SECRETS_KEY=.' "${env_file}"; then
    warn "CRITICAL: VULYK_SECRETS_KEY missing in ${env_file} - encrypted LLM/SSO tokens will not decrypt"
    missing=1
  fi
  if ! grep -qE '^VULYK_JWT_SECRET=.' "${env_file}"; then
    warn "CRITICAL: VULYK_JWT_SECRET missing in ${env_file}"
    missing=1
  fi
  if [[ "${env_file}" == *.prod ]] || grep -qE '^POSTGRES_PASSWORD=' "${env_file}" 2>/dev/null; then
    if ! grep -qE '^POSTGRES_PASSWORD=.' "${env_file}"; then
      warn "CRITICAL: POSTGRES_PASSWORD missing in ${env_file}"
      missing=1
    fi
  fi
  if [[ "${missing}" -eq 1 && "${ASSUME_YES}" -eq 0 ]]; then
    confirm_or_yes "Continue upgrade with missing critical secrets?"
  elif [[ "${missing}" -eq 1 ]]; then
    warn "Continuing with --yes despite missing critical secrets"
  fi
}

resolve_sql_dump_from_backup() {
  local path="$1"
  if [[ -d "${path}" && -f "${path}/db.sql.gz" ]]; then
    printf '%s' "${path}/db.sql.gz"
  elif [[ -f "${path}" ]]; then
    printf '%s' "${path}"
  else
    die "backup not found: ${path}"
  fi
}

restore_db_from_dump() {
  local dump="$1"
  dump="$(resolve_sql_dump_from_backup "${dump}")"
  wait_for_db
  log "Restoring database from ${dump}..."
  compose_cmd stop api web 2>/dev/null || true
  compose_cmd exec -T db \
    psql -U "${POSTGRES_USER:-vulyk}" -d "${POSTGRES_DB:-vulyk}" -v ON_ERROR_STOP=1 \
    -c "DROP SCHEMA public CASCADE; CREATE SCHEMA public; GRANT ALL ON SCHEMA public TO ${POSTGRES_USER:-vulyk}; GRANT ALL ON SCHEMA public TO public;"
  gzip -dc "${dump}" | compose_cmd exec -T db \
    psql -U "${POSTGRES_USER:-vulyk}" -d "${POSTGRES_DB:-vulyk}" -v ON_ERROR_STOP=1
  log "Database restore complete"
}

# Capture currently running api/web image refs and tag them for rollback.
snapshot_running_images_for_rollback() {
  PREV_API_IMAGE=""
  PREV_WEB_IMAGE=""
  local api_id web_id
  api_id="$(compose_cmd ps -q api 2>/dev/null || true)"
  web_id="$(compose_cmd ps -q web 2>/dev/null || true)"
  if [[ -n "${api_id}" ]]; then
    PREV_API_IMAGE="$(docker inspect -f '{{.Config.Image}}' "${api_id}" 2>/dev/null || true)"
    if [[ -n "${PREV_API_IMAGE}" ]]; then
      docker tag "${PREV_API_IMAGE}" "vulyk-api:rollback" 2>/dev/null || true
      PREV_API_IMAGE="vulyk-api:rollback"
    fi
  fi
  if [[ -n "${web_id}" ]]; then
    PREV_WEB_IMAGE="$(docker inspect -f '{{.Config.Image}}' "${web_id}" 2>/dev/null || true)"
    if [[ -n "${PREV_WEB_IMAGE}" ]]; then
      docker tag "${PREV_WEB_IMAGE}" "vulyk-web:rollback" 2>/dev/null || true
      PREV_WEB_IMAGE="vulyk-web:rollback"
    fi
  fi
  # Prefer explicit env pins when set (release installs)
  if [[ -n "${VULYK_API_IMAGE:-}" && "${VULYK_API_IMAGE}" != "vulyk-api:local" ]]; then
    # Keep rollback tag as primary; also remember env pin in image-state via backup
    :
  fi
  log "Rollback images snapshotted: api=${PREV_API_IMAGE:-none} web=${PREV_WEB_IMAGE:-none}"
}

auto_rollback_upgrade() {
  local backup_path="${1:-}"
  local reason="${2:-upgrade failed}"
  warn "AUTO-ROLLBACK: ${reason}"
  if [[ -z "${backup_path}" ]]; then
    print_restore_hint ""
    die "${reason} (no backup available for automatic rollback)"
  fi
  if [[ -n "${PREV_API_IMAGE}" ]]; then
    export VULYK_API_IMAGE="${PREV_API_IMAGE}"
  fi
  if [[ -n "${PREV_WEB_IMAGE}" ]]; then
    export VULYK_WEB_IMAGE="${PREV_WEB_IMAGE}"
  fi
  if [[ -n "${COMPOSE_ENV_FILE:-}" && -f "${COMPOSE_ENV_FILE}" ]]; then
    if [[ -n "${PREV_API_IMAGE}" ]]; then
      env_upsert "${COMPOSE_ENV_FILE}" "VULYK_API_IMAGE" "${PREV_API_IMAGE}"
    fi
    if [[ -n "${PREV_WEB_IMAGE}" ]]; then
      env_upsert "${COMPOSE_ENV_FILE}" "VULYK_WEB_IMAGE" "${PREV_WEB_IMAGE}"
    fi
  fi
  set +e
  restore_db_from_dump "${backup_path}"
  local rc=$?
  set -e
  if [[ "${rc}" -ne 0 ]]; then
    print_restore_hint "${backup_path}"
    die "${reason}; automatic DB restore also failed - restore manually from ${backup_path}"
  fi
  log "Starting previous images after rollback..."
  if ! compose_cmd up -d; then
    print_restore_hint "${backup_path}"
    die "${reason}; rolled back DB but failed to start previous images"
  fi
  print_restore_hint "${backup_path}"
  die "${reason} - automatic rollback applied (previous images + DB restore). Verify /health."
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
  need_cmd docker
  detect_compose_files
  if [[ -n "${COMPOSE_ENV_FILE}" && -f "${COMPOSE_ENV_FILE}" ]]; then
    # shellcheck disable=SC1091
    set -a
    # shellcheck source=/dev/null
    source "${COMPOSE_ENV_FILE}"
    set +a
  fi
  local ver
  ver="$(api_reported_version)"
  [[ -n "${ver}" ]] || ver="$(current_git_version)"
  run_full_backup "${ver}"
}

cmd_restore() {
  local file="${RESTORE_FILE:-}"
  [[ -n "${file}" ]] || die "usage: ./scripts/install.sh restore <backup-bundle-dir|backup.sql.gz>"
  [[ -e "${file}" ]] || die "backup not found: ${file}"
  resolve_compose
  [[ ${#COMPOSE[@]} -gt 0 ]] || die "Docker Compose required for restore"
  need_cmd gzip
  detect_compose_files
  if [[ -n "${COMPOSE_ENV_FILE}" && -f "${COMPOSE_ENV_FILE}" ]]; then
    # shellcheck disable=SC1091
    set -a
    # shellcheck source=/dev/null
    source "${COMPOSE_ENV_FILE}"
    set +a
  fi
  confirm_or_yes "Restore ${file} into the running Postgres (DESTROYS current DB data)?"
  # Prefer restoring config from bundle without overwriting unless user confirms
  if [[ -d "${file}/config" ]]; then
    local cfg
    for cfg in .env .env.prod; do
      if [[ -f "${file}/config/${cfg}" ]]; then
        if [[ -f "${ROOT}/${cfg}" ]]; then
          warn "Keeping existing ${cfg} (bundle also has a copy at ${file}/config/${cfg})"
        else
          cp -p "${file}/config/${cfg}" "${ROOT}/${cfg}"
          chmod 600 "${ROOT}/${cfg}" 2>/dev/null || true
          log "Restored missing ${cfg} from backup bundle"
        fi
      fi
    done
  fi
  restore_db_from_dump "${file}"
  log "Starting stack..."
  compose_cmd up -d
  log "Done. Verify: curl -fsS \${VULYK_API_URL:-http://127.0.0.1:8080}/health"
  log "If encrypted secrets fail to decrypt, restore VULYK_SECRETS_KEY from the backup config/ copy."
}

print_restore_hint() {
  local backup_file="${1:-}"
  cat <<EOF >&2

========================================================================
UPGRADE FAILED - Postgres data volume was NOT removed.
Never run: docker compose down -v   (the -v flag DELETES your database)

To restore from backup:
  ./scripts/install.sh restore ${backup_file:-./backups/vulyk-VERSION-TIMESTAMP}/ --yes

Manual DB-only restore (legacy .sql.gz also supported):
  ./scripts/install.sh restore ${backup_file:-./backups/vulyk-....sql.gz} --yes

See docs/guides/upgrade.md (Data-safety guarantees)
========================================================================
EOF
}

version_backups_cmd() {
  export VULYK_VERSION_BACKUP_DIR="${VERSION_BACKUP_DIR}"
  export VULYK_ROLLBACK_RETENTION_DAYS="${ROLLBACK_RETENTION_DAYS}"
  export VULYK_VERSION_BACKUP_COUNT="${VERSION_BACKUP_COUNT}"
  export VULYK_ROOT="${ROOT}"
  python3 "${VERSION_BACKUPS_PY}" "$@"
}

# After a successful upgrade: register pre-upgrade bundle as rollback (N days)
# + version retention snapshot, then prune.
register_post_upgrade_version_backups() {
  local pre_bundle="$1" from_ver="$2" to_ver="$3"
  [[ -n "${pre_bundle}" && -d "${pre_bundle}" ]] || return 0
  mkdir -p "${VERSION_BACKUP_DIR}"
  local api_img web_img
  api_img="${PREV_API_IMAGE:-${VULYK_API_IMAGE:-}}"
  web_img="${PREV_WEB_IMAGE:-${VULYK_WEB_IMAGE:-}}"
  if [[ -f "${pre_bundle}/config/image-state.env" ]]; then
    local pinned_api pinned_web
    pinned_api="$(grep -E '^VULYK_API_IMAGE=' "${pre_bundle}/config/image-state.env" | head -n1 | cut -d= -f2- || true)"
    pinned_web="$(grep -E '^VULYK_WEB_IMAGE=' "${pre_bundle}/config/image-state.env" | head -n1 | cut -d= -f2- || true)"
    [[ -n "${pinned_api}" ]] && api_img="${pinned_api}"
    [[ -n "${pinned_web}" ]] && web_img="${pinned_web}"
  fi
  log "Registering version backup + ${ROLLBACK_RETENTION_DAYS}-day rollback point for ${from_ver}..."
  version_backups_cmd register \
    --source "${pre_bundle}" \
    --version "${from_ver}" \
    --from-version "${from_ver}" \
    --to-version "${to_ver}" \
    --api-image "${api_img}" \
    --web-image "${web_img}" \
    --rollback >/tmp/vulyk-vb-register.json || warn "version-backup register failed"
  version_backups_cmd prune --current-version "${to_ver}" >/tmp/vulyk-vb-prune.json || true
  if [[ -f /tmp/vulyk-vb-register.json ]]; then
    log "Rollback point kept under ${VERSION_BACKUP_DIR}/ (see Admin → Updates or install.sh rollback)"
  fi
}

require_rollback_confirmation() {
  local target_ver="$1"
  cat <<EOF >&2

************************************************************************
ROLLBACK WARNING
************************************************************************
Rolling back to v${target_ver} restores the pre-upgrade database, config
snapshot, and previous images.

  Data created AFTER the upgrade will be LOST.

Type ROLLBACK to continue (or pass --confirm=ROLLBACK / VULYK_ROLLBACK_CONFIRM).
************************************************************************
EOF
  local got="${ROLLBACK_CONFIRM}"
  if [[ -z "${got}" ]]; then
    if [[ ! -t 0 ]]; then
      die "rollback requires --confirm=ROLLBACK (non-interactive)"
    fi
    printf 'Confirm: ' >&2
    read -r got || true
  fi
  if [[ "${got}" != "ROLLBACK" ]]; then
    die "aborted (expected typed confirmation ROLLBACK)"
  fi
}

cmd_rollback() {
  local cur target_ver bundle_id bundle_path meta_json api_img web_img fresh_backup
  resolve_compose
  [[ ${#COMPOSE[@]} -gt 0 ]] || die "Docker Compose required for rollback"
  need_cmd curl
  need_cmd gzip
  need_cmd docker
  need_cmd python3
  [[ -f "${VERSION_BACKUPS_PY}" ]] || die "missing ${VERSION_BACKUPS_PY}"

  detect_compose_files
  if [[ -n "${COMPOSE_ENV_FILE}" && -f "${COMPOSE_ENV_FILE}" ]]; then
    # shellcheck disable=SC1091
    set -a
    # shellcheck source=/dev/null
    source "${COMPOSE_ENV_FILE}"
    set +a
  fi

  cur="$(api_reported_version)"
  [[ -n "${cur}" ]] || cur="$(current_git_version)"
  log "Current version: ${cur}"

  # Optional prune of expired points first (skips in-progress)
  version_backups_cmd prune --current-version "${cur}" >/tmp/vulyk-vb-prune.json || true

  if [[ -n "${RELEASE_VERSION}" ]]; then
    meta_json="$(version_backups_cmd list --current-version "${cur}")"
    bundle_id="$(
      printf '%s' "${meta_json}" | python3 -c '
import json,sys
want=sys.argv[1].lstrip("v")
data=json.load(sys.stdin)
for it in data.get("items",[]):
    ver=(it.get("version") or it.get("from_version") or "").lstrip("v")
    if ver==want and "rollback" in (it.get("roles") or []):
        print(it["id"]); raise SystemExit
for it in data.get("items",[]):
    ver=(it.get("version") or "").lstrip("v")
    if ver==want:
        print(it["id"]); raise SystemExit
raise SystemExit(1)
' "${RELEASE_VERSION}"
    )" || die "no version backup / rollback point for ${RELEASE_VERSION}"
  else
    meta_json="$(version_backups_cmd active-rollback --current-version "${cur}")"
    [[ -n "${meta_json}" && "${meta_json}" != "null" ]] || die "no active rollback point (expired or missing)"
    bundle_id="$(printf '%s' "${meta_json}" | python3 -c 'import json,sys; m=json.load(sys.stdin); print(m.get("_id") or m.get("id") or "")')"
  fi
  [[ -n "${bundle_id}" ]] || die "rollback bundle id missing"
  meta_json="$(version_backups_cmd find --id "${bundle_id}")"
  [[ -n "${meta_json}" && "${meta_json}" != "null" ]] || die "bundle not found: ${bundle_id}"
  bundle_path="$(python3 -c 'import json,sys; print(json.load(sys.stdin)["_path"])' <<<"${meta_json}")"
  target_ver="$(python3 -c 'import json,sys; m=json.load(sys.stdin); print(m.get("version") or m.get("from_version") or "")' <<<"${meta_json}")"
  api_img="$(python3 -c 'import json,sys; print(json.load(sys.stdin).get("api_image") or "")' <<<"${meta_json}")"
  web_img="$(python3 -c 'import json,sys; print(json.load(sys.stdin).get("web_image") or "")' <<<"${meta_json}")"
  [[ -d "${bundle_path}" && -f "${bundle_path}/db.sql.gz" ]] || die "invalid rollback bundle: ${bundle_path}"

  require_rollback_confirmation "${target_ver}"

  version_backups_cmd set-in-progress --id "${bundle_id}" --value true >/dev/null
  trap 'version_backups_cmd set-in-progress --id "'"${bundle_id}"'" --value false >/dev/null 2>&1 || true' EXIT

  log "Taking fresh backup of current state before rollback..."
  compose_cmd up -d db
  wait_for_db
  fresh_backup="$(run_full_backup "${cur}-pre-rollback")"

  if [[ -n "${api_img}" ]]; then
    export VULYK_API_IMAGE="${api_img}"
    if [[ -n "${COMPOSE_ENV_FILE}" && -f "${COMPOSE_ENV_FILE}" ]]; then
      env_upsert "${COMPOSE_ENV_FILE}" "VULYK_API_IMAGE" "${api_img}"
    fi
  fi
  if [[ -n "${web_img}" ]]; then
    export VULYK_WEB_IMAGE="${web_img}"
    if [[ -n "${COMPOSE_ENV_FILE}" && -f "${COMPOSE_ENV_FILE}" ]]; then
      env_upsert "${COMPOSE_ENV_FILE}" "VULYK_WEB_IMAGE" "${web_img}"
    fi
  fi

  log "Restoring database from rollback point ${bundle_path}..."
  restore_db_from_dump "${bundle_path}"

  # Restore missing env files from rollback config (do not overwrite existing)
  if [[ -d "${bundle_path}/config" ]]; then
    local cfg
    for cfg in .env .env.prod; do
      if [[ -f "${bundle_path}/config/${cfg}" && ! -f "${ROOT}/${cfg}" ]]; then
        cp -p "${bundle_path}/config/${cfg}" "${ROOT}/${cfg}"
        chmod 600 "${ROOT}/${cfg}" 2>/dev/null || true
        log "Restored missing ${cfg} from rollback point"
      fi
    done
  fi

  log "Starting rolled-back stack..."
  compose_cmd up -d

  local health_url
  health_url="${VULYK_API_URL:-http://127.0.0.1:8080}/health"
  if [[ -n "${API_PORT:-}" ]]; then
    health_url="http://127.0.0.1:${API_PORT}/health"
  fi
  local _
  for _ in $(seq 1 90); do
    if curl -fsS "${health_url}" >/dev/null 2>&1; then
      local new_ver
      new_ver="$(api_reported_version)"
      log "Rollback complete. Health OK (version=${new_ver:-unknown})"
      log "Pre-rollback safety backup: ${fresh_backup}"
      # Append a local audit marker (API also writes audit_events when triggered in-app)
      mkdir -p "${VERSION_BACKUP_DIR}"
      printf '%s\n' "{\"action\":\"rollback\",\"to\":\"${target_ver}\",\"from\":\"${cur}\",\"bundle\":\"${bundle_id}\",\"at\":\"$(date -u +%Y-%m-%dT%H:%M:%SZ)\"}" \
        >>"${VERSION_BACKUP_DIR}/audit.log"
      version_backups_cmd set-in-progress --id "${bundle_id}" --value false >/dev/null || true
      trap - EXIT
      version_backups_cmd prune --current-version "${new_ver:-${target_ver}}" >/dev/null || true
      return 0
    fi
    sleep 2
  done
  version_backups_cmd set-in-progress --id "${bundle_id}" --value false >/dev/null || true
  trap - EXIT
  die "rollback health check failed; current-state backup at ${fresh_backup}"
}

cmd_upgrade() {
  local backup_file="" ver free_mb pending_rc=0 target_label="" cmp=0

  if [[ -n "${RELEASE_VERSION}" && -n "${UPGRADE_TO}" ]]; then
    die "use either --version (GHCR images) or --to (git + source build), not both"
  fi

  if [[ -n "${RELEASE_VERSION}" ]]; then
    apply_release_images "${RELEASE_VERSION}"
    target_label="GHCR ${RELEASE_VERSION}"
  elif [[ -n "${UPGRADE_TO}" ]]; then
    target_label="git ${UPGRADE_TO}"
  else
    target_label="latest source (git pull + rebuild)"
  fi

  resolve_compose
  [[ ${#COMPOSE[@]} -gt 0 ]] || die "Docker Compose required for upgrade"
  need_cmd curl
  need_cmd gzip
  need_cmd docker
  need_cmd python3

  if [[ -z "${RELEASE_VERSION}" ]]; then
    need_cmd git
  fi

  log "Upgrade preflight..."
  docker info >/dev/null 2>&1 || die "Docker daemon not reachable"
  free_mb="$(disk_free_mb)"
  log "Free disk: ${free_mb} MB (minimum ${MIN_FREE_MB} MB)"
  if [[ "${free_mb}" -gt 0 && "${free_mb}" -lt "${MIN_FREE_MB}" ]]; then
    die "Insufficient free disk (${free_mb} MB < ${MIN_FREE_MB} MB). Free space or set VULYK_MIN_FREE_MB."
  fi

  ver="$(api_reported_version)"
  [[ -n "${ver}" ]] || ver="$(current_git_version)"
  log "Current version: ${ver}"
  if command -v git >/dev/null 2>&1; then
    log "Git HEAD: $(current_git_version)"
  fi
  log "Target: ${target_label}"

  # Block app-version downgrades unless explicitly allowed (schema-newer still always blocked).
  if [[ -n "${RELEASE_VERSION}" && "${ver}" =~ ^[0-9]+\.[0-9]+ ]]; then
    cmp="$(semver_cmp "${RELEASE_VERSION}" "${ver}")"
    if [[ "${cmp}" -lt 0 ]]; then
      if [[ "${ALLOW_DOWNGRADE}" -eq 1 ]]; then
        warn "Target ${RELEASE_VERSION} < current ${ver} (--allow-downgrade). Prefer restore-from-backup."
      else
        die "Refusing downgrade ${ver} -> ${RELEASE_VERSION}. Restore a matching backup, or pass --allow-downgrade."
      fi
    fi
  fi

  # Ensure DB is up for status / backup (do not start new API yet)
  detect_compose_files
  log "Ensuring database is running..."
  compose_cmd up -d db
  wait_for_db

  # Load env for POSTGRES_* when using prod (preserve release image exports)
  if [[ -n "${COMPOSE_ENV_FILE}" && -f "${COMPOSE_ENV_FILE}" ]]; then
    local saved_api_image="${VULYK_API_IMAGE:-}"
    local saved_web_image="${VULYK_WEB_IMAGE:-}"
    # shellcheck disable=SC1091
    set -a
    # shellcheck source=/dev/null
    source "${COMPOSE_ENV_FILE}"
    set +a
    if [[ -n "${RELEASE_VERSION}" ]]; then
      [[ -n "${saved_api_image}" ]] && export VULYK_API_IMAGE="${saved_api_image}"
      [[ -n "${saved_web_image}" ]] && export VULYK_WEB_IMAGE="${saved_web_image}"
    fi
    # Additive defaults only - never regenerate secrets / admin password / user settings
    merge_env_defaults_for_upgrade "${COMPOSE_ENV_FILE}"
  fi

  log "Migration status (current image, if available)..."
  set +e
  compose_cmd run --rm --no-deps api vulyk-api migrate status
  pending_rc=$?
  set -e
  if [[ "${pending_rc}" -eq 2 ]]; then
    die "database schema is newer than the current API binary (refusing upgrade - restore matching backup)"
  fi

  snapshot_running_images_for_rollback

  if [[ "${NO_BACKUP}" -eq 1 ]]; then
    warn "Skipping backup (--no-backup) - data loss risk if migrate/health fails"
  else
    backup_file="$(run_full_backup "${ver}")"
  fi

  if [[ "${ASSUME_YES}" -eq 0 ]]; then
    confirm_or_yes "Proceed with upgrade to ${target_label}?"
  fi

  if [[ -n "${RELEASE_VERSION}" ]]; then
    if [[ -n "${COMPOSE_ENV_FILE}" ]]; then
      # Only upsert image pins - never rewrite secrets/admin via write_setup_env
      persist_release_images_env "${COMPOSE_ENV_FILE}"
    fi
  elif [[ -n "${UPGRADE_TO}" ]]; then
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

  if [[ -n "${RELEASE_VERSION}" ]]; then
    log "Pulling release images (no local rebuild)..."
    if ! pull_release_images; then
      auto_rollback_upgrade "${backup_file}" "image pull failed"
    fi
  else
    log "Rebuilding images..."
    if ! compose_cmd build api web; then
      auto_rollback_upgrade "${backup_file}" "image build failed"
    fi
  fi

  log "Pre-flight migrate status on NEW image (before applying)..."
  set +e
  compose_cmd run --rm --no-deps \
    -e VULYK_AUTO_MIGRATE=false \
    api vulyk-api migrate status
  pending_rc=$?
  set -e
  if [[ "${pending_rc}" -eq 2 ]]; then
    auto_rollback_upgrade "${backup_file}" "new image cannot migrate (schema newer than binary)"
  fi

  log "Running migrations (one-shot, before starting API)..."
  if ! compose_cmd run --rm --no-deps \
    -e VULYK_AUTO_MIGRATE=false \
    api vulyk-api migrate run; then
    auto_rollback_upgrade "${backup_file}" "migrate run failed"
  fi

  log "Starting updated stack..."
  if ! compose_cmd up -d; then
    auto_rollback_upgrade "${backup_file}" "compose up failed"
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
      if [[ -n "${RELEASE_VERSION}" && -n "${new_ver}" && "${new_ver}" != "${RELEASE_VERSION}" ]]; then
        warn "Expected version ${RELEASE_VERSION} but /health reported ${new_ver}"
      fi
      if [[ -n "${backup_file}" ]]; then
        log "Backup kept at: ${backup_file}"
        register_post_upgrade_version_backups \
          "${backup_file}" \
          "${ver}" \
          "${new_ver:-${RELEASE_VERSION:-unknown}}"
      fi
      cat <<EOF

Next:
  curl -fsS ${health_url}
  ./scripts/install.sh backup     # optional extra backup after upgrade
  ./scripts/install.sh rollback --confirm=ROLLBACK   # within ${ROLLBACK_RETENTION_DAYS} days

WARNING: do not run 'docker compose down -v' - that deletes Postgres data.
Config (.env / secrets / admin password) was NOT regenerated.
EOF
      return 0
    fi
    sleep 2
  done
  auto_rollback_upgrade "${backup_file}" "API health check failed after upgrade"
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
OS="$(detect_os)"
log "Detected OS: ${OS}"

parse_args "$@"

case "${MODE}" in
  compose|prod|dev|setup|check|helm-template|perf-smoke|upgrade|backup|restore|rollback)
    ;;
  -h|--help|help)
    usage
    exit 0
    ;;
  *)
    die "unknown mode: ${MODE} (try --help)"
    ;;
esac

# Fail fast on bad --version before Docker/toolchain work
if [[ -n "${RELEASE_VERSION}" ]]; then
  case "${MODE}" in
    compose|prod|upgrade|rollback) ;;
    *)
      die "--version is only supported for compose, prod, upgrade, and rollback modes"
      ;;
  esac
  RELEASE_VERSION="$(normalize_release_version "${RELEASE_VERSION}")"
fi

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

if [[ "${MODE}" == "rollback" ]]; then
  cmd_rollback
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
  if [[ -n "${RELEASE_VERSION}" ]]; then
    apply_release_images "${RELEASE_VERSION}"
  fi
  collect_setup_options "prod"
  ensure_prod_env ".env.prod"
  if [[ -n "${RELEASE_VERSION}" ]]; then
    persist_release_images_env ".env.prod"
  fi
  resolve_compose
  [[ ${#COMPOSE[@]} -gt 0 ]] || die "Docker Compose plugin or docker-compose required"
  # shellcheck disable=SC1091
  set -a
  # shellcheck source=/dev/null
  source .env.prod
  set +a
  if [[ -n "${RELEASE_VERSION}" ]]; then
    log "Pulling and starting Vulyk production Compose (GHCR ${RELEASE_VERSION})..."
    pull_release_images
    "${COMPOSE[@]}" -f docker-compose.prod.yml --env-file .env.prod up -d
  else
    log "Building and starting Vulyk production Compose..."
    "${COMPOSE[@]}" -f docker-compose.prod.yml --env-file .env.prod up --build -d
  fi
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
Upgrade: ./scripts/install.sh upgrade --yes${RELEASE_VERSION:+ --version ${RELEASE_VERSION}}
Backups: ./scripts/install.sh backup  (see docs/guides/upgrade.md)
EOF
  print_setup_next_steps "prod"
  exit 0
fi

# Default: compose - write .env first, then require Docker
if [[ -n "${RELEASE_VERSION}" ]]; then
  apply_release_images "${RELEASE_VERSION}"
fi
collect_setup_options "compose"
ensure_compose_env ".env"
if [[ -n "${RELEASE_VERSION}" ]]; then
  persist_release_images_env ".env"
fi
resolve_compose
[[ ${#COMPOSE[@]} -gt 0 ]] || die "Docker Compose plugin or docker-compose required (first-run settings already in .env)"
# shellcheck disable=SC1091
set -a
# shellcheck source=/dev/null
source .env
set +a
if [[ -n "${RELEASE_VERSION}" ]]; then
  log "Pulling and starting Vulyk (docker compose, GHCR ${RELEASE_VERSION})..."
  pull_release_images
  "${COMPOSE[@]}" --env-file .env up -d
else
  log "Building and starting Vulyk (docker compose)..."
  "${COMPOSE[@]}" --env-file .env up --build -d
fi

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
Upgrade: ./scripts/install.sh upgrade --yes${RELEASE_VERSION:+ --version ${RELEASE_VERSION}}
Logs:  docker compose logs -f
Stop:  docker compose down
       (NEVER use down -v - that deletes the Postgres volume)
EOF
print_setup_next_steps "compose"
