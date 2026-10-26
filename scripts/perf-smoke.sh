#!/usr/bin/env bash
# Performance smoke: seed ~100k cases and check list/search latency.
# Prefers DATABASE_URL for fast SQL seed; falls back to HTTP creates.
set -euo pipefail

API_URL="${VULYK_API_URL:-http://127.0.0.1:8080}"
TARGET_CASES="${PERF_CASES:-100000}"
LIST_BUDGET_MS="${PERF_LIST_BUDGET_MS:-2000}"
SEARCH_BUDGET_MS="${PERF_SEARCH_BUDGET_MS:-2500}"
AUTH_HEADER=()
if [[ -n "${VULYK_API_TOKEN:-}" ]]; then
  AUTH_HEADER=(-H "Authorization: Bearer ${VULYK_API_TOKEN}")
fi

need() { command -v "$1" >/dev/null 2>&1 || { echo "missing $1" >&2; exit 1; }; }
need curl
need python3

log() { printf '==> %s\n' "$*"; }

log "API: $API_URL  target cases: ${TARGET_CASES}"
curl -fsS "${AUTH_HEADER[@]}" "${API_URL}/health" >/dev/null

PROJECT="$(curl -fsS "${AUTH_HEADER[@]}" -H 'Content-Type: application/json' \
  -d '{"key":"PERF","name":"Perf smoke","description":"seed cases for latency check"}' \
  "${API_URL}/api/v1/projects" 2>/dev/null || true)"

if [[ -z "${PROJECT}" || "${PROJECT}" == *"error"* ]]; then
  PROJECTS="$(curl -fsS "${AUTH_HEADER[@]}" "${API_URL}/api/v1/projects")"
  PROJECT="$(python3 -c 'import json,sys; ps=json.loads(sys.argv[1]);
print(next((json.dumps(p) for p in ps if p.get("key")=="PERF"), "{}"))' "${PROJECTS}")"
fi
PROJECT_ID="$(python3 -c 'import json,sys; print(json.loads(sys.argv[1])["id"])' "${PROJECT}")"
log "Project id=${PROJECT_ID}"

EXISTING="$(curl -fsS "${AUTH_HEADER[@]}" \
  "${API_URL}/api/v1/projects/${PROJECT_ID}/cases?limit=1" | \
  python3 -c 'import json,sys; print(json.load(sys.stdin).get("total",0))')"
log "Existing cases: ${EXISTING}"

NEED=$((TARGET_CASES - EXISTING))
if (( NEED > 0 )); then
  if [[ -n "${DATABASE_URL:-}" ]] && command -v psql >/dev/null 2>&1; then
    log "Seeding $NEED cases via SQL (DATABASE_URL)..."
    START=$((EXISTING + 1))
    END=$((EXISTING + NEED))
    psql "${DATABASE_URL}" -v ON_ERROR_STOP=1 <<SQL
INSERT INTO test_cases (project_id, key, title, description, precondition, priority, status)
SELECT
  '${PROJECT_ID}'::uuid,
  'PERF-' || lpad(g::text, 6, '0'),
  'Perf case ' || g::text,
  'seed',
  '',
  'medium',
  'ready'
FROM generate_series(${START}, ${END}) AS g
ON CONFLICT (project_id, key) DO NOTHING;
-- touch search_vector for a representative subset
UPDATE test_cases
SET title = title
WHERE project_id = '${PROJECT_ID}'::uuid
  AND key IN ('PERF-000001', 'PERF-050000', 'PERF-' || lpad('${END}'::text, 6, '0'));
SQL
  else
    log "Seeding $NEED cases via HTTP (slower; set DATABASE_URL+psql for SQL seed)..."
    # Cap HTTP seed for CI friendliness unless PERF_FORCE_HTTP=1
    if (( NEED > 2000 )) && [[ "${PERF_FORCE_HTTP:-}" != "1" ]]; then
      log "Refusing to HTTP-seed >2000 cases without DATABASE_URL (set PERF_FORCE_HTTP=1 to override)"
      exit 1
    fi
    python3 - "${API_URL}" "${PROJECT_ID}" "${NEED}" "${EXISTING}" "${VULYK_API_TOKEN:-}" <<'PY'
import json, sys, urllib.request
api, project_id, need, existing, token = sys.argv[1], sys.argv[2], int(sys.argv[3]), int(sys.argv[4]), sys.argv[5]
headers = {"Content-Type": "application/json"}
if token:
    headers["Authorization"] = f"Bearer {token}"
for i in range(1, need + 1):
    idx = existing + i
    body = {
        "key": f"PERF-{idx:06d}",
        "title": f"Perf case {idx}",
        "description": "seed",
        "priority": "medium",
        "status": "ready",
        "steps": [{"action": "noop", "expected_result": "ok"}],
    }
    req = urllib.request.Request(
        f"{api}/api/v1/projects/{project_id}/cases",
        data=json.dumps(body).encode(),
        headers=headers,
        method="POST",
    )
    with urllib.request.urlopen(req, timeout=60) as r:
        r.read()
    if i % 200 == 0:
        print(f"created {i}/{need}", flush=True)
PY
  fi
else
  log "Already at/above target; skipping seed"
fi

measure() {
  local label="$1"
  local url="$2"
  python3 - "${label}" "${url}" "${VULYK_API_TOKEN:-}" <<'PY'
import sys, time, urllib.request
label, url, token = sys.argv[1], sys.argv[2], sys.argv[3]
headers = {}
if token:
    headers["Authorization"] = f"Bearer {token}"
req = urllib.request.Request(url, headers=headers)
t0 = time.perf_counter()
with urllib.request.urlopen(req, timeout=60) as r:
    body = r.read()
    status = r.status
elapsed_ms = (time.perf_counter() - t0) * 1000
print(f"{label}: HTTP {status} {elapsed_ms:.1f}ms bytes={len(body)}")
open("/tmp/vulyk-perf-last-ms", "w").write(str(elapsed_ms))
PY
}

measure "list" "${API_URL}/api/v1/projects/${PROJECT_ID}/cases?limit=50&offset=0"
LIST_MS="$(python3 -c 'print(float(open("/tmp/vulyk-perf-last-ms").read()))')"

measure "search" "${API_URL}/api/v1/projects/${PROJECT_ID}/cases?q=Perf%20case%2050000&limit=25"
SEARCH_MS="$(python3 -c 'print(float(open("/tmp/vulyk-perf-last-ms").read()))')"

# Also hit dedicated search endpoint when present
if curl -fsS -o /dev/null -w '' "${AUTH_HEADER[@]}" \
  "${API_URL}/api/v1/projects/${PROJECT_ID}/cases/search?q=Perf&limit=25" 2>/dev/null; then
  measure "search-endpoint" "${API_URL}/api/v1/projects/${PROJECT_ID}/cases/search?q=Perf%20case%2050000&limit=25"
fi

python3 - "${LIST_MS}" "${SEARCH_MS}" "${LIST_BUDGET_MS}" "${SEARCH_BUDGET_MS}" <<'PY'
import sys
list_ms, search_ms, list_budget, search_budget = map(float, sys.argv[1:])
ok = True
if list_ms > list_budget:
    print(f"FAIL list latency {list_ms:.1f}ms > budget {list_budget:.0f}ms", flush=True)
    ok = False
else:
    print(f"PASS list latency {list_ms:.1f}ms <= {list_budget:.0f}ms", flush=True)
if search_ms > search_budget:
    print(f"FAIL search latency {search_ms:.1f}ms > budget {search_budget:.0f}ms", flush=True)
    ok = False
else:
    print(f"PASS search latency {search_ms:.1f}ms <= {search_budget:.0f}ms", flush=True)
sys.exit(0 if ok else 1)
PY
