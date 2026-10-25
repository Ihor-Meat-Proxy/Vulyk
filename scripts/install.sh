#!/usr/bin/env bash
# Install / bootstrap Vulyk on Linux, macOS, or WSL2.
# Verified modes: compose | prod | dev | setup | check | helm-template | perf-smoke
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

log() { printf '==> %s\n' "$*"; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }
warn() { printf 'warning: %s\n' "$*" >&2; }

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || die "missing required command: $1"
}

detect_os() {
  local uname_s
  uname_s="$(uname -s)"
  case "$uname_s" in
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
    *) die "unsupported OS: $uname_s" ;;
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
  setup           Run first-run setup (docker compose run api … or cargo run)
  check           Verify local toolchain (rustc, cargo, node, docker/helm optional)
  helm-template   Render the Helm chart (requires helm) as a dry-run validation
  perf-smoke      Seed ~100k cases and check list/search latency (needs running API)

First-run options (compose / prod / dev / setup):
  --admin-email=EMAIL
  --admin-password=PASSWORD
  --locale=en|uk
  --timezone=TZ            e.g. UTC, Europe/Kyiv
  --registration-mode=MODE open | invite | disabled
  --instance-name=NAME
  --seed-demo=0|1
  --base-url=URL
  --yes, -y                Non-interactive (use flags/env; skip prompts)

Environment equivalents:
  VULYK_ADMIN_EMAIL, VULYK_ADMIN_PASSWORD, VULYK_DEFAULT_LOCALE,
  VULYK_DEFAULT_TIMEZONE, VULYK_REGISTRATION_MODE, VULYK_INSTANCE_NAME,
  VULYK_SEED_DEMO, VULYK_BASE_URL

Written into .env (compose/dev) or .env.prod (prod). Defaults:
  compose/dev: seed demo=1, registration=open, admin@vulyk.local / admin
  prod:        seed demo=0, registration=disabled

After compose/prod/dev, complete setup if needed:
  ./scripts/install.sh setup --yes
  # or: docker compose run --rm api vulyk-api setup --yes …
  # or: cargo run -p vulyk-api -- setup …

See docs/guides/first-run-setup.md

Environment:
  Linux, macOS, and WSL2 are supported. Native Windows is not — use WSL2.
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
        # Flags without a mode → default compose
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
    case "$arg" in
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
      --*=*)
        die "unknown option: $arg (try --help)"
        ;;
      -*)
        die "unknown option: $arg (try --help)"
        ;;
      *)
        die "unexpected argument: $arg (try --help)"
        ;;
    esac
  done
}

prompt_default() {
  local label="$1" default="$2" reply
  if [[ -t 0 && "$ASSUME_YES" -eq 0 ]]; then
    printf '%s [%s]: ' "$label" "$default" >&2
    read -r reply || true
    if [[ -n "${reply:-}" ]]; then
      printf '%s' "$reply"
    else
      printf '%s' "$default"
    fi
  else
    printf '%s' "$default"
  fi
}

# Prompt for a password with confirmation. Empty input returns empty (caller may apply a default).
# When $2 (hint_default) is non-empty, show that Enter keeps the default (value not echoed).
prompt_secret_confirm() {
  local label="$1" hint_default="${2:-}" pass1 pass2
  if [[ ! -t 0 || "$ASSUME_YES" -eq 1 ]]; then
    die "admin password required (set --admin-password= or VULYK_ADMIN_PASSWORD)"
  fi
  while true; do
    if [[ -n "$hint_default" ]]; then
      printf '%s [Enter=default]: ' "$label" >&2
    else
      printf '%s: ' "$label" >&2
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
      printf 'Confirm %s: ' "$label" >&2
      stty -echo 2>/dev/null || true
      read -r pass2 || true
      stty echo 2>/dev/null || true
      printf '\n' >&2
    else
      printf 'Confirm %s: ' "$label" >&2
      read -r pass2 || true
    fi
    if [[ "$pass1" != "${pass2:-}" ]]; then
      warn "passwords do not match"
      continue
    fi
    printf '%s' "$pass1"
    return 0
  done
}

validate_locale() {
  case "$1" in
    en|uk) return 0 ;;
    *) die "locale must be en or uk (got: $1)" ;;
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

  case "$profile" in
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
      die "internal: unknown setup profile $profile"
      ;;
  esac

  if [[ "$ASSUME_YES" -eq 0 && -t 0 ]]; then
    log "First-run setup (press Enter to accept defaults)…"
  fi

  ADMIN_EMAIL="$(prompt_default "Admin email" "${ADMIN_EMAIL:-$def_email}")"
  if [[ -z "$ADMIN_PASSWORD" ]]; then
    if [[ "$ASSUME_YES" -eq 1 || ! -t 0 ]]; then
      ADMIN_PASSWORD="$def_password"
      [[ -n "$ADMIN_PASSWORD" ]] \
        || die "admin password required (set --admin-password= or VULYK_ADMIN_PASSWORD)"
    else
      # Interactive: confirm; empty Enter keeps profile default when present
      local prompted
      prompted="$(prompt_secret_confirm "Admin password" "$def_password")"
      if [[ -z "$prompted" ]]; then
        [[ -n "$def_password" ]] || die "admin password is required"
        ADMIN_PASSWORD="$def_password"
        log "Using default local admin password for $profile"
      else
        ADMIN_PASSWORD="$prompted"
      fi
    fi
  fi
  DEFAULT_LOCALE="$(prompt_default "Locale (en/uk)" "${DEFAULT_LOCALE:-$def_locale}")"
  DEFAULT_TIMEZONE="$(prompt_default "Timezone" "${DEFAULT_TIMEZONE:-$def_tz}")"
  REGISTRATION_MODE="$(prompt_default "Registration mode (open/invite/disabled)" "${REGISTRATION_MODE:-$def_reg}")"
  INSTANCE_NAME="$(prompt_default "Instance name" "${INSTANCE_NAME:-$def_name}")"
  SEED_DEMO="$(prompt_default "Seed demo users (0/1)" "${SEED_DEMO:-$def_seed}")"
  if [[ -z "$BASE_URL" ]]; then
    BASE_URL=""
  fi

  validate_locale "$DEFAULT_LOCALE"
  validate_registration_mode "$REGISTRATION_MODE"
  case "$SEED_DEMO" in
    0|1|true|false|yes|no) ;;
    *) die "seed-demo must be 0 or 1 (got: $SEED_DEMO)" ;;
  esac
  # Normalize seed demo to 0/1
  case "$(printf '%s' "$SEED_DEMO" | tr '[:upper:]' '[:lower:]')" in
    1|true|yes) SEED_DEMO=1 ;;
    *) SEED_DEMO=0 ;;
  esac

  [[ -n "$ADMIN_EMAIL" ]] || die "admin email is required"
  [[ -n "$ADMIN_PASSWORD" ]] || die "admin password is required"
}

# Upsert KEY=VALUE in an env file (create file if missing).
env_upsert() {
  local file="$1" key="$2" value="$3"
  local tmp
  touch "$file"
  tmp="$(mktemp)"
  if grep -qE "^${key}=" "$file" 2>/dev/null; then
    # shellcheck disable=SC2016
    awk -v k="$key" -v v="$value" '
      BEGIN { done=0 }
      index($0, k "=") == 1 && !done { print k "=" v; done=1; next }
      { print }
      END { if (!done) print k "=" v }
    ' "$file" >"$tmp"
    mv "$tmp" "$file"
  else
    rm -f "$tmp"
    printf '%s=%s\n' "$key" "$value" >>"$file"
  fi
}

write_setup_env() {
  local env_file="$1"
  touch "$env_file"
  env_upsert "$env_file" "VULYK_ADMIN_EMAIL" "$ADMIN_EMAIL"
  env_upsert "$env_file" "VULYK_ADMIN_PASSWORD" "$ADMIN_PASSWORD"
  env_upsert "$env_file" "VULYK_DEFAULT_LOCALE" "$DEFAULT_LOCALE"
  env_upsert "$env_file" "VULYK_DEFAULT_TIMEZONE" "$DEFAULT_TIMEZONE"
  env_upsert "$env_file" "VULYK_REGISTRATION_MODE" "$REGISTRATION_MODE"
  env_upsert "$env_file" "VULYK_INSTANCE_NAME" "$INSTANCE_NAME"
  env_upsert "$env_file" "VULYK_SEED_DEMO" "$SEED_DEMO"
  if [[ -n "$BASE_URL" ]]; then
    env_upsert "$env_file" "VULYK_BASE_URL" "$BASE_URL"
  fi
  chmod 600 "$env_file" 2>/dev/null || true
  log "Wrote first-run settings to $env_file"
}

ensure_prod_env() {
  local env_file="${1:-.env.prod}"
  if [[ -f "$env_file" ]]; then
    log "Using existing $env_file"
  else
    log "Generating $env_file with random secrets…"
    cat >"$env_file" <<EOF
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
    chmod 600 "$env_file" || true
    log "Wrote $env_file (mode 600)."
  fi
  write_setup_env "$env_file"
}

ensure_compose_env() {
  local env_file="${1:-.env}"
  if [[ ! -f "$env_file" ]]; then
    log "Creating $env_file…"
    cat >"$env_file" <<EOF
# Generated by scripts/install.sh — local/dev Compose
VULYK_AUTH_REQUIRED=false
VULYK_JWT_SECRET=vulyk-compose-dev-secret
VULYK_LLM_MOCK=1
EOF
  fi
  write_setup_env "$env_file"
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
  if [[ "$profile" == "compose" || "$profile" == "dev" ]]; then
    cat <<EOF

Default local credentials (when seeded):
  Admin: ${ADMIN_EMAIL} / (password from .env — default admin)
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
    --admin-email="$ADMIN_EMAIL"
    --admin-password="$ADMIN_PASSWORD"
    --locale="$DEFAULT_LOCALE"
    --timezone="$DEFAULT_TIMEZONE"
    --registration-mode="$REGISTRATION_MODE"
    --instance-name="$INSTANCE_NAME"
  )
  if [[ -n "$BASE_URL" ]]; then
    setup_args+=(--base-url="$BASE_URL")
  fi
  if [[ "$ASSUME_YES" -eq 1 ]]; then
    setup_args+=(--yes)
  fi

  resolve_compose
  if [[ ${#COMPOSE[@]} -gt 0 ]] && docker info >/dev/null 2>&1; then
    # Prefer compose stack when available
    if [[ -f docker-compose.yml ]]; then
      log "Running setup via docker compose…"
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
  log "Docker unavailable — running setup via cargo…"
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
  export VULYK_ADMIN_EMAIL="$ADMIN_EMAIL"
  export VULYK_ADMIN_PASSWORD="$ADMIN_PASSWORD"
  export VULYK_DEFAULT_LOCALE="$DEFAULT_LOCALE"
  export VULYK_DEFAULT_TIMEZONE="$DEFAULT_TIMEZONE"
  export VULYK_REGISTRATION_MODE="$REGISTRATION_MODE"
  export VULYK_INSTANCE_NAME="$INSTANCE_NAME"
  [[ -n "$BASE_URL" ]] && export VULYK_BASE_URL="$BASE_URL"
  cargo run -p vulyk-api -- "${setup_args[@]}"
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
OS="$(detect_os)"
log "Detected OS: $OS"

parse_args "$@"

case "$MODE" in
  compose|prod|dev|setup|check|helm-template|perf-smoke)
    ;;
  -h|--help|help)
    usage
    exit 0
    ;;
  *)
    die "unknown mode: $MODE (try --help)"
    ;;
esac

if [[ "$MODE" == "check" || "$MODE" == "dev" || "$MODE" == "perf-smoke" ]]; then
  need_cmd rustc
  need_cmd cargo
  need_cmd node
  need_cmd npm
  log "rustc $(rustc --version)"
  log "node $(node --version)"
fi

if [[ "$MODE" == "check" ]]; then
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
  log "Toolchain OK for $OS"
  exit 0
fi

if [[ "$MODE" == "helm-template" ]]; then
  need_cmd helm
  log "Rendering Helm chart…"
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
    || die "Helm render missing setup Job"
  grep -q 'VULYK_REGISTRATION_MODE' /tmp/vulyk-helm-render.yaml \
    || die "Helm render missing VULYK_REGISTRATION_MODE"
  log "Helm chart rendered OK → /tmp/vulyk-helm-render.yaml"
  exit 0
fi

if [[ "$MODE" == "perf-smoke" ]]; then
  exec "$ROOT/scripts/perf-smoke.sh" "${@:2}"
fi

if [[ "$MODE" == "setup" ]]; then
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

if [[ "$MODE" == "dev" ]]; then
  collect_setup_options "dev"
  ensure_compose_env ".env"
  resolve_compose
  if [[ ${#COMPOSE[@]} -gt 0 ]]; then
    log "Starting Postgres (dev overlay)…"
    "${COMPOSE[@]}" -f docker-compose.dev.yml up -d
  else
    warn "Docker Compose not found — skipping Postgres container; install Docker or start Postgres yourself"
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

if [[ "$MODE" == "prod" ]]; then
  collect_setup_options "prod"
  ensure_prod_env ".env.prod"
  resolve_compose
  [[ ${#COMPOSE[@]} -gt 0 ]] || die "Docker Compose plugin or docker-compose required"
  log "Building and starting Vulyk production Compose…"
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
Backups: see docs/admin-guide.md
EOF
  print_setup_next_steps "prod"
  exit 0
fi

# Default: compose — write .env first, then require Docker
collect_setup_options "compose"
ensure_compose_env ".env"
resolve_compose
[[ ${#COMPOSE[@]} -gt 0 ]] || die "Docker Compose plugin or docker-compose required (first-run settings already in .env)"
log "Building and starting Vulyk (docker compose)…"
"${COMPOSE[@]}" --env-file .env up --build -d

cat <<EOF

Vulyk is starting (dev compose — auth optional).

  UI:        http://localhost:5173
  API:       http://localhost:8080
  OpenAPI:   http://localhost:8080/api/docs
  Health:    http://localhost:8080/health

Admin/demo seeding uses .env (VULYK_ADMIN_EMAIL=${ADMIN_EMAIL}, VULYK_SEED_DEMO=${SEED_DEMO}).
If you need an explicit setup pass:
  ./scripts/install.sh setup --yes
  # or: docker compose run --rm api vulyk-api setup --yes

For production hardening: ./scripts/install.sh prod
Logs:  docker compose logs -f
Stop:  docker compose down
EOF
print_setup_next_steps "compose"
