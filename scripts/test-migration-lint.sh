#!/usr/bin/env bash
# Proof that Squawk flags deliberately risky migration SQL.
# CI runs this so "bad migration gets flagged" is automated (no human test PR).
# Exit 0 only when Squawk reports expected rule hits on the fixture.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

FIXTURE="${ROOT}/scripts/fixtures/bad-migration.sql"
SQUAWK_VERSION="${SQUAWK_VERSION:-2.73.0}"
SQUAWK_INSTALL_DIR="${SQUAWK_INSTALL_DIR:-/tmp}"
SQUAWK_BIN="${SQUAWK_BIN:-}"

die() { printf 'error: %s\n' "$*" >&2; exit 1; }
log() { printf '==> %s\n' "$*"; }

[[ -f "${FIXTURE}" ]] || die "fixture missing: ${FIXTURE}"

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
    *) die "unsupported platform for Squawk download: ${os}-${arch}" ;;
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

ensure_squawk
log "Proof-lint fixture with ${SQUAWK_BIN}"

JSON_OUT="$(mktemp /tmp/squawk-proof-XXXXXX.json)"
trap 'rm -f "${JSON_OUT}"' EXIT

set +e
"${SQUAWK_BIN}" --reporter json "${FIXTURE}" > "${JSON_OUT}"
RC=$?
"${SQUAWK_BIN}" --reporter tty "${FIXTURE}" >/tmp/squawk-proof-tty.txt 2>&1
set -e

if [[ "${RC}" -eq 0 ]]; then
  die "Squawk exited 0 on deliberately bad fixture (expected findings)"
fi

python3 - "${JSON_OUT}" <<'PY'
import json, sys
from pathlib import Path

data = json.loads(Path(sys.argv[1]).read_text() or "[]")
rules = {item.get("rule_name") for item in data if isinstance(item, dict)}
required = {
    "ban-drop-column",
    "adding-required-field",
    "require-concurrent-index-creation",
}
missing = sorted(required - rules)
if missing:
    print("missing expected Squawk rules:", ", ".join(missing), file=sys.stderr)
    print("seen:", ", ".join(sorted(rules)) or "(none)", file=sys.stderr)
    sys.exit(1)
print("OK: Squawk flagged fixture rules:", ", ".join(sorted(required)))
print(f"total findings: {len(data)}")
PY

echo "OK: migration lint proof passed"
