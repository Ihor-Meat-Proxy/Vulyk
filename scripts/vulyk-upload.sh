#!/usr/bin/env bash
# Upload JUnit XML, Vulyk JSON, Playwright JSON, or pytest JSON results to Vulyk.
# Supports CI lifecycle: --open-run -> upload -> --close-run (also on failure).
# Used by the GitHub Action and GitLab CI template (no Rust toolchain required).
set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  vulyk-upload.sh --open-run [metadata options]
  vulyk-upload.sh --file PATH [--format auto|junit|...] [options]
  vulyk-upload.sh --close-run [--cancelled] [options]

Options:
  --api-url URL          Vulyk API base (or VULYK_API_URL)
  --token TOKEN          Bearer token (or VULYK_API_TOKEN / VULYK_CI_TOKEN)
  --run-id UUID          Existing run (or VULYK_RUN_ID)
  --project-id UUID      Project id (or VULYK_PROJECT_ID)
  --project-key KEY      Project key (or VULYK_PROJECT_KEY)
  --run-name NAME        Name when creating a legacy run (or VULYK_RUN_NAME)
  --suite-name NAME      CI suite name (or VULYK_RUN_NAME)
  --create-run           Legacy: create a run when --run-id is unset
  --open-run             Open a CI lifecycle run (status=open)
  --close-run            Close a CI run
  --cancelled            Mark close as cancelled
  --pipeline             Legacy one-shot campaign/iteration ingest
  --provider NAME        github | gitlab | other
  --repo TEXT            owner/repo or path
  --branch TEXT          Git branch
  --commit SHA           Git commit
  --pipeline-url URL     CI pipeline URL
  --job-url URL          CI job URL
  --external-ref TEXT    Idempotency key for open-run
  --campaign NAME        Campaign name (or VULYK_CAMPAIGN)
  --iteration NAME       Iteration name (or VULYK_ITERATION)
  --cadence TEXT         Cadence label (or VULYK_CADENCE)
  --created-by NAME      CI bot display name (or VULYK_CREATED_BY)
  --environment JSON     Environment metadata JSON object
  --artifacts JSON       Artifacts array/object JSON
  --async                Force async ingest
  --help                 Show this help
EOF
}

FILE=""
FORMAT="auto"
API_URL="${VULYK_API_URL:-}"
TOKEN="${VULYK_API_TOKEN:-${VULYK_CI_TOKEN:-}}"
RUN_ID="${VULYK_RUN_ID:-}"
PROJECT_ID="${VULYK_PROJECT_ID:-}"
PROJECT_KEY="${VULYK_PROJECT_KEY:-}"
RUN_NAME="${VULYK_RUN_NAME:-}"
SUITE_NAME=""
CREATE_RUN=0
OPEN_RUN=0
CLOSE_RUN=0
CANCELLED=0
PIPELINE=0
PROVIDER=""
REPO=""
BRANCH=""
COMMIT=""
PIPELINE_URL=""
JOB_URL=""
EXTERNAL_REF="${VULYK_EXTERNAL_REF:-}"
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
    --suite-name) SUITE_NAME="${2:-}"; shift 2 ;;
    --create-run) CREATE_RUN=1; shift ;;
    --open-run) OPEN_RUN=1; shift ;;
    --close-run) CLOSE_RUN=1; shift ;;
    --cancelled) CANCELLED=1; shift ;;
    --pipeline) PIPELINE=1; shift ;;
    --provider) PROVIDER="${2:-}"; shift 2 ;;
    --repo) REPO="${2:-}"; shift 2 ;;
    --branch) BRANCH="${2:-}"; shift 2 ;;
    --commit) COMMIT="${2:-}"; shift 2 ;;
    --pipeline-url) PIPELINE_URL="${2:-}"; shift 2 ;;
    --job-url) JOB_URL="${2:-}"; shift 2 ;;
    --external-ref) EXTERNAL_REF="${2:-}"; shift 2 ;;
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

if [[ -z "${API_URL}" ]]; then
  echo "error: --api-url or VULYK_API_URL required" >&2
  exit 2
fi

API_URL="${API_URL%/}"
AUTH=()
if [[ -n "${TOKEN}" ]]; then
  AUTH=(-H "Authorization: Bearer ${TOKEN}")
fi

if [[ -z "${SUITE_NAME}" && -n "${RUN_NAME}" ]]; then
  SUITE_NAME="${RUN_NAME}"
fi

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

# - open CI run -
if [[ "${OPEN_RUN}" -eq 1 ]]; then
  resolve_project_id
  if [[ -z "${SUITE_NAME}" ]]; then
    SUITE_NAME="CI run"
  fi
  if [[ -z "${CREATED_BY}" ]]; then
    CREATED_BY="${GITHUB_ACTOR:-${GITLAB_USER_LOGIN:-ci-bot}}"
  fi
  PAYLOAD="$(SUITE_NAME="${SUITE_NAME}" PROVIDER="${PROVIDER}" REPO="${REPO}" \
    BRANCH="${BRANCH}" COMMIT="${COMMIT}" PIPELINE_URL="${PIPELINE_URL}" \
    JOB_URL="${JOB_URL}" EXTERNAL_REF="${EXTERNAL_REF}" CAMPAIGN="${CAMPAIGN}" \
    ITERATION="${ITERATION}" CREATED_BY="${CREATED_BY}" ENVIRONMENT="${ENVIRONMENT:-{}}" \
    ARTIFACTS="${ARTIFACTS:-[]}" python3 - <<'PY'
import json, os
env = json.loads(os.environ.get("ENVIRONMENT") or "{}")
arts = json.loads(os.environ.get("ARTIFACTS") or "[]")
payload = {
    "suite_name": os.environ["SUITE_NAME"],
    "name": os.environ["SUITE_NAME"],
    "created_by_name": os.environ.get("CREATED_BY") or None,
    "environment": env,
    "artifacts": arts,
}
for k, envk in [
    ("provider", "PROVIDER"),
    ("repo", "REPO"),
    ("branch", "BRANCH"),
    ("commit", "COMMIT"),
    ("pipeline_url", "PIPELINE_URL"),
    ("job_url", "JOB_URL"),
    ("external_ref", "EXTERNAL_REF"),
    ("campaign", "CAMPAIGN"),
    ("iteration", "ITERATION"),
]:
    v = os.environ.get(envk) or ""
    if v:
        payload[k] = v
print(json.dumps(payload))
PY
)"
  RESP="$(curl -fsS "${AUTH[@]}" -H 'Content-Type: application/json' \
    -d "${PAYLOAD}" \
    "${API_URL}/api/v1/projects/${PROJECT_ID}/ci/runs")"
  echo "${RESP}"
  RUN_ID="$(printf '%s' "${RESP}" | python3 -c 'import json,sys
d=json.load(sys.stdin)
print(d.get("id") or d.get("run_id") or (d.get("run") or {}).get("id") or "")')"
  if [[ -n "${GITHUB_ENV:-}" && -n "${RUN_ID}" ]]; then
    echo "VULYK_RUN_ID=${RUN_ID}" >> "${GITHUB_ENV}"
  fi
  exit 0
fi

# - close CI run -
if [[ "${CLOSE_RUN}" -eq 1 ]]; then
  if [[ -z "${RUN_ID}" ]]; then
    echo "error: --run-id required for --close-run" >&2
    exit 2
  fi
  BODY='{}'
  if [[ "${CANCELLED}" -eq 1 ]]; then
    BODY='{"cancelled":true}'
  fi
  # Prefer CI close endpoint; fall back to legacy status patch.
  if curl -fsS "${AUTH[@]}" -H 'Content-Type: application/json' \
      -d "${BODY}" \
      "${API_URL}/api/v1/projects/${PROJECT_ID:-_}/ci/runs/${RUN_ID}/close" 2>/dev/null; then
    echo
  elif [[ -n "${PROJECT_ID}" ]]; then
    curl -fsS "${AUTH[@]}" -H 'Content-Type: application/json' \
      -d "${BODY}" \
      "${API_URL}/api/v1/projects/${PROJECT_ID}/ci/runs/${RUN_ID}/close"
    echo
  else
    # Discover project via run detail is not always available; try path without project.
    curl -fsS "${AUTH[@]}" -H 'Content-Type: application/json' \
      -d "${BODY}" \
      "${API_URL}/api/v1/ci/runs/${RUN_ID}/close" \
      || curl -fsS "${AUTH[@]}" -H 'Content-Type: application/json' \
        -d '{"status":"closed"}' \
        "${API_URL}/api/v1/runs/${RUN_ID}"
    echo
  fi
  exit 0
fi

if [[ -z "${FILE}" || ! -f "${FILE}" ]]; then
  echo "error: --file is required and must exist (unless --open-run / --close-run)" >&2
  exit 2
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
      echo "error: --run-id required unless --create-run, --pipeline, or --open-run" >&2
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

  # Prefer CI ingest path when project id known; else legacy run ingest.
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

  if [[ -n "${PROJECT_ID}" ]]; then
    RESP="$(curl -fsS "${AUTH[@]}" -H 'Content-Type: application/json' \
      -d "${PAYLOAD}" \
      "${API_URL}/api/v1/projects/${PROJECT_ID}/ci/runs/${RUN_ID}/ingest" \
      || curl -fsS "${AUTH[@]}" -H 'Content-Type: application/json' \
        -d "${PAYLOAD}" \
        "${API_URL}/api/v1/runs/${RUN_ID}/ingest")"
  else
    RESP="$(curl -fsS "${AUTH[@]}" -H 'Content-Type: application/json' \
      -d "${PAYLOAD}" \
      "${API_URL}/api/v1/runs/${RUN_ID}/ingest")"
  fi
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
