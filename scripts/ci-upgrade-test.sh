#!/usr/bin/env bash
# CI: start API at BASE_SHA with seeded data, upgrade to HEAD, verify integrity.
# Requires: DATABASE_URL, rust toolchain, postgres reachable.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

BASE_SHA="${1:-${VULYK_UPGRADE_BASE_SHA:-}}"
API_URL="${VULYK_API_URL:-http://127.0.0.1:8080}"
export VULYK_API_URL="${API_URL}"
BIND_ADDR="${BIND_ADDR:-127.0.0.1:8080}"
PERF_CASES="${PERF_CASES:-200}"

die() { printf 'error: %s\n' "$*" >&2; exit 1; }
log() { printf '==> %s\n' "$*"; }

[[ -n "${BASE_SHA}" ]] || die "usage: $0 <base-sha> (or set VULYK_UPGRADE_BASE_SHA)"
[[ -n "${DATABASE_URL:-}" ]] || die "DATABASE_URL is required"

need_cmd() { command -v "$1" >/dev/null 2>&1 || die "missing $1"; }
need_cmd cargo
need_cmd curl
need_cmd python3
need_cmd git

wait_health() {
  local _
  for _ in $(seq 1 90); do
    if curl -fsS "${API_URL}/health" >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.5
  done
  return 1
}

stop_api() {
  if [[ -f /tmp/vulyk-upgrade-api.pid ]]; then
    kill "$(cat /tmp/vulyk-upgrade-api.pid)" 2>/dev/null || true
    rm -f /tmp/vulyk-upgrade-api.pid
    sleep 1
  fi
}

HEAD_SHA="$(git rev-parse HEAD)"
log "Upgrade test: ${BASE_SHA} -> ${HEAD_SHA}"

WORKDIR="$(mktemp -d /tmp/vulyk-upgrade-XXXXXX)"
cleanup() {
  stop_api
  rm -rf "${WORKDIR}"
}
trap cleanup EXIT

log "Building base API at ${BASE_SHA}..."
git archive "${BASE_SHA}" | tar -x -C "${WORKDIR}"
# Reuse workspace target dir for faster incremental builds when possible
mkdir -p "${WORKDIR}/target"
if [[ -d "${ROOT}/target" ]]; then
  # Share compiled deps via CARGO_TARGET_DIR at HEAD; build base in workdir target
  :
fi
(
  cd "${WORKDIR}"
  cargo build -p vulyk-api --release
)

stop_api
log "Starting base API (auto-migrate + seed)..."
export BIND_ADDR
export VULYK_AUTO_MIGRATE=true
export VULYK_AUTH_REQUIRED="${VULYK_AUTH_REQUIRED:-false}"
export VULYK_JWT_SECRET="${VULYK_JWT_SECRET:-upgrade-ci-jwt}"
export VULYK_SECRETS_KEY="${VULYK_SECRETS_KEY:-upgrade-ci-secrets}"
export VULYK_ADMIN_EMAIL="${VULYK_ADMIN_EMAIL:-admin@vulyk.local}"
export VULYK_ADMIN_PASSWORD="${VULYK_ADMIN_PASSWORD:-admin12345}"
export VULYK_SEED_DEMO="${VULYK_SEED_DEMO:-1}"
export VULYK_SEED_SAMPLE="${VULYK_SEED_SAMPLE:-1}"
export VULYK_LLM_MOCK=1

"${WORKDIR}/target/release/vulyk-api" > /tmp/vulyk-upgrade-base.log 2>&1 &
echo $! > /tmp/vulyk-upgrade-api.pid
if ! wait_health; then
  cat /tmp/vulyk-upgrade-base.log >&2 || true
  die "base API failed to become healthy"
fi

log "Capturing seeded project snapshot..."
curl -fsS "${API_URL}/api/v1/projects" > /tmp/vulyk-upgrade-projects-before.json
python3 - <<'PY'
import json, sys
data = json.load(open("/tmp/vulyk-upgrade-projects-before.json"))
items = data["items"] if isinstance(data, dict) else data
keys = sorted(p["key"] for p in items)
print("projects:", ", ".join(keys) or "(none)")
wanted = {"KYIV_CONCERT_2026", "VULYK_QA"}
found = wanted.intersection(keys)
if not found:
    print(
        "warning: sample project keys not found yet; continuing with whatever was seeded",
        file=sys.stderr,
    )
open("/tmp/vulyk-upgrade-project-keys.txt", "w").write("\n".join(keys) + "\n")
PY

if command -v psql >/dev/null 2>&1 && [[ "${PERF_CASES}" -gt 0 ]]; then
  log "Seeding perf subset (${PERF_CASES} cases)..."
  PERF_JSON="$(curl -fsS -H 'Content-Type: application/json' \
    -d '{"key":"PERF","name":"Upgrade Perf","description":"ci upgrade subset"}' \
    "${API_URL}/api/v1/projects" 2>/dev/null || true)"
  if [[ -z "${PERF_JSON}" ]]; then
    PERF_JSON="$(python3 - <<'PY'
import json
data = json.load(open("/tmp/vulyk-upgrade-projects-before.json"))
items = data["items"] if isinstance(data, dict) else data
for p in items:
    if p["key"] == "PERF":
        print(json.dumps(p))
        break
PY
)"
  fi
  if [[ -n "${PERF_JSON}" ]]; then
    PERF_ID="$(python3 -c 'import json,sys; print(json.loads(sys.argv[1])["id"])' "${PERF_JSON}")"
    psql "${DATABASE_URL}" -v ON_ERROR_STOP=1 <<SQL
INSERT INTO test_cases (project_id, key, title, description, precondition, priority, status)
SELECT
  '${PERF_ID}'::uuid,
  'PERF-' || lpad(g::text, 6, '0'),
  'Perf case ' || g::text,
  'upgrade-ci',
  '',
  'medium',
  'ready'
FROM generate_series(1, ${PERF_CASES}) AS g
ON CONFLICT (project_id, key) DO NOTHING;
SQL
    log "Perf subset seeded for project ${PERF_ID}"
    curl -fsS "${API_URL}/api/v1/projects" > /tmp/vulyk-upgrade-projects-before.json
    python3 - <<'PY'
import json
data = json.load(open("/tmp/vulyk-upgrade-projects-before.json"))
items = data["items"] if isinstance(data, dict) else data
keys = sorted(p["key"] for p in items)
open("/tmp/vulyk-upgrade-project-keys.txt", "w").write("\n".join(keys) + "\n")
PY
  fi
fi

python3 - <<'PY'
import json, os, urllib.request
api = os.environ["VULYK_API_URL"]
data = json.load(open("/tmp/vulyk-upgrade-projects-before.json"))
items = data["items"] if isinstance(data, dict) else data
counts = {}
for p in items:
    url = f"{api}/api/v1/projects/{p['id']}/cases?limit=1&offset=0"
    with urllib.request.urlopen(url) as r:
        body = json.load(r)
    total = body.get("total", len(body.get("items", [])))
    counts[p["key"]] = total
    print(f"{p['key']}={total}")
open("/tmp/vulyk-upgrade-case-counts-before.json", "w").write(json.dumps(counts))
PY

BASE_HEALTH="$(curl -fsS "${API_URL}/health")"
log "Base health: ${BASE_HEALTH}"

stop_api
log "Building HEAD API..."
cargo build -p vulyk-api --release

log "Running explicit migrate on HEAD binary..."
export VULYK_AUTO_MIGRATE=false
./target/release/vulyk-api migrate status || true
./target/release/vulyk-api migrate run
./target/release/vulyk-api version

log "Starting HEAD API (auto-migrate off)..."
./target/release/vulyk-api > /tmp/vulyk-upgrade-head.log 2>&1 &
echo $! > /tmp/vulyk-upgrade-api.pid
if ! wait_health; then
  cat /tmp/vulyk-upgrade-head.log >&2 || true
  die "head API failed to become healthy"
fi

HEAD_HEALTH="$(curl -fsS "${API_URL}/health")"
log "Head health: ${HEAD_HEALTH}"
python3 -c 'import json,sys; h=json.loads(sys.argv[1]); assert h.get("status")=="ok", h; assert h.get("version"), h; print("version", h["version"])' "${HEAD_HEALTH}"

log "Verifying projects and case counts..."
curl -fsS "${API_URL}/api/v1/projects" > /tmp/vulyk-upgrade-projects-after.json
python3 - <<'PY'
import json, os, urllib.request, sys
api = os.environ["VULYK_API_URL"]
before_keys = [k for k in open("/tmp/vulyk-upgrade-project-keys.txt").read().splitlines() if k]
before_counts = json.load(open("/tmp/vulyk-upgrade-case-counts-before.json"))
after = json.load(open("/tmp/vulyk-upgrade-projects-after.json"))
items = after["items"] if isinstance(after, dict) else after
after_keys = sorted(p["key"] for p in items)
missing = [k for k in before_keys if k not in after_keys]
if missing:
    print("missing projects after upgrade:", missing, file=sys.stderr)
    sys.exit(1)
for p in items:
    url = f"{api}/api/v1/projects/{p['id']}/cases?limit=1&offset=0"
    with urllib.request.urlopen(url) as r:
        body = json.load(r)
    total = body.get("total", len(body.get("items", [])))
    prev = before_counts.get(p["key"])
    if prev is not None and total < prev:
        print(f"case count dropped for {p['key']}: {prev} -> {total}", file=sys.stderr)
        sys.exit(1)
    print(f"ok {p['key']}: cases={total} (before={prev})")
print("upgrade data integrity OK")
PY

log "Upgrade CI test passed"
