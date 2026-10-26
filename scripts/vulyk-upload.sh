#!/usr/bin/env bash
# Upload JUnit XML or Vulyk JSON results to a Vulyk API instance.
# Used by the GitHub Action and GitLab CI template (no Rust toolchain required).
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: vulyk-upload.sh --file PATH [--format auto|junit|vulyk_json] [options]

Options:
  --api-url URL          Vulyk API base (or VULYK_API_URL)
  --token TOKEN          Bearer token (or VULYK_API_TOKEN)
  --run-id UUID          Existing run (or VULYK_RUN_ID)
  --project-id UUID      Project id for --create-run (or VULYK_PROJECT_ID)
  --project-key KEY      Project key for --create-run (or VULYK_PROJECT_KEY)
  --run-name NAME        Name when creating a run (or VULYK_RUN_NAME)
  --create-run           Create a run when --run-id is unset
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

json_escape() {
  # shellcheck disable=SC2001
  printf '%s' "$1" | python3 -c 'import json,sys; print(json.dumps(sys.stdin.read()), end="")'
}

detect_format() {
  local f="$1" content
  content="$(head -c 256 "${f}" || true)"
  case "${FORMAT}" in
    junit|vulyk_json) echo "${FORMAT}"; return ;;
    auto) ;;
    *) echo "error: bad --format" >&2; exit 2 ;;
  esac
  if [[ "${f}" == *.xml ]] || [[ "${content}" == "<?xml"* ]] || [[ "${content}" == "<testsuites"* ]] || [[ "${content}" == "<testsuite"* ]]; then
    echo junit
  elif [[ "${f}" == *.json ]] || [[ "${content}" == "{"* ]]; then
    echo vulyk_json
  else
    echo "error: cannot detect format; pass --format" >&2
    exit 2
  fi
}

FORMAT="$(detect_format "${FILE}")"

if [[ -z "${RUN_ID}" ]]; then
  if [[ "${CREATE_RUN}" -ne 1 ]]; then
    echo "error: --run-id required unless --create-run" >&2
    exit 2
  fi
  if [[ -z "${PROJECT_ID}" ]]; then
    if [[ -z "${PROJECT_KEY}" ]]; then
      echo "error: --project-id or --project-key required with --create-run" >&2
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
  fi
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

CONTENT="$(cat "${FILE}")"
ASYNC_BOOL=false
if [[ "${ASYNC}" -eq 1 ]]; then ASYNC_BOOL=true; fi

PAYLOAD="$(FORMAT="${FORMAT}" CONTENT="${CONTENT}" ASYNC_BOOL="${ASYNC_BOOL}" python3 - <<'PY'
import json, os
print(json.dumps({
    "format": os.environ["FORMAT"],
    "content": os.environ["CONTENT"],
    "async_mode": os.environ["ASYNC_BOOL"] == "true",
    "source": os.environ["FORMAT"],
}))
PY
)"

RESP="$(curl -fsS "${AUTH[@]}" -H 'Content-Type: application/json' \
  -d "${PAYLOAD}" \
  "${API_URL}/api/v1/runs/${RUN_ID}/ingest")"
echo "${RESP}"

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
