#!/usr/bin/env bash
# Lint new/changed migration SQL with Squawk (risky DDL warnings).
# Exit codes follow Squawk: non-zero when findings are reported.
# CI treats Squawk as soft (continue-on-error / annotations); immutability
# remains a hard fail via scripts/check-migrations.sh.
#
# Usage:
#   ./scripts/lint-migrations-squawk.sh [base-ref]
# Env:
#   SQUAWK_BIN           Path to squawk binary (default: squawk on PATH)
#   SQUAWK_VERSION       Version to download when missing (default: 2.73.0)
#   SQUAWK_INSTALL_DIR   Where to place a downloaded binary (default: /tmp)
#   VULYK_SQUAWK_ALL     If 1, lint every up migration (not only changed)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

MIGRATIONS_DIR="backend/migrations"
BASE_REF="${1:-}"
SQUAWK_VERSION="${SQUAWK_VERSION:-2.73.0}"
SQUAWK_INSTALL_DIR="${SQUAWK_INSTALL_DIR:-/tmp}"
SQUAWK_BIN="${SQUAWK_BIN:-}"

die() { printf 'error: %s\n' "$*" >&2; exit 1; }
log() { printf '==> %s\n' "$*"; }

ensure_squawk() {
  if [[ -n "${SQUAWK_BIN}" && -x "${SQUAWK_BIN}" ]]; then
    return 0
  fi
  if command -v squawk >/dev/null 2>&1; then
    SQUAWK_BIN="$(command -v squawk)"
    return 0
  fi

  local os arch asset dest
  os="$(uname -s | tr '[:upper:]' '[:lower:]')"
  arch="$(uname -m)"
  case "${os}-${arch}" in
    linux-x86_64|linux-amd64) asset="squawk-linux-x64" ;;
    linux-aarch64|linux-arm64) asset="squawk-linux-arm64" ;;
    darwin-x86_64|darwin-amd64) asset="squawk-darwin-x64" ;;
    darwin-arm64) asset="squawk-darwin-arm64" ;;
    *) die "unsupported platform for Squawk download: ${os}-${arch} (install squawk manually)" ;;
  esac

  dest="${SQUAWK_INSTALL_DIR}/squawk-${SQUAWK_VERSION}"
  if [[ ! -x "${dest}" ]]; then
    log "Downloading Squawk ${SQUAWK_VERSION} (${asset})..."
    curl -fsSL -o "${dest}" \
      "https://github.com/sbdchd/squawk/releases/download/v${SQUAWK_VERSION}/${asset}"
    chmod +x "${dest}"
  fi
  SQUAWK_BIN="${dest}"
}

resolve_base_ref() {
  if [[ -n "${BASE_REF}" ]]; then
    return 0
  fi
  if git rev-parse --verify origin/main >/dev/null 2>&1; then
    BASE_REF="origin/main"
  elif git rev-parse --verify main >/dev/null 2>&1; then
    BASE_REF="main"
  else
    BASE_REF=""
  fi
}

collect_all_ups() {
  local path base
  TARGETS=()
  for path in "${MIGRATIONS_DIR}"/*.sql; do
    [[ -f "${path}" ]] || continue
    base="$(basename "${path}")"
    case "${base}" in
      *.down.sql) continue ;;
    esac
    TARGETS+=("${path}")
  done
}

collect_targets() {
  local path changed
  TARGETS=()

  if [[ "${VULYK_SQUAWK_ALL:-0}" == "1" ]]; then
    collect_all_ups
    return 0
  fi

  resolve_base_ref
  if [[ -z "${BASE_REF}" ]] || ! git rev-parse --verify "${BASE_REF}" >/dev/null 2>&1; then
    log "No base ref; linting all up migrations"
    collect_all_ups
    return 0
  fi

  changed="$(git diff --name-only --diff-filter=AM "${BASE_REF}"...HEAD -- "${MIGRATIONS_DIR}" 2>/dev/null || true)"
  if [[ -z "${changed}" ]]; then
    changed="$(git diff --name-only --diff-filter=AM "${BASE_REF}" -- "${MIGRATIONS_DIR}" 2>/dev/null || true)"
  fi

  while IFS= read -r path; do
    [[ -z "${path}" ]] && continue
    [[ -f "${path}" ]] || continue
    case "${path}" in
      *.down.sql) continue ;;
      *.sql) TARGETS+=("${path}") ;;
    esac
  done <<< "${changed}"
}

emit_github_annotations() {
  local json_file="$1"
  python3 - "$json_file" <<'PY'
import json, sys
from pathlib import Path

path = Path(sys.argv[1])
if not path.is_file() or path.stat().st_size == 0:
    sys.exit(0)
try:
    data = json.loads(path.read_text())
except Exception:
    sys.exit(0)
if not isinstance(data, list):
    sys.exit(0)
for item in data:
    file = item.get("file") or "backend/migrations"
    # Squawk JSON uses 0-based lines; GitHub wants 1-based.
    line = int(item.get("line") or 0) + 1
    col = int(item.get("column") or 0) + 1
    rule = item.get("rule_name") or "squawk"
    msg = item.get("message") or "Squawk finding"
    help_ = item.get("help") or ""
    text = f"[{rule}] {msg}"
    if help_:
        text = f"{text} ({help_})"
    # Sanitize for workflow command
    text = text.replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")
    print(f"::warning file={file},line={line},col={col}::{text}")
PY
}

ensure_squawk
collect_targets

if [[ ${#TARGETS[@]} -eq 0 ]]; then
  log "No new/changed migration SQL vs ${BASE_REF:-none}; Squawk skip"
  echo "OK: squawk (nothing to lint)"
  exit 0
fi

log "Squawk ${SQUAWK_VERSION} linting ${#TARGETS[@]} file(s) (bin=${SQUAWK_BIN})"
printf '  - %s\n' "${TARGETS[@]}"

JSON_OUT="$(mktemp /tmp/squawk-XXXXXX.json)"
trap 'rm -f "${JSON_OUT}"' EXIT

set +e
"${SQUAWK_BIN}" --reporter json "${TARGETS[@]}" > "${JSON_OUT}" 2>/tmp/squawk-stderr.txt
JSON_RC=$?
"${SQUAWK_BIN}" --reporter tty "${TARGETS[@]}"
TTY_RC=$?
set -e

if [[ -n "${GITHUB_ACTIONS:-}" ]]; then
  emit_github_annotations "${JSON_OUT}"
fi

ISSUE_COUNT="$(python3 - "${JSON_OUT}" <<'PY'
import json, sys
from pathlib import Path
p = Path(sys.argv[1])
try:
    data = json.loads(p.read_text() or "[]")
except Exception:
    data = []
print(len(data) if isinstance(data, list) else 0)
PY
)"

echo
echo "---- Squawk summary ----"
echo "files: ${#TARGETS[@]}"
echo "issues: ${ISSUE_COUNT}"
if [[ "${ISSUE_COUNT}" -gt 0 ]]; then
  echo "status: WARNINGS (risky DDL; review before merge)"
  echo "CI treats Squawk as soft; release-tag immutability remains hard-fail."
else
  echo "status: clean"
fi
echo "------------------------"

# Prefer TTY exit (Squawk returns non-zero on findings)
if [[ "${TTY_RC}" -ne 0 ]]; then
  exit "${TTY_RC}"
fi
exit "${JSON_RC}"
