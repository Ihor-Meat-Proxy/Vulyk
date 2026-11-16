#!/usr/bin/env bash
# Local Playwright e2e against an isolated Compose stack + official Playwright image.
# Never touches the operator's .env or the default vulyk_pg / vulyk_pg_dev volumes.
#
# Usage:
#   ./scripts/e2e-local.sh              # wipe e2e volume, up stack, run suite, leave stack up
#   ./scripts/e2e-local.sh --keep-data  # reuse existing e2e DB volume (skip wipe)
#   ./scripts/e2e-local.sh --build      # force rebuild api/web images
#   ./scripts/e2e-local.sh --sync       # create/sync VUE project + cases before run
#   ./scripts/e2e-local.sh --report     # sync + mint CI token + post results to VUE
#   ./scripts/e2e-local.sh --ui         # Playwright UI mode (needs local display/X11)
#   ./scripts/e2e-local.sh --down       # stop stack (keeps e2e volume)
#   ./scripts/e2e-local.sh --reset      # stop stack AND delete e2e DB volume (exit)
#   ./scripts/e2e-local.sh --repeat 3   # run the suite N times (flake gate; wipe once at start)
#
# Isolation: by default each invocation resets the e2e Compose volumes before
# bringing the stack up, so leftover projects/repos/backups from prior runs
# cannot poison locators. Use --keep-data to skip that wipe. --repeat reuses
# the same fresh volume across iterations (wipe once at start, not per loop).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

COMPOSE=(docker compose -f docker-compose.e2e.yml --env-file e2e/.docker.env)
ENV_FILE="$ROOT/e2e/.docker.env"
ENV_EXAMPLE="$ROOT/e2e/docker.env.example"
ARTDIR="$ROOT/e2e/artifacts"
PW_VERSION=""

DO_BUILD=0
DO_SYNC=0
DO_REPORT=0
DO_UI=0
DO_DOWN=0
DO_RESET=0
DO_KEEP_DATA=0
DO_RUN=1
REPEAT=1
EXTRA_PW_ARGS=()

usage() {
  sed -n '2,22p' "$0" | sed 's/^# \?//'
  exit "${1:-0}"
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help) usage 0 ;;
    --build) DO_BUILD=1; shift ;;
    --sync) DO_SYNC=1; shift ;;
    --report) DO_REPORT=1; DO_SYNC=1; shift ;;
    --ui) DO_UI=1; shift ;;
    --down) DO_DOWN=1; DO_RUN=0; shift ;;
    --reset) DO_RESET=1; DO_RUN=0; shift ;;
    --keep-data) DO_KEEP_DATA=1; shift ;;
    --repeat)
      REPEAT="${2:?--repeat needs a number}"
      shift 2
      ;;
    --)
      shift
      EXTRA_PW_ARGS+=("$@")
      break
      ;;
    *)
      EXTRA_PW_ARGS+=("$1")
      shift
      ;;
  esac
done

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "error: required command not found: $1" >&2
    exit 1
  }
}

need_cmd docker
docker compose version >/dev/null 2>&1 || {
  echo "error: docker compose plugin required" >&2
  exit 1
}

# Pin Playwright image to the same version as @playwright/test in the lockfile.
resolve_playwright_version() {
  node -e "
    const fs = require('fs');
    const lock = JSON.parse(fs.readFileSync('e2e/package-lock.json','utf8'));
    const v =
      lock.packages?.['node_modules/@playwright/test']?.version ||
      lock.dependencies?.['@playwright/test']?.version;
    if (!v) { console.error('cannot resolve @playwright/test version'); process.exit(1); }
    process.stdout.write(v);
  "
}

ensure_env() {
  if [[ ! -f "$ENV_FILE" ]]; then
    cp "$ENV_EXAMPLE" "$ENV_FILE"
    echo "Wrote $ENV_FILE from docker.env.example"
  fi
  set -a
  # shellcheck disable=SC1090,SC1091
  source "$ENV_FILE"
  set +a
  PW_VERSION="$(resolve_playwright_version)"
  export VULYK_E2E_PLAYWRIGHT_IMAGE="${VULYK_E2E_PLAYWRIGHT_IMAGE:-mcr.microsoft.com/playwright:v${PW_VERSION}-jammy}"
  mkdir -p "$ARTDIR/html-report" "$ARTDIR/test-results" "$ARTDIR/traces"
}

# Some Cloud / nested-Docker hosts drop bridge forwards unless DOCKER-USER allows
# them. When iptables-nft and iptables-legacy both exist, Docker may write nft
# rules while the kernel still enforces a legacy FORWARD DROP — fix both.
ensure_docker_forward() {
  local ipt
  for ipt in iptables iptables-nft iptables-legacy; do
    command -v "$ipt" >/dev/null 2>&1 || continue
    if ! "$ipt" -C DOCKER-USER -j ACCEPT 2>/dev/null; then
      if "$ipt" -I DOCKER-USER -j ACCEPT 2>/dev/null; then
        echo "Inserted $ipt DOCKER-USER ACCEPT (bridge forward)"
      elif command -v sudo >/dev/null 2>&1 && sudo "$ipt" -I DOCKER-USER -j ACCEPT 2>/dev/null; then
        echo "Inserted $ipt DOCKER-USER ACCEPT via sudo (bridge forward)"
      fi
    fi
    # Accept forwarded traffic even when the table policy defaults to DROP.
    if "$ipt" -P FORWARD ACCEPT 2>/dev/null; then
      :
    elif command -v sudo >/dev/null 2>&1; then
      sudo "$ipt" -P FORWARD ACCEPT 2>/dev/null || true
    fi
  done
}

compose_up() {
  echo "==> Starting isolated e2e stack (project vulyk-e2e)"
  ensure_docker_forward
  if [[ "$DO_BUILD" -eq 1 ]]; then
    "${COMPOSE[@]}" build api web
  fi
  "${COMPOSE[@]}" up -d db mock-oidc api web
  echo "==> Waiting for API health..."
  local api_port="${E2E_API_PORT:-18080}"
  for _ in $(seq 1 90); do
    if curl -fsS "http://127.0.0.1:${api_port}/health" >/dev/null 2>&1; then
      echo "API ready on :${api_port}"
      break
    fi
    sleep 2
  done
  curl -fsS "http://127.0.0.1:${api_port}/health" >/dev/null
  local web_port="${E2E_WEB_PORT:-15173}"
  for _ in $(seq 1 60); do
    if curl -fsS "http://127.0.0.1:${web_port}/" >/dev/null 2>&1; then
      echo "Web ready on :${web_port}"
      return 0
    fi
    sleep 1
  done
  echo "error: web did not become ready on :${web_port}" >&2
  "${COMPOSE[@]}" logs --tail=80 web api
  exit 1
}

host_api_url() {
  echo "http://127.0.0.1:${E2E_API_PORT:-18080}"
}

upsert_env_kv() {
  local key="$1" value="$2"
  if rg -q "^${key}=" "$ENV_FILE"; then
    sed -i "s|^${key}=.*|${key}=${value}|" "$ENV_FILE"
  else
    printf '\n%s=%s\n' "$key" "$value" >>"$ENV_FILE"
  fi
}

sync_vue_project() {
  echo "==> Syncing Vulyk UI E2E (VUE) project via API"
  VULYK_API_URL="$(host_api_url)" \
  VULYK_ADMIN_EMAIL="${VULYK_ADMIN_EMAIL:-admin@vulyk.local}" \
  VULYK_ADMIN_PASSWORD="${VULYK_ADMIN_PASSWORD:-admin12345678901}" \
  node e2e/scripts/sync-vue-project.mjs ${DO_REPORT:+--mint-ci-token} | tee "$ARTDIR/vue-sync.log"
  # Idempotency gate (S1): a second sync against the live stack must create 0.
  echo "==> Re-syncing VUE project (idempotency check)"
  VULYK_API_URL="$(host_api_url)" \
  VULYK_ADMIN_EMAIL="${VULYK_ADMIN_EMAIL:-admin@vulyk.local}" \
  VULYK_ADMIN_PASSWORD="${VULYK_ADMIN_PASSWORD:-admin12345678901}" \
  node e2e/scripts/sync-vue-project.mjs | tee "$ARTDIR/vue-sync-2.log"
  if ! rg -q 'created=0' "$ARTDIR/vue-sync-2.log"; then
    echo "error: second VUE sync did not report created=0 (see $ARTDIR/vue-sync-2.log)" >&2
    exit 1
  fi
  local project_id
  project_id="$(node -e "const m=require('./e2e/vulyk-vue-case-map.json'); process.stdout.write(m.project?.id||'')")"
  if [[ -n "${project_id:-}" ]]; then
    upsert_env_kv VULYK_PROJECT_ID "$project_id"
    export VULYK_PROJECT_ID="$project_id"
    echo "Stored VULYK_PROJECT_ID=${project_id}"
  fi
  if [[ "$DO_REPORT" -eq 1 ]]; then
    local token
    token="$(rg -o 'VULYK_CI_TOKEN=\S+' "$ARTDIR/vue-sync.log" | tail -1 | cut -d= -f2- || true)"
    if [[ -n "${token:-}" ]]; then
      upsert_env_kv VULYK_CI_TOKEN "$token"
      export VULYK_CI_TOKEN="$token"
      echo "Stored CI token in e2e/.docker.env"
    fi
  fi
}

run_playwright_once() {
  local run_idx="$1"
  local stamp
  stamp="$(date +%Y%m%d-%H%M%S)-run${run_idx}"
  echo "==> Playwright run #${run_idx} (image ${VULYK_E2E_PLAYWRIGHT_IMAGE})"

  # Reporters come from playwright.config.ts (list + html + vulyk-reporter).
  local pw_cmd=(npx playwright test)
  if [[ "$DO_UI" -eq 1 ]]; then
    pw_cmd=(npx playwright test --ui --ui-host=0.0.0.0 --ui-port=9323)
  fi
  if [[ ${#EXTRA_PW_ARGS[@]} -gt 0 ]]; then
    pw_cmd+=("${EXTRA_PW_ARGS[@]}")
  fi

  # Re-source env so --report token is visible to compose.
  set -a
  # shellcheck disable=SC1090,SC1091
  source "$ENV_FILE"
  set +a

  set +e
  "${COMPOSE[@]}" --profile playwright run --rm --no-deps \
    -e "PLAYWRIGHT_HTML_REPORT=/work/e2e/artifacts/html-report/${stamp}" \
    -e "PLAYWRIGHT_JSON_OUTPUT_NAME=/work/e2e/artifacts/results-${stamp}.json" \
    -e "VULYK_CI_TOKEN=${VULYK_CI_TOKEN:-}" \
    -e "VULYK_PROJECT=${VULYK_PROJECT_ID:-${VULYK_PROJECT:-VUE}}" \
    -e "VULYK_PROJECT_ID=${VULYK_PROJECT_ID:-}" \
    -e "VULYK_CASE_KEY_PREFIX=${VULYK_CASE_KEY_PREFIX:-VUE}" \
    -e "VULYK_RUN_NAME=${VULYK_RUN_NAME:-Local Playwright e2e #${run_idx}}" \
    playwright \
    "set -euo pipefail
     apt-get update -qq >/dev/null
     apt-get install -y -qq postgresql-client >/dev/null
     cd /work/e2e
     npm ci --no-audit --no-fund
     mkdir -p artifacts/html-report artifacts/test-results artifacts/traces
     ${pw_cmd[*]}
    "
  local rc=$?
  set -e

  # Keep a copy of the latest HTML report at a stable path.
  if [[ -d "$ARTDIR/html-report/${stamp}" ]]; then
    rm -rf "$ARTDIR/html-report/latest"
    cp -a "$ARTDIR/html-report/${stamp}" "$ARTDIR/html-report/latest"
  fi
  echo "HTML report: e2e/artifacts/html-report/${stamp}/index.html"
  return "$rc"
}

ensure_env

if [[ "$DO_RESET" -eq 1 ]]; then
  echo "==> Resetting e2e stack + volumes"
  "${COMPOSE[@]}" --profile playwright down -v --remove-orphans || true
  exit 0
fi

if [[ "$DO_DOWN" -eq 1 ]]; then
  echo "==> Stopping e2e stack (volume kept)"
  "${COMPOSE[@]}" --profile playwright down --remove-orphans || true
  exit 0
fi

# Default: wipe e2e volumes once at start so prior-run leftovers cannot flake
# the suite. --keep-data skips this; --repeat still only wipes once here.
if [[ "$DO_KEEP_DATA" -eq 0 ]]; then
  echo "==> Wiping e2e volumes for a clean run (pass --keep-data to reuse)"
  "${COMPOSE[@]}" --profile playwright down -v --remove-orphans || true
else
  echo "==> Keeping existing e2e volumes (--keep-data)"
fi

compose_up

if [[ "$DO_SYNC" -eq 1 ]]; then
  sync_vue_project
fi

if [[ "$DO_RUN" -eq 1 ]]; then
  fail=0
  for i in $(seq 1 "$REPEAT"); do
    if ! run_playwright_once "$i"; then
      fail=1
      echo "error: Playwright run #${i} failed" >&2
      break
    fi
  done
  if [[ "$fail" -ne 0 ]]; then
    echo "Tips: open e2e/artifacts/html-report/latest/index.html or inspect traces under e2e/artifacts/" >&2
    exit 1
  fi
  echo "==> ${REPEAT} green run(s) complete"
fi
