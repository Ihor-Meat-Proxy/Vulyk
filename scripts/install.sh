#!/usr/bin/env bash
# Install / bootstrap Vulyk on Linux, macOS, or WSL2.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

log() { printf '==> %s\n' "$*"; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || die "missing required command: $1"
}

detect_os() {
  case "$(uname -s)" in
    Linux*) echo linux ;;
    Darwin*) echo macos ;;
    *) die "unsupported OS: $(uname -s)" ;;
  esac
}

OS="$(detect_os)"
log "Detected OS: $OS"

MODE="${1:-compose}"

case "$MODE" in
  compose|dev|check)
    ;;
  -h|--help|help)
    cat <<'EOF'
Usage: scripts/install.sh [compose|dev|check]

  compose  Build and start full stack with Docker Compose (default)
  dev      Start Postgres only; print commands to run API + UI locally
  check    Verify local toolchain (rustc, cargo, node, docker)
EOF
    exit 0
    ;;
  *)
    die "unknown mode: $MODE (try --help)"
    ;;
esac

if [[ "$MODE" == "check" || "$MODE" == "dev" ]]; then
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
  else
    log "docker not found (optional for host-only dev)"
  fi
  log "Toolchain OK"
  exit 0
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

log "Building and starting Vulyk (docker compose)…"
"${COMPOSE[@]}" up --build -d

cat <<EOF

Vulyk is starting.

  UI:        http://localhost:5173
  API:       http://localhost:8080
  OpenAPI:   http://localhost:8080/api/docs
  Health:    http://localhost:8080/health

Logs:  docker compose logs -f
Stop:  docker compose down
EOF
