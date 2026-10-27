#!/usr/bin/env bash
# CI: simulate upgrading an operator .env from the previous release to HEAD.
# Fails when HEAD introduces a synced/required key without a safe default, or
# when Compose cannot interpolate using the synced env file.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

die() { printf 'error: %s\n' "$*" >&2; exit 1; }
log() { printf '==> %s\n' "$*"; }
warn() { printf 'warning: %s\n' "$*" >&2; }

need_cmd() { command -v "$1" >/dev/null 2>&1 || die "missing $1"; }
need_cmd python3
need_cmd git

PREV_TAG="${VULYK_PREV_RELEASE_TAG:-}"
if [[ -z "${PREV_TAG}" ]]; then
  # Default: last published release train tag present in the clone.
  if git rev-parse -q --verify refs/tags/v0.2.0 >/dev/null; then
    PREV_TAG="v0.2.0"
  elif git rev-parse -q --verify refs/tags/v0.1.0 >/dev/null; then
    PREV_TAG="v0.1.0"
  else
    PREV_TAG="$(git tag -l 'v*' --sort=-v:refname | head -n1 || true)"
  fi
fi
[[ -n "${PREV_TAG}" ]] || die "no previous release tag found (set VULYK_PREV_RELEASE_TAG)"

log "Previous release tag: ${PREV_TAG}"
log "Validating HEAD .env.example annotations..."
python3 scripts/env_settings.py validate \
  --example "${ROOT}/.env.example" \
  --compose "${ROOT}/docker-compose.yml" "${ROOT}/docker-compose.prod.yml"

WORKDIR="$(mktemp -d /tmp/vulyk-env-upgrade-XXXXXX)"
cleanup() { rm -rf "${WORKDIR}"; }
trap cleanup EXIT

OLD_EXAMPLE="${WORKDIR}/old.env.example"
if ! git show "${PREV_TAG}:.env.example" >"${OLD_EXAMPLE}" 2>/dev/null; then
  die "cannot read ${PREV_TAG}:.env.example (fetch tags?)"
fi

OPERATOR_ENV="${WORKDIR}/.env"
PREV_TAG="${PREV_TAG}" OLD_EXAMPLE="${OLD_EXAMPLE}" OPERATOR_ENV="${OPERATOR_ENV}" python3 - <<'PY'
import os, re
from pathlib import Path

src = Path(os.environ["OLD_EXAMPLE"]).read_text(encoding="utf-8").splitlines()
out = []
skipped_empty = []
assign = re.compile(r"^(#\s*)?([A-Za-z_][A-Za-z0-9_]*)=(.*)$")
for line in src:
    s = line.strip()
    m = assign.match(s)
    if not m or m.group(1):
        # Skip non-assignments and commented keys (never shipped in operator .env).
        continue
    key, raw = m.group(2), m.group(3)
    value = raw.split("#", 1)[0].rstrip()
    # Empty template placeholders are not a working install's secrets - omit so
    # HEAD sync can add/generate them (never-overwrite still applies to real values).
    if value == "":
        skipped_empty.append(key)
        continue
    out.append(f"{key}={value}")
Path(os.environ["OPERATOR_ENV"]).write_text("\n".join(out) + "\n", encoding="utf-8")
print(
    f"seeded operator .env with {len(out)} keys from {os.environ['PREV_TAG']}"
    + (f" (omitted empty: {', '.join(skipped_empty)})" if skipped_empty else "")
)
PY

if grep -qE '^VULYK_UPDATER_TOKEN=' "${OPERATOR_ENV}"; then
  log "Note: previous example already had VULYK_UPDATER_TOKEN"
else
  log "Operator .env intentionally missing VULYK_UPDATER_TOKEN (pre-sync)"
fi

BEFORE="$(grep -cE '^[A-Za-z_][A-Za-z0-9_]*=' "${OPERATOR_ENV}" || true)"
log "Syncing missing keys from HEAD .env.example..."
SYNC_JSON="$(python3 scripts/env_settings.py sync --example "${ROOT}/.env.example" "${OPERATOR_ENV}")"
printf '%s\n' "${SYNC_JSON}"
AFTER="$(grep -cE '^[A-Za-z_][A-Za-z0-9_]*=' "${OPERATOR_ENV}" || true)"
log "Keys before=${BEFORE} after=${AFTER}"

printf '%s' "${SYNC_JSON}" | python3 -c 'import json,sys
data=json.load(sys.stdin)
added=data.get("added", [])
print("added:", ", ".join("%s/%s" % (a["key"], a["how"]) for a in added) or "(none)")
'

if grep -qE '^VULYK_UPDATER_TOKEN=$' "${OPERATOR_ENV}"; then
  die "VULYK_UPDATER_TOKEN was added empty - mark @vulyk:secret in .env.example"
fi
if ! grep -qE '^VULYK_UPDATER_TOKEN=.' "${OPERATOR_ENV}"; then
  die "VULYK_UPDATER_TOKEN missing after sync (mark @vulyk:sync @vulyk:secret on it)"
fi

# Every Compose ${VAR:?...} must be non-empty after sync (catches empty-template seeds).
OPERATOR_ENV="${OPERATOR_ENV}" python3 - <<'PY'
import os, re, sys
from pathlib import Path

root = Path(".")
env = {}
for line in Path(os.environ["OPERATOR_ENV"]).read_text(encoding="utf-8").splitlines():
    s = line.strip()
    if not s or s.startswith("#") or "=" not in s:
        continue
    k, _, v = s.partition("=")
    env[k.strip()] = v

pat = re.compile(r"\$\{([A-Za-z_][A-Za-z0-9_]*)\?:")
missing = []
for path in (root / "docker-compose.yml", root / "docker-compose.prod.yml"):
    for key in pat.findall(path.read_text(encoding="utf-8")):
        if not str(env.get(key, "")).strip():
            missing.append(f"{key} ({path.name})")
if missing:
    print("compose ?: vars empty/missing after sync:", ", ".join(missing), file=sys.stderr)
    sys.exit(1)
print("compose ?: vars satisfied after sync")
PY

if command -v docker >/dev/null 2>&1 && docker compose version >/dev/null 2>&1; then
  log "docker compose config with synced .env..."
  docker compose --env-file "${OPERATOR_ENV}" -f docker-compose.yml config >/dev/null
  docker compose --env-file "${OPERATOR_ENV}" -f docker-compose.prod.yml config >/dev/null
  log "compose config OK"
else
  warn "docker compose not available - skipping compose config probe"
fi

log "Env upgrade test passed (${PREV_TAG} -> HEAD)"
