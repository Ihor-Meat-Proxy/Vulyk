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
# Sample seeds are include_str!'d at compile time. If HEAD fixed duplicate case
# keys (or other seed fixtures) that prevent the base binary from booting with
# VULYK_SEED_SAMPLE=1, compile base against HEAD's seed JSON so this job still
# exercises migrate + data/secrets survival. Schema/migrations still come from base.
if [[ -d "${ROOT}/backend/seed" && -d "${WORKDIR}/backend/seed" ]]; then
  log "Overlaying HEAD backend/seed onto base workdir (seed fixture fix-forward)"
  cp -a "${ROOT}/backend/seed/." "${WORKDIR}/backend/seed/"
fi
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
export VULYK_ADMIN_PASSWORD="${VULYK_ADMIN_PASSWORD:-admin12345678901}"
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

log "Snapshotting settings, requirements, runs, and inserting encrypted LLM secret..."
python3 - <<'PY'
import json, os, urllib.request, hashlib, secrets
from pathlib import Path

try:
    from cryptography.hazmat.primitives.ciphers.aead import AESGCM
except ImportError:
    # Fallback: use openssl-free pure approach via PyNaCl-less AES - install cryptography if missing
    import subprocess, sys
    subprocess.check_call([sys.executable, "-m", "pip", "install", "-q", "cryptography"])
    from cryptography.hazmat.primitives.ciphers.aead import AESGCM

api = os.environ["VULYK_API_URL"]
db_url = os.environ["DATABASE_URL"]
secrets_key = os.environ.get("VULYK_SECRETS_KEY", "upgrade-ci-secrets")
key = hashlib.sha256(secrets_key.encode()).digest()
plain = "sk-upgrade-ci-must-survive"
nonce = secrets.token_bytes(12)
ct = AESGCM(key).encrypt(nonce, plain.encode(), None)
# AESGCM returns ct||tag combined; store as-is
Path("/tmp/vulyk-upgrade-secret-plain.txt").write_text(plain)

# Prefer API insert for llm config (auth optional in this job)
projects = json.load(urllib.request.urlopen(f"{api}/api/v1/projects"))
items = projects["items"] if isinstance(projects, dict) else projects
assert items, "expected projects"
pid = items[0]["id"]
body = json.dumps({
    "provider": "openrouter",
    "name": "upgrade-ci-secret",
    "api_key": plain,
    "base_url": "https://openrouter.ai/api/v1",
    "model": "openai/gpt-4o-mini",
    "is_default": False,
}).encode()
req = urllib.request.Request(
    f"{api}/api/v1/projects/{pid}/llm/configs",
    data=body,
    headers={"Content-Type": "application/json"},
    method="POST",
)
try:
    with urllib.request.urlopen(req) as r:
        cfg = json.load(r)
    Path("/tmp/vulyk-upgrade-llm-config-id.txt").write_text(cfg["id"])
    print("llm config created via API", cfg["id"])
except Exception as e:
    print("API llm insert failed, using SQL fallback:", e)
    import subprocess
    hex_ct = ct.hex()
    hex_nonce = nonce.hex()
    sql = f"""
    INSERT INTO llm_configs (project_id, provider, name, model, base_url, api_key_ciphertext, api_key_nonce, is_default)
    VALUES ('{pid}'::uuid, 'openrouter', 'upgrade-ci-secret', 'openai/gpt-4o-mini',
            'https://openrouter.ai/api/v1', decode('{hex_ct}','hex'), decode('{hex_nonce}','hex'), false)
    RETURNING id;
    """
    out = subprocess.check_output(["psql", db_url, "-Atc", sql], text=True).strip()
    Path("/tmp/vulyk-upgrade-llm-config-id.txt").write_text(out.splitlines()[-1])
    print("llm config created via SQL", out)

# Counts that must survive
import subprocess
snap = {}
for label, q in [
    ("requirements", "SELECT COUNT(*) FROM requirements"),
    ("test_runs", "SELECT COUNT(*) FROM test_runs"),
    ("system_settings", "SELECT COUNT(*) FROM system_settings"),
    ("llm_configs", "SELECT COUNT(*) FROM llm_configs WHERE name = 'upgrade-ci-secret'"),
    # Legacy per-project table on base; PR migrates to custom_field_definitions.
    ("custom_fields", "SELECT COUNT(*) FROM custom_fields"),
    ("custom_field_definitions", "SELECT COUNT(*) FROM custom_field_definitions"),
]:
    try:
        snap[label] = int(subprocess.check_output(["psql", db_url, "-Atc", q], text=True).strip() or "0")
    except Exception:
        # table may not exist on very old bases / pre-migration HEAD
        snap[label] = -1
        print(f"skip count {label}")
Path("/tmp/vulyk-upgrade-entity-counts-before.json").write_text(json.dumps(snap))
print("entity snapshot", snap)
PY

log "Saving pre-upgrade DB dump for rollback assertion..."
need_cmd pg_dump
# Filter GUCs that newer pg_dump emits but Postgres 16 rejects (e.g. transaction_timeout).
pg_dump "${DATABASE_URL}" --no-owner --no-acl \
  | grep -Ev '^(SET |SELECT pg_catalog\.set_config\().*transaction_timeout' \
  | gzip -c > /tmp/vulyk-pre-upgrade.sql.gz
[[ -s /tmp/vulyk-pre-upgrade.sql.gz ]] || die "pre-upgrade dump empty"
printf '%s\n' "VULYK_SECRETS_KEY=${VULYK_SECRETS_KEY}" "VULYK_JWT_SECRET=${VULYK_JWT_SECRET}" \
  > /tmp/vulyk-pre-upgrade.env
# Register as a version-backup rollback point (filesystem only; prune unit-tested separately)
export VULYK_VERSION_BACKUP_DIR="${WORKDIR}/version-backups"
mkdir -p "${VULYK_VERSION_BACKUP_DIR}/src-bundle/config"
cp /tmp/vulyk-pre-upgrade.sql.gz "${VULYK_VERSION_BACKUP_DIR}/src-bundle/db.sql.gz"
cp /tmp/vulyk-pre-upgrade.env "${VULYK_VERSION_BACKUP_DIR}/src-bundle/config/.env"
python3 "${ROOT}/scripts/version_backups.py" register \
  --source "${VULYK_VERSION_BACKUP_DIR}/src-bundle" \
  --version "$(python3 -c 'import json,sys; print(json.loads(sys.argv[1]).get("version","0.0.0"))' "${BASE_HEALTH}")" \
  --from-version "$(python3 -c 'import json,sys; print(json.loads(sys.argv[1]).get("version","0.0.0"))' "${BASE_HEALTH}")" \
  --to-version "head" \
  --rollback >/tmp/vulyk-vb-register.json
python3 "${ROOT}/scripts/test-version-backups.py" -q

stop_api
log "Building HEAD API..."
cargo build -p vulyk-api --release

log "Running explicit migrate on HEAD binary..."
export VULYK_AUTO_MIGRATE=false
./target/release/vulyk-api migrate status || true
./target/release/vulyk-api migrate run
./target/release/vulyk-api version

log "Starting HEAD API (auto-migrate off) - same VULYK_SECRETS_KEY must still decrypt..."
./target/release/vulyk-api > /tmp/vulyk-upgrade-head.log 2>&1 &
echo $! > /tmp/vulyk-upgrade-api.pid
if ! wait_health; then
  cat /tmp/vulyk-upgrade-head.log >&2 || true
  die "head API failed to become healthy"
fi

HEAD_HEALTH="$(curl -fsS "${API_URL}/health")"
log "Head health: ${HEAD_HEALTH}"
python3 -c 'import json,sys; h=json.loads(sys.argv[1]); assert h.get("status")=="ok", h; assert h.get("version"), h; print("version", h["version"])' "${HEAD_HEALTH}"

log "Verifying projects, case counts, entities, and encrypted secret decrypt..."
curl -fsS "${API_URL}/api/v1/projects" > /tmp/vulyk-upgrade-projects-after.json
python3 - <<'PY'
import json, os, urllib.request, sys, subprocess, hashlib
from pathlib import Path

try:
    from cryptography.hazmat.primitives.ciphers.aead import AESGCM
except ImportError:
    import subprocess as sp
    sp.check_call([sys.executable, "-m", "pip", "install", "-q", "cryptography"])
    from cryptography.hazmat.primitives.ciphers.aead import AESGCM

api = os.environ["VULYK_API_URL"]
db_url = os.environ["DATABASE_URL"]
secrets_key = os.environ.get("VULYK_SECRETS_KEY", "upgrade-ci-secrets")
key = hashlib.sha256(secrets_key.encode()).digest()
plain_expected = Path("/tmp/vulyk-upgrade-secret-plain.txt").read_text()

before_keys = [k for k in open("/tmp/vulyk-upgrade-project-keys.txt").read().splitlines() if k]
before_counts = json.load(open("/tmp/vulyk-upgrade-case-counts-before.json"))
before_entities = json.load(open("/tmp/vulyk-upgrade-entity-counts-before.json"))
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

for label, q in [
    ("requirements", "SELECT COUNT(*) FROM requirements"),
    ("test_runs", "SELECT COUNT(*) FROM test_runs"),
    ("system_settings", "SELECT COUNT(*) FROM system_settings"),
    ("llm_configs", "SELECT COUNT(*) FROM llm_configs WHERE name = 'upgrade-ci-secret'"),
]:
    prev = before_entities.get(label, -1)
    if prev < 0:
        continue
    try:
        now = int(subprocess.check_output(["psql", db_url, "-Atc", q], text=True).strip() or "0")
    except Exception as e:
        print(f"count failed for {label}: {e}", file=sys.stderr)
        sys.exit(1)
    if now < prev:
        print(f"{label} count dropped: {prev} -> {now}", file=sys.stderr)
        sys.exit(1)

# custom_fields -> custom_field_definitions (migration may rename/move rows)
prev_cf = before_entities.get("custom_fields", -1)
if prev_cf >= 0:
    try:
        now_defs = int(
            subprocess.check_output(
                ["psql", db_url, "-Atc", "SELECT COUNT(*) FROM custom_field_definitions"],
                text=True,
            ).strip()
            or "0"
        )
    except Exception as e:
        print(f"count failed for custom_field_definitions: {e}", file=sys.stderr)
        sys.exit(1)
    if now_defs < prev_cf:
        print(
            f"custom_field_definitions count dropped vs legacy custom_fields: {prev_cf} -> {now_defs}",
            file=sys.stderr,
        )
        sys.exit(1)
    print(f"ok custom_fields->definitions: {prev_cf} -> {now_defs}")
    print(f"ok {label}: {now} (before={prev})")

cfg_id = Path("/tmp/vulyk-upgrade-llm-config-id.txt").read_text().strip()
row = subprocess.check_output(
    ["psql", db_url, "-Atc",
     f"SELECT encode(api_key_ciphertext,'hex') || ',' || encode(api_key_nonce,'hex') FROM llm_configs WHERE id = '{cfg_id}'::uuid"],
    text=True,
).strip()
if not row or "," not in row:
    print("encrypted llm row missing after upgrade", file=sys.stderr)
    sys.exit(1)
hex_ct, hex_nonce = row.split(",", 1)
ct = bytes.fromhex(hex_ct)
nonce = bytes.fromhex(hex_nonce)
# cryptography AESGCM decrypt expects ciphertext+tag together (as stored)
plain = AESGCM(key).decrypt(nonce, ct, None).decode()
if plain != plain_expected:
    print(f"secret decrypt mismatch: {plain!r} != {plain_expected!r}", file=sys.stderr)
    sys.exit(1)
print("ok encrypted secret decrypts with preserved VULYK_SECRETS_KEY")
print("upgrade data integrity OK")
PY

log "Simulating post-upgrade mutation then rollback to pre-upgrade dump..."
psql "${DATABASE_URL}" -v ON_ERROR_STOP=1 -c \
  "INSERT INTO system_settings (key, value) VALUES ('upgrade_ci_post_marker', '\"should-vanish-on-rollback\"'::jsonb)
   ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value;" >/dev/null
MARKER_BEFORE="$(psql "${DATABASE_URL}" -Atc "SELECT value::text FROM system_settings WHERE key='upgrade_ci_post_marker'" || true)"
[[ -n "${MARKER_BEFORE}" ]] || die "failed to insert post-upgrade marker"

stop_api
log "Restoring pre-upgrade dump (rollback)..."
# Drop and recreate public schema then restore (mirrors install.sh restore_db_from_dump)
psql "${DATABASE_URL}" -v ON_ERROR_STOP=1 -c \
  "DROP SCHEMA public CASCADE; CREATE SCHEMA public;" >/dev/null
gzip -dc /tmp/vulyk-pre-upgrade.sql.gz \
  | grep -Ev '^(SET |SELECT pg_catalog\.set_config\().*transaction_timeout' \
  | psql "${DATABASE_URL}" -v ON_ERROR_STOP=1 -q

# Config snapshot must still match preserved secrets key
grep -q "VULYK_SECRETS_KEY=${VULYK_SECRETS_KEY}" /tmp/vulyk-pre-upgrade.env \
  || die "pre-upgrade env snapshot missing secrets key"

log "Re-starting HEAD API against rolled-back DB (migrations already applied on dump may be older)..."
# Dump is from base schema; run migrate again so HEAD can serve, then assert data rows.
export VULYK_AUTO_MIGRATE=false
./target/release/vulyk-api migrate run
./target/release/vulyk-api > /tmp/vulyk-upgrade-rollback.log 2>&1 &
echo $! > /tmp/vulyk-upgrade-api.pid
if ! wait_health; then
  cat /tmp/vulyk-upgrade-rollback.log >&2 || true
  die "API failed after rollback restore"
fi

python3 - <<'PY'
import json, os, urllib.request, sys, subprocess, hashlib
from pathlib import Path
try:
    from cryptography.hazmat.primitives.ciphers.aead import AESGCM
except ImportError:
    import subprocess as sp
    sp.check_call([sys.executable, "-m", "pip", "install", "-q", "cryptography"])
    from cryptography.hazmat.primitives.ciphers.aead import AESGCM

api = os.environ["VULYK_API_URL"]
db_url = os.environ["DATABASE_URL"]
secrets_key = os.environ.get("VULYK_SECRETS_KEY", "upgrade-ci-secrets")
key = hashlib.sha256(secrets_key.encode()).digest()
plain_expected = Path("/tmp/vulyk-upgrade-secret-plain.txt").read_text()
before_keys = [k for k in open("/tmp/vulyk-upgrade-project-keys.txt").read().splitlines() if k]
before_counts = json.load(open("/tmp/vulyk-upgrade-case-counts-before.json"))
before_entities = json.load(open("/tmp/vulyk-upgrade-entity-counts-before.json"))

marker = subprocess.check_output(
    ["psql", db_url, "-Atc", "SELECT value::text FROM system_settings WHERE key='upgrade_ci_post_marker'"],
    text=True,
).strip()
if marker:
    print("post-upgrade marker survived rollback:", marker, file=sys.stderr)
    sys.exit(1)
print("ok post-upgrade marker removed by rollback")

after = json.load(urllib.request.urlopen(f"{api}/api/v1/projects"))
items = after["items"] if isinstance(after, dict) else after
after_keys = sorted(p["key"] for p in items)
missing = [k for k in before_keys if k not in after_keys]
if missing:
    print("missing projects after rollback:", missing, file=sys.stderr)
    sys.exit(1)
for p in items:
    url = f"{api}/api/v1/projects/{p['id']}/cases?limit=1&offset=0"
    with urllib.request.urlopen(url) as r:
        body = json.load(r)
    total = body.get("total", len(body.get("items", [])))
    prev = before_counts.get(p["key"])
    if prev is not None and total != prev:
        print(f"case count mismatch after rollback for {p['key']}: {prev} -> {total}", file=sys.stderr)
        sys.exit(1)
for label, q in [
    ("requirements", "SELECT COUNT(*) FROM requirements"),
    ("test_runs", "SELECT COUNT(*) FROM test_runs"),
    ("system_settings", "SELECT COUNT(*) FROM system_settings"),
    ("llm_configs", "SELECT COUNT(*) FROM llm_configs WHERE name = 'upgrade-ci-secret'"),
    ("custom_fields", "SELECT COUNT(*) FROM custom_fields"),
]:
    prev = before_entities.get(label, -1)
    if prev < 0:
        continue
    now = int(subprocess.check_output(["psql", db_url, "-Atc", q], text=True).strip() or "0")
    if now != prev:
        print(f"{label} count mismatch after rollback: {prev} -> {now}", file=sys.stderr)
        sys.exit(1)
    print(f"ok rollback {label}: {now}")

cfg_id = Path("/tmp/vulyk-upgrade-llm-config-id.txt").read_text().strip()
row = subprocess.check_output(
    ["psql", db_url, "-Atc",
     f"SELECT encode(api_key_ciphertext,'hex') || ',' || encode(api_key_nonce,'hex') FROM llm_configs WHERE id = '{cfg_id}'::uuid"],
    text=True,
).strip()
hex_ct, hex_nonce = row.split(",", 1)
plain = AESGCM(key).decrypt(bytes.fromhex(hex_nonce), bytes.fromhex(hex_ct), None).decode()
if plain != plain_expected:
    print("secret decrypt mismatch after rollback", file=sys.stderr)
    sys.exit(1)
print("ok rollback restored pre-upgrade data + encrypted secret")
PY

# Active rollback point still listed
python3 "${ROOT}/scripts/version_backups.py" active-rollback --current-version head \
  | python3 -c 'import json,sys; m=json.load(sys.stdin); assert m and m.get("roles"), m; print("ok active rollback point", m.get("_id") or m.get("id"))'

log "Upgrade + rollback CI test passed"
