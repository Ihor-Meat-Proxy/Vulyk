#!/usr/bin/env bash
# Install / bootstrap Vulyk on Linux, macOS, or WSL2.
# Verified modes: compose | prod | dev | check | helm-template | perf-smoke
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

ensure_prod_env() {
  local env_file="${1:-.env.prod}"
  if [[ -f "$env_file" ]]; then
    log "Using existing $env_file"
    return 0
  fi
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
  log "Wrote $env_file (mode 600). Bootstrap the first admin at the UI login screen."
}

OS="$(detect_os)"
log "Detected OS: $OS"

MODE="${1:-compose}"

case "$MODE" in
  compose|prod|dev|check|helm-template|perf-smoke)
    ;;
  -h|--help|help)
    cat <<'EOF'
Usage: scripts/install.sh [compose|prod|dev|check|helm-template|perf-smoke]

  compose         Build and start the local/dev Docker Compose stack (default)
  prod            Production Compose (auth required, volumes, healthchecks)
  dev             Start Postgres only; print commands to run API + UI locally
  check           Verify local toolchain (rustc, cargo, node, docker/helm optional)
  helm-template   Render the Helm chart (requires helm) as a dry-run validation
  perf-smoke      Seed ~100k cases and check list/search latency (needs running API)

Environment:
  Linux, macOS, and WSL2 are supported. Native Windows is not — use WSL2.
EOF
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
    >/tmp/vulyk-helm-render.yaml
  # Basic sanity: auth required + core resources present
  grep -q 'VULYK_AUTH_REQUIRED: "true"' /tmp/vulyk-helm-render.yaml \
    || die "Helm render missing VULYK_AUTH_REQUIRED=true"
  grep -q 'kind: Deployment' /tmp/vulyk-helm-render.yaml \
    || die "Helm render missing Deployments"
  log "Helm chart rendered OK → /tmp/vulyk-helm-render.yaml"
  exit 0
fi

if [[ "$MODE" == "perf-smoke" ]]; then
  exec "$ROOT/scripts/perf-smoke.sh" "${@:2}"
fi

need_cmd docker
if docker compose version >/dev/null 2>&1; then
  COMPOSE=(docker compose)
elif command -v docker-compose >/dev/null 2>&1; then
  COMPOSE=(docker-compose)
else
  die "Docker Compose plugin or docker-compose required"
fi

if [[ "$MODE" == "dev" ]]; then
  log "Starting Postgres (dev overlay)…"
  "${COMPOSE[@]}" -f docker-compose.dev.yml up -d
  cat <<EOF

Postgres is up on localhost:5432 (user/pass/db: vulyk).

In one terminal:
  export DATABASE_URL=postgres://vulyk:vulyk@localhost:5432/vulyk
  cargo run -p vulyk-api

In another:
  cd frontend && npm install && npm run dev

API docs: http://localhost:8080/api/docs
UI:       http://localhost:5173
EOF
  exit 0
fi

if [[ "$MODE" == "prod" ]]; then
  ensure_prod_env ".env.prod"
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

Create the first admin on the login screen (bootstrap).
Logs:  docker compose -f docker-compose.prod.yml logs -f
Stop:  docker compose -f docker-compose.prod.yml down
Backups: see docs/admin-guide.md
EOF
  exit 0
fi

log "Building and starting Vulyk (docker compose)…"
"${COMPOSE[@]}" up --build -d

cat <<EOF

Vulyk is starting (dev compose — auth optional).

  UI:        http://localhost:5173
  API:       http://localhost:8080
  OpenAPI:   http://localhost:8080/api/docs
  Health:    http://localhost:8080/health

For production hardening: ./scripts/install.sh prod
Logs:  docker compose logs -f
Stop:  docker compose down
EOF
