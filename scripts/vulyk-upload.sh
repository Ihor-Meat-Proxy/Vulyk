#!/usr/bin/env bash
# Upload JUnit XML, Vulyk JSON, Playwright JSON, or pytest JSON results to Vulyk.
# Supports CI lifecycle: --open-run -> upload -> --close-run (also on failure).
# Used by the GitHub Action and GitLab CI template (no Rust toolchain required).
#
# Large result files are streamed via a temp JSON body + curl --data-binary @file
# so Linux agents do not hit "Argument list too long" / env size limits.
set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  vulyk-upload.sh --open-run [metadata options]
  vulyk-upload.sh --start-run [metadata options]
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
  --start-run            Attach / mark a queued run in_progress (branch, commit, pipeline URL)
  --close-run            Close a CI run
  --cancelled            Mark close as cancelled
  --pipeline             Legacy one-shot campaign/iteration ingest
  --provider NAME        github | gitlab | jenkins | forgejo | gitea | woodpecker | other
                         (or VULYK_CI_PROVIDER; auto-detects common CI hosts)
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
  --print-run-id         On --open-run: print only the run UUID on stdout (preferred capture)
  --run-id-file PATH     Write the opened run id to PATH (also for open-run)
  --dotenv-file PATH     Append VULYK_RUN_ID=... for GitLab dotenv / generic CI
  --help                 Show this help

Capture the opened run id with --print-run-id (same shell) or --run-id-file /
--dotenv-file. On GitHub Actions, --open-run also appends VULYK_RUN_ID to
$GITHUB_ENV for *later* steps only - it is not visible to the next command in
the same step. Prefer:

  VULYK_RUN_ID="$(./scripts/vulyk-upload.sh --open-run --print-run-id ...)"
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
START_RUN=0
CLOSE_RUN=0
CANCELLED=0
PIPELINE=0
PROVIDER="${VULYK_CI_PROVIDER:-}"
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
PRINT_RUN_ID=0
RUN_ID_FILE="${VULYK_RUN_ID_FILE:-}"
DOTENV_FILE="${VULYK_DOTENV_FILE:-}"

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
    --start-run) START_RUN=1; shift ;;
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
    --print-run-id) PRINT_RUN_ID=1; shift ;;
    --run-id-file) RUN_ID_FILE="${2:-}"; shift 2 ;;
    --dotenv-file) DOTENV_FILE="${2:-}"; shift 2 ;;
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

# Detect CI provider when --provider / VULYK_CI_PROVIDER unset.
detect_ci_provider() {
  if [[ -n "${PROVIDER}" ]]; then
    return
  fi
  if [[ -n "${FORGEJO_ACTIONS:-}${FORGEJO:-}" ]]; then
    PROVIDER="forgejo"
    return
  fi
  if [[ -n "${GITEA_ACTIONS:-}" ]]; then
    PROVIDER="gitea"
    return
  fi
  if [[ -n "${CI:-}" && -n "${CI_REPO:-}${CI_PIPELINE_NUMBER:-}${WOODPECKER_BACKEND:-}" ]]; then
    # Woodpecker sets CI=woodpecker on modern agents; older agents still export CI_REPO.
    case "${CI}" in
      woodpecker|woodpecker-ci) PROVIDER="woodpecker"; return ;;
    esac
    if [[ -n "${CI_REPO:-}" && -n "${CI_PIPELINE_NUMBER:-}" && -z "${GITHUB_ACTIONS:-}${GITLAB_CI:-}" ]]; then
      PROVIDER="woodpecker"
      return
    fi
  fi
  if [[ -n "${GITHUB_ACTIONS:-}" ]]; then
    case "${GITHUB_SERVER_URL:-}" in
      *forgejo*|*Forgejo*) PROVIDER="forgejo" ;;
      *gitea*|*Gitea*) PROVIDER="gitea" ;;
      *) PROVIDER="github" ;;
    esac
    return
  fi
  if [[ -n "${GITLAB_CI:-}" ]]; then
    PROVIDER="gitlab"
    return
  fi
  if [[ -n "${JENKINS_URL:-}${BUILD_URL:-}" ]]; then
    PROVIDER="jenkins"
    return
  fi
}
detect_ci_provider

# Under GitLab CI, default dotenv path so later jobs can pick up the run id.
if [[ -z "${DOTENV_FILE}" && -n "${GITLAB_CI:-}" ]]; then
  DOTENV_FILE="vulyk.env"
fi

TMP_FILES=()
cleanup() {
  local f
  # Bash 3.2 (macOS): empty "${arr[@]}" under set -u can abort.
  if [ "${#TMP_FILES[@]}" -gt 0 ]; then
    for f in "${TMP_FILES[@]}"; do
      rm -f "${f}" 2>/dev/null || true
    done
  fi
}
trap cleanup EXIT

mktemp_payload() {
  local t
  # macOS mktemp needs the Xs at the end of the template path.
  t="$(mktemp "${TMPDIR:-/tmp}/vulyk-upload.XXXXXX" 2>/dev/null \
    || mktemp -t vulyk-upload)"
  TMP_FILES+=("${t}")
  printf '%s' "${t}"
}

# POST JSON body from a file (avoids ARG_MAX / env size limits).
curl_json_file() {
  local method="$1" url="$2" body_file="$3"
  curl -fsS -X "${method}" ${AUTH[@]+"${AUTH[@]}"} \
    -H 'Content-Type: application/json' \
    --data-binary @"${body_file}" \
    "${url}"
}

export_run_id() {
  local id="$1"
  if [[ -z "${id}" ]]; then
    echo "error: open-run response did not include a run id" >&2
    exit 1
  fi
  echo "VULYK_RUN_ID=${id}" >&2
  if [[ -n "${GITHUB_ENV:-}" ]]; then
    echo "VULYK_RUN_ID=${id}" >> "${GITHUB_ENV}"
  fi
  if [[ -n "${RUN_ID_FILE}" ]]; then
    printf '%s\n' "${id}" > "${RUN_ID_FILE}"
  fi
  if [[ -n "${DOTENV_FILE}" ]]; then
    # Replace prior value if re-running in the same job.
    if [[ -f "${DOTENV_FILE}" ]]; then
      grep -v '^VULYK_RUN_ID=' "${DOTENV_FILE}" > "${DOTENV_FILE}.tmp" || true
      mv "${DOTENV_FILE}.tmp" "${DOTENV_FILE}"
    fi
    echo "VULYK_RUN_ID=${id}" >> "${DOTENV_FILE}"
  fi
}

resolve_project_id() {
  if [[ -n "${PROJECT_ID}" ]]; then
    return
  fi
  if [[ -z "${PROJECT_KEY}" ]]; then
    echo "error: --project-id or --project-key required" >&2
    exit 2
  fi
  local projects
  projects="$(curl -fsS ${AUTH[@]+"${AUTH[@]}"} "${API_URL}/api/v1/projects")"
  PROJECT_ID="$(PROJECT_KEY="${PROJECT_KEY}" PROJECTS="${projects}" python3 - <<'PY'
import json, os
key = os.environ["PROJECT_KEY"].upper()
for p in json.loads(os.environ["PROJECTS"]):
    if str(p.get("key", "")).upper() == key:
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
    CREATED_BY="${GITHUB_ACTOR:-${GITLAB_USER_LOGIN:-${BUILD_USER:-ci-bot}}}"
  fi
  BODY_FILE="$(mktemp_payload)"
  SUITE_NAME="${SUITE_NAME}" PROVIDER="${PROVIDER}" REPO="${REPO}" \
    BRANCH="${BRANCH}" COMMIT="${COMMIT}" PIPELINE_URL="${PIPELINE_URL}" \
    JOB_URL="${JOB_URL}" EXTERNAL_REF="${EXTERNAL_REF}" CAMPAIGN="${CAMPAIGN}" \
    ITERATION="${ITERATION}" CREATED_BY="${CREATED_BY}" \
    ENVIRONMENT="${ENVIRONMENT:-{}}" ARTIFACTS="${ARTIFACTS:-[]}" \
    BODY_FILE="${BODY_FILE}" python3 - <<'PY'
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
with open(os.environ["BODY_FILE"], "w", encoding="utf-8") as f:
    json.dump(payload, f, ensure_ascii=False)
PY
  RESP="$(curl_json_file POST \
    "${API_URL}/api/v1/projects/${PROJECT_ID}/ci/runs" \
    "${BODY_FILE}")"
  RUN_ID="$(printf '%s' "${RESP}" | python3 -c 'import json,sys
d=json.load(sys.stdin)
print(d.get("id") or d.get("run_id") or (d.get("run") or {}).get("id") or "")')"
  export_run_id "${RUN_ID}"
  if [[ "${PRINT_RUN_ID}" -eq 1 ]]; then
    printf '%s\n' "${RUN_ID}"
  else
    echo "${RESP}"
  fi
  exit 0
fi

# - start / attach CI run -
if [[ "${START_RUN}" -eq 1 ]]; then
  if [[ -z "${RUN_ID}" ]]; then
    echo "error: --run-id or VULYK_RUN_ID required for --start-run" >&2
    exit 2
  fi
  resolve_project_id
  BODY_FILE="$(mktemp_payload)"
  BRANCH="${BRANCH}" COMMIT="${COMMIT}" PIPELINE_URL="${PIPELINE_URL}" \
    JOB_URL="${JOB_URL}" BODY_FILE="${BODY_FILE}" python3 - <<'PY'
import json, os
payload = {}
for k, envk in [
    ("branch", "BRANCH"),
    ("commit", "COMMIT"),
    ("pipeline_url", "PIPELINE_URL"),
    ("job_url", "JOB_URL"),
]:
    v = os.environ.get(envk) or ""
    if v:
        payload[k] = v
with open(os.environ["BODY_FILE"], "w", encoding="utf-8") as f:
    json.dump(payload, f, ensure_ascii=False)
PY
  curl_json_file POST \
    "${API_URL}/api/v1/projects/${PROJECT_ID}/ci/runs/${RUN_ID}/start" \
    "${BODY_FILE}"
  echo
  exit 0
fi

# - close CI run -
if [[ "${CLOSE_RUN}" -eq 1 ]]; then
  if [[ -z "${RUN_ID}" ]]; then
    echo "error: --run-id or VULYK_RUN_ID required for --close-run" >&2
    exit 2
  fi
  resolve_project_id
  BODY='{}'
  if [[ "${CANCELLED}" -eq 1 ]]; then
    BODY='{"cancelled":true}'
  fi
  BODY_FILE="$(mktemp_payload)"
  printf '%s' "${BODY}" > "${BODY_FILE}"
  curl_json_file POST \
    "${API_URL}/api/v1/projects/${PROJECT_ID}/ci/runs/${RUN_ID}/close" \
    "${BODY_FILE}"
  echo
  exit 0
fi

if [[ -z "${FILE}" || ! -f "${FILE}" ]]; then
  echo "error: --file is required and must exist (unless --open-run / --start-run / --close-run)" >&2
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
ASYNC_BOOL=false
if [[ "${ASYNC}" -eq 1 ]]; then ASYNC_BOOL=true; fi

# Build ingest JSON on disk: python reads --file content (never via argv/env).
write_ingest_payload() {
  local out="$1" mode="$2"
  FORMAT="${FORMAT}" ASYNC_BOOL="${ASYNC_BOOL}" FILE_PATH="${FILE}" \
    RUN_NAME="${RUN_NAME}" CAMPAIGN="${CAMPAIGN}" ITERATION="${ITERATION}" \
    CADENCE="${CADENCE}" CREATED_BY="${CREATED_BY}" \
    ENVIRONMENT="${ENVIRONMENT}" ARTIFACTS="${ARTIFACTS}" \
    MODE="${mode}" OUT="${out}" python3 - <<'PY'
import json, os
from pathlib import Path

content = Path(os.environ["FILE_PATH"]).read_text(encoding="utf-8", errors="replace")
mode = os.environ["MODE"]
payload = {
    "format": os.environ["FORMAT"],
    "content": content,
    "async_mode": os.environ["ASYNC_BOOL"] == "true",
}
if mode == "pipeline":
    env = json.loads(os.environ.get("ENVIRONMENT") or "{}")
    arts = json.loads(os.environ.get("ARTIFACTS") or "[]")
    payload.update({
        "source": "ci",
        "run_name": os.environ["RUN_NAME"],
        "created_by_name": os.environ["CREATED_BY"],
        "environment": env,
        "artifacts": arts,
    })
    if os.environ.get("CAMPAIGN"):
        payload["campaign"] = os.environ["CAMPAIGN"]
    if os.environ.get("ITERATION"):
        payload["iteration"] = os.environ["ITERATION"]
    if os.environ.get("CADENCE"):
        payload["cadence"] = os.environ["CADENCE"]
else:
    payload["source"] = os.environ["FORMAT"]
    if os.environ.get("CREATED_BY"):
        payload["created_by_name"] = os.environ["CREATED_BY"]
    if os.environ.get("ENVIRONMENT"):
        payload["environment"] = json.loads(os.environ["ENVIRONMENT"])
    if os.environ.get("ARTIFACTS"):
        payload["artifacts"] = json.loads(os.environ["ARTIFACTS"])
with open(os.environ["OUT"], "w", encoding="utf-8") as f:
    json.dump(payload, f, ensure_ascii=False)
PY
}

if [[ "${PIPELINE}" -eq 1 ]]; then
  resolve_project_id
  if [[ -z "${RUN_NAME}" ]]; then
    RUN_NAME="CI upload $(basename "${FILE}")"
  fi
  if [[ -z "${CREATED_BY}" ]]; then
    CREATED_BY="${GITHUB_ACTOR:-${GITLAB_USER_LOGIN:-${BUILD_USER:-ci-bot}}}"
  fi
  if [[ -z "${ENVIRONMENT}" ]]; then
    ENVIRONMENT='{}'
  fi
  if [[ -z "${ARTIFACTS}" ]]; then
    ARTIFACTS='[]'
  fi
  BODY_FILE="$(mktemp_payload)"
  write_ingest_payload "${BODY_FILE}" pipeline
  RESP="$(curl_json_file POST \
    "${API_URL}/api/v1/projects/${PROJECT_ID}/ingest" \
    "${BODY_FILE}")"
  echo "${RESP}"
  RUN_ID="$(printf '%s' "${RESP}" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("run_id") or "")')"
else
  if [[ -z "${RUN_ID}" ]]; then
    if [[ "${CREATE_RUN}" -ne 1 ]]; then
      echo "error: --run-id / VULYK_RUN_ID required unless --create-run, --pipeline, or --open-run" >&2
      echo "hint: capture open-run with --print-run-id or --run-id-file, or export VULYK_RUN_ID" >&2
      exit 2
    fi
    resolve_project_id
    if [[ -z "${RUN_NAME}" ]]; then
      RUN_NAME="CI upload $(basename "${FILE}")"
    fi
    BODY_FILE="$(mktemp_payload)"
    RUN_NAME="${RUN_NAME}" BODY_FILE="${BODY_FILE}" python3 - <<'PY'
import json, os
with open(os.environ["BODY_FILE"], "w", encoding="utf-8") as f:
    json.dump({"name": os.environ["RUN_NAME"], "status": "running", "source": "ci"}, f)
PY
    RUN_JSON="$(curl_json_file POST \
      "${API_URL}/api/v1/projects/${PROJECT_ID}/runs" \
      "${BODY_FILE}")"
    RUN_ID="$(printf '%s' "${RUN_JSON}" | python3 -c 'import json,sys
d=json.load(sys.stdin)
print(d.get("id") or d["run"]["id"])')"
    echo "created run ${RUN_ID}"
  fi

  if [[ -z "${RUN_ID}" ]]; then
    echo "error: empty run id - refuse to upload" >&2
    exit 2
  fi

  BODY_FILE="$(mktemp_payload)"
  write_ingest_payload "${BODY_FILE}" ingest

  # Prefer CI ingest when project is known; else legacy run ingest.
  if [[ -n "${PROJECT_ID}" ]] || [[ -n "${PROJECT_KEY}" ]]; then
    resolve_project_id
    RESP="$(curl_json_file POST \
      "${API_URL}/api/v1/projects/${PROJECT_ID}/ci/runs/${RUN_ID}/ingest" \
      "${BODY_FILE}" \
      || curl_json_file POST \
        "${API_URL}/api/v1/runs/${RUN_ID}/ingest" \
        "${BODY_FILE}")"
  else
    RESP="$(curl_json_file POST \
      "${API_URL}/api/v1/runs/${RUN_ID}/ingest" \
      "${BODY_FILE}")"
  fi
  echo "${RESP}"
fi

JOB_ID="$(printf '%s' "${RESP}" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("job_id") or "")')"
if [[ -n "${JOB_ID}" ]]; then
  for _ in $(seq 1 60); do
    JOB="$(curl -fsS ${AUTH[@]+"${AUTH[@]}"} "${API_URL}/api/v1/jobs/${JOB_ID}")"
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
