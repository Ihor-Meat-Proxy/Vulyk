#!/usr/bin/env bash
# Regression: install.sh log()/warn() must not pollute $() captures of backup paths.
# Also asserts version-backups registration + polluted-path rejection (U1).
# No Docker required.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

die() { printf 'error: %s\n' "$*" >&2; exit 1; }
pass() { printf 'ok: %s\n' "$*"; }

need_cmd() { command -v "$1" >/dev/null 2>&1 || die "missing $1"; }
need_cmd bash
need_cmd python3
need_cmd gzip

# --- 1. Source of truth: log/warn/die write to stderr ---
grep -qE "log\(\) \{ printf '==> %s\\\\n' \"\\\$\*\" >&2; \}" scripts/install.sh \
  || die "log() must printf to stderr (>&2)"
grep -qE "warn\(\) \{ printf 'warning: %s\\\\n' \"\\\$\*\" >&2; \}" scripts/install.sh \
  || die "warn() must printf to stderr (>&2)"
grep -q "capture_stdout_path" scripts/install.sh \
  || die "capture_stdout_path helper missing from install.sh"
grep -qE 'backup_file="\$\(capture_stdout_path run_full_backup' scripts/install.sh \
  || die "cmd_upgrade must capture run_full_backup via capture_stdout_path"
pass "install.sh log/warn redirect + capture_stdout_path wiring"

# --- 2. Exercise the real helpers in isolation ---
HELPERS="$(mktemp /tmp/vulyk-backup-helpers-XXXXXX.sh)"
WORKDIR="$(mktemp -d /tmp/vulyk-backup-capture-XXXXXX)"
cleanup() { rm -f "${HELPERS}"; rm -rf "${WORKDIR}"; }
trap cleanup EXIT

# Extract the three helpers (log/warn/capture_stdout_path) from install.sh
awk '
  /^log\(\)/ { keep=1 }
  /^die\(\)/ { keep=1 }
  /^warn\(\)/ { keep=1 }
  /^capture_stdout_path\(\)/ { keep=1 }
  keep { print }
  keep && /^}/ { keep=0; print "" }
' scripts/install.sh >"${HELPERS}"

# shellcheck disable=SC1090
source "${HELPERS}"

# Producer that intentionally logs to stdout (simulates the pre-fix bug) then
# prints a real path last.
fake_backup_polluted() {
  local bundle="$1"
  printf '==> Creating verified backup bundle: %s\n' "${bundle}"
  printf '==> Verifying dump integrity (gzip -t)...\n'
  printf '==> Verified backup at %s\n' "${bundle}"
  printf '%s\n' "${bundle}"
}

# Producer that only logs via log() (stderr) and prints the path on stdout.
fake_backup_clean() {
  local bundle="$1"
  log "Creating verified backup bundle: ${bundle}"
  log "Verifying dump integrity (gzip -t)..."
  printf '%s\n' "${bundle}"
}

BUNDLE="${WORKDIR}/backups/vulyk-0.3.0-test"
mkdir -p "${BUNDLE}/config"
# Minimal gzip member (valid for gzip -t)
printf 'hello-vulyk-backup' | gzip -c >"${BUNDLE}/db.sql.gz"
gzip -t "${BUNDLE}/db.sql.gz" || die "test fixture db.sql.gz failed gzip -t"
printf 'VULYK_SECRETS_KEY=preserve-me\nVULYK_JWT_SECRET=preserve-jwt\nVULYK_ADMIN_PASSWORD=preserve-admin\n' \
  >"${BUNDLE}/config/.env"
printf 'VULYK_API_IMAGE=vulyk-api:0.3.0\nVULYK_WEB_IMAGE=vulyk-web:0.3.0\n' \
  >"${BUNDLE}/config/image-state.env"

# With polluted stdout, raw $() would be multi-line garbage; capture_stdout_path
# must still yield the last line (the real path).
polluted_raw="$(fake_backup_polluted "${BUNDLE}" || true)"
[[ "${polluted_raw}" == *$'\n'* ]] || die "expected multi-line polluted raw capture"
captured="$(capture_stdout_path fake_backup_polluted "${BUNDLE}")"
[[ "${captured}" == "${BUNDLE}" ]] || die "capture_stdout_path got '${captured}' want '${BUNDLE}'"
pass "capture_stdout_path returns last line under polluted stdout"

# Clean path: log on stderr must not appear in $()
clean_out="$(fake_backup_clean "${BUNDLE}")"
[[ "${clean_out}" == "${BUNDLE}" ]] || die "clean capture polluted: '${clean_out}'"
# Explicitly confirm stderr logs do not mix into stdout capture
mixed="$( { fake_backup_clean "${BUNDLE}" ; } 2>/tmp/vulyk-backup-capture-stderr.txt )"
[[ "${mixed}" == "${BUNDLE}" ]] || die "stderr-redirected capture wrong: '${mixed}'"
grep -q 'Creating verified backup' /tmp/vulyk-backup-capture-stderr.txt \
  || die "expected log lines on stderr"
pass "log() writes to stderr only (path capture clean)"

# --- 3. Upgrade path validation: polluted multi-line value must fail -d / db.sql.gz ---
garbage=$'==> Creating verified backup bundle: '"${BUNDLE}"$'\n'"${BUNDLE}"
if [[ -d "${garbage}" ]]; then
  die "unexpected: multi-line garbage path is a directory"
fi
if [[ -f "${garbage}/db.sql.gz" ]]; then
  die "unexpected: multi-line garbage path has db.sql.gz"
fi
pass "polluted backup_file fails directory/db.sql.gz validation (would die in cmd_upgrade)"

# --- 4. version-backups registration with a valid bundle ---
export VULYK_VERSION_BACKUP_DIR="${WORKDIR}/version-backups"
export VULYK_ROLLBACK_RETENTION_DAYS=7
export VULYK_VERSION_BACKUP_COUNT=3
export VULYK_ROOT="${ROOT}"
mkdir -p "${VULYK_VERSION_BACKUP_DIR}"

python3 "${ROOT}/scripts/version_backups.py" register \
  --source "${BUNDLE}" \
  --version "0.3.0" \
  --from-version "0.3.0" \
  --to-version "0.3.1" \
  --api-image "vulyk-api:0.3.0" \
  --web-image "vulyk-web:0.3.0" \
  --rollback >"${WORKDIR}/register.json"

python3 - <<PY
import json, gzip, sys
from pathlib import Path
meta = json.load(open("${WORKDIR}/register.json"))
assert meta.get("roles") and "rollback" in meta["roles"], meta
bundle = Path(meta["_path"]) if "_path" in meta else None
# locate registered dir
root = Path("${VULYK_VERSION_BACKUP_DIR}")
dirs = [p for p in root.iterdir() if p.is_dir() and (p / "db.sql.gz").is_file()]
assert dirs, f"no version-backups bundle under {root}"
db = dirs[0] / "db.sql.gz"
with gzip.open(db, "rb") as f:
    data = f.read()
assert data == b"hello-vulyk-backup", data
env = (dirs[0] / "config" / ".env").read_text(encoding="utf-8")
assert "VULYK_SECRETS_KEY=preserve-me" in env
assert "VULYK_ADMIN_PASSWORD=preserve-admin" in env
print("registered", dirs[0].name)
PY

active="$(python3 "${ROOT}/scripts/version_backups.py" active-rollback --current-version 0.3.1)"
printf '%s\n' "${active}" | python3 -c 'import json,sys; m=json.load(sys.stdin); assert m and "rollback" in m.get("roles",[]), m; print("active", m.get("version"))'
pass "version-backups rollback point exists with valid gzip archive + preserved secrets"

# --- 5. Simulated auto-rollback path check (valid path vs garbage) ---
# Mirrors auto_rollback_upgrade's first gate: empty/garbage path must not be treated as a bundle.
simulate_rollback_path_ok() {
  local backup_path="$1"
  [[ -n "${backup_path}" ]] || return 1
  [[ -d "${backup_path}" ]] || return 1
  [[ -f "${backup_path}/db.sql.gz" ]] || return 1
  gzip -t "${backup_path}/db.sql.gz" || return 1
  return 0
}

simulate_rollback_path_ok "${BUNDLE}" || die "valid bundle rejected by auto-rollback path gate"
if simulate_rollback_path_ok "${garbage}"; then
  die "garbage multi-line path accepted by auto-rollback path gate"
fi
pass "forced-failure auto-rollback path gate accepts valid bundle, rejects polluted path"

# --- 6. Secrets must never be regenerated by env sync on upgrade (spot-check) ---
OPERATOR_ENV="${WORKDIR}/operator.env"
cp "${BUNDLE}/config/.env" "${OPERATOR_ENV}"
# Append a HEAD-synced key that must be added without touching existing secrets
python3 scripts/env_settings.py sync --example "${ROOT}/.env.example" "${OPERATOR_ENV}" >/dev/null
grep -q 'VULYK_SECRETS_KEY=preserve-me' "${OPERATOR_ENV}" \
  || die "env sync overwrote VULYK_SECRETS_KEY"
grep -q 'VULYK_ADMIN_PASSWORD=preserve-admin' "${OPERATOR_ENV}" \
  || die "env sync overwrote VULYK_ADMIN_PASSWORD"
pass "env sync preserves secrets/admin password (add-missing only)"

pass "U1 backup-capture + version-backups regression suite passed"
