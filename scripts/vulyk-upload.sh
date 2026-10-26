#!/usr/bin/env bash
# Upload JUnit XML, Vulyk JSON, Playwright JSON, or pytest JSON results to Vulyk.
# Used by the GitHub Action and GitLab CI template (no Rust toolchain required).
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: vulyk-upload.sh --file PATH [--format auto|junit|vulyk_json|playwright_json|pytest_json] [options]

Options:
  --api-url URL          Vulyk API base (or VULYK_API_URL)
  --token TOKEN          Bearer token (or VULYK_API_TOKEN)
  --run-id UUID          Existing run (or VULYK_RUN_ID)
  --project-id UUID      Project id for --create-run / pipeline ingest (or VULYK_PROJECT_ID)
  --project-key KEY      Project key for --create-run / pipeline ingest (or VULYK_PROJECT_KEY)
  --run-name NAME        Name when creating a run (or VULYK_RUN_NAME)
  --create-run           Create a run when --run-id is unset
  --pipeline             Auto-create campaign/iteration/run via project ingest API
  --campaign NAME        Campaign name for --pipeline (or VULYK_CAMPAIGN)
  --iteration NAME       Iteration name for --pipeline (or VULYK_ITERATION)
  --cadence TEXT         Cadence label e.g. daily / on-push (or VULYK_CADENCE)
  --created-by NAME      CI bot / token display name (or VULYK_CREATED_BY)
  --environment JSON     Environment metadata JSON object (or VULYK_ENVIRONMENT)
  --artifacts JSON       Artifacts array/object JSON (or VULYK_ARTIFACTS)
  --async                Force async ingest
  --help                 Show this help
EOF
}

FILE=""
FORMAT="auto"
API_URL="${VULYK_API_URL:-}"
TOKEN="${VULYK_API_TOKEN:-}"
RUN_ID="${VULYK_RUN_ID:-}"
PROJECT_ID="${VULYK_PROJECT_ID:-}"
PROJECT_KEY="${VULYK_PROJECT_KEY:-}"
RUN_NAME="${VULYK_RUN_NAME:-}"
CREATE_RUN=0
PIPELINE=0
CAMPAIGN="${VULYK_CAMPAIGN:-}"
ITERATION="${VULYK_ITERATION:-}"
CADENCE="${VULYK_CADENCE:-}"
CREATED_BY="${VULYK_CREATED_BY:-}"
ENVIRONMENT="${VULYK_ENVIRONMENT:-}"
ARTIFACTS="${VULYK_ARTIFACTS:-}"
ASYNC=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --file) FILE="${2:-}"; shift 2 ;;
    --format) FORMAT="${2:-}"; shift 2 ;;
    --api-url) API_URL="${2:-}"; shift 2 ;;
    --token) TOKEN="${2:-}"; shift 2 ;;
    --run-id) RUN_ID="${2:-}"; shift 2 ;;
    --project-id) PROJECT_ID="${2:-}"; shift 2 ;;
    --project-key) PROJECT_KEY="${2:-}"; shift 2 ;;
    --run-name) RUN_NAME="${2:-}"; shift 2 ;;
    --create-run) CREATE_RUN=1; shift ;;
    --pipeline) PIPELINE=1; shift ;;
    --campaign) CAMPAIGN="${2:-}"; shift 2 ;;
    --iteration) ITERATION="${2:-}"; shift 2 ;;
    --cadence) CADENCE="${2:-}"; shift 2 ;;
    --created-by) CREATED_BY="${2:-}"; shift 2 ;;
    --environment) ENVIRONMENT="${2:-}"; shift 2 ;;
    --artifacts) ARTIFACTS="${2:-}"; shift 2 ;;
    --async) ASYNC=1; shift ;;
    --help|-h) usage; exit 0 ;;
    *) echo "unknown arg: $1" >&2; usage; exit 2 ;;
  esac
done

if [[ -z "${FILE}" || ! -f "${FILE}" ]]; then
  echo "error: --file is required and must exist" >&2
  exit 2
fi
if [[ -z "${API_URL}" ]]; then
  echo "error: --api-url or VULYK_API_URL required" >&2
  exit 2
fi

API_URL="${API_URL%/}"
AUTH=()
if [[ -n "${TOKEN}" ]]; then
  AUTH=(-H "Authorization: Bearer ${TOKEN}")
fi

detect_format() {
  local f="$1" content
  content="$(head -c 512 "${f}" || true)"
  case "${FORMAT}" in
    junit|vulyk_json|playwright_json|pytest_json) echo "${FORMAT}"; return ;;
    auto) ;;
    *) echo "error: bad --format" >&2; exit 2 ;;
  esac
  if [[ "${f}" == *.xml ]] || [[ "${content}" == "<?xml"* ]] || [[ "${content}" == "<testsuites"* ]] || [[ "${content}" == "<testsuite"* ]]; then
    echo junit
  elif [[ "${content}" == *"\"suites\""* ]] && [[ "${content}" == *"\"specs\""* ]]; then
    echo playwright_json
  elif [[ "${content}" == *"\"nodeid\""* ]] || [[ "${content}" == *"\"created\""* && "${content}" == *"\"tests\""* ]]; then
    echo pytest_json
  elif [[ "${f}" == *.json ]] || [[ "${content}" == "{"* ]]; then
    echo vulyk_json
  else
    echo "error: cannot detect format; pass --format" >&2
    exit 2
  fi
}

FORMAT="$(detect_format "${FILE}")"

resolve_project_id() {
  if [[ -n "${PROJECT_ID}" ]]; then
    return
  fi
  if [[ -z "${PROJECT_KEY}" ]]; then
    echo "error: --project-id or --project-key required" >&2
    exit 2
  fi
  PROJECTS="$(curl -fsS "${AUTH[@]}" "${API_URL}/api/v1/projects")"
  PROJECT_ID="$(PROJECT_KEY="${PROJECT_KEY}" PROJECTS="${PROJECTS}" python3 - <<'PY'
import json, os
key = os.environ["PROJECT_KEY"].upper()
for p in json.loads(os.environ["PROJECTS"]):
    if str(p.get("key","")).upper() == key:
        print(p["id"])
        break
else:
    raise SystemExit(f"project key {key} not found")
PY
)"
}

CONTENT="$(cat "${FILE}")"
ASYNC_BOOL=false
if [[ "${ASYNC}" -eq 1 ]]; then ASYNC_BOOL=true; fi

if [[ "${PIPELINE}" -eq 1 ]]; then
  resolve_project_id
  if [[ -z "${RUN_NAME}" ]]; then
    RUN_NAME="CI upload $(basename "${FILE}")"
  fi
  if [[ -z "${CREATED_BY}" ]]; then
    CREATED_BY="${GITHUB_ACTOR:-${GITLAB_USER_LOGIN:-ci-bot}}"
  fi
  if [[ -z "${ENVIRONMENT}" ]]; then
    ENVIRONMENT='{}'
  fi
  if [[ -z "${ARTIFACTS}" ]]; then
    ARTIFACTS='[]'
  fi
  PAYLOAD="$(FORMAT="${FORMAT}" CONTENT="${CONTENT}" ASYNC_BOOL="${ASYNC_BOOL}" \
    RUN_NAME="${RUN_NAME}" CAMPAIGN="${CAMPAIGN}" ITERATION="${ITERATION}" \
    CADENCE="${CADENCE}" CREATED_BY="${CREATED_BY}" ENVIRONMENT="${ENVIRONMENT}" \
    ARTIFACTS="${ARTIFACTS}" python3 - <<'PY'
import json, os
env = json.loads(os.environ.get("ENVIRONMENT") or "{}")
arts = json.loads(os.environ.get("ARTIFACTS") or "[]")
payload = {
    "format": os.environ["FORMAT"],
    "content": os.environ["CONTENT"],
    "async_mode": os.environ["ASYNC_BOOL"] == "true",
    "source": "ci",
    "run_name": os.environ["RUN_NAME"],
    "created_by_name": os.environ["CREATED_BY"],
    "environment": env,
    "artifacts": arts,
}
if os.environ.get("CAMPAIGN"):
    payload["campaign"] = os.environ["CAMPAIGN"]
if os.environ.get("ITERATION"):
    payload["iteration"] = os.environ["ITERATION"]
if os.environ.get("CADENCE"):
    payload["cadence"] = os.environ["CADENCE"]
print(json.dumps(payload))
PY
)"
  RESP="$(curl -fsS "${AUTH[@]}" -H 'Content-Type: application/json' \
    -d "${PAYLOAD}" \
    "${API_URL}/api/v1/projects/${PROJECT_ID}/ingest")"
  echo "${RESP}"
  RUN_ID="$(printf '%s' "${RESP}" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("run_id") or "")')"
else
  if [[ -z "${RUN_ID}" ]]; then
    if [[ "${CREATE_RUN}" -ne 1 ]]; then
      echo "error: --run-id required unless --create-run or --pipeline" >&2
      exit 2
    fi
    resolve_project_id
    if [[ -z "${RUN_NAME}" ]]; then
      RUN_NAME="CI upload $(basename "${FILE}")"
    fi
    RUN_PAYLOAD="$(RUN_NAME="${RUN_NAME}" python3 - <<'PY'
import json, os
print(json.dumps({"name": os.environ["RUN_NAME"], "status": "running", "source": "ci"}))
PY
)"
    RUN_JSON="$(curl -fsS "${AUTH[@]}" -H 'Content-Type: application/json' \
      -d "${RUN_PAYLOAD}" \
      "${API_URL}/api/v1/projects/${PROJECT_ID}/runs")"
    RUN_ID="$(printf '%s' "${RUN_JSON}" | python3 -c 'import json,sys
d=json.load(sys.stdin)
print(d.get("id") or d["run"]["id"])')"
    echo "created run ${RUN_ID}"
  fi

  PAYLOAD="$(FORMAT="${FORMAT}" CONTENT="${CONTENT}" ASYNC_BOOL="${ASYNC_BOOL}" \
    CREATED_BY="${CREATED_BY}" ENVIRONMENT="${ENVIRONMENT}" ARTIFACTS="${ARTIFACTS}" python3 - <<'PY'
import json, os
payload = {
    "format": os.environ["FORMAT"],
    "content": os.environ["CONTENT"],
    "async_mode": os.environ["ASYNC_BOOL"] == "true",
    "source": os.environ["FORMAT"],
}
if os.environ.get("CREATED_BY"):
    payload["created_by_name"] = os.environ["CREATED_BY"]
if os.environ.get("ENVIRONMENT"):
    payload["environment"] = json.loads(os.environ["ENVIRONMENT"])
if os.environ.get("ARTIFACTS"):
    payload["artifacts"] = json.loads(os.environ["ARTIFACTS"])
print(json.dumps(payload))
PY
)"

  RESP="$(curl -fsS "${AUTH[@]}" -H 'Content-Type: application/json' \
    -d "${PAYLOAD}" \
    "${API_URL}/api/v1/runs/${RUN_ID}/ingest")"
  echo "${RESP}"
fi

JOB_ID="$(printf '%s' "${RESP}" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("job_id") or "")')"
if [[ -n "${JOB_ID}" ]]; then
  for _ in $(seq 1 60); do
    JOB="$(curl -fsS "${AUTH[@]}" "${API_URL}/api/v1/jobs/${JOB_ID}")"
    STATUS="$(printf '%s' "${JOB}" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("status",""))')"
    if [[ "${STATUS}" == "completed" || "${STATUS}" == "failed" || "${STATUS}" == "cancelled" ]]; then
      echo "${JOB}"
      if [[ "${STATUS}" != "completed" ]]; then
        echo "error: ingest job ${STATUS}" >&2
        exit 1
      fi
      break
    fi
    sleep 0.5
  done
fi

echo "uploaded to run ${RUN_ID}"
