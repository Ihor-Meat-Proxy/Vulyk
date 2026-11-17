#!/usr/bin/env bash
# Unit-ish tests for scripts/vulyk-upload.sh (mock HTTP server; no live API).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="${ROOT}/scripts/vulyk-upload.sh"
[[ -x "${SCRIPT}" ]] || chmod +x "${SCRIPT}"

WORKDIR="$(mktemp -d /tmp/vulyk-upload-test.XXXXXX)"
cleanup() { rm -rf "${WORKDIR}"; }
trap cleanup EXIT

pass=0
fail=0
assert_eq() {
  local name="$1" got="$2" want="$3"
  if [[ "${got}" == "${want}" ]]; then
    echo "PASS ${name}"
    pass=$((pass + 1))
  else
    echo "FAIL ${name}: got='${got}' want='${want}'" >&2
    fail=$((fail + 1))
  fi
}

# --- mock API ---
PROJECT_ID="11111111-1111-1111-1111-111111111111"
RUN_ID="22222222-2222-2222-2222-222222222222"
cat > "${WORKDIR}/server.py" <<'PY'
import json, os, threading
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path

PORT = int(os.environ["PORT"])
PROJECT_ID = os.environ["PROJECT_ID"]
RUN_ID = os.environ["RUN_ID"]
LOG = Path(os.environ["LOG"])
MAX_BODY = Path(os.environ["MAX_BODY"])
OPEN_BODY = Path(os.environ["OPEN_BODY"])

class H(BaseHTTPRequestHandler):
    def log_message(self, *a):  # quiet
        pass

    def _read(self):
        n = int(self.headers.get("Content-Length") or 0)
        body = self.rfile.read(n) if n else b""
        MAX_BODY.write_text(str(max(int(MAX_BODY.read_text() or "0"), len(body))))
        with LOG.open("a") as f:
            f.write(f"{self.command} {self.path} bytes={len(body)}\n")
        return body

    def _json(self, code, obj):
        data = json.dumps(obj).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        if self.path.startswith("/api/v1/projects"):
            self._json(200, [{"id": PROJECT_ID, "key": "KOC"}])
            return
        self._json(404, {"error": "not found"})

    def do_POST(self):
        body = self._read()
        path = self.path
        if path == f"/api/v1/projects/{PROJECT_ID}/ci/runs":
            OPEN_BODY.write_text(body.decode("utf-8") or "{}")
            self._json(200, {"id": RUN_ID, "status": "open"})
            return
        if path == f"/api/v1/projects/{PROJECT_ID}/ci/runs/{RUN_ID}/start":
            payload = json.loads(body.decode("utf-8") or "{}")
            assert payload.get("branch") or payload.get("pipeline_url")
            self._json(200, {"id": RUN_ID, "status": "in_progress", **payload})
            return
        if path == f"/api/v1/projects/{PROJECT_ID}/ci/runs/{RUN_ID}/close":
            self._json(200, {"id": RUN_ID, "status": "closed"})
            return
        if path == f"/api/v1/projects/{PROJECT_ID}/ci/runs/{RUN_ID}/ingest":
            # Ensure body was JSON with a content field (large ok).
            payload = json.loads(body.decode("utf-8"))
            assert "content" in payload and len(payload["content"]) > 0
            self._json(200, {"run_id": RUN_ID, "matched": 0})
            return
        if path == f"/api/v1/projects/{PROJECT_ID}/ingest":
            payload = json.loads(body.decode("utf-8"))
            OPEN_BODY.write_text(json.dumps(payload))
            assert payload.get("content")
            self._json(200, {"run_id": RUN_ID, "inserted": 0})
            return
        if path == f"/api/v1/projects/_/ci/runs/{RUN_ID}/close":
            self._json(404, {"error": "not found"})
            return
        if path == f"/api/v1/runs/{RUN_ID}":
            self._json(405, {"error": "method not allowed"})
            return
        self._json(404, {"error": f"unexpected {path}"})

httpd = HTTPServer(("127.0.0.1", PORT), H)
threading.Thread(target=httpd.serve_forever, daemon=True).start()
Path(os.environ["READY"]).write_text("ok")
threading.Event().wait()
PY

: > "${WORKDIR}/log.txt"
echo 0 > "${WORKDIR}/max_body.txt"
: > "${WORKDIR}/ready"
PORT=0
# pick a free port
PORT="$(python3 - <<'PY'
import socket
s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()
PY
)"
: > "${WORKDIR}/open_body.json"
PORT="${PORT}" PROJECT_ID="${PROJECT_ID}" RUN_ID="${RUN_ID}" \
  LOG="${WORKDIR}/log.txt" MAX_BODY="${WORKDIR}/max_body.txt" \
  OPEN_BODY="${WORKDIR}/open_body.json" READY="${WORKDIR}/ready" \
  python3 -u "${WORKDIR}/server.py" &
SERVER_PID=$!
ready=0
i=0
while [ "${i}" -lt 50 ]; do
  if [ -s "${WORKDIR}/ready" ]; then
    ready=1
    break
  fi
  i=$((i + 1))
  sleep 0.05
done
if [ "${ready}" -ne 1 ]; then
  echo "FAIL mock server did not become ready (python3=$(command -v python3))" >&2
  kill "${SERVER_PID}" 2>/dev/null || true
  exit 1
fi
API="http://127.0.0.1:${PORT}"
echo "mock API at ${API}"

# Bitbucket auto-detect + default pipeline_url on open-run.
# Clear host CI markers so this simulation wins when the suite runs under Actions.
: > "${WORKDIR}/open_body.json"
env -u GITHUB_ACTIONS -u GITLAB_CI -u GITHUB_SERVER_URL \
  BITBUCKET_BUILD_NUMBER=42 BITBUCKET_REPO_FULL_NAME='acme/widgets' \
  BITBUCKET_BRANCH=main BITBUCKET_COMMIT=deadbeef \
  "${SCRIPT}" --api-url "${API}" --project-key KOC --open-run \
  --suite-name "bb" --print-run-id >/dev/null 2>"${WORKDIR}/bb-err.txt"
BB_PAYLOAD="$(cat "${WORKDIR}/open_body.json")"
if PAYLOAD="${BB_PAYLOAD}" python3 - <<'PY'
import json, os, sys
payload = json.loads(os.environ["PAYLOAD"])
assert payload.get("provider") == "bitbucket", payload
want = "https://bitbucket.org/acme/widgets/pipelines/results/42"
assert payload.get("pipeline_url") == want, payload
PY
then
  echo "PASS bitbucket-open-run-detect"
  pass=$((pass + 1))
else
  echo "FAIL bitbucket-open-run-detect payload=${BB_PAYLOAD}" >&2
  fail=$((fail + 1))
fi

# J1: --print-run-id + dotenv
DOT="${WORKDIR}/vulyk.env"
OUT="$("${SCRIPT}" --api-url "${API}" --project-key KOC --open-run \
  --suite-name "test" --print-run-id --dotenv-file "${DOT}" 2>"${WORKDIR}/err.txt" | tr -d '\r')"
assert_eq "print-run-id" "${OUT}" "${RUN_ID}"
assert_eq "dotenv" "$(grep '^VULYK_RUN_ID=' "${DOT}")" "VULYK_RUN_ID=${RUN_ID}"
grep -q "VULYK_RUN_ID=${RUN_ID}" "${WORKDIR}/err.txt" || {
  echo "FAIL stderr export line" >&2
  fail=$((fail + 1))
}

# J1: upload without run id fails loudly
set +e
"${SCRIPT}" --api-url "${API}" --project-key KOC --file /etc/hosts --format junit \
  >/dev/null 2>"${WORKDIR}/no-run.txt"
rc=$?
set -e
assert_eq "upload-without-run-id-exit" "${rc}" "2"
grep -qi 'run-id' "${WORKDIR}/no-run.txt" || {
  echo "FAIL expected run-id error message" >&2
  fail=$((fail + 1))
}
grep -qi 'same shell\|GITHUB_ENV\|Jenkins' "${WORKDIR}/no-run.txt" || {
  echo "FAIL expected same-step / GITHUB_ENV migration hint" >&2
  fail=$((fail + 1))
}

# J12: --pipeline forwards provider + pipeline_url
: > "${WORKDIR}/open_body.json"
echo '<testsuite tests="0"/>' > "${WORKDIR}/empty.xml"
"${SCRIPT}" --api-url "${API}" --project-key KOC --pipeline \
  --file "${WORKDIR}/empty.xml" --format junit \
  --provider gitlab --pipeline-url "https://gitlab.example/p/-/pipelines/9" \
  --branch main --commit deadbeef >/dev/null
if PAYLOAD="$(cat "${WORKDIR}/open_body.json")" python3 - <<'PY'
import json, os, sys
payload = json.loads(os.environ["PAYLOAD"])
assert payload.get("provider") == "gitlab", payload
assert payload.get("pipeline_url") == "https://gitlab.example/p/-/pipelines/9", payload
assert payload.get("branch") == "main", payload
assert payload.get("commit") == "deadbeef", payload
PY
then
  echo "PASS pipeline-provider-url"
  pass=$((pass + 1))
else
  echo "FAIL pipeline-provider-url payload=$(cat "${WORKDIR}/open_body.json")" >&2
  fail=$((fail + 1))
fi

# J11: --start-run with project-key (attach branch/commit/pipeline_url)
: > "${WORKDIR}/log.txt"
"${SCRIPT}" --api-url "${API}" --project-key KOC --start-run --run-id "${RUN_ID}" \
  --branch "feature/j11" --commit "abc123" --pipeline-url "https://ci.example/1" >/dev/null
if grep -q "POST /api/v1/projects/${PROJECT_ID}/ci/runs/${RUN_ID}/start" "${WORKDIR}/log.txt"; then
  echo "PASS start-run-resolves-project-key"
  pass=$((pass + 1))
else
  echo "FAIL start-run did not hit /start" >&2
  fail=$((fail + 1))
fi

# J2: close with project-key (must not hit /projects/_/... or POST /runs/{id})
: > "${WORKDIR}/log.txt"
"${SCRIPT}" --api-url "${API}" --project-key KOC --close-run --run-id "${RUN_ID}" >/dev/null
if grep -q '/projects/_/' "${WORKDIR}/log.txt"; then
  echo "FAIL close used placeholder project _" >&2
  fail=$((fail + 1))
else
  echo "PASS close-resolves-project-key"
  pass=$((pass + 1))
fi
if grep -q "POST /api/v1/runs/${RUN_ID} " "${WORKDIR}/log.txt"; then
  echo "FAIL close fell back to POST /runs/{id}" >&2
  fail=$((fail + 1))
else
  echo "PASS close-no-bogus-fallback"
  pass=$((pass + 1))
fi

# J3: >1 MiB junit streamed (body must exceed 1MiB on server)
BIG="${WORKDIR}/big.xml"
python3 - <<PY
from pathlib import Path
# ~1.2 MiB of testcases
parts = ['<?xml version="1.0"?><testsuite name="big" tests="1">']
chunk = "x" * 8000
# keep under parser limits but large content overall
parts.append(f'<testcase classname="C" name="n" time="0"><system-out>{chunk}</system-out></testcase>' * 160)
parts.append("</testsuite>")
Path("${BIG}").write_text("".join(parts), encoding="utf-8")
print(Path("${BIG}").stat().st_size)
PY
: > "${WORKDIR}/log.txt"
echo 0 > "${WORKDIR}/max_body.txt"
"${SCRIPT}" --api-url "${API}" --project-id "${PROJECT_ID}" \
  --file "${BIG}" --format junit --run-id "${RUN_ID}" >/dev/null
MAX_B="$(cat "${WORKDIR}/max_body.txt")"
if [[ "${MAX_B}" -gt 1048576 ]]; then
  echo "PASS large-file-streamed (${MAX_B} bytes)"
  pass=$((pass + 1))
else
  echo "FAIL large-file body too small: ${MAX_B}" >&2
  fail=$((fail + 1))
fi

kill "${SERVER_PID}" 2>/dev/null || true
wait "${SERVER_PID}" 2>/dev/null || true

echo "summary: pass=${pass} fail=${fail}"
[[ "${fail}" -eq 0 ]]
