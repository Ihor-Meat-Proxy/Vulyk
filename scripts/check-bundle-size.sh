#!/usr/bin/env bash
# Fail if the main frontend JS bundle grows more than 10% vs a base ref.
#
# Env:
#   BUNDLE_BASE_REF   git ref to compare (default: origin/main)
#   BUNDLE_GROWTH_MAX fraction allowed growth (default: 0.10)
#   BUNDLE_SKIP_BASE_BUILD=1  compare against BUNDLE_BASE_BYTES instead
#   BUNDLE_BASE_BYTES         baseline raw byte size (when skipping rebuild)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

BASE_REF="${BUNDLE_BASE_REF:-origin/main}"
MAX_GROWTH="${BUNDLE_GROWTH_MAX:-0.10}"

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "error: missing '$1'" >&2
    exit 1
  fi
}

need_cmd npm
need_cmd node
need_cmd git

measure_main_bundle() {
  local dist_dir="$1"
  node - "$dist_dir" <<'NODE'
const fs = require('fs')
const path = require('path')
const dir = path.join(process.argv[2], 'assets')
if (!fs.existsSync(dir)) {
  console.error('missing assets dir:', dir)
  process.exit(1)
}
const files = fs.readdirSync(dir)
  .filter((f) => /^index-.*\.js$/.test(f))
  .map((f) => {
    const p = path.join(dir, f)
    return { f, bytes: fs.statSync(p).size }
  })
  .sort((a, b) => b.bytes - a.bytes)
if (!files.length) {
  console.error('no index-*.js bundle in', dir)
  process.exit(1)
}
const main = files[0]
process.stdout.write(String(main.bytes))
process.stderr.write(`main bundle ${main.f}: ${main.bytes} bytes\n`)
NODE
}

build_frontend() {
  local workdir="$1"
  (
    cd "${workdir}/frontend"
    if [[ ! -d node_modules ]]; then
      npm ci --silent
    fi
    npm run build
  )
}

echo "==> build current frontend"
build_frontend "${ROOT}"
CURRENT_BYTES="$(measure_main_bundle "${ROOT}/frontend/dist")"
echo "current: ${CURRENT_BYTES} bytes"

if [[ "${BUNDLE_SKIP_BASE_BUILD:-}" == "1" ]]; then
  if [[ -z "${BUNDLE_BASE_BYTES:-}" ]]; then
    echo "error: BUNDLE_SKIP_BASE_BUILD requires BUNDLE_BASE_BYTES" >&2
    exit 2
  fi
  BASE_BYTES="${BUNDLE_BASE_BYTES}"
  echo "base (provided): ${BASE_BYTES} bytes"
else
  if ! git rev-parse --verify "${BASE_REF}" >/dev/null 2>&1; then
    echo "OK: base ref ${BASE_REF} missing; skipping growth gate (first run)"
    exit 0
  fi
  BASE_SHA="$(git rev-parse "${BASE_REF}")"
  HEAD_SHA="$(git rev-parse HEAD)"
  if [[ "${BASE_SHA}" == "${HEAD_SHA}" ]]; then
    echo "OK: HEAD is ${BASE_REF}; no growth to compare"
    exit 0
  fi

  WORKTREE="$(mktemp -d /tmp/vulyk-bundle-base.XXXXXX)"
  cleanup() { git worktree remove --force "${WORKTREE}" 2>/dev/null || rm -rf "${WORKTREE}"; }
  trap cleanup EXIT

  echo "==> build baseline frontend (${BASE_REF} @ ${BASE_SHA})"
  git worktree add --detach "${WORKTREE}" "${BASE_SHA}"
  # Reuse node_modules from current tree when lockfile matches.
  if [[ -f "${ROOT}/frontend/package-lock.json" ]] \
    && cmp -s "${ROOT}/frontend/package-lock.json" "${WORKTREE}/frontend/package-lock.json" \
    && [[ -d "${ROOT}/frontend/node_modules" ]]; then
    ln -s "${ROOT}/frontend/node_modules" "${WORKTREE}/frontend/node_modules"
  fi
  build_frontend "${WORKTREE}"
  BASE_BYTES="$(measure_main_bundle "${WORKTREE}/frontend/dist")"
  echo "base: ${BASE_BYTES} bytes"
fi

python3 - "${CURRENT_BYTES}" "${BASE_BYTES}" "${MAX_GROWTH}" <<'PY'
import sys
cur, base, max_g = int(sys.argv[1]), int(sys.argv[2]), float(sys.argv[3])
if base <= 0:
    raise SystemExit("base bundle size must be > 0")
growth = (cur - base) / base
pct = growth * 100
print(f"growth: {pct:+.2f}% (limit {max_g*100:.0f}%)")
if growth > max_g + 1e-12:
    raise SystemExit(
        f"error: main JS bundle grew {pct:.2f}% "
        f"({base} -> {cur} bytes); limit is {max_g*100:.0f}%"
    )
print("OK: bundle size within budget")
PY
